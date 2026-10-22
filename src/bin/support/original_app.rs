//! Internal original-asset viewer and opt-in single-script diagnostic; NOT a complete mission.
#[path = "battle_view.rs"]
mod battle_view;
#[path = "first_mission_view.rs"]
mod first_mission_view;
#[path = "presentation.rs"]
mod presentation;
#[path = "script_preview.rs"]
mod script_preview;
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
};
use bw_rust::{
    ObjectGraph, model,
    model_render::{self, RenderModel},
    original_scene, parse_objects,
    skirmish::Ground,
    terrain,
    terrain_render::{self, TerrainMaterial},
    texture,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
};
#[derive(Resource)]
struct Loaded {
    terrain: Option<terrain::Terrain>,
    models: Vec<model::Model>,
    textures: BTreeMap<String, texture::DecodedTexture>,
    scene: original_scene::Scene,
    gallery: bool,
    animation: Option<bw_rust::animation::Animation>,
    ground: std::sync::Arc<Ground>,
}
#[derive(Resource, Default)]
struct CaptureTarget(Option<Handle<Image>>);
impl CaptureTarget {
    fn request(&self) -> Screenshot {
        self.0
            .as_ref()
            .map(|image| Screenshot::image(image.clone()))
            .unwrap_or_else(Screenshot::primary_window)
    }
}
#[derive(Resource)]
struct SkinParts(Vec<model_render::Part>);
#[derive(Resource)]
struct CameraState {
    center: Vec3,
    distance: f32,
    yaw: f32,
    resolved_yaw: f32,
    pitch: f32,
}
#[allow(dead_code)]
pub fn run(battle_mode: bool) {
    run_mode(battle_mode, false);
}
#[allow(dead_code)]
pub fn run_first_mission() {
    run_mode(false, true);
}
fn extract_skydome_name(graph: &ObjectGraph) -> Option<String> {
    for obj in graph.objects.values() {
        if obj.kind == "cNodeHierarchyResource"
            && let Some(name) = bw_rust::original_scene::field(obj, "mName")
            && name.starts_with("SKY_")
        {
            return Some(name.to_string());
        }
    }
    None
}
#[allow(clippy::type_complexity)]
fn load_level_bundle(
    data: &std::path::Path,
    level_name: &str,
    gallery_name: Option<&str>,
    mission_mode: bool,
) -> (
    Vec<u8>,
    terrain::Terrain,
    std::sync::Arc<Ground>,
    ObjectGraph,
    original_scene::Scene,
    Vec<model::Model>,
    BTreeMap<String, texture::DecodedTexture>,
    Vec<String>,
) {
    let archive = fs::read(data.join(format!("{level_name}_Level.res"))).unwrap();
    let terrain =
        terrain::decode(&fs::read(data.join(format!("{level_name}.out"))).unwrap()).unwrap();
    let ground = std::sync::Arc::new(Ground::from_terrain(&terrain));
    let mut graph = ObjectGraph::default();
    let level_xml = fs::read_to_string(data.join(format!("{level_name}_Level.xml"))).unwrap();
    let preload_xml = fs::read_to_string(data.join(format!("{level_name}_Level_preload.xml")))
        .or_else(|_| fs::read_to_string(data.join(format!("{level_name}_Level_Preload.xml"))))
        .unwrap();
    for xml in [&level_xml, &preload_xml] {
        graph.merge_dedup(parse_objects(xml).unwrap()).unwrap();
    }
    let scene = original_scene::load(&graph).unwrap();
    let mut wanted: BTreeSet<String> = if let Some(name) = gallery_name {
        [name.to_string()].into_iter().collect()
    } else {
        scene
            .placements
            .iter()
            .flat_map(|p| {
                std::iter::once(p.model.clone()).chain(p.attachments.iter().map(|(_, n)| n.clone()))
            })
            .collect()
    };
    if mission_mode {
        if let Some(sky) = extract_skydome_name(&graph) {
            wanted.insert(sky);
        } else {
            wanted.insert("SKY_C4Gambit".into());
        }
    }
    let mut models = Vec::new();
    let mut missing = Vec::new();
    for name in wanted {
        match model::from_archive(&archive, &name) {
            Ok(m) => models.push(m),
            Err(e) => missing.push(e),
        }
    }
    let mut textures = BTreeMap::new();
    for name in models
        .iter()
        .flat_map(|m| &m.nodes)
        .filter(|n| {
            !n.name.contains("NODRAW")
                && !n.name.contains("COLLIDE")
                && !n.name.contains("COLLISION")
        })
        .flat_map(|n| {
            n.surfaces
                .iter()
                .filter(|s| !s.indices.is_empty())
                .filter_map(|s| n.materials.get(s.material))
        })
        .filter_map(|m| m.iter().find(|s| !s.is_empty()))
        .map(|s| s.to_ascii_lowercase())
        .collect::<BTreeSet<_>>()
    {
        match texture::decode_named(&archive, &name) {
            Ok(t) => {
                textures.insert(name, t);
            }
            Err(e) => missing.push(e),
        }
    }
    for name in terrain
        .materials
        .iter()
        .flatten()
        .map(|n| n.to_ascii_lowercase())
        .collect::<BTreeSet<_>>()
    {
        textures.entry(name.clone()).or_insert_with(|| {
            texture::decode_named(&archive, &name).unwrap_or_else(|_| texture::DecodedTexture {
                width: 4,
                height: 4,
                rgba: vec![255; 64],
            })
        });
    }
    (
        archive, terrain, ground, graph, scene, models, textures, missing,
    )
}
fn run_mode(battle_mode: bool, mission_mode: bool) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let data = root.join("local/extracted/files/Data/CompoundFiles");
    let level_name = std::env::var("BW_LEVEL").unwrap_or_else(|_| "SP_0.1".to_string());
    let gallery_name = std::env::var("BW_MODEL").ok();
    let presenting = std::env::var_os("BW_PRESENTATION").is_some();
    assert!(
        !mission_mode
            || (!presenting
                && gallery_name.is_none()
                && std::env::var_os("BW_SCRIPT_PREVIEW").is_none()),
        "First mission cannot share diagnostic modes"
    );
    assert!(
        !presenting
            || (!battle_mode
                && gallery_name.is_none()
                && std::env::var_os("BW_SCRIPT_PREVIEW").is_none()),
        "Presentation requires the original static scene only"
    );
    let (archive, terrain, ground, graph, scene, models, textures, missing) =
        load_level_bundle(&data, &level_name, gallery_name.as_deref(), mission_mode);
    println!(
        "ORIGINAL_ASSET_AUDIT models={} placements={} omissions={} missing={missing:?}; mission_runtime={mission_mode}",
        models.len(),
        scene.placements.len(),
        scene.omitted.len()
    );
    if gallery_name.is_some() && !missing.is_empty() {
        panic!("Gallery dependencies missing");
    }
    let animation = std::env::var("BW_ANIMATION")
        .ok()
        .map(|name| bw_rust::animation::from_archive(&archive, &name).expect("Original animation"));
    assert!(
        animation.is_none() || gallery_name.is_some(),
        "Animation preview requires BW_MODEL gallery"
    );
    assert!(
        !battle_mode || gallery_name.is_none(),
        "Battle cannot run in model-gallery mode"
    );
    let battle = if battle_mode {
        Some(
            battle_view::BattleView::load(&graph, &archive, &scene)
                .expect("Original battle assets"),
        )
    } else {
        None
    };
    let preview = if !battle_mode && std::env::var_os("BW_SCRIPT_PREVIEW").is_some() {
        assert!(
            gallery_name.is_none(),
            "Script integration preview requires full scene, not gallery"
        );
        Some(
            script_preview::Preview::load(&graph, &archive, &scene).expect("Script preview inputs"),
        )
    } else {
        None
    };
    let mission = mission_mode.then(|| {
        first_mission_view::View::load(root, &level_name, &graph, &archive)
            .expect("First mission runtime")
    });
    let loaded = Loaded {
        animation,
        terrain: Some(terrain),
        ground,
        models,
        textures,
        scene,
        gallery: gallery_name.is_some(),
    };
    let mut app = App::new();
    let plugins = DefaultPlugins
        .set(AssetPlugin {
            file_path: root.join("assets").to_string_lossy().into(),
            ..default()
        })
        .set(WindowPlugin {
            primary_window: Some(Window {
                visible: !(mission_mode && std::env::var_os("BW_OFFSCREEN").is_some()),
                title: if mission_mode {
                    "Battalion Wars 2 — Rust Edition"
                } else if battle_mode {
                    "Battalion Wars Rust — Fort Assault (early playable)"
                } else if presenting {
                    "Battalion Wars Rust — Original Scene Restoration Preview"
                } else {
                    "Original asset restoration — NOT a restored mission"
                }
                .into(),
                resolution: (1280, 800).into(),
                ..default()
            }),
            ..default()
        });
    if mission_mode && std::env::var_os("BW_OFFSCREEN").is_some() {
        app.add_plugins(
            plugins
                .disable::<bevy::winit::WinitPlugin>()
                .set(WindowPlugin {
                    primary_window: None,
                    exit_condition: bevy::window::ExitCondition::DontExit,
                    ..default()
                }),
        );
        app.add_plugins(bevy::app::ScheduleRunnerPlugin::run_loop(
            std::time::Duration::from_millis(16),
        ));
    } else {
        app.add_plugins(plugins);
    }
    app.add_plugins(MaterialPlugin::<TerrainMaterial>::default())
        .init_resource::<bw_rust::mission_lightning_render::Renderer>()
        .insert_resource(loaded)
        .insert_resource(ClearColor(Color::srgb(0.46, 0.54, 0.64)))
        .add_systems(Startup, (setup, first_mission_view::setup).chain())
        .add_systems(
            Update,
            (
                assign_ui_camera,
                animate,
                script_preview::update,
                battle_view::update,
                presentation::update,
                reload_mission_level,
                first_mission_view::verify_input,
                first_mission_view::update,
                first_mission_view::lightning,
                first_mission_view::markers,
                first_mission_view::traces,
                update,
            )
                .chain(),
        );
    if presenting {
        app.insert_resource(presentation::Presentation::new());
    }
    if let Some(preview) = preview {
        app.insert_resource(preview);
    }
    if let Some(mission) = mission {
        app.insert_resource(mission);
    }
    if let Some(battle) = battle {
        app.insert_resource(battle);
    }
    if battle_mode || mission_mode {
        app.insert_resource(
            bw_rust::controls::Controls::parse(
                &fs::read_to_string(root.join("config/controls.json")).expect("Controls config"),
            )
            .expect("Valid controls"),
        );
    }
    app.run();
}
#[allow(clippy::too_many_arguments)] // Independent Bevy startup resources.
fn setup(
    mut commands: Commands,
    mut loaded: ResMut<Loaded>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut tmats: ResMut<Assets<TerrainMaterial>>,
    mut preview: Option<ResMut<script_preview::Preview>>,
    mut battle: Option<ResMut<battle_view::BattleView>>,
    mut mission: Option<ResMut<first_mission_view::View>>,
) {
    let mut terrain = loaded.terrain.take().unwrap();
    let mut textures = std::mem::take(&mut loaded.textures);
    let mut ttextures = BTreeMap::new();
    for name in terrain
        .materials
        .iter()
        .flatten()
        .map(|n| n.to_ascii_lowercase())
        .collect::<BTreeSet<_>>()
    {
        ttextures.insert(name.clone(), textures[&name].clone());
    }
    let handles = textures
        .iter_mut()
        .map(|(n, t)| (n.clone(), model_render::image(t.clone(), &mut images)))
        .collect();
    let mut camera_collision = bw_rust::camera_collision::CameraCollision::default();
    let mut collision_shapes = BTreeMap::new();
    // Keep source node identities; flattening loses the ability to retain specific debris.
    type NodeTriangles = BTreeMap<usize, Vec<[Vec3; 3]>>;
    let mut physical_shapes: BTreeMap<String, NodeTriangles> = BTreeMap::new();
    let mut unit_collision = bw_rust::unit_collision::Scene::default();
    unit_collision.ground = Some(loaded.ground.clone());
    let mut models: BTreeMap<String, RenderModel> = BTreeMap::new();
    for model in &loaded.models {
        match model_render::build(model, &handles, &mut meshes, &mut materials) {
            Ok(m) => {
                if preview.is_some() || battle.is_some() {
                    let mut nodes = NodeTriangles::new();
                    for part in &m.parts {
                        let triangles = nodes.entry(part.node_index).or_default();
                        let surface = &model.nodes[part.node_index].surfaces[part.surface_index];
                        for indices in surface.indices.chunks_exact(3) {
                            triangles.push([indices[0], indices[1], indices[2]].map(|i| {
                                part.transform.transform_point3(Vec3::from_array(
                                    surface.vertices[i as usize].position,
                                ))
                            }));
                        }
                    }
                    let triangles: Vec<_> = nodes.values().flatten().copied().collect();
                    if !triangles.is_empty() {
                        physical_shapes.insert(model.name.clone(), nodes);
                        collision_shapes.insert(
                            model.name.clone(),
                            camera_collision
                                .add_shape(triangles)
                                .expect("Camera collision shape"),
                        );
                    }
                }
                models.insert(model.name.clone(), m);
            }
            Err(e) => eprintln!("MODEL_OMITTED {}: {e}", model.name),
        }
    }
    let (center, distance) = if loaded.gallery {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for model in &loaded.models {
            let rm = &models[&model.name];
            for node in &model.nodes {
                let matrix = rm.bones[&node.name];
                for surface in &node.surfaces {
                    for v in &surface.vertices {
                        let p = matrix.transform_point3(Vec3::from_array(v.position));
                        min = min.min(p);
                        max = max.max(p);
                    }
                }
            }
            model_render::spawn(&mut commands, rm, Mat4::IDENTITY);
            if loaded.animation.is_some() {
                commands.insert_resource(SkinParts(rm.parts.clone()));
            }
        }
        assert!(min.is_finite() && max.is_finite());
        let center = (min + max) * 0.5;
        let distance = (max - min).length() * 1.6;
        (center, distance)
    } else {
        for p in &mut terrain.positions {
            *p = original_scene::grid_to_source(*p);
        }
        terrain_render::spawn(
            &mut commands,
            terrain,
            ttextures,
            &mut meshes,
            &mut images,
            &mut tmats,
        );
        let model_bounds: BTreeMap<String, bw_rust::model::Bounds> = loaded
            .models
            .iter()
            .map(|m| {
                (
                    m.name.clone(),
                    bw_rust::model_geometry::hierarchy_bounds(m).expect("Model hierarchy bounds"),
                )
            })
            .collect();
        let (resolved_matrices, outside_set) = loaded
            .scene
            .resolve_placements(&loaded.ground, &model_bounds)
            .expect("Scene placement resolution");
        let mut rendered = 0;
        let mut outside = 0;
        for (allocation_index, placement) in loaded.scene.placements.iter().enumerate() {
            if battle
                .as_ref()
                .is_some_and(|b| placement.kind == "cTroop" && !b.contains(placement.id))
            {
                continue;
            }
            let combatant = battle.as_ref().is_some_and(|b| b.contains(placement.id));
            let Some(model) = models.get(&placement.model) else {
                continue;
            };
            let root = Mat4::from_cols_array(&resolved_matrices[allocation_index]);
            if outside_set.contains(&placement.id) {
                outside += 1;
                if mission.is_none() {
                    continue;
                }
            }
            let actor = preview.as_ref().is_some_and(|p| p.id == placement.id);
            let blocker = preview
                .as_ref()
                .is_some_and(|p| p.blocker == Some(placement.id));
            if !actor
                && !blocker
                && !combatant
                && let Some(shape) = collision_shapes.get(&placement.model)
            {
                camera_collision
                    .add_instance(*shape, root)
                    .expect("Camera obstacle placement");
            }
            if !actor
                && !blocker
                && !combatant
                && let Some(nodes) = physical_shapes.get(&placement.model)
            {
                // Diagnostic viewer policy: one allocation token per placement for this
                // scene's lifetime. Not a native pointer or an authored object-ID cast.
                let owner = bw_rust::mission_destruction::Handle(
                    std::num::NonZeroU64::new(
                        u64::try_from(allocation_index)
                            .unwrap()
                            .checked_add(1)
                            .unwrap(),
                    )
                    .unwrap(),
                );
                unit_collision
                    .register_object_meshes(
                        placement.id,
                        owner,
                        nodes
                            .iter()
                            .map(|(node, triangles)| {
                                (
                                    *node,
                                    triangles
                                        .iter()
                                        .map(|t| t.map(|v| root.transform_point3(v).to_array()))
                                        .collect(),
                                )
                            })
                            .collect(),
                    )
                    .expect("Owned unit collision nodes");
            }
            if combatant {
                battle.as_mut().unwrap().spawn(
                    placement.id,
                    &mut commands,
                    model,
                    root,
                    &mut meshes,
                );
            } else if actor {
                preview
                    .as_mut()
                    .unwrap()
                    .spawn(&mut commands, model, root, &mut meshes);
            } else if blocker {
                script_preview::Preview::spawn_blocker(&mut commands, model, root, Mat4::IDENTITY);
            } else if let Some(mission) = mission.as_mut() {
                let (model_index, source) = loaded
                    .models
                    .iter()
                    .enumerate()
                    .find(|(_, m)| m.name == placement.model)
                    .unwrap();
                mission.spawn_placement(
                    &mut commands,
                    placement,
                    model,
                    source,
                    model_index,
                    root,
                    &mut meshes,
                );
            } else {
                model_render::spawn(&mut commands, model, root);
            }
            rendered += 1;
            for (bone, name) in &placement.attachments {
                if let (Some(matrix), Some(attachment)) = (model.bones.get(bone), models.get(name))
                {
                    if combatant {
                        let source = loaded
                            .models
                            .iter()
                            .find(|m| m.name == placement.model)
                            .unwrap();
                        let index = source
                            .nodes
                            .iter()
                            .position(|n| &n.name == bone)
                            .expect("Combatant attachment bone");
                        battle_view::BattleView::attachment(
                            placement.id,
                            &mut commands,
                            attachment,
                            index,
                            root * matrix,
                        );
                    } else if actor {
                        let source = loaded
                            .models
                            .iter()
                            .find(|m| m.name == placement.model)
                            .unwrap();
                        let index = source
                            .nodes
                            .iter()
                            .position(|n| &n.name == bone)
                            .expect("Attachment bone");
                        script_preview::Preview::attachment(
                            &mut commands,
                            attachment,
                            index,
                            root * matrix,
                        );
                    } else if blocker {
                        script_preview::Preview::spawn_blocker(
                            &mut commands,
                            attachment,
                            root,
                            *matrix,
                        );
                    } else if let Some(mission) = mission.as_ref() {
                        let source = loaded
                            .models
                            .iter()
                            .find(|m| m.name == placement.model)
                            .unwrap();
                        let attachment_source =
                            loaded.models.iter().find(|m| m.name == *name).unwrap();
                        let index = source.nodes.iter().position(|n| &n.name == bone);
                        mission.spawn_attachment(
                            &mut commands,
                            placement.id,
                            &placement.kind,
                            attachment,
                            attachment_source,
                            root,
                            *matrix,
                            index,
                            placement.stick_to_floor,
                        );
                    } else {
                        model_render::spawn(&mut commands, attachment, root * matrix);
                    }
                }
            }
        }
        if let Some(mission_ref) = mission.as_ref() {
            first_mission_view::spawn_water_plane(
                &mut commands,
                &mut meshes,
                &mut materials,
                mission_ref.water_height,
            );
            let sky_name = mission_ref
                .skydome_name
                .as_deref()
                .unwrap_or("SKY_C4Gambit");
            if let Some(sky_model) = models.get(sky_name) {
                first_mission_view::spawn_skydome(&mut commands, sky_model);
            }
        }
        println!(
            "ORIGINAL_SCENE_RENDER rendered={rendered} outside_terrain={outside}; runtime_visibility_is_applied_by_opt_in_game_drivers"
        );
        println!(
            "OWNED_COLLISION_SCENE objects={} nodes={} nonempty_meshes={}; render_derived_not_native_collision",
            unit_collision.registered_object_count(),
            unit_collision.owned_node_count(),
            unit_collision.mesh_count()
        );
        if let Some(b) = battle.as_mut() {
            b.install(unit_collision, &mut commands, &mut meshes, &mut materials)
                .expect("Battle spawn collision checks");
        } else if let Some(p) = preview.as_mut() {
            p.install_collision(unit_collision)
                .expect("Collision-aware actor placement");
        }
        if let Some(b) = battle.as_ref() {
            (b.center(), 22.0)
        } else if let Some(p) = preview.as_ref() {
            (p.center(), 18.0)
        } else {
            (Vec3::new(0.0, 20.0, 150.0), 400.0)
        }
    };
    if preview.is_some() || battle.is_some() {
        println!(
            "CAMERA_COLLISION_INSTANCES {}",
            camera_collision.instance_count()
        );
        commands.insert_resource(camera_collision);
    }
    commands.insert_resource(CameraState {
        center,
        distance,
        yaw: if battle.is_some() {
            std::f32::consts::FRAC_PI_2
        } else {
            0.55
        },
        resolved_yaw: if battle.is_some() {
            std::f32::consts::FRAC_PI_2
        } else {
            0.55
        },
        pitch: if battle.is_some() { 0.38 } else { 0.25 },
    });
    let camera_entity = commands
        .spawn((
            Camera3d::default(),
            Projection::Perspective(PerspectiveProjection {
                far: 25000.0,
                ..default()
            }),
            Transform::from_translation(
                center + Vec3::new(distance * 0.5, distance * 0.3, distance),
            )
            .looking_at(center, Vec3::Y),
        ))
        .id();
    let mut capture = CaptureTarget::default();
    if std::env::var_os("BW_OFFSCREEN").is_some() {
        use bevy::render::render_resource::{TextureFormat, TextureUsages};
        let mut image = Image::new_target_texture(
            1280,
            800,
            TextureFormat::Rgba8Unorm,
            Some(TextureFormat::Rgba8UnormSrgb),
        );
        image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
        let handle = images.add(image);
        commands
            .entity(camera_entity)
            .insert(bevy::camera::RenderTarget::Image(handle.clone().into()));
        capture.0 = Some(handle);
    }
    commands.insert_resource(capture);
    commands.spawn((
        DirectionalLight {
            illuminance: 14000.0,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, -0.4, 0.0)),
    ));
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.96, 0.97, 1.0),
        brightness: 950.0,
        ..default()
    });
    if std::env::var_os("BW_PRESENTATION").is_some() {
        commands.spawn((
            Text::new("BATTALION WARS / RUST"),
            TextFont {
                font_size: FontSize::Px(30.0),
                ..default()
            },
            TextColor(Color::srgb(0.93, 0.78, 0.39)),
            Node {
                position_type: PositionType::Absolute,
                top: px(22),
                left: px(26),
                ..default()
            },
        ));
        commands.spawn((
            presentation::Status,
            Text::new("Loading original battlefield…"),
            TextFont {
                font_size: FontSize::Px(16.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.025, 0.04, 0.06, 0.94)),
            Node {
                position_type: PositionType::Absolute,
                bottom: px(20),
                left: px(20),
                padding: UiRect::all(px(16)),
                ..default()
            },
        ));
    } else if battle.is_none() && mission.is_none() {
        let mut hud=commands.spawn((Text::new(if loaded.animation.is_some(){"ORIGINAL ROTATION CLIP | experimental skinning, no root motion\nArrows orbit | W/S zoom | A/D pan | Esc close"}else{"ORIGINAL ASSET RECOVERY | static/rest pose - no mission logic\nArrows orbit | W/S zoom | A/D pan | Esc close"}),TextFont{font_size:FontSize::Px(20.0),..default()},Node{position_type:PositionType::Absolute,top:px(14),left:px(14),..default()}));
        if preview.is_some() {
            hud.insert(script_preview::DiagnosticHud);
        }
    }
}
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn reload_mission_level(
    mut commands: Commands,
    mut loaded: ResMut<Loaded>,
    mut mission: Option<ResMut<first_mission_view::View>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut tmats: ResMut<Assets<TerrainMaterial>>,
    mut audio_assets: ResMut<Assets<AudioSource>>,
    old_scene: Query<
        Entity,
        Or<(
            With<first_mission_view::SceneEntity>,
            With<MeshMaterial3d<TerrainMaterial>>,
            With<first_mission_view::GoldStar>,
        )>,
    >,
) {
    let Some(mission) = mission.as_mut() else {
        return;
    };
    let Some(level_name) = mission.pending_level.take() else {
        return;
    };
    for entity in &old_scene {
        commands.entity(entity).despawn();
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let data = root.join("local/extracted/files/Data/CompoundFiles");
    let (archive, mut terrain, ground, graph, scene, models, mut textures, _missing) =
        load_level_bundle(&data, &level_name, None, true);
    mission
        .reload_level(root, &level_name, &graph, &archive)
        .expect("Reload campaign mission runtime");
    for (name, decoded) in std::mem::take(&mut mission.ui_textures) {
        mission
            .ui_handles
            .insert(name, model_render::image(decoded, &mut images));
    }
    for (name, wav) in std::mem::take(&mut mission.music_wavs) {
        mission
            .music_handles
            .insert(name, audio_assets.add(AudioSource { bytes: wav.into() }));
    }
    for (name, wav) in std::mem::take(&mut mission.sfx_wavs) {
        mission
            .sfx_handles
            .entry(name)
            .or_insert_with(|| audio_assets.add(AudioSource { bytes: wav.into() }));
    }

    let mut ttextures = BTreeMap::new();
    for name in terrain
        .materials
        .iter()
        .flatten()
        .map(|n| n.to_ascii_lowercase())
        .collect::<BTreeSet<_>>()
    {
        ttextures.insert(name.clone(), textures[&name].clone());
    }
    let handles = textures
        .iter_mut()
        .map(|(n, t)| (n.clone(), model_render::image(t.clone(), &mut images)))
        .collect();
    let mut render_models: BTreeMap<String, RenderModel> = BTreeMap::new();
    for model in &models {
        if let Ok(m) = model_render::build(model, &handles, &mut meshes, &mut materials) {
            render_models.insert(model.name.clone(), m);
        }
    }
    for p in &mut terrain.positions {
        *p = original_scene::grid_to_source(*p);
    }
    terrain_render::spawn(
        &mut commands,
        terrain,
        ttextures,
        &mut meshes,
        &mut images,
        &mut tmats,
    );
    let model_bounds: BTreeMap<String, bw_rust::model::Bounds> = models
        .iter()
        .map(|m| {
            (
                m.name.clone(),
                bw_rust::model_geometry::hierarchy_bounds(m).expect("Model hierarchy bounds"),
            )
        })
        .collect();
    let (resolved_matrices, _) = scene
        .resolve_placements(&ground, &model_bounds)
        .expect("Scene placement resolution");
    loaded.ground = ground;
    loaded.models = models;
    loaded.scene = scene;
    for (allocation_index, placement) in loaded.scene.placements.iter().enumerate() {
        let Some(model) = render_models.get(&placement.model) else {
            continue;
        };
        let root_mat = Mat4::from_cols_array(&resolved_matrices[allocation_index]);
        let (model_index, source) = loaded
            .models
            .iter()
            .enumerate()
            .find(|(_, m)| m.name == placement.model)
            .unwrap();
        mission.spawn_placement(
            &mut commands,
            placement,
            model,
            source,
            model_index,
            root_mat,
            &mut meshes,
        );
        for (bone, name) in &placement.attachments {
            if let (Some(matrix), Some(attachment)) =
                (model.bones.get(bone), render_models.get(name))
            {
                let source = loaded
                    .models
                    .iter()
                    .find(|m| m.name == placement.model)
                    .unwrap();
                let attachment_source = loaded.models.iter().find(|m| m.name == *name).unwrap();
                let index = source.nodes.iter().position(|n| &n.name == bone);
                mission.spawn_attachment(
                    &mut commands,
                    placement.id,
                    &placement.kind,
                    attachment,
                    attachment_source,
                    root_mat,
                    *matrix,
                    index,
                    placement.stick_to_floor,
                );
            }
        }
    }
    first_mission_view::spawn_water_plane(
        &mut commands,
        &mut meshes,
        &mut materials,
        mission.water_height,
    );
    let sky_name = mission.skydome_name.as_deref().unwrap_or("SKY_C4Gambit");
    if let Some(sky_model) = render_models.get(sky_name) {
        first_mission_view::spawn_skydome(&mut commands, sky_model);
    }
    first_mission_view::spawn_gold_stars(
        &mut commands,
        &mission.session.markers,
        &mut meshes,
        &mut materials,
    );
}
#[allow(clippy::type_complexity)]
fn assign_ui_camera(
    mut commands: Commands,
    cameras: Query<Entity, With<Camera3d>>,
    roots: Query<Entity, (With<Node>, Without<ChildOf>, Without<UiTargetCamera>)>,
) {
    if let Ok(camera) = cameras.single() {
        for root in &roots {
            commands.entity(root).insert(UiTargetCamera(camera));
        }
    }
}
fn animate(
    loaded: Res<Loaded>,
    parts: Option<Res<SkinParts>>,
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut frame: Local<u32>,
) {
    let (Some(animation), Some(parts)) = (&loaded.animation, parts) else {
        return;
    };
    *frame += 1;
    let seconds = std::env::var("BW_POSE_TIME")
        .ok()
        .map(|s| s.parse::<f32>().expect("Finite pose time"))
        .unwrap_or_else(|| {
            if std::env::var_os("BW_SMOKE").is_some() {
                *frame as f32 / 30.0
            } else {
                time.elapsed_secs()
            }
        });
    assert!(seconds.is_finite());
    let t = seconds.rem_euclid(animation.duration.max(1.0 / 30.0));
    let model = &loaded.models[0];
    let pose = bw_rust::skinning::pose(model, Some((animation, t))).expect("Bone pose");
    for part in &parts.0 {
        if !model.nodes[part.node_index].has_skin_data {
            continue;
        }
        let positions =
            bw_rust::skinning::deform(model, &pose, part.node_index, part.surface_index)
                .expect("Skin palette");
        let mut mesh = meshes.get_mut(&part.mesh).unwrap();
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    }
}
#[derive(Default)]
struct CameraChecks {
    clipped_frames: u32,
    max_adjustment: f32,
}
#[allow(clippy::too_many_arguments)] // Bevy system resources.
fn update(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut state: ResMut<CameraState>,
    mut camera: Query<&mut Transform, With<Camera3d>>,
    mut frame: Local<u32>,
    mut exit: MessageWriter<AppExit>,
    collision: Option<Res<bw_rust::camera_collision::CameraCollision>>,
    mut checks: Local<CameraChecks>,
    battle: Option<Res<battle_view::BattleView>>,
    capture: Res<CaptureTarget>,
    presentation: Option<Res<presentation::Presentation>>,
    mission: Option<Res<first_mission_view::View>>,
) {
    *frame += 1;
    let dt = time.delta_secs();
    let interactive = std::env::var_os("BW_SMOKE").is_none()
        && battle.is_none()
        && presentation.is_none()
        && mission.is_none();
    if interactive && keys.pressed(KeyCode::ArrowLeft) {
        state.yaw += dt;
    }
    if interactive && keys.pressed(KeyCode::ArrowRight) {
        state.yaw -= dt;
    }
    if interactive && keys.pressed(KeyCode::ArrowUp) {
        state.pitch = (state.pitch + dt).min(1.4);
    }
    if interactive && keys.pressed(KeyCode::ArrowDown) {
        state.pitch = (state.pitch - dt).max(-1.4);
    }
    if interactive && keys.pressed(KeyCode::KeyW) {
        state.distance *= (-dt).exp();
    }
    if interactive && keys.pressed(KeyCode::KeyS) {
        state.distance *= dt.exp();
    }
    let speed = state.distance * dt;
    if interactive && keys.pressed(KeyCode::KeyA) {
        state.center.x -= speed;
    }
    if interactive && keys.pressed(KeyCode::KeyD) {
        state.center.x += speed;
    }
    if collision.is_some() {
        state.distance = state.distance.clamp(0.5, 4000.0);
    }
    for mut t in &mut camera {
        if mission.is_some() {
            continue;
        }
        let direction = Vec3::new(
            state.yaw.sin() * state.pitch.cos(),
            state.pitch.sin(),
            state.yaw.cos() * state.pitch.cos(),
        );
        let desired = state.center + direction * state.distance;
        t.translation = if let Some(collision) = &collision {
            collision
                .follow_position(state.center, desired, 0.35)
                .expect("Camera probe")
        } else {
            desired
        };
        if std::env::var_os("BW_REQUIRE_CAMERA_CLIP").is_some()
            && let Some(collision) = &collision
        {
            assert!(
                collision
                    .nearest_fraction(state.center, t.translation)
                    .expect("Corrected camera segment")
                    .is_none(),
                "Camera still crosses scene geometry"
            );
        }
        let relative = t.translation - state.center;
        state.resolved_yaw = relative.x.atan2(relative.z);
        let aim = (state.center - t.translation)
            .try_normalize()
            .unwrap_or(-direction);
        t.look_to(aim, Vec3::Y);
        let retraction = t.translation.distance(desired);
        if retraction > 0.001 {
            checks.clipped_frames += 1;
            checks.max_adjustment = checks.max_adjustment.max(retraction);
        }
    }
    if *frame == 180 && collision.is_some() && std::env::var_os("BW_SMOKE").is_some() {
        println!(
            "CAMERA_COLLISION_SMOKE adjusted_frames={} max_position_adjustment={}",
            checks.clipped_frames, checks.max_adjustment
        );
        if std::env::var_os("BW_REQUIRE_CAMERA_CLIP").is_some() {
            assert!(
                checks.clipped_frames > 0,
                "Regression scene did not exercise obstruction handling"
            );
        }
    }
    let capture_frame = std::env::var("BW_CAPTURE_FRAME")
        .map(|s| s.parse::<u32>().expect("Capture frame must be an integer"))
        .unwrap_or(100);
    if *frame == capture_frame
        && let Ok(path) = std::env::var("BW_CAPTURE")
    {
        commands
            .spawn(capture.request())
            .observe(save_to_disk(path));
    }
    if let (Ok(frames), Ok(path)) = (
        std::env::var("BW_CAPTURE_FRAMES"),
        std::env::var("BW_CAPTURE"),
    ) {
        let frames: Vec<u32> = frames
            .split(',')
            .map(|n| n.parse().expect("Invalid capture series frame"))
            .collect();
        assert!(frames.len() <= 16 && frames.iter().all(|n| (1..=4000).contains(n)));
        if frames.contains(&*frame) {
            commands
                .spawn(capture.request())
                .observe(save_to_disk(format!("{path}.frame-{:04}.png", *frame)));
        }
    }
    if mission
        .as_ref()
        .map_or(keys.just_pressed(KeyCode::Escape), |v| v.should_exit)
        || std::env::var_os("BW_SMOKE").is_some()
            && *frame >= mission.as_ref().map_or(180, |v| v.smoke_frames)
            && (battle.is_none() || std::env::var_os("BW_BATTLE_VERIFY").is_none())
    {
        exit.write(AppExit::Success);
    }
}
