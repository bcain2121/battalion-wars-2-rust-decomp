use super::{CameraState, Loaded};
use bevy::{input::mouse::MouseWheel, prelude::*, render::view::screenshot::save_to_disk};
use bw_rust::{
    ObjectGraph, ObjectId, Result,
    animation::{self, Animation},
    archive::{chunks, named_payload},
    battle::{Battle, Input, Phase, Team},
    controls::Controls,
    lua50::{self, Bootstrap, Value},
    mission_world::World,
    model_render::{Part, RenderModel},
    unit_animation::Motion,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
struct Actor {
    model: String,
    parts: Vec<Part>,
}
#[derive(Resource)]
pub struct BattleView {
    pub game: Battle,
    actors: BTreeMap<ObjectId, Actor>,
    clips: BTreeMap<String, Animation>,
    anchor: [f32; 3],
    initial: Option<Battle>,
    frames: u32,
    verify: Option<String>,
    finished: Option<u32>,
    restarted: bool,
    bot_path: VecDeque<[f64; 2]>,
    seen: BTreeSet<Motion>,
}
#[derive(Component)]
pub struct ActorPart {
    id: ObjectId,
    offset: Mat4,
    bone: Option<usize>,
}
#[derive(Component)]
pub struct Marker(ObjectId);
#[derive(Component)]
pub struct Hud;
#[derive(Component)]
pub struct BeamLife(f32);
#[derive(Resource)]
pub struct Visuals {
    beam: Handle<Mesh>,
    blue: Handle<StandardMaterial>,
    red: Handle<StandardMaterial>,
    gold: Handle<StandardMaterial>,
}
impl BattleView {
    pub fn load(
        graph: &ObjectGraph,
        archive: &[u8],
        scene: &bw_rust::original_scene::Scene,
    ) -> Result<Self> {
        let mut vm = Bootstrap::new();
        for c in chunks(archive)? {
            if c.tag == *b"PRCS" {
                let (n, b) = named_payload(c.payload)?;
                vm.execute(n, &lua50::parse(b)?, graph, 100000)?;
            }
        }
        let Value::Table(t) = vm.globals["Troop"] else {
            return Err("Missing troop symbols".into());
        };
        let Value::Object(player) = vm.tables[t]["Sgrunt02"] else {
            return Err("Missing player".into());
        };
        let world = World::from_graph(graph)?;
        let p = scene
            .placements
            .iter()
            .find(|p| p.id == player)
            .ok_or("Missing original player placement")?;
        let anchor = [p.matrix[12], p.matrix[13], p.matrix[14]];
        let available = scene
            .placements
            .iter()
            .filter(|p| {
                p.kind == "cTroop"
                    && world.units.get(&p.id).is_some_and(|u| {
                        u.animation.as_ref().is_some_and(|a| a.bindings.len() >= 5)
                    })
            })
            .collect::<Vec<_>>();
        let mut teams = BTreeMap::from([(player, Team::Friendly)]);
        for p in available
            .iter()
            .filter(|p| {
                p.id != player
                    && world.units[&p.id].army == world.units[&player].army
                    && p.model
                        == scene
                            .placements
                            .iter()
                            .find(|p| p.id == player)
                            .unwrap()
                            .model
            })
            .take(2)
        {
            teams.insert(p.id, Team::Friendly);
        }
        for p in available
            .iter()
            .filter(|p| world.units[&p.id].army != world.units[&player].army)
            .take(4)
        {
            teams.insert(p.id, Team::Enemy);
        }
        if teams.len() != 7 {
            return Err("Expected player, two squadmates and four original enemy troops".into());
        }
        let mut clips = BTreeMap::new();
        let mut actors = BTreeMap::new();
        for &id in teams.keys() {
            let model = scene
                .placements
                .iter()
                .find(|p| p.id == id)
                .ok_or("Missing combatant model")?
                .model
                .clone();
            for name in world.units[&id]
                .animation
                .as_ref()
                .unwrap()
                .bindings
                .values()
            {
                if !clips.contains_key(name) {
                    clips.insert(name.clone(), animation::from_archive(archive, name)?);
                }
            }
            println!(
                "BATTLE_ACTOR id={} model={} army={} team={:?}",
                id.0, model, world.units[&id].army, teams[&id]
            );
            actors.insert(
                id,
                Actor {
                    model,
                    parts: vec![],
                },
            );
        }
        let verify = std::env::var("BW_BATTLE_VERIFY").ok();
        if verify
            .as_ref()
            .is_some_and(|s| !matches!(s.as_str(), "win" | "loss" | "restart"))
        {
            return Err("Battle verify must be win/loss/restart".into());
        }
        Ok(Self {
            game: Battle::new(world, player, teams)?,
            actors,
            clips,
            anchor,
            initial: None,
            frames: 0,
            verify,
            finished: None,
            restarted: false,
            bot_path: VecDeque::new(),
            seen: BTreeSet::new(),
        })
    }
    pub fn contains(&self, id: ObjectId) -> bool {
        self.actors.contains_key(&id)
    }
    pub fn spawn(
        &mut self,
        id: ObjectId,
        commands: &mut Commands,
        model: &RenderModel,
        root: Mat4,
        meshes: &mut Assets<Mesh>,
    ) {
        for part in &model.parts {
            let mut part = part.clone();
            part.mesh = meshes.add(meshes.get(&part.mesh).unwrap().clone());
            commands.spawn((
                ActorPart {
                    id,
                    offset: part.transform,
                    bone: None,
                },
                Mesh3d(part.mesh.clone()),
                MeshMaterial3d(part.material.clone()),
                Transform::from_matrix(root * part.transform),
            ));
            self.actors.get_mut(&id).unwrap().parts.push(part);
        }
    }
    pub fn attachment(
        id: ObjectId,
        commands: &mut Commands,
        model: &RenderModel,
        bone: usize,
        root: Mat4,
    ) {
        for p in &model.parts {
            commands.spawn((
                ActorPart {
                    id,
                    offset: p.transform,
                    bone: Some(bone),
                },
                Mesh3d(p.mesh.clone()),
                MeshMaterial3d(p.material.clone()),
                Transform::from_matrix(root * p.transform),
            ));
        }
    }
    pub fn install(
        &mut self,
        scene: bw_rust::unit_collision::Scene,
        commands: &mut Commands,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
    ) -> Result<()> {
        if self.actors.values().any(|a| a.parts.is_empty()) {
            return Err("An original combatant failed to render".into());
        }
        self.game.install(scene, self.anchor)?;
        self.initial = Some(self.game.clone());
        let beam = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
        let blue = materials.add(StandardMaterial {
            base_color: Color::srgb(0.1, 0.9, 0.75),
            unlit: true,
            ..default()
        });
        let red = materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.15, 0.08),
            unlit: true,
            ..default()
        });
        let gold = materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.85, 0.05),
            unlit: true,
            ..default()
        });
        for (&id, &team) in &self.game.teams {
            commands.spawn((
                Marker(id),
                Mesh3d(beam.clone()),
                MeshMaterial3d(if team == Team::Friendly {
                    blue.clone()
                } else {
                    red.clone()
                }),
                Transform::default(),
            ));
        }
        let goal = self
            .game
            .world
            .collision
            .as_ref()
            .unwrap()
            .world_point(self.game.objective, 0.0)?;
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(0.2, 8.0))),
            MeshMaterial3d(blue.clone()),
            Transform::from_translation(Vec3::from_array(goal) + Vec3::Y * 4.0),
        ));
        commands.spawn((
            Hud,
            Text::new("Loading battle"),
            TextFont {
                font_size: FontSize::Px(20.0),
                ..default()
            },
            TextColor(Color::WHITE),
            Node {
                position_type: PositionType::Absolute,
                top: px(16),
                left: px(18),
                padding: UiRect::all(px(12)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.015, 0.025, 0.045, 0.88)),
        ));
        commands.spawn((Text::new("WASD move | Shift sprint | Space/LMB fire | Tab target | C squad hold/follow\nQ/E/arrows camera | Wheel zoom | P pause | R restart | Esc quit\nEarly port: original assets; replacement battle rules. Gamepad supported."),TextFont{font_size:FontSize::Px(16.0),..default()},TextColor(Color::srgb(0.85,0.9,0.95)),Node{position_type:PositionType::Absolute,bottom:px(16),left:px(18),padding:UiRect::all(px(10)),..default()},BackgroundColor(Color::srgba(0.015,0.025,0.045,0.8))));
        commands.insert_resource(Visuals {
            beam,
            blue,
            red,
            gold,
        });
        Ok(())
    }
    pub fn center(&self) -> Vec3 {
        Vec3::from_array(
            self.game
                .world
                .collision
                .as_ref()
                .unwrap()
                .world_point(self.game.positions[&self.game.player], 1.5)
                .unwrap(),
        )
    }
    fn restart(&mut self) {
        self.game = self.initial.as_ref().unwrap().clone();
        self.game.phase = Phase::Playing;
        self.bot_path.clear();
        self.finished = None;
    }
    fn bot(&mut self) -> Input {
        let lose = self.verify.as_deref() == Some("loss") || self.restarted;
        let mut input = Input {
            start: true,
            fire: !lose,
            sprint: self.frames % 120 >= 60,
            ..default()
        };
        if lose || self.game.phase != Phase::Playing {
            return input;
        }
        let player = self.game.player;
        let position = self.game.positions[&player];
        let enemy = self
            .game
            .teams
            .iter()
            .filter(|(id, t)| **t == Team::Enemy && self.game.alive(**id))
            .min_by(|(a, _), (b, _)| {
                let d = |id: &ObjectId| {
                    let p = self.game.positions[id];
                    (p[0] - position[0]).hypot(p[2] - position[2])
                };
                d(a).total_cmp(&d(b))
            })
            .map(|(id, _)| *id);
        let (goal, radius) = if let Some(enemy) = enemy {
            let p = self.game.positions[&enemy];
            if self.game.target != Some(enemy) {
                input.cycle = true;
            }
            let clear = self
                .game
                .world
                .collision
                .as_ref()
                .unwrap()
                .line_clear(
                    self.game.eye(player).unwrap(),
                    self.game.eye(enemy).unwrap(),
                )
                .unwrap();
            let d = (p[0] - position[0]).hypot(p[2] - position[2]);
            if clear && d < 24.0 {
                return input;
            }
            (p, if clear { 18.0 } else { 2.0 })
        } else {
            (self.game.objective, 5.0)
        };
        if self.frames.is_multiple_of(20) || self.bot_path.is_empty() {
            let others = self
                .game
                .teams
                .keys()
                .filter(|id| **id != player && self.game.alive(**id))
                .map(|id| bw_rust::unit_collision::Other {
                    position: self.game.positions[id],
                    body: self.game.world.bodies[id],
                })
                .collect::<Vec<_>>();
            let plan = bw_rust::navigation::plan(
                self.game.world.collision.as_ref().unwrap(),
                position,
                [goal[0] as f64, goal[2] as f64],
                radius,
                self.game.world.bodies[&player],
                &others,
                bw_rust::navigation::Limits::default(),
            )
            .unwrap();
            self.bot_path = plan.points.into();
        }
        while self
            .bot_path
            .front()
            .is_some_and(|p| (p[0] - position[0] as f64).hypot(p[1] - position[2] as f64) < 0.35)
        {
            self.bot_path.pop_front();
        }
        if let Some(p) = self.bot_path.front() {
            let dx = p[0] - position[0] as f64;
            let dz = p[1] - position[2] as f64;
            let d = dx.hypot(dz);
            if d > 0.01 {
                input.movement = [dx / d, dz / d];
            }
        }
        input
    }
}
#[allow(clippy::too_many_arguments, clippy::type_complexity)] // Independent Bevy resources and disjoint component queries.
pub fn update(
    mut commands: Commands,
    view: Option<ResMut<BattleView>>,
    loaded: Res<Loaded>,
    time: Res<Time>,
    input: (
        Res<ButtonInput<KeyCode>>,
        Res<ButtonInput<MouseButton>>,
        Query<&Gamepad>,
        Option<Res<Controls>>,
    ),
    mut wheel: MessageReader<MouseWheel>,
    mut camera: ResMut<CameraState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut parts: Query<(&ActorPart, &mut Transform), Without<Marker>>,
    mut markers: Query<
        (
            &Marker,
            &mut Transform,
            &mut Visibility,
            &mut MeshMaterial3d<StandardMaterial>,
        ),
        Without<ActorPart>,
    >,
    mut beams: Query<(Entity, &mut BeamLife)>,
    visuals: Option<Res<Visuals>>,
    mut hud: Query<(&mut Text, &mut TextColor), With<Hud>>,
    mut exit: MessageWriter<AppExit>,
    capture: Res<super::CaptureTarget>,
) {
    let Some(mut v) = view else {
        return;
    };
    let (keys, mouse, pads, controls) = input;
    let visuals = visuals.unwrap();
    v.frames += 1;
    let verified = v.verify.is_some();
    let smoke = std::env::var_os("BW_SMOKE").is_some();
    let automated = verified || smoke;
    let dt = if automated {
        1.0 / 30.0
    } else {
        time.delta_secs_f64().min(0.1)
    };
    let actions = controls.unwrap().sample(&keys, &mouse, pads.iter().next());
    if !automated && actions.quit {
        exit.write(AppExit::Success);
    }
    if !automated
        && (actions.input.restart
            || (actions.start && matches!(v.game.phase, Phase::Won | Phase::Lost)))
    {
        v.restart();
    }
    if !automated {
        camera.yaw -= actions.input.turn * dt as f32 * 1.8;
        camera.pitch = (camera.pitch + actions.look_pitch * dt as f32).clamp(0.15, 0.9);
    }
    for event in wheel.read() {
        if !automated {
            camera.distance = (camera.distance - event.y * 2.0).clamp(10.0, 45.0);
        }
    }
    let input = if verified {
        v.bot()
    } else if smoke {
        Input::default()
    } else {
        Input {
            movement: bw_rust::battle::camera_movement(
                actions.input.forward as f64,
                actions.input.strafe as f64,
                camera.resolved_yaw as f64,
            )
            .unwrap(),
            sprint: actions.input.sprint,
            fire: actions.input.fire,
            cycle: actions.input.cycle,
            start: actions.start,
            pause: actions.pause,
            hold: keys.just_pressed(KeyCode::KeyC)
                || pads
                    .iter()
                    .next()
                    .is_some_and(|p| p.just_pressed(GamepadButton::West)),
        }
    };
    v.game.tick(dt, &input).expect("Battle simulation error");
    let mut poses = BTreeMap::new();
    for (&id, actor) in &v.actors {
        let model = loaded
            .models
            .iter()
            .find(|m| m.name == actor.model)
            .unwrap();
        let animation = v.game.world.units[&id].animation.as_ref().unwrap();
        let clip = &v.clips[&animation.bindings[&animation.motion]];
        let pose =
            bw_rust::skinning::pose(model, Some((clip, animation.sample_time(clip).unwrap())))
                .unwrap();
        for p in &actor.parts {
            if model.nodes[p.node_index].has_skin_data {
                let vertices =
                    bw_rust::skinning::deform(model, &pose, p.node_index, p.surface_index).unwrap();
                let mut mesh = meshes.get_mut(&p.mesh).unwrap();
                mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
                mesh.compute_smooth_normals();
            }
        }
        poses.insert(id, pose);
    }
    for id in v.game.teams.keys().copied().collect::<Vec<_>>() {
        let motion = v.game.world.units[&id].animation.as_ref().unwrap().motion;
        v.seen.insert(motion);
    }
    for (part, mut transform) in &mut parts {
        let p = v
            .game
            .world
            .collision
            .as_ref()
            .unwrap()
            .world_point(v.game.positions[&part.id], 0.0)
            .unwrap();
        let facing = v.game.facings[&part.id];
        let root = Mat4::from_rotation_translation(
            Quat::from_rotation_y(facing[0].atan2(facing[1])),
            Vec3::from_array(p),
        );
        *transform = Transform::from_matrix(
            root * part
                .bone
                .map(|i| poses[&part.id][i])
                .unwrap_or(Mat4::IDENTITY)
                * part.offset,
        );
    }
    for (marker, mut transform, mut visibility, mut material) in &mut markers {
        let unit = &v.game.world.units[&marker.0];
        *visibility = if unit.health > 0.0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        let p = v
            .game
            .world
            .collision
            .as_ref()
            .unwrap()
            .world_point(v.game.positions[&marker.0], 2.7)
            .unwrap();
        *transform = Transform {
            translation: Vec3::from_array(p),
            rotation: Quat::from_rotation_y(camera.resolved_yaw),
            scale: Vec3::new((unit.health / unit.max_health) as f32 * 1.4, 0.09, 0.045),
        };
        material.0 = if v.game.target == Some(marker.0) {
            visuals.gold.clone()
        } else if v.game.teams[&marker.0] == Team::Enemy {
            visuals.red.clone()
        } else {
            visuals.blue.clone()
        };
    }
    for (entity, mut life) in &mut beams {
        life.0 -= dt as f32;
        if life.0 <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
    for shot in &v.game.shots {
        let a = Vec3::from_array(shot.from);
        let b = Vec3::from_array(shot.to);
        let delta = b - a;
        if delta.length() > 0.01 {
            commands.spawn((
                BeamLife(0.09),
                Mesh3d(visuals.beam.clone()),
                MeshMaterial3d(if shot.team == Team::Enemy {
                    visuals.red.clone()
                } else {
                    visuals.gold.clone()
                }),
                Transform {
                    translation: (a + b) * 0.5,
                    rotation: Quat::from_rotation_arc(Vec3::Z, delta.normalize()),
                    scale: Vec3::new(0.045, 0.045, delta.length()),
                },
            ));
        }
    }
    camera.center = v.center();
    let hp = v.game.world.units[&v.game.player].health
        / v.game.world.units[&v.game.player].max_health
        * 100.0;
    let squad = v
        .game
        .teams
        .iter()
        .filter(|(id, t)| **t == Team::Friendly && **id != v.game.player && v.game.alive(**id))
        .count();
    let goal = v.game.objective;
    let p = v.game.positions[&v.game.player];
    let distance = (goal[0] - p[0]).hypot(goal[2] - p[2]);
    let target = v
        .game
        .target
        .filter(|id| v.game.alive(*id))
        .map(|id| {
            format!(
                "Target: {:.0}% HP | Tab cycles",
                v.game.world.units[&id].health / v.game.world.units[&id].max_health * 100.0
            )
        })
        .unwrap_or("No remaining target".into());
    let state=match v.game.phase{Phase::Briefing=>"FORT ASSAULT\nEliminate four enemy riflemen, then secure the cyan uplink.\nYour two squadmates follow and provide covering fire.\n\nENTER / gamepad South: begin".into(),Phase::Won=>format!("OUTPOST SECURED\nAssault complete in {:.1}s. R / ENTER: play again",v.game.time),Phase::Lost=>"SQUAD LEADER DOWN\nR / ENTER: retry the assault".into(),Phase::Playing=>if v.game.paused{"PAUSED - P / gamepad Start: resume".into()}else if v.game.enemies()>0{format!("Eliminate sentries: {} remaining\n{}",v.game.enemies(),target)}else{format!("Secure cyan uplink: {:.1} / 5.0s | {:.0}m away",v.game.capture,distance)}};
    for (mut text, mut color) in &mut hud {
        **text = format!(
            "BATTALION WARS RUST | HP {hp:.0}% | Squad {squad}/2: {}\n{state}",
            if v.game.hold { "HOLD" } else { "FOLLOW" }
        );
        color.0 = if hp < 25.0 {
            Color::srgb(1.0, 0.5, 0.4)
        } else {
            Color::WHITE
        };
    }
    if smoke && !verified && v.frames == 180 {
        let initial = v.initial.as_ref().unwrap();
        assert_eq!(v.game.phase, Phase::Briefing);
        assert_eq!(v.game.time, 0.0);
        assert_eq!(v.game.positions, initial.positions);
        assert_eq!(v.game.shots_fired, 0);
        assert_eq!(
            v.game.world.units[&v.game.player].health,
            initial.world.units[&initial.player].health
        );
        println!(
            "BATTLE_BRIEFING_SMOKE_PASS waiting_for_start=true unchanged_health_positions=true"
        );
    }
    if verified {
        if matches!(v.game.phase, Phase::Won | Phase::Lost) {
            if v.finished.is_none() {
                v.finished = Some(v.frames);
                println!(
                    "BATTLE_FINISH phase={:?} time={} hp={} enemies={} shots={} enemy_attacks={} damage={} animations={:?}",
                    v.game.phase,
                    v.game.time,
                    hp,
                    v.game.enemies(),
                    v.game.shots_fired,
                    v.game.enemy_attacks,
                    v.game.player_damage,
                    v.seen
                );
            }
            if v.frames - v.finished.unwrap() == 30
                && let Ok(path) = std::env::var("BW_CAPTURE_RESULT")
            {
                commands
                    .spawn(capture.request())
                    .observe(save_to_disk(path));
            }
            if v.frames - v.finished.unwrap() > 45 {
                if v.verify.as_deref() == Some("restart") && !v.restarted {
                    assert_eq!(v.game.phase, Phase::Won);
                    v.restart();
                    v.restarted = true;
                    assert_eq!(v.game.enemies(), 4);
                    let initial = v.initial.as_ref().unwrap();
                    assert_eq!(v.game.positions, initial.positions);
                    assert_eq!(v.game.time, 0.0);
                    assert_eq!(v.game.capture, 0.0);
                    assert!(!v.game.paused && v.game.world.orders.is_empty());
                    for (id, unit) in &initial.world.units {
                        assert_eq!(v.game.world.units[id].health, unit.health);
                        assert_eq!(v.game.world.units[id].active, unit.active);
                    }
                    println!("BATTLE_RESTART restored_health_roster_positions=true");
                } else {
                    let expected = if v.verify.as_deref() == Some("loss") || v.restarted {
                        Phase::Lost
                    } else {
                        Phase::Won
                    };
                    assert_eq!(v.game.phase, expected);
                    assert!(v.game.enemy_attacks > 0);
                    println!(
                        "BATTLE_VERIFY_PASS mode={} phase={:?} restart={}",
                        v.verify.as_ref().unwrap(),
                        v.game.phase,
                        v.restarted
                    );
                    exit.write(AppExit::Success);
                }
            }
        }
        assert!(
            v.frames < 2400,
            "Battle verification failed to finish in 80 simulated seconds"
        );
    }
}
