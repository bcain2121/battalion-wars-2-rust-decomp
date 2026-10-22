//! Opt-in original-script body checks; all fixture transitions are explicit.
use bw_rust::{
    ObjectGraph,
    archive::{chunks, named_payload},
    lua50::{self, Bootstrap, Prototype, Value},
    mission_vm::{Host, Scheduler, Status, Thread},
    parse_objects,
};
use std::{collections::BTreeMap, fs};
fn fixture() -> (Bootstrap, Host, BTreeMap<String, Prototype>) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("local/extracted/files/Data/CompoundFiles");
    let mut graph = ObjectGraph::default();
    for name in ["SP_0.1_Level.xml", "SP_0.1_Level_preload.xml"] {
        graph
            .merge(
                parse_objects(
                    &fs::read_to_string(root.join(name)).expect("Local extraction required"),
                )
                .unwrap(),
            )
            .unwrap();
    }
    let archive = fs::read(root.join("SP_0.1_Level.res")).unwrap();
    let mut scripts = BTreeMap::new();
    for c in chunks(&archive).unwrap() {
        if c.tag == *b"PRCS" {
            let (n, b) = named_payload(c.payload).unwrap();
            scripts.insert(n.to_string(), lua50::parse(b).unwrap());
        }
    }
    let mut vm = Bootstrap::new();
    for (n, p) in &scripts {
        vm.execute(n, p, &graph, 100000).unwrap();
    }
    let host = Host::from_graph(&graph).unwrap();
    host.install(&mut vm);
    (vm, host, scripts)
}
#[test]
#[ignore = "requires local disc extraction"]
fn original_initializers() {
    let (mut vm, mut host, scripts) = fixture();
    let mut instructions = 0;
    for name in ["ArtVariables", "Gambit_Part_1_Global_Variables"] {
        let mut t = Thread::new(name, scripts[name].children[0].clone(), &[Value::Nil]).unwrap();
        assert!(matches!(
            t.resume(&mut vm, &mut host, 100000).unwrap(),
            Status::Complete(_)
        ));
        instructions += t.instructions;
    }
    assert_eq!(instructions, 365);
    assert_eq!(vm.globals["MissionOver"], Value::Number(0.0));
    assert_eq!(vm.globals["ViewDistance1"], Value::Number(350.0));
    assert_eq!(host.debug.len(), 11);
    assert_eq!(host.bounds_message.as_ref().unwrap().message, 490.0);
    let Value::Table(troops) = vm.globals["Troop"] else {
        panic!("Troop table")
    };
    let Value::Object(id) = vm.tables[troops]["Sstrikebazooka01"] else {
        panic!("Troop identity")
    };
    assert_eq!(
        vm.globals["Sstrikebazooka01X"],
        Value::Number(host.positions[&id][0] as f64)
    );
    assert_eq!(
        vm.globals["Sstrikebazooka01Z"],
        Value::Number(host.positions[&id][2] as f64)
    );
    println!(
        "original initializer bodies=2 instructions={instructions} checked coordinate reads and message configuration"
    );
}
#[test]
#[ignore = "requires local disc extraction"]
fn original_pause_timer_coroutine() {
    let (mut vm, mut host, scripts) = fixture();
    // These inputs simulate external cutscene/scoring state. We do not claim to run
    // the complete ScoreScreen or cinematic scripts that own these globals.
    vm.globals
        .insert("PauseTimerActive".into(), Value::Number(1.0));
    vm.globals
        .insert("LevelPauseTimer".into(), Value::Number(0.0));
    let mut scheduler = Scheduler::default();
    scheduler.threads.insert(
        1,
        Thread::new(
            "Pause_Scoring_Timer",
            scripts["Pause_Scoring_Timer"].children[0].clone(),
            &[Value::Nil],
        )
        .unwrap(),
    );
    scheduler.tick(&mut vm, &mut host, 0.0, 1000).unwrap();
    assert_eq!(vm.globals["LevelPauseTimer"], Value::Number(0.0));
    scheduler.tick(&mut vm, &mut host, 0.5, 1000).unwrap();
    assert_eq!(vm.globals["LevelPauseTimer"], Value::Number(0.0));
    scheduler.paused = true;
    scheduler.tick(&mut vm, &mut host, 50.0, 1000).unwrap();
    assert_eq!(host.time, 0.5);
    scheduler.paused = false;
    scheduler.tick(&mut vm, &mut host, 0.5, 1000).unwrap();
    assert_eq!(vm.globals["LevelPauseTimer"], Value::Number(1.0));
    vm.globals
        .insert("PauseTimerActive".into(), Value::Number(0.0));
    scheduler.tick(&mut vm, &mut host, 1.0, 1000).unwrap();
    // The actual script increments once after an already-started wait, before
    // rechecking its active flag. Preserve that source behavior, don't 'correct' it.
    assert_eq!(vm.globals["LevelPauseTimer"], Value::Number(2.0));
    assert_eq!(host.debug.len(), 2);
    scheduler.tick(&mut vm, &mut host, 0.5, 1000).unwrap();
    assert_eq!(vm.globals["LevelPauseTimer"], Value::Number(2.0));
    assert!(matches!(scheduler.threads[&1].status, Status::Sleeping(_)));
    println!(
        "original pause timer: WaitFor resume, preserved locals, active/disabled branches and final in-flight increment verified; scheduler timing policy remains provisional"
    );
}

