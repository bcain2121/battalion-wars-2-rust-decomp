//! Original-data startup tests. No fabricated cinematic-completion flags.
#[test]
#[ignore = "requires local STR and DSP extraction"]
fn original_mission_localization_and_voice_duration() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("local/extracted/files/Data");
    let language = bw_rust::localization::Language::parse(
        &std::fs::read(root.join("Strings/SP_0.1English.str")).unwrap(),
    )
    .unwrap();
    assert_eq!(language.messages.len(), 151);
    assert_eq!(
        language.message(90).unwrap().text,
        "ADVANCE to the GOLD STAR"
    );
    assert_eq!(language.message(100).unwrap().text, "COLLECT the STAFF");
    assert!(
        language
            .messages
            .iter()
            .all(|m| m.declared_duration.is_none())
    );
    let voice = bw_rust::dsp_audio::Audio::decode(
        &std::fs::read(root.join("Audio/Voiceover/CO/Qalen/sp_0.1_se_qalen_001.adp")).unwrap(),
    )
    .unwrap();
    assert_eq!(voice.sample_rate, 24000);
    assert_eq!(voice.pcm.len(), 209832);
    assert!((voice.duration() - 8.743).abs() < 1e-9);
    assert!(!voice.looping);
    assert!(voice.pcm.iter().any(|v| *v != 0));
    let bank = bw_rust::mission_audio::Bank::load(root.parent().unwrap(), &language).unwrap();
    assert_eq!(bank.unique_assets, 57);
    assert!(
        bank.voices
            .values()
            .all(|a| a.duration() > 0.0 && !a.looping)
    );
    assert_eq!(bank.voice(1).unwrap().pcm, voice.pcm);
}
use bw_rust::{
    ObjectGraph,
    lua50::Value,
    mission_program::{Program, Scope},
    mission_vm::{Status, Thread},
    parse_objects,
};
fn original_graph() -> ObjectGraph {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("local/extracted/files/Data/CompoundFiles");
    let mut graph = ObjectGraph::default();
    for name in ["SP_0.1_Level.xml", "SP_0.1_Level_preload.xml"] {
        graph
            .merge(parse_objects(&std::fs::read_to_string(root.join(name)).unwrap()).unwrap())
            .unwrap();
    }
    graph
}
fn fixture() -> Program {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("local/extracted/files/Data/CompoundFiles");
    Program::load(
        &original_graph(),
        &std::fs::read(root.join("SP_0.1_Level.res")).unwrap(),
    )
    .unwrap()
}
#[test]
#[ignore = "requires local original extraction"]
fn authored_presence_and_damageable_scenery() {
    let mut graph = original_graph();
    let mut world = bw_rust::mission_world::World::from_graph(&graph).unwrap();
    assert_eq!(world.units.len(), 231);
    assert_eq!(world.props.len(), 720);
    assert_eq!(world.buildings.len(), 141);
    world.configure_authored_units(&graph).unwrap();
    assert!(world.units.values().all(|u| u.active == Some(true)));
    assert_eq!(
        world
            .props
            .values()
            .filter(|u| u.active == Some(true))
            .count(),
        141
    );
    assert_eq!(
        world
            .activity
            .values()
            .filter(|a| a.enabled == Some(false))
            .count(),
        71
    );
    assert!(world.activity.values().all(|a| a.weapons_enabled.is_none()));
    assert!(world.player.is_none());
    assert!(world.roster.is_empty());
    let id = *world.buildings.first().unwrap();
    let max = world.props[&id].max_health;
    world.damage(id, max / 2.0).unwrap();
    assert_eq!(world.props[&id].health, max / 2.0);
    let last = *world.units.keys().last().unwrap();
    let state = graph
        .objects
        .get_mut(&last)
        .unwrap()
        .fields
        .iter_mut()
        .find(|f| f.attributes.get("name").is_some_and(|s| s == "State"))
        .unwrap();
    state.items[0] = "UNSUPPORTED".into();
    let mut unresolved = bw_rust::mission_world::World::from_graph(&graph).unwrap();
    assert!(unresolved.configure_authored_units(&graph).is_err());
    assert!(unresolved.units.values().all(|u| u.active.is_none()));
    assert!(unresolved.activity.values().all(|a| a.enabled.is_none()));
}
#[test]
#[ignore = "requires local original extraction"]
fn authored_bindings_and_all_initializers() {
    let mut p = fixture();
    assert_eq!(
        p.bindings
            .iter()
            .filter(|b| b.scope == Scope::Initializer)
            .count(),
        6
    );
    assert_eq!(
        p.bindings
            .iter()
            .filter(|b| b.scope == Scope::Global)
            .count(),
        40
    );
    assert_eq!(
        p.bindings.iter().filter(|b| b.scope == Scope::Unit).count(),
        57
    );
    let paused = p
        .bindings
        .iter()
        .filter(|b| b.update_when_paused)
        .collect::<Vec<_>>();
    assert_eq!(paused.len(), 1);
    assert_eq!(paused[0].script, "Gambit_Part_1_Pause_Tutorial");
    assert_eq!(p.initialize().unwrap(), 6);
    assert!(p.initialize().is_err());
    assert_eq!(p.vm.upvalues.len(), 3);
    assert_eq!(p.vm.closures.len(), 26);
    assert_eq!(p.vm.globals["MissionOver"], Value::Number(0.0));
    assert!(p.host.world.player.is_none());
    assert!(p.host.world.units.values().all(|u| u.active.is_none()));
    let cells = p.vm.upvalues.clone();
    let globals = p.vm.globals.clone();
    let probes = p.probe_globals().unwrap();
    assert_eq!(probes.len(), 40);
    assert!(probes.iter().any(|v| v.outcome.starts_with("blocked:")));
    assert_eq!(p.vm.globals, globals);
    assert_eq!(p.vm.upvalues, cells);
}
#[test]
#[ignore = "requires local original extraction"]
fn original_music_helpers_share_closed_capture() {
    let mut p = fixture();
    p.initialize().unwrap();
    let setter = p.vm.globals["SetActivityMusicLevel"].clone();
    let Value::Closure(set_id) = setter else {
        panic!("original captured setter")
    };
    let Value::Closure(update_id) = p.vm.globals["UpdateMusic"] else {
        panic!("original captured updater")
    };
    let cell = p.vm.closures[set_id].upvalues[0];
    assert!(p.vm.closures[update_id].upvalues.contains(&cell));
    assert_eq!(p.vm.upvalues[cell], Value::Number(0.15));
    let mut thread = Thread::from_callable(
        "original activity setter",
        &setter,
        &[Value::Number(0.75)],
        &p.vm,
    )
    .unwrap();
    assert!(matches!(
        thread.resume(&mut p.vm, &mut p.host, 100).unwrap(),
        Status::Complete(_)
    ));
    assert_eq!(p.vm.upvalues[cell], Value::Number(0.75));
    let Value::Closure(start) = p.vm.globals["StartIntro"] else {
        panic!()
    };
    let Value::Closure(end) = p.vm.globals["EndIntro"] else {
        panic!()
    };
    assert_eq!(p.vm.closures[start].upvalues, p.vm.closures[end].upvalues);
}
#[test]
#[ignore = "requires local original extraction"]
fn authored_player_camera_resolves_without_inventing_squad_ownership() {
    let mut graph = original_graph();
    let mut w = bw_rust::mission_world::World::from_graph(&graph).unwrap();
    assert!(w.configure_authored_player(&graph).is_err());
    assert!(w.player.is_none());
    w.configure_authored_units(&graph).unwrap();
    let id = w.configure_authored_player(&graph).unwrap();
    assert_eq!(id, bw_rust::ObjectId(550030791));
    assert_eq!(w.player, Some(id));
    assert!(w.roster.is_empty());
    let mut duplicate = graph.objects[&bw_rust::ObjectId(550030276)].clone();
    duplicate.id = bw_rust::ObjectId(999999999);
    graph.objects.insert(duplicate.id, duplicate);
    assert!(w.configure_authored_player(&graph).is_err());
    assert_eq!(w.player, Some(id));
    assert!(w.roster.is_empty());
}
#[test]
#[ignore = "requires original level and stereo music extraction"]
fn original_start_intro_generates_stereo_pcm_and_scheduler_pause_holds_audio() {
    use bw_rust::{
        mission_music::{Bank, Transport},
        mission_vm::Scheduler,
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("local/extracted/files");
    let bank = std::sync::Arc::new(Bank::load(&root, &original_graph()).unwrap());
    assert_eq!(bank.tracks["INTRO"].left.pcm.len(), 650272);
    assert_eq!(bank.tracks["MAIN_MUSIC"].left.pcm.len(), 3065776);
    assert!(std::sync::Arc::ptr_eq(
        &bank.tracks["INTRO"],
        &bank.tracks["OUTRO"]
    ));
    assert_ne!(
        bank.tracks["INTRO"].left.pcm,
        bank.tracks["INTRO"].right.pcm
    );
    let mut p = fixture();
    p.host.world.music = Some(Transport::new(bank));
    p.initialize().unwrap();
    let mut intro =
        Thread::from_callable("StartIntro", &p.vm.globals["StartIntro"], &[], &p.vm).unwrap();
    assert!(matches!(
        intro.resume(&mut p.vm, &mut p.host, 1000).unwrap(),
        Status::Complete(_)
    ));
    let m = p.host.world.music.as_ref().unwrap();
    assert_eq!(m.playing.as_deref(), Some("INTRO"));
    assert_eq!(m.gain, 0.0);
    let mut scheduler = Scheduler::default();
    scheduler.tick(&mut p.vm, &mut p.host, 1.0, 1000).unwrap();
    let m = p.host.world.music.as_ref().unwrap();
    assert_eq!(m.cursor, 24000);
    assert_eq!(m.block.len(), 48000);
    assert!(m.block.iter().any(|s| s.abs() > 0.0001));
    assert!(m.block.iter().all(|s| s.is_finite() && s.abs() <= 0.801));
    let cursor = m.cursor;
    let sequence = m.sequence;
    scheduler.paused = true;
    scheduler.tick(&mut p.vm, &mut p.host, 0.5, 1000).unwrap();
    let m = p.host.world.music.as_ref().unwrap();
    assert_eq!(m.cursor, cursor);
    assert_eq!(m.sequence, sequence);
    let probes = p.probe_globals().unwrap();
    let intro_probe = probes
        .iter()
        .find(|q| q.script == "Gambit_Part_1_Cutscene_INTRO")
        .unwrap();
    assert!(
        intro_probe.outcome.contains("ForceUnitTransfer"),
        "{}",
        intro_probe.outcome
    );
}
#[test]
#[ignore = "requires original level and stereo music extraction"]
fn original_intro_prefix_self_transfer_and_explicit_switch_fixture_are_atomic() {
    use bw_rust::{
        ObjectId,
        mission_music::{Bank, Transport},
        mission_world::MoveOrder,
    };
    let graph = original_graph();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("local/extracted/files");
    let mut p = fixture();
    p.host.world.configure_authored_units(&graph).unwrap();
    let old = p.host.world.configure_authored_player(&graph).unwrap();
    p.host.world.music = Some(Transport::new(std::sync::Arc::new(
        Bank::load(&root, &graph).unwrap(),
    )));
    p.initialize().unwrap();
    let Value::Table(troops) = p.vm.globals["Troop"] else {
        panic!()
    };
    let Value::Object(target) = p.vm.tables[troops]["Sstaffbearer03"] else {
        panic!()
    };
    // The authored initial camera ALREADY targets Sstaffbearer03.
    assert_eq!(old, target);
    let binding = p
        .bindings
        .iter()
        .find(|b| b.script == "Gambit_Part_1_Cutscene_INTRO")
        .unwrap();
    let owner = binding.entity;
    let Value::Function { script, child } = &p.vm.globals[&binding.script] else {
        panic!()
    };
    let mut prefix = p.vm.functions[&(script.clone(), *child)].clone();
    // Explicit bounded original instruction fixture, NOT a complete cinematic.
    assert_eq!(prefix.code[74] & 63, 25);
    let next = ((prefix.code[75] >> 6) & 262143) as usize;
    assert_eq!(
        prefix.constants[next],
        bw_rust::lua50::Constant::String("SetCamera".into())
    );
    prefix.code.truncate(75);
    prefix.code.push(27 | (1 << 15));
    let mut bad = prefix.clone();
    assert_eq!(bad.code[72] & 63, 2);
    bad.code[72] &= !(511 << 15);
    let cells = p.vm.upvalues.clone();
    let mut thread =
        Thread::new("unsupported transfer fixture", bad, &[Value::Object(owner)]).unwrap();
    assert!(
        thread
            .resume(&mut p.vm, &mut p.host, 2000)
            .unwrap_err()
            .contains("ForceUnitTransfer")
    );
    assert_eq!(p.host.world.player, Some(old));
    assert!(p.host.world.music.as_ref().unwrap().playing.is_none());
    assert!(!p.host.world.frozen);
    assert_eq!(p.vm.upvalues, cells);
    let mut thread = Thread::new(
        "original intro prefix",
        prefix.clone(),
        &[Value::Object(owner)],
    )
    .unwrap();
    assert!(matches!(
        thread.resume(&mut p.vm, &mut p.host, 2000).unwrap(),
        Status::Complete(_)
    ));
    assert_eq!(p.host.world.player, Some(target));
    let view = p.host.world.player_view.as_ref().unwrap();
    assert_eq!(view.target, target);
    assert_eq!(view.camera, ObjectId(550030276));
    assert_eq!(view.cut_revision, 0);
    assert!(p.host.world.frozen);
    assert!(p.host.world.roster.is_empty());
    assert!(!p.host.world.direct_velocity.contains_key(&old));
    assert!(!p.host.world.orders.contains_key(&target));
    assert_eq!(
        p.host.world.music.as_ref().unwrap().playing.as_deref(),
        Some("INTRO")
    );
    // A separately labelled altered-target fixture exercises a REAL switch, not
    // the original intro's redundant self-transfer. Do not call it original playback.
    let Value::Object(switched) = p.vm.tables[troops]["Sgrunt02"] else {
        panic!()
    };
    assert_ne!(switched, old);
    for constant in &mut prefix.constants {
        if *constant == bw_rust::lua50::Constant::String("Sstaffbearer03".into()) {
            *constant = bw_rust::lua50::Constant::String("Sgrunt02".into());
        }
    }
    p.host.world.direct_velocity.insert(old, [1.0, 0.0]);
    p.host.world.orders.insert(
        switched,
        MoveOrder {
            target: [0.0, 0.0],
            radius: 1.0,
        },
    );
    let mut fixture = Thread::new(
        "altered-target transfer fixture",
        prefix,
        &[Value::Object(owner)],
    )
    .unwrap();
    fixture.resume(&mut p.vm, &mut p.host, 2000).unwrap();
    assert_eq!(p.host.world.player, Some(switched));
    let view = p.host.world.player_view.as_ref().unwrap();
    assert_eq!(view.target, switched);
    assert_eq!(view.cut_revision, 1);
    assert!(!p.host.world.direct_velocity.contains_key(&old));
    assert!(!p.host.world.orders.contains_key(&switched));
    let probe = p
        .probe_globals()
        .unwrap()
        .into_iter()
        .find(|q| q.script == "Gambit_Part_1_Cutscene_INTRO")
        .unwrap();
    assert!(probe.outcome.contains("SetCamera"), "{}", probe.outcome);
}
#[test]
#[ignore = "requires original level and stereo music extraction"]
fn original_intro_camera_prefix_samples_authored_paths() {
    use bw_rust::{mission_camera, mission_music, mission_vm::Scheduler};
    use std::sync::Arc;
    let graph = original_graph();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("local/extracted/files");
    let bank = Arc::new(mission_camera::Bank::load(&graph).unwrap());
    assert_eq!(bank.cameras.len(), 10);
    assert_eq!(bank.waypoints.len(), 637);
    let mut p = fixture();
    p.host.world.configure_authored_units(&graph).unwrap();
    p.host.world.configure_authored_player(&graph).unwrap();
    p.host.world.cameras = Some(mission_camera::Rig::new(bank));
    p.host.world.music = Some(mission_music::Transport::new(Arc::new(
        mission_music::Bank::load(&root, &graph).unwrap(),
    )));
    p.initialize().unwrap();
    let b = p
        .bindings
        .iter()
        .find(|b| b.script == "Gambit_Part_1_Cutscene_INTRO")
        .unwrap();
    let owner = b.entity;
    let Value::Function { script, child } = &p.vm.globals[&b.script] else {
        panic!()
    };
    let mut prefix = p.vm.functions[&(script.clone(), *child)].clone();
    let next = ((prefix.code[93] >> 6) & 262143) as usize;
    assert_eq!(
        prefix.constants[next],
        bw_rust::lua50::Constant::String("ClearMessageQueue".into())
    );
    prefix.code.truncate(93);
    prefix.code.push(27 | (1 << 15));
    // Bounded original prefix fixture, not an entire rendered intro.
    let mut t = Thread::new("original camera prefix", prefix, &[Value::Object(owner)]).unwrap();
    assert!(matches!(
        t.resume(&mut p.vm, &mut p.host, 2000).unwrap(),
        Status::Complete(_)
    ));
    let before = p
        .host
        .world
        .cameras
        .as_ref()
        .unwrap()
        .pose(&p.host.positions)
        .unwrap()
        .unwrap();
    assert!((before.fov_degrees - 45.0).abs() < 0.001);
    let mut s = Scheduler::default();
    s.tick(&mut p.vm, &mut p.host, 0.5, 2000).unwrap();
    let after = p
        .host
        .world
        .cameras
        .as_ref()
        .unwrap()
        .pose(&p.host.positions)
        .unwrap()
        .unwrap();
    assert_ne!(before.position, after.position);
    assert!(
        after
            .position
            .iter()
            .chain(&after.look_at)
            .all(|x| x.is_finite())
    );
    s.paused = true;
    s.tick(&mut p.vm, &mut p.host, 0.5, 2000).unwrap();
    assert_eq!(
        p.host
            .world
            .cameras
            .as_ref()
            .unwrap()
            .pose(&p.host.positions)
            .unwrap()
            .unwrap(),
        after
    );
    let probe = p
        .probe_globals()
        .unwrap()
        .into_iter()
        .find(|q| q.script == "Gambit_Part_1_Cutscene_INTRO")
        .unwrap();
    assert!(
        probe.outcome.contains("ClearMessageQueue"),
        "{}",
        probe.outcome
    );
}
#[test]
#[ignore = "requires original level, stereo music and voices"]
fn unmodified_intro_yields_for_fade_and_rolls_back_at_unsupported_dialogue() {
    use bw_rust::{
        localization, mission_audio, mission_camera, mission_messages, mission_music,
        mission_vm::Scheduler,
    };
    use std::sync::Arc;
    let graph = original_graph();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("local/extracted/files");
    let mut p = fixture();
    p.host.world.configure_authored_units(&graph).unwrap();
    p.host.world.configure_authored_player(&graph).unwrap();
    p.host.world.cameras = Some(mission_camera::Rig::new(Arc::new(
        mission_camera::Bank::load(&graph).unwrap(),
    )));
    p.host.world.music = Some(mission_music::Transport::new(Arc::new(
        mission_music::Bank::load(&root, &graph).unwrap(),
    )));
    let language = localization::Language::parse(
        &std::fs::read(root.join("Data/Strings/SP_0.1English.str")).unwrap(),
    )
    .unwrap();
    p.host.world.messages = Some(mission_messages::Queue::new(Arc::new(
        mission_audio::Bank::load(&root, &language).unwrap(),
    )));
    p.initialize().unwrap();
    let b = p
        .bindings
        .iter()
        .find(|b| b.script == "Gambit_Part_1_Cutscene_INTRO")
        .unwrap();
    let owner = b.entity;
    let mut intro = p.thread(b).unwrap();
    assert!(matches!(
        intro.resume(&mut p.vm, &mut p.host, 4000).unwrap(),
        Status::Sleeping(_)
    ));
    assert_eq!(p.host.world.cameras.as_ref().unwrap().black_alpha, 1.0);
    assert_eq!(
        p.host
            .world
            .messages
            .as_ref()
            .unwrap()
            .cancellation_revision,
        1
    );
    let mut s = Scheduler::default();
    s.threads.insert(owner.0, intro);
    s.tick(&mut p.vm, &mut p.host, 0.5, 4000).unwrap();
    assert_eq!(p.host.world.cameras.as_ref().unwrap().black_alpha, 0.5);
    s.paused = true;
    s.tick(&mut p.vm, &mut p.host, 0.5, 4000).unwrap();
    assert_eq!(p.host.time, 0.5);
    assert_eq!(p.host.world.cameras.as_ref().unwrap().black_alpha, 0.5);
    s.paused = false;
    s.tick(&mut p.vm, &mut p.host, 0.49, 4000).unwrap();
    let alpha = p.host.world.cameras.as_ref().unwrap().black_alpha;
    let cursor = p.host.world.music.as_ref().unwrap().cursor;
    let time = p.host.time;
    let error = s.tick(&mut p.vm, &mut p.host, 0.02, 4000).unwrap_err();
    assert!(error.contains("PhoneMessage"), "{error}");
    assert_eq!(p.host.time, time);
    assert_eq!(p.host.world.cameras.as_ref().unwrap().black_alpha, alpha);
    assert_eq!(p.host.world.music.as_ref().unwrap().cursor, cursor);
    assert_eq!(p.host.world.messages.as_ref().unwrap().count(), 0);
    p.host
        .world
        .messages
        .as_mut()
        .unwrap()
        .configure_voiced_playback(Arc::new(language), p.host.known_objects.clone());
    let mut blocker = None;
    let mut heard = false;
    for _ in 0..1800 {
        if let Err(error) = s.tick(&mut p.vm, &mut p.host, 1.0 / 60.0, 4000) {
            blocker = Some(error);
            break;
        }
        heard |= p
            .host
            .world
            .messages
            .as_ref()
            .unwrap()
            .block
            .iter()
            .any(|x| x.abs() > 0.0001);
    }
    let blocker = blocker.expect("Further intro capabilities still unimplemented");
    assert!(
        heard,
        "Actual original voice PCM must be generated before the next blocker"
    );
    assert!(blocker.contains("FollowWaypoint"), "{blocker}");
}
#[test]
#[ignore = "requires original level extraction"]
fn original_infantry_waypoint_chain_moves_in_explicit_isolated_collision_fixture() {
    use bw_rust::{
        ObjectId, mission_vm::Scheduler, mission_waypoints::Bank, mission_world::Navigation,
    };
    use std::sync::Arc;
    let graph = original_graph();
    let bank = Arc::new(Bank::load(&graph).unwrap());
    let mut p = fixture();
    p.initialize().unwrap();
    p.host.world.configure_authored_units(&graph).unwrap();
    let object = |table: &str, name: &str| -> ObjectId {
        let Value::Table(index) = p.vm.globals[table] else {
            panic!()
        };
        let Value::Object(id) = p.vm.tables[index][name] else {
            panic!()
        };
        id
    };
    let actor = object("Troop", "Sstrikebazooka01");
    let waypoint = object("Waypoint", "WP01");
    // Original unit/route/offsets, isolated from the rest of the level. Empty
    // collision geometry is NOT the original terrain or full mission acceptance.
    p.host.world.units.retain(|id, _| *id == actor);
    p.host.positions.retain(|id, _| *id == actor);
    p.host.world.navigation = Navigation::CollisionAware;
    p.host.world.collision = Some(Arc::new(bw_rust::unit_collision::Scene::default()));
    p.host.world.waypoints = Some(bank.clone());
    let before = p.host.positions[&actor];
    p.host
        .world
        .follow_waypoint(actor, waypoint, [15.0; 2], &p.host.positions)
        .unwrap();
    assert_eq!(
        p.host.world.orders[&actor].target,
        bank.target(waypoint, [15.0; 2]).unwrap()
    );
    let mut visited = std::collections::BTreeSet::new();
    let mut scheduler = Scheduler::default();
    for _ in 0..2000 {
        if let Some(route) = p.host.world.following.get(&actor) {
            visited.insert(route.node);
        } else {
            break;
        }
        scheduler.tick(&mut p.vm, &mut p.host, 0.25, 1000).unwrap();
    }
    assert!(p.host.world.following.is_empty());
    assert!(p.host.world.orders.is_empty());
    assert!(visited.len() >= 4, "{visited:?}");
    assert_ne!(p.host.positions[&actor], before);
}
#[test]
#[ignore = "requires local original extraction"]
fn initialization_failure_rolls_back_prior_initializers_and_captures() {
    let mut p = fixture();
    let last = p
        .bindings
        .iter()
        .rfind(|b| b.scope == Scope::Initializer)
        .unwrap()
        .script
        .clone();
    p.vm.globals.insert(last, Value::Nil);
    let before = p.vm.globals.clone();
    assert!(p.initialize().is_err());
    assert!(!p.initialized);
    assert_eq!(p.vm.globals, before);
    assert!(p.vm.upvalues.is_empty());
    assert!(p.vm.closures.is_empty());
    assert!(p.host.debug.is_empty());
}
#[test]
#[ignore = "requires local original extraction"]
fn original_battle_waypoint_branches_validate_and_advance_with_crt_rng() {
    use bw_rust::{
        ObjectId, mission_vm::Scheduler, mission_waypoints::Bank, mission_world::Navigation,
    };
    use std::sync::Arc;
    let graph = original_graph();
    let bank = Arc::new(Bank::load(&graph).unwrap());
    let mut p = fixture();
    p.initialize().unwrap();
    p.host.world.configure_authored_units(&graph).unwrap();
    let object = |table: &str, name: &str| -> ObjectId {
        let Value::Table(index) = p.vm.globals[table] else {
            panic!()
        };
        let Value::Object(id) = p.vm.tables[index][name] else {
            panic!()
        };
        id
    };
    for name in [
        "BattleLeft02",
        "BattleRight02",
        "BattleLeft03",
        "BattleRight03",
        "BattleLeft04",
        "BattleRight04",
        "BattleLeft05",
        "BattleMainLeft03",
        "BattleMainLeft02",
        "BattleMainLeft01",
        "BattleMainRight01",
        "BattleMainRight02",
        "BattleMainRight03",
        "BattleLastLeft01",
        "BattleLastRight01",
    ] {
        bw_rust::mission_waypoints::Route::new(&bank, object("Waypoint", name), [5.0, 5.0])
            .unwrap();
    }
    let actor = object("Troop", "Sstrikebazooka01");
    let start = object("Waypoint", "BattleLeft02");
    assert_eq!(start, ObjectId(550030756));
    p.host.world.units.retain(|id, _| *id == actor);
    p.host.positions.retain(|id, _| *id == actor);
    p.host.positions.insert(actor, [21.0, 0.0, -36.0]);
    p.host.world.navigation = Navigation::CollisionAware;
    p.host.world.collision = Some(Arc::new(bw_rust::unit_collision::Scene::default()));
    p.host.world.waypoints = Some(bank);
    let initial_seed = p.host.world.waypoint_rand.state();
    p.host
        .world
        .follow_waypoint(actor, start, [5.0, 5.0], &p.host.positions)
        .unwrap();
    let mut visited = Vec::new();
    let mut scheduler = Scheduler::default();
    for _ in 0..800 {
        if let Some(route) = p.host.world.following.get(&actor) {
            if visited.last() != Some(&route.node) {
                visited.push(route.node);
            }
        } else {
            break;
        }
        scheduler.tick(&mut p.vm, &mut p.host, 0.25, 1000).unwrap();
    }
    // 550030756 -> 550030757 (unconditional) -> 550030882 (first CrtRand draw: 16838 % 3 == 2 -> slot 2).
    assert_eq!(
        &visited[..3],
        &[
            ObjectId(550030756),
            ObjectId(550030757),
            ObjectId(550030882),
        ]
    );
    assert_ne!(p.host.world.waypoint_rand.state(), initial_seed);
}
