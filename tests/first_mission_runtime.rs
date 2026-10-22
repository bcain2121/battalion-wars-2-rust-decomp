use bw_rust::{ObjectGraph, ObjectId, mission_world::World, parse_objects};
#[test]
fn player_aim_angles_are_real_geometry_not_tutorial_completion_flags() {
    let mut world = World::default();
    world.configure_rewrite(&ObjectGraph::default()).unwrap();
    world.player = Some(ObjectId(1));
    world.set_player_aim([0.0, 0.0, 2.0]).unwrap();
    let mut p = [
        (ObjectId(1), [0.0, 0.0, 0.0]),
        (ObjectId(2), [0.0, 1.6, 10.0]),
    ]
    .into_iter()
    .collect();
    assert!(world.player_angle_to_target(ObjectId(2), &p).unwrap() < 1e-6);
    p.insert(ObjectId(2), [10.0, 1.6, 0.0]);
    assert!((world.player_angle_to_target(ObjectId(2), &p).unwrap() - 90.0).abs() < 1e-6);
    p.insert(ObjectId(2), [0.0, 1.6, -10.0]);
    assert!((world.player_angle_to_target(ObjectId(2), &p).unwrap() - 180.0).abs() < 1e-6);
    let before = world.player_aim;
    assert!(world.set_player_aim([f64::NAN, 0.0, 0.0]).is_err());
    assert_eq!(world.player_aim, before);
}
#[test]
fn authored_damage_channels_and_resistance_use_linear_rust_falloff() {
    let objects=parse_objects(r#"<Instances><Object id="1" type="sExplodeBase">
    <Attribute name="mDamageType"><Item>DAMAGE_EXPLOSIVE</Item><Item>DAMAGE_FIRE</Item><Item>DAMAGE_NO_DAMAGE</Item><Item>DAMAGE_NO_DAMAGE</Item></Attribute>
    <Attribute name="mDamageAmount"><Item>100</Item><Item>20</Item><Item>0</Item><Item>0</Item></Attribute>
    <Attribute name="mMinBlastRadius"><Item>1</Item></Attribute><Attribute name="mMaxBlastRadius"><Item>10</Item></Attribute>
    </Object><Object id="2" type="sTroopBase">
    <Attribute name="mDamageType"><Item>DAMAGE_EXPLOSIVE</Item><Item>DAMAGE_FIRE</Item></Attribute>
    <Attribute name="mDamageFactor"><Item>0.5</Item><Item>0.2</Item></Attribute><Attribute name="mDefaultDamageFactor"><Item>1</Item></Attribute>
    </Object></Instances>"#).unwrap();
    let mut graph = ObjectGraph::default();
    graph.merge(objects).unwrap();
    let blast = bw_rust::mission_damage::Blast::from_explosion(
        &graph.objects[&ObjectId(1)],
        ObjectId(9),
        [0.; 3],
    )
    .unwrap();
    assert_eq!(
        blast
            .damage_to(&graph.objects[&ObjectId(2)], false)
            .unwrap(),
        54.0
    );
    assert_eq!(blast.attenuation(0.5), 1.0);
    assert_eq!(blast.attenuation(5.5), 0.5);
    assert_eq!(blast.attenuation(11.0), 0.0);
    graph
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .fields
        .iter_mut()
        .find(|f| {
            f.attributes
                .get("name")
                .is_some_and(|n| n == "mDamageAmount")
        })
        .unwrap()
        .items[0] = "-1".into();
    assert!(
        bw_rust::mission_damage::Blast::from_explosion(
            &graph.objects[&ObjectId(1)],
            ObjectId(9),
            [0.; 3]
        )
        .is_err()
    );
}
#[test]
fn recruitment_creates_real_follow_movement_and_rejects_enemies() {
    use bw_rust::{
        lua50::Bootstrap,
        mission_vm::{Host, Scheduler},
        mission_world::{Navigation, Unit},
        unit_collision::{Body, Scene},
    };
    let mut world = World::default();
    world.configure_rewrite(&ObjectGraph::default()).unwrap();
    for (id, army) in [(1, "A"), (2, "A"), (3, "B")] {
        world.units.insert(
            ObjectId(id),
            Unit {
                health: 10.0,
                max_health: 10.0,
                army: army.into(),
                active: Some(true),
                forward_speed: Some(5.0),
                invulnerable: false,
                animation: None,
            },
        );
        world.bodies.insert(ObjectId(id), Body::INFANTRY);
    }
    world.player = Some(ObjectId(1));
    world.roster.insert(ObjectId(1));
    world.navigation = Navigation::CollisionAware;
    world.collision = Some(std::sync::Arc::new(Scene::default()));
    let positions = [
        (ObjectId(1), [0., 0., 0.]),
        (ObjectId(2), [10., 0., 0.]),
        (ObjectId(3), [20., 0., 0.]),
    ]
    .into_iter()
    .collect();
    assert!(world.recruit(ObjectId(3), &positions).is_err());
    world.recruit(ObjectId(2), &positions).unwrap();
    assert!(world.squad_follow.contains(&ObjectId(2)));
    let mut host = Host {
        world,
        positions,
        ..Default::default()
    };
    let mut scheduler = Scheduler::default();
    let mut vm = Bootstrap::new();
    for _ in 0..10 {
        scheduler.tick(&mut vm, &mut host, 0.1, 100).unwrap();
    }
    assert!(host.positions[&ObjectId(2)][0] < 9.0);
    assert!(!host.world.roster.contains(&ObjectId(3)));
}
#[test]
#[cfg(feature = "viewer")]
#[ignore = "requires private original assets"]
fn original_intro_movement_look_pause_and_first_combat_objective() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let data = root.join("local/extracted/files/Data/CompoundFiles");
    let mut graph = ObjectGraph::default();
    for file in ["SP_0.1_Level.xml", "SP_0.1_Level_preload.xml"] {
        graph
            .merge(parse_objects(&std::fs::read_to_string(data.join(file)).unwrap()).unwrap())
            .unwrap();
    }
    let archive = std::fs::read(data.join("SP_0.1_Level.res")).unwrap();
    let mut session = bw_rust::first_mission::Session::load(root, &graph, &archive).unwrap();
    session.begin_recording();
    assert_eq!(session.scheduler.threads.len(), 97);
    for _ in 0..2700 {
        session.tick(1.0 / 60.0, Default::default()).unwrap();
    }
    let host = &session.program.host;
    assert!(!host.world.frozen);
    assert!(
        host.world
            .cameras
            .as_ref()
            .unwrap()
            .chase_target()
            .is_some()
    );
    let binding = session
        .program
        .bindings
        .iter()
        .find(|b| b.script == "Gambit_Part_1_Cutscene_INTRO")
        .unwrap();
    assert!(matches!(
        session.scheduler.threads[&binding.entity.0].status,
        bw_rust::mission_vm::Status::Complete(_)
    ));
    let visible = session
        .markers
        .iter()
        .filter_map(|m| m.position(host).unwrap().map(|p| (m.id, p)))
        .collect::<Vec<_>>();
    assert!(
        !visible.is_empty(),
        "Original intro must expose authored guidance"
    );
    println!("AUTHORED_GUIDANCE {visible:?}");
    let player = host.world.player.unwrap();
    let before = host.positions[&player];
    for _ in 0..60 {
        session
            .tick(
                1.0 / 60.0,
                bw_rust::first_mission::Input {
                    movement: [0.0, 1.0],
                    ..Default::default()
                },
            )
            .unwrap();
    }
    let after = session.program.host.positions[&player];
    assert!((after[0] - before[0]).hypot(after[2] - before[2]) > 0.1);
    let first = visible[0].0;
    for _ in 0..1800 {
        if session.program.host.world.objective_visibility[&first] == Some(false) {
            break;
        }
        let host = &session.program.host;
        let p = host.positions[&player];
        let target = host.positions[&first];
        let aim = host.world.player_aim.unwrap();
        let angle =
            ((target[0] - p[0]) as f64).atan2((target[2] - p[2]) as f64) - aim[0].atan2(aim[2]);
        let angle =
            (angle + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
        let distance = (target[0] - p[0]).hypot(target[2] - p[2]);
        session
            .tick(
                1. / 60.,
                bw_rust::first_mission::Input {
                    movement: [
                        0.,
                        if distance > 2. && angle.abs() < 0.1 {
                            1.
                        } else {
                            0.
                        },
                    ],
                    look: [angle.clamp(-0.05, 0.05), 0.],
                    ..Default::default()
                },
            )
            .unwrap();
    }
    assert_eq!(
        session.program.host.world.objective_visibility[&first],
        Some(false),
        "First authored star did not complete via movement"
    );
    println!(
        "FIRST_AUTHORED_STAR_COMPLETE time={} position={:?}",
        session.program.host.time, session.program.host.positions[&player]
    );
    use bw_rust::lua50::Value;
    let Value::Table(air) = session.program.vm.globals["Air_Vehicle"] else {
        panic!("Missing aircraft aliases")
    };
    let Value::Object(plane) = session
        .program
        .vm
        .table_get(air, &Value::String("ILbomberSTART01".into()))
        .unwrap()
    else {
        panic!("Missing tutorial plane")
    };
    for _ in 0..1200 {
        if session.program.vm.globals.get("LookDone") == Some(&Value::Number(2.)) {
            break;
        }
        let host = &session.program.host;
        let origin = host
            .world
            .collision
            .as_ref()
            .unwrap()
            .world_point(host.positions[&player], 1.6)
            .unwrap();
        let p = host.positions[&plane];
        let aim = host.world.player_aim.unwrap();
        let dx = (p[0] - origin[0]) as f64;
        let dz = (p[2] - origin[2]) as f64;
        let pitch = ((p[1] - origin[1]) as f64).atan2(dx.hypot(dz));
        let yaw = (dx.atan2(dz) + 0.01 - aim[0].atan2(aim[2]) + std::f64::consts::PI)
            .rem_euclid(std::f64::consts::TAU)
            - std::f64::consts::PI;
        session
            .tick(
                1. / 60.,
                bw_rust::first_mission::Input {
                    look: [
                        yaw.clamp(-0.05, 0.05),
                        (pitch - aim[1].asin()).clamp(-0.05, 0.05),
                    ],
                    ..Default::default()
                },
            )
            .unwrap();
    }
    assert_eq!(
        session.program.vm.globals.get("LookDone"),
        Some(&Value::Number(2.)),
        "Original look tutorial must follow real aim"
    );
    println!(
        "ORIGINAL_LOOK_COMPLETE time={} markers={:?}",
        session.program.host.time,
        session
            .markers
            .iter()
            .filter_map(|m| m
                .position(&session.program.host)
                .unwrap()
                .map(|p| (m.id, p)))
            .collect::<Vec<_>>()
    );
    let Value::Table(buildings) = session.program.vm.globals["Building"] else {
        panic!("Missing buildings")
    };
    let Value::Object(barrel) = session
        .program
        .vm
        .table_get(buildings, &Value::String("Barrel05".into()))
        .unwrap()
    else {
        panic!("Missing tutorial barrel")
    };
    let goal = session
        .markers
        .iter()
        .find_map(|m| m.position(&session.program.host).unwrap())
        .unwrap();
    let mut paused_frames = 0;
    for frame in 0..2400 {
        if session.program.host.world.entity(barrel).unwrap().health <= 0. {
            break;
        }
        let host = &session.program.host;
        let p = host.positions[&player];
        let aim = host.world.player_aim.unwrap();
        let was_paused = host.world.tutorial.as_ref().unwrap().paused;
        let old_time = host.time;
        let old_no_pause = host.no_pause_time;
        let distance = (goal[0] - p[0]).hypot(goal[2] - p[2]);
        let shooting = distance < 5.
            && host
                .world
                .controller
                .as_ref()
                .unwrap()
                .accepts("CONTROL_FIRE")
                .unwrap();
        let target = if shooting {
            host.world
                .collision
                .as_ref()
                .unwrap()
                .world_point(host.positions[&barrel], 1.)
                .unwrap()
        } else {
            goal
        };
        let origin = host
            .world
            .collision
            .as_ref()
            .unwrap()
            .world_point(p, (host.world.bodies[&player].height * 0.65) as f32)
            .unwrap();
        let dx = (target[0] - origin[0]) as f64;
        let dz = (target[2] - origin[2]) as f64;
        let yaw = (dx.atan2(dz) - aim[0].atan2(aim[2]) + std::f64::consts::PI)
            .rem_euclid(std::f64::consts::TAU)
            - std::f64::consts::PI;
        let pitch = if shooting {
            ((target[1] - origin[1]) as f64).atan2(dx.hypot(dz))
        } else {
            0.
        };
        session
            .tick(
                1. / 60.,
                bw_rust::first_mission::Input {
                    movement: [
                        0.,
                        if !shooting && distance > 2. && yaw.abs() < 0.1 {
                            1.
                        } else {
                            0.
                        },
                    ],
                    look: [
                        yaw.clamp(-0.05, 0.05),
                        (pitch - aim[1].asin()).clamp(-0.05, 0.05),
                    ],
                    fire: shooting && yaw.abs() < 0.05,
                    lock: shooting,
                    confirm: frame % 120 == 0,
                    ..Default::default()
                },
            )
            .unwrap();
        if was_paused {
            paused_frames += 1;
            assert_eq!(session.program.host.time, old_time);
            assert!(session.program.host.no_pause_time > old_no_pause);
            assert_eq!(session.program.host.positions[&player], p);
        }
    }
    println!(
        "ORIGINAL_BARREL_COMBAT time={} health={} fired={} hits={} position={:?} objective={:?}",
        session.program.host.time,
        session.program.host.world.entity(barrel).unwrap().health,
        session.combat.fired,
        session.combat.hits,
        session.program.host.positions[&player],
        session.program.host.world.objectives
    );
    assert_eq!(
        session.program.host.world.entity(barrel).unwrap().health,
        0.,
        "Original tutorial barrel must die from real shots"
    );
    assert!(
        paused_frames > 0,
        "Original pause tutorial must actually pause gameplay"
    );
    let Value::Table(objectives) = session.program.vm.globals["Objective"] else {
        panic!("Missing objectives")
    };
    let Value::Object(objective) = session
        .program
        .vm
        .table_get(objectives, &Value::String("Tutorial02".into()))
        .unwrap()
    else {
        panic!("Missing combat objective")
    };
    assert!(
        session.program.host.world.objectives[&objective].1,
        "Authored combat objective must complete"
    );
    for name in ["Barrel06", "Barrel07"] {
        let Value::Object(id) = session
            .program
            .vm
            .table_get(buildings, &Value::String(name.into()))
            .unwrap()
        else {
            panic!("Missing barrel")
        };
        assert_eq!(session.program.host.world.entity(id).unwrap().health, 0.);
    }
    let recording = session.take_recording().unwrap();
    std::fs::write(
        root.join("local/analysis/first-combat-inputs.json"),
        serde_json::to_vec(&recording).unwrap(),
    )
    .unwrap();
    println!(
        "FIRST_COMBAT_OBJECTIVE_COMPLETE objective={} paused_frames={} replay_frames={}",
        objective.0,
        paused_frames,
        recording.len()
    );
    let after = session.program.host.positions[&player];
    let frame = session.program.host.frame;
    assert!(session.tick(f64::NAN, Default::default()).is_err());
    assert_eq!(session.program.host.frame, frame);
    assert_eq!(session.program.host.positions[&player], after);
    session.restart();
    assert_eq!(session.program.host.frame, 1);
    assert!(session.program.host.world.frozen);
}
