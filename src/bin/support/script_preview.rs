//! Explicit original-script/renderer integration diagnostic, not a mission launcher.
use super::{CameraState, Loaded};
use bevy::prelude::*;
use bw_rust::{
    ObjectGraph, ObjectId, Result,
    animation::{self, Animation},
    archive::{chunks, named_payload},
    lua50::{self, Bootstrap, Value},
    mission_vm::{Host, Scheduler, Thread},
    mission_world::Navigation,
    model_render::{Part, RenderModel},
    original_scene::Scene,
};
use std::collections::BTreeMap;
#[derive(Resource)]
pub struct Preview {
    pub id: ObjectId,
    pub blocker: Option<ObjectId>,
    pub model: String,
    vm: Bootstrap,
    host: Host,
    scheduler: Scheduler,
    clips: BTreeMap<String, Animation>,
    parts: Vec<Part>,
    home: [f32; 3],
    last: [f32; 3],
    facing: Quat,
    frames: u32,
}
#[derive(Component)]
pub struct ActorPart {
    offset: Mat4,
    bone: Option<usize>,
}
#[derive(Component)]
pub struct BlockerPart {
    offset: Mat4,
    orientation: Quat,
}
#[derive(Component)]
pub struct DiagnosticHud;
impl Preview {
    pub fn load(graph: &ObjectGraph, archive: &[u8], scene: &Scene) -> Result<Self> {
        let mut scripts = BTreeMap::new();
        for c in chunks(archive)? {
            if c.tag == *b"PRCS" {
                let (n, b) = named_payload(c.payload)?;
                scripts.insert(n.to_string(), lua50::parse(b)?);
            }
        }
        let mut vm = Bootstrap::new();
        for (n, p) in &scripts {
            vm.execute(n, p, graph, 100000)?;
        }
        let Value::Table(t) = vm.globals["Troop"] else {
            return Err("Missing troop table".into());
        };
        let Value::Object(id) = vm.tables[t]["Sgrunt02"] else {
            return Err("Missing actor".into());
        };
        let blocker = if std::env::var_os("BW_UNIT_PAIR").is_some() {
            let Value::Object(other) = vm.tables[t]["Sgrunt03"] else {
                return Err("Missing blocker".into());
            };
            Some(other)
        } else {
            None
        };
        if blocker.is_some() && std::env::var_os("BW_UNIT_COLLISION_DISABLED").is_some() {
            return Err("Pair fixture requires unit collision".into());
        }
        if std::env::var_os("BW_REQUIRE_ROUTE").is_some()
            && (std::env::var_os("BW_UNIT_COLLISION_DISABLED").is_some()
                || std::env::var_os("BW_PATHFINDING_DISABLED").is_some())
        {
            return Err("Required route gate conflicts with disabled collision/navigation".into());
        }
        let mut host = Host::from_graph(graph)?;
        host.install(&mut vm);
        bw_rust::mission_world::World::install(&mut vm)?;
        host.world.navigation = Navigation::StraightLineDiagnostic;
        host.world
            .units
            .get_mut(&id)
            .ok_or("Missing actor unit")?
            .active = Some(true);
        let mut scheduler = Scheduler::default();
        scheduler.threads.insert(
            1,
            Thread::new(
                "Gambit_Part_1_Stay_Here",
                scripts["Gambit_Part_1_Stay_Here"].children[0].clone(),
                &[Value::Object(id)],
            )?,
        );
        scheduler.tick(&mut vm, &mut host, 0.0, 10000)?;
        // Explicit diagnostic input, NOT an executed original opening cinematic.
        vm.globals
            .insert("cutsceneStart".into(), Value::Number(1.0));
        let home = host.positions[&id];
        if let Some(other) = blocker {
            host.world
                .units
                .get_mut(&other)
                .ok_or("Missing blocker unit")?
                .active = Some(true);
            host.positions
                .insert(other, [home[0] + 12.0, home[1], home[2]]);
        }
        let placement = scene
            .placements
            .iter()
            .find(|p| p.id == id)
            .ok_or("Missing actor model")?;
        let mut clips = BTreeMap::new();
        for n in host.world.units[&id]
            .animation
            .as_ref()
            .ok_or("Missing animation bindings")?
            .bindings
            .values()
        {
            clips.insert(n.clone(), animation::from_archive(archive, n)?);
        }
        host.positions.get_mut(&id).unwrap()[0] += 100.0;
        let last = host.positions[&id];
        Ok(Self {
            id,
            blocker,
            model: placement.model.clone(),
            vm,
            host,
            scheduler,
            clips,
            parts: vec![],
            home,
            last,
            facing: Quat::IDENTITY,
            frames: 0,
        })
    }
    pub fn spawn(
        &mut self,
        commands: &mut Commands,
        model: &RenderModel,
        root: Mat4,
        meshes: &mut Assets<Mesh>,
    ) {
        self.facing = Transform::from_matrix(root).rotation;
        for part in &model.parts {
            let mut p = part.clone();
            p.mesh = meshes.add(meshes.get(&part.mesh).expect("Source mesh").clone());
            commands.spawn((
                ActorPart {
                    offset: p.transform,
                    bone: None,
                },
                Mesh3d(p.mesh.clone()),
                MeshMaterial3d(p.material.clone()),
                Transform::from_matrix(root * p.transform),
            ));
            self.parts.push(p);
        }
    }
    pub fn attachment(commands: &mut Commands, model: &RenderModel, bone: usize, root: Mat4) {
        for p in &model.parts {
            commands.spawn((
                ActorPart {
                    offset: p.transform,
                    bone: Some(bone),
                },
                Mesh3d(p.mesh.clone()),
                MeshMaterial3d(p.material.clone()),
                Transform::from_matrix(root * p.transform),
            ));
        }
    }
    pub fn spawn_blocker(commands: &mut Commands, model: &RenderModel, root: Mat4, bone: Mat4) {
        for p in &model.parts {
            commands.spawn((
                BlockerPart {
                    offset: bone * p.transform,
                    orientation: Transform::from_matrix(root).rotation,
                },
                Mesh3d(p.mesh.clone()),
                MeshMaterial3d(p.material.clone()),
                Transform::from_matrix(root * bone * p.transform),
            ));
        }
    }
    pub fn install_collision(&mut self, scene: bw_rust::unit_collision::Scene) -> Result<()> {
        if std::env::var_os("BW_UNIT_COLLISION_DISABLED").is_some() {
            return Ok(());
        }
        let mut others = Vec::new();
        if let Some(other) = self.blocker {
            let body = self.host.world.bodies[&other];
            let position = scene.find_free(self.host.positions[&other], body, &[])?;
            self.host.positions.insert(other, position);
            others.push(bw_rust::unit_collision::Other { position, body });
        }
        let preferred = [self.home[0] + 40.0, self.home[1], self.home[2]];
        let position = scene.find_free(preferred, self.host.world.bodies[&self.id], &others)?;
        println!(
            "UNIT_COLLISION_SCENE meshes={} initial_adjustment={}",
            scene.mesh_count(),
            Vec3::from_array(position).distance(Vec3::from_array(preferred))
        );
        self.host.positions.insert(self.id, position);
        self.last = position;
        self.host.world.collision = Some(std::sync::Arc::new(scene));
        self.host.world.navigation = if std::env::var_os("BW_PATHFINDING_DISABLED").is_some() {
            Navigation::CollisionAware
        } else {
            Navigation::PlannedInfantry
        };
        Ok(())
    }
    pub fn center(&self) -> Vec3 {
        Vec3::from_array(self.host.positions[&self.id])
    }
}
#[allow(clippy::too_many_arguments)] // Bevy system parameters are independently borrowed resources.
pub fn update(
    mut preview: Option<ResMut<Preview>>,
    loaded: Res<Loaded>,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut transforms: Query<(&ActorPart, &mut Transform), Without<BlockerPart>>,
    mut blockers: Query<(&BlockerPart, &mut Transform), Without<ActorPart>>,
    mut camera: ResMut<CameraState>,
    mut hud: Query<&mut Text, With<DiagnosticHud>>,
) {
    let Some(ref mut p) = preview else { return };
    assert!(
        !p.parts.is_empty(),
        "Selected original actor was not rendered"
    );
    p.frames += 1;
    let interactive = std::env::var_os("BW_SMOKE").is_none();
    if interactive && keys.just_pressed(KeyCode::KeyP) {
        p.scheduler.paused = !p.scheduler.paused;
    }
    if interactive && keys.just_pressed(KeyCode::KeyR) && p.host.world.units[&p.id].health > 0.0 {
        let id = p.id;
        let home = p.home;
        let offset = if p.host.world.collision.is_some() {
            40.0
        } else {
            100.0
        };
        let mut position = [home[0] + offset, home[1], home[2]];
        if let Some(scene) = &p.host.world.collision {
            let others = p
                .blocker
                .map(|other| bw_rust::unit_collision::Other {
                    position: p.host.positions[&other],
                    body: p.host.world.bodies[&other],
                })
                .into_iter()
                .collect::<Vec<_>>();
            position = scene
                .find_free(position, p.host.world.bodies[&id], &others)
                .expect("Safe diagnostic displacement");
        }
        p.host.positions.insert(id, position);
        p.host.world.routes.remove(&id);
    }
    let damage_frame = std::env::var("BW_SCRIPT_DAMAGE_FRAME")
        .map(|s| s.parse::<u32>().expect("Damage frame must be an integer"))
        .unwrap_or(100);
    if interactive && keys.just_pressed(KeyCode::KeyK)
        || p.frames == damage_frame
            && std::env::var_os("BW_SMOKE").is_some()
            && std::env::var_os("BW_SCRIPT_SMOKE_DEATH").is_some()
    {
        let id = p.id;
        let hp = p.host.world.units[&id].max_health;
        p.host.world.damage(id, hp).expect("Diagnostic damage");
    }
    let dt = if std::env::var_os("BW_SMOKE").is_some() {
        1.0 / 30.0
    } else {
        time.delta_secs_f64().min(0.1)
    };
    let Preview {
        vm,
        host,
        scheduler,
        ..
    } = &mut **p;
    scheduler
        .tick(vm, host, dt, 10000)
        .expect("Original script stopped on unsupported behavior");
    let position = p.host.positions[&p.id];
    let delta = Vec3::from_array(position) - Vec3::from_array(p.last);
    if delta.x * delta.x + delta.z * delta.z > 1e-8 {
        p.facing = Quat::from_rotation_y(delta.x.atan2(delta.z));
    }
    p.last = position;
    let model = loaded
        .models
        .iter()
        .find(|m| m.name == p.model)
        .expect("Actor model");
    let player = p.host.world.units[&p.id].animation.as_ref().unwrap();
    let clip = &p.clips[&player.bindings[&player.motion]];
    let pose = bw_rust::skinning::pose(model, Some((clip, player.sample_time(clip).unwrap())))
        .expect("Experimental pose");
    for part in &p.parts {
        if model.nodes[part.node_index].has_skin_data {
            let vertices =
                bw_rust::skinning::deform(model, &pose, part.node_index, part.surface_index)
                    .expect("Skinning");
            let mut mesh = meshes.get_mut(&part.mesh).unwrap();
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
            mesh.compute_smooth_normals();
        }
    }
    let [x, z] = bw_rust::original_scene::source_to_grid(position[0], position[2]);
    let ground = loaded
        .ground
        .height(x, z)
        .expect("Diagnostic path left extracted terrain");
    let center = Vec3::new(position[0], position[1] + ground * 2.0, position[2]);
    let root = Mat4::from_rotation_translation(p.facing, center);
    for (part, mut transform) in &mut transforms {
        *transform = Transform::from_matrix(
            root * part.bone.map(|i| pose[i]).unwrap_or(Mat4::IDENTITY) * part.offset,
        );
    }
    if let Some(other) = p.blocker {
        let position = p.host.positions[&other];
        let [x, z] = bw_rust::original_scene::source_to_grid(position[0], position[2]);
        let y = loaded.ground.height(x, z).expect("Blocker ground") * 2.0 + position[1];
        for (part, mut transform) in &mut blockers {
            *transform = Transform::from_matrix(
                Mat4::from_rotation_translation(
                    part.orientation,
                    Vec3::new(position[0], y, position[2]),
                ) * part.offset,
            );
        }
    }
    camera.center = center + Vec3::Y * 2.0;
    let distance = ((position[0] - p.home[0]).powi(2) + (position[2] - p.home[2]).powi(2)).sqrt();
    for mut text in &mut hud {
        **text = format!(
            "ORIGINAL SCRIPT INTEGRATION - NOT COMPLETE MISSION\nStay_Here | {:?} | home distance {:.1} | paused={}\nR displace | P pause | K damage | arrows orbit | W/S zoom\n{} | searches={} arrivals={} failures={}\nExperimental poses; replacement navigation, not original mission startup.",
            player.motion,
            distance,
            p.scheduler.paused,
            if p.host.world.navigation == Navigation::PlannedInfantry {
                "Collision + bounded route planning"
            } else if p.host.world.collision.is_some() {
                "Collision enabled; planning disabled"
            } else {
                "Collision disabled (legacy diagnostic)"
            },
            p.host.world.route_searches,
            p.host.world.route_arrivals,
            p.host.world.route_failures
        );
    }
    if p.frames == 180 && std::env::var_os("BW_SMOKE").is_some() {
        if std::env::var_os("BW_SCRIPT_SMOKE_DEATH").is_some() {
            assert_eq!(p.host.world.units[&p.id].health, 0.0);
            assert!(matches!(
                p.scheduler.threads[&1].status,
                bw_rust::mission_vm::Status::Complete(_)
            ));
            assert_eq!(player.sample_time(clip).unwrap(), clip.duration);
            println!(
                "ORIGINAL_SCRIPT_RENDER_SMOKE death=true script_complete=true death_clip_clamped=true native_mission_complete=false"
            );
        } else if let Some(scene) = &p.host.world.collision {
            if let Some(other) = p.blocker {
                assert!(
                    p.host.world.unit_contacts > 0,
                    "Pair fixture must exercise live-body collision"
                );
                let partner = bw_rust::unit_collision::Other {
                    position: p.host.positions[&other],
                    body: p.host.world.bodies[&other],
                };
                assert!(
                    !scene
                        .overlaps(position, p.host.world.bodies[&p.id], &[partner])
                        .unwrap()
                );
            }
            assert!(
                !scene
                    .overlaps(position, p.host.world.bodies[&p.id], &[])
                    .unwrap()
            );
            if std::env::var_os("BW_REQUIRE_UNIT_CONTACT").is_some() {
                assert!(p.host.world.mesh_contacts > 0);
            }
            if std::env::var_os("BW_REQUIRE_ROUTE").is_some() {
                assert!(
                    p.host.world.route_searches > 0
                        && p.host.world.route_arrivals > 0
                        && distance <= 5.01,
                    "Route gate must reach home after an actual plan"
                );
            }
            println!(
                "UNIT_NAVIGATION_SMOKE searches={} arrivals={} failures={} expansions={} probes={} distance={distance}",
                p.host.world.route_searches,
                p.host.world.route_arrivals,
                p.host.world.route_failures,
                p.host.world.route_expansions,
                p.host.world.route_probes
            );
            println!(
                "UNIT_COLLISION_SMOKE overlap=false contacts={} mesh_contacts={} unit_contacts={} remaining_home_distance={distance}; full_mission=false",
                p.host.world.blocked_contacts,
                p.host.world.mesh_contacts,
                p.host.world.unit_contacts
            );
        } else {
            assert!(distance <= 5.01);
            println!(
                "ORIGINAL_SCRIPT_RENDER_SMOKE returned_home=true distance={distance} animation={:?} native_mission_complete=false",
                player.motion
            );
        }
    }
}