#[test]
#[ignore = "requires local disc extraction"]
fn original_stay_here_issues_and_observes_movement() {
    use bw_rust::{mission_world::Navigation, unit_animation::Motion};
    let (mut vm, mut host, scripts) = gameplay_fixture();
    let id = symbol(&vm, "Troop", "Sgrunt02");
    host.world.units.get_mut(&id).unwrap().active = Some(true);
    host.world.navigation = Navigation::StraightLineDiagnostic;
    let home = host.positions[&id];
    let mut scheduler = Scheduler::default();
    scheduler.threads.insert(
        1,
        Thread::new(
            "Gambit_Part_1_Stay_Here",
            scripts["Gambit_Part_1_Stay_Here"].children[0].clone(),
            &[Value::Object(id)],
        )
        .unwrap(),
    );
    scheduler.tick(&mut vm, &mut host, 0.0, 10000).unwrap();
    vm.globals
        .insert("cutsceneStart".into(), Value::Number(1.0));
    host.positions.get_mut(&id).unwrap()[0] += 100.0;
    scheduler.tick(&mut vm, &mut host, 0.1, 10000).unwrap();
    assert!(host.world.orders.contains_key(&id));
    let displaced = host.positions[&id];
    assert_eq!(displaced[0], home[0] + 100.0);
    host.world.navigation = Navigation::Unavailable;
    let before = host.time;
    assert!(
        scheduler
            .tick(&mut vm, &mut host, 1.0, 10000)
            .unwrap_err()
            .contains("Navigation backend")
    );
    assert_eq!(host.time, before);
    assert_eq!(host.positions[&id], displaced);
    assert!(host.world.orders.contains_key(&id));
    host.world.navigation = Navigation::StraightLineDiagnostic;
    scheduler.tick(&mut vm, &mut host, 1.0, 10000).unwrap();
    assert!(host.positions[&id][0] < displaced[0]);
    assert!(host.positions[&id][0] > home[0] + 5.0);
    assert_eq!(
        host.world.units[&id].animation.as_ref().unwrap().motion,
        Motion::Run
    );
    let paused_position = host.positions[&id];
    let clock = host.time;
    scheduler.paused = true;
    scheduler.tick(&mut vm, &mut host, 100.0, 10000).unwrap();
    assert_eq!(host.positions[&id], paused_position);
    assert_eq!(host.time, clock);
    scheduler.paused = false;
    for _ in 0..20 {
        scheduler.tick(&mut vm, &mut host, 1.0, 10000).unwrap();
    }
    let position = host.positions[&id];
    let distance = ((position[0] - home[0]).powi(2) + (position[2] - home[2]).powi(2)).sqrt();
    assert!((distance - 5.0).abs() < 0.001);
    assert_eq!(position[1], home[1]);
    assert!(!host.world.orders.contains_key(&id));
    assert_eq!(
        host.world.units[&id].animation.as_ref().unwrap().motion,
        Motion::Idle
    );
    host.world
        .damage(id, host.world.units[&id].max_health)
        .unwrap();
    scheduler.tick(&mut vm, &mut host, 1.0, 10000).unwrap();
    assert!(matches!(scheduler.threads[&1].status, Status::Complete(_)));
    println!(
        "Original Stay_Here captures home, issues forced return, observes actual position changes, idles on arrival and exits on death; navigation backend explicitly straight-line diagnostic"
    );
}
#[test]
#[ignore = "requires local disc extraction"]
fn original_stay_here_routes_around_wall_and_troop() {
    use bw_rust::{
        mission_world::{MoveOrder, Navigation},
        navigation::Route,
        unit_collision::{Other, Scene},
    };
    let (mut vm, mut host, scripts) = gameplay_fixture();
    for unit in host.world.units.values_mut() {
        unit.active = Some(false);
    }
    let a = symbol(&vm, "Troop", "Sgrunt02");
    let b = symbol(&vm, "Troop", "Sgrunt03");
    for id in [a, b] {
        host.world.units.get_mut(&id).unwrap().active = Some(true);
    }
    host.positions.insert(a, [0.0; 3]);
    host.positions.insert(b, [12.0, 0.0, 0.0]);
    let mut scene = Scene::default();
    scene
        .add_mesh(vec![
            [[20.0, 0.0, -2.0], [20.0, 4.0, -2.0], [20.0, 4.0, 2.0]],
            [[20.0, 0.0, -2.0], [20.0, 4.0, 2.0], [20.0, 0.0, 2.0]],
        ])
        .unwrap();
    host.world.collision = Some(std::sync::Arc::new(scene));
    host.world.navigation = Navigation::PlannedInfantry;
    let mut scheduler = Scheduler::default();
    scheduler.threads.insert(
        1,
        Thread::new(
            "Gambit_Part_1_Stay_Here",
            scripts["Gambit_Part_1_Stay_Here"].children[0].clone(),
            &[Value::Object(a)],
        )
        .unwrap(),
    );
    scheduler.tick(&mut vm, &mut host, 0.0, 10000).unwrap();
    vm.globals
        .insert("cutsceneStart".into(), Value::Number(1.0));
    host.positions.insert(a, [40.0, 0.0, 0.0]);
    let mut pause_checked = false;
    let mut moved_blocker = false;
    for _ in 0..600 {
        scheduler
            .tick(&mut vm, &mut host, 1.0 / 30.0, 10000)
            .unwrap();
        let other = Other {
            position: host.positions[&b],
            body: host.world.bodies[&b],
        };
        assert!(
            !host
                .world
                .collision
                .as_ref()
                .unwrap()
                .overlaps(host.positions[&a], host.world.bodies[&a], &[other])
                .unwrap()
        );
        if !pause_checked
            && host
                .world
                .routes
                .get(&a)
                .is_some_and(|r| r.pending && r.retry > 0.0)
        {
            let searches = host.world.route_searches;
            let position = host.positions[&a];
            let retry = host.world.routes[&a].retry;
            scheduler.paused = true;
            scheduler.tick(&mut vm, &mut host, 1.0, 10000).unwrap();
            assert_eq!(host.world.route_searches, searches);
            assert_eq!(host.positions[&a], position);
            assert_eq!(host.world.routes[&a].retry, retry);
            scheduler.paused = false;
            pause_checked = true;
        }
        if !moved_blocker
            && let Some(point) = host
                .world
                .routes
                .get(&a)
                .and_then(|r| r.points.get(1))
                .copied()
        {
            let new_position = [point[0] as f32, 0.0, point[1] as f32];
            let actor = Other {
                position: host.positions[&a],
                body: host.world.bodies[&a],
            };
            if !host
                .world
                .collision
                .as_ref()
                .unwrap()
                .overlaps(new_position, host.world.bodies[&b], &[actor])
                .unwrap()
            {
                host.positions.insert(b, new_position);
                moved_blocker = true;
            }
        }
        if host.world.route_arrivals > 0 {
            break;
        }
    }
    assert!(pause_checked);
    assert!(moved_blocker);
    assert!(host.world.unit_contacts > 0);
    assert!(host.world.mesh_contacts > 0);
    assert!(host.world.route_searches >= 2);
    assert!(host.world.route_arrivals > 0);
    assert!(host.positions[&a][0].hypot(host.positions[&a][2]) <= 5.01);
    host.world.routes.insert(a, Route::new([0.0, 0.0], 5.0));
    host.world.orders.insert(
        a,
        MoveOrder {
            target: [40.0, 0.0],
            radius: 0.5,
        },
    );
    scheduler.tick(&mut vm, &mut host, 0.01, 10000).unwrap();
    assert!(
        !host.world.routes.contains_key(&a),
        "changed destinations invalidate cached paths"
    );
}
#[test]
#[ignore = "requires local disc extraction"]
fn original_units_head_on_and_crossing_do_not_interpenetrate() {
    use bw_rust::{
        mission_world::{MoveOrder, Navigation},
        unit_collision::{Other, Scene},
    };
    for crossing in [false, true] {
        let (mut vm, mut host, _) = gameplay_fixture();
        for u in host.world.units.values_mut() {
            u.active = Some(false);
        }
        let a = symbol(&vm, "Troop", "Sgrunt01");
        let b = symbol(&vm, "Troop", "Sgrunt02");
        for id in [a, b] {
            host.world.units.get_mut(&id).unwrap().active = Some(true);
        }
        host.world.navigation = Navigation::CollisionAware;
        host.world.collision = Some(std::sync::Arc::new(Scene::default()));
        host.positions.insert(a, [-5.0, 0.0, 0.0]);
        host.positions.insert(
            b,
            if crossing {
                [0.0, 0.0, -5.0]
            } else {
                [5.0, 0.0, 0.0]
            },
        );
        host.world.orders.insert(
            a,
            MoveOrder {
                target: [5.0, 0.0],
                radius: 0.0,
            },
        );
        host.world.orders.insert(
            b,
            MoveOrder {
                target: if crossing { [0.0, 5.0] } else { [-5.0, 0.0] },
                radius: 0.0,
            },
        );
        let mut scheduler = Scheduler::default();
        scheduler.tick(&mut vm, &mut host, 1.0, 10000).unwrap();
        let other = Other {
            position: host.positions[&b],
            body: host.world.bodies[&b],
        };
        assert!(
            !host
                .world
                .collision
                .as_ref()
                .unwrap()
                .overlaps(host.positions[&a], host.world.bodies[&a], &[other])
                .unwrap()
        );
        assert!(host.world.unit_contacts > 0);
        if !crossing {
            assert!(host.positions[&a][0] < host.positions[&b][0]);
        }
        host.world.orders.insert(
            a,
            MoveOrder {
                target: [1000.0, 1000.0],
                radius: 0.0,
            },
        );
        let old_time = host.time;
        let old_position = host.positions[&a];
        assert!(scheduler.tick(&mut vm, &mut host, 1000.0, 10000).is_err());
        assert_eq!(host.time, old_time);
        assert_eq!(host.positions[&a], old_position);
        scheduler.paused = true;
        scheduler.tick(&mut vm, &mut host, 1.0, 10000).unwrap();
        assert_eq!(host.positions[&a], old_position);
        scheduler.paused = false;
        host.world.frozen = true;
        host.world.player = Some(a);
        scheduler.tick(&mut vm, &mut host, 0.1, 10000).unwrap();
        assert_eq!(host.positions[&a], old_position);
        assert_eq!(
            host.world.units[&a].animation.as_ref().unwrap().motion,
            bw_rust::unit_animation::Motion::Idle
        );
        host.world.frozen = false;
        host.world.damage(b, 1000000.0).unwrap();
        let dead_position = host.positions[&b];
        host.world.orders.insert(
            a,
            MoveOrder {
                target: [dead_position[0] as f64, dead_position[2] as f64],
                radius: 0.0,
            },
        );
        scheduler.tick(&mut vm, &mut host, 1.0, 10000).unwrap();
        assert!(
            (host.positions[&a][0] - dead_position[0])
                .hypot(host.positions[&a][2] - dead_position[2])
                < 0.001
        );
        host.world.bodies.remove(&a);
        let time = host.time;
        assert!(scheduler.tick(&mut vm, &mut host, 0.1, 10000).is_err());
        assert_eq!(host.time, time);
    }
}
fn symbol(vm: &Bootstrap, table: &str, key: &str) -> bw_rust::ObjectId {
    let Value::Table(t) = vm.globals[table] else {
        panic!("Missing symbol table")
    };
    let Value::Object(id) = vm.tables[t][key] else {
        panic!("Missing object symbol")
    };
    id
}
fn gameplay_fixture() -> (Bootstrap, Host, BTreeMap<String, Prototype>) {
    let (mut vm, mut host, scripts) = fixture();
    bw_rust::mission_world::World::install(&mut vm).unwrap();
    for name in [
        "ScoreScreen",
        "ArtVariables",
        "Gambit_Part_1_Global_Variables",
    ] {
        let mut t = Thread::new(name, scripts[name].children[0].clone(), &[Value::Nil]).unwrap();
        assert!(matches!(
            t.resume(&mut vm, &mut host, 10000).unwrap(),
            Status::Complete(_)
        ));
    }
    let player = symbol(&vm, "Troop", "Sgrunt01");
    host.world
        .configure_player(player, [player].into())
        .unwrap();
    host.world.units.get_mut(&player).unwrap().active = Some(true);
    let staff = symbol(&vm, "Troop", "Sstaffbearer01");
    host.world.units.get_mut(&staff).unwrap().active = Some(true);
    host.positions.insert(player, [100000.0, 0.0, 100000.0]);
    host.positions.insert(staff, [100000.0, 0.0, 100000.0]);
    (vm, host, scripts)
}
#[test]
#[ignore = "requires local disc extraction"]
fn original_victory_trigger_executes_scoring_helper() {
    let (mut vm, mut host, scripts) = gameplay_fixture();
    let player = host.world.player.unwrap();
    let zone = symbol(&vm, "Map_Zone", "Tower01");
    let m = host.world.zones[&zone].matrix;
    host.positions.insert(player, [m[12], m[13], m[14]]);
    host.time = 12.5;
    let mut t = Thread::new(
        "Gambit_Part_1_WinLose",
        scripts["Gambit_Part_1_WinLose"].children[0].clone(),
        &[Value::Nil],
    )
    .unwrap();
    assert!(matches!(
        t.resume(&mut vm, &mut host, 10000).unwrap(),
        Status::Sleeping(_)
    ));
    assert_eq!(vm.globals["MissionOver"], Value::Number(1.0));
    assert_eq!(vm.globals["cutscenePlayerWinsBegin"], Value::Number(1.0));
    assert_eq!(vm.globals["LevelStopTime"], Value::Number(12.5));
    assert!(host.world.frozen);
    assert_eq!(
        host.world.objective_visibility[&symbol(&vm, "Objective_Marker", "Tower01")],
        Some(false)
    );
    host.frame += 1;
    host.time += 1.0;
    t.resume(&mut vm, &mut host, 10000).unwrap();
    assert!(matches!(t.status, Status::Sleeping(_)));
    assert_eq!(vm.globals["LevelStopTime"], Value::Number(12.5));
    println!(
        "Original tower victory condition, freeze, objective hiding, Lua scoring helper and cinematic wait verified; end cinematic not simulated"
    );
}
#[test]
#[ignore = "requires local disc extraction"]
fn original_staff_death_enters_nested_loss_coroutine() {
    let (mut vm, mut host, scripts) = gameplay_fixture();
    let staff = symbol(&vm, "Troop", "Sstaffbearer01");
    host.world
        .damage(staff, host.world.units[&staff].max_health)
        .unwrap();
    let mut scheduler = Scheduler::default();
    scheduler.threads.insert(
        1,
        Thread::new(
            "Gambit_Part_1_WinLose",
            scripts["Gambit_Part_1_WinLose"].children[0].clone(),
            &[Value::Nil],
        )
        .unwrap(),
    );
    for _ in 0..3 {
        scheduler.tick(&mut vm, &mut host, 1.0, 10000).unwrap();
    }
    assert_eq!(vm.globals["MissionOver"], Value::Number(1.0));
    assert_eq!(vm.globals["cutscenePlayerLosesBegin"], Value::Number(1.0));
    assert_eq!(
        host.world.declared_victory.as_deref(),
        Some("ARMY_UNDERWORLD")
    );
    assert!(host.world.frozen);
    assert_eq!(vm.globals["LevelStopTime"], Value::Number(1.0));
    scheduler.tick(&mut vm, &mut host, 1.0, 10000).unwrap();
    assert!(matches!(scheduler.threads[&1].status, Status::Sleeping(_)));
    println!(
        "Original staff-bearer death condition enters captured-free nested loss function and preserves its suspended call stack"
    );
}
#[test]
#[ignore = "requires local disc extraction"]
fn original_invulnerability_changes_damage_behavior() {
    let (mut vm, mut host, scripts) = gameplay_fixture();
    let staff = symbol(&vm, "Troop", "Sstaffbearer01");
    let health = host.world.units[&staff].health;
    let mut t = Thread::new(
        "Gambit_Part_1_Invincibility",
        scripts["Gambit_Part_1_Invincibility"].children[0].clone(),
        &[Value::Object(staff)],
    )
    .unwrap();
    assert!(matches!(
        t.resume(&mut vm, &mut host, 100).unwrap(),
        Status::Complete(_)
    ));
    host.world.damage(staff, health * 2.0).unwrap();
    assert_eq!(host.world.units[&staff].health, health);
}
#[test]
#[ignore = "requires local disc extraction"]
fn original_unit_animation_bindings_and_death_playback() {
    use bw_rust::unit_animation::Motion;
    let (_, mut host, _) = gameplay_fixture();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("local/extracted/files/Data/CompoundFiles/SP_0.1_Level.res");
    let archive = fs::read(root).unwrap();
    let mut clips = BTreeMap::new();
    let mut troops = 0;
    for unit in host.world.units.values() {
        if let Some(player) = &unit.animation {
            troops += 1;
            assert_eq!(player.bindings.len(), 5);
            for name in player.bindings.values() {
                if !clips.contains_key(name) {
                    clips.insert(
                        name.clone(),
                        bw_rust::animation::from_archive(&archive, name).unwrap(),
                    );
                }
            }
        }
    }
    assert_eq!(troops, 183);
    let id = host.world.player.unwrap();
    host.world
        .damage(id, host.world.units[&id].max_health)
        .unwrap();
    let player = host.world.units[&id].animation.as_ref().unwrap();
    assert_eq!(player.motion, Motion::Death);
    let clip = &clips[&player.bindings[&Motion::Death]];
    let mut scheduler = Scheduler::default();
    let mut vm = Bootstrap::new();
    scheduler
        .tick(&mut vm, &mut host, clip.duration as f64 + 10.0, 100)
        .unwrap();
    assert_eq!(
        host.world.units[&id]
            .animation
            .as_ref()
            .unwrap()
            .sample_time(clip)
            .unwrap(),
        clip.duration
    );
    let elapsed = host.world.units[&id].animation.as_ref().unwrap().elapsed;
    scheduler.paused = true;
    scheduler.tick(&mut vm, &mut host, 10.0, 100).unwrap();
    assert_eq!(
        host.world.units[&id].animation.as_ref().unwrap().elapsed,
        elapsed
    );
    println!(
        "troops={troops} authored_state_bindings={} unique_clips={} death playback clamps and scheduler pause freezes animation; BAN pose fidelity still experimental",
        troops * 5,
        clips.len()
    );
}
