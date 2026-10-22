use bw_rust::{
    ObjectGraph,
    mission_program::{Program, Scope},
    mission_vm::Status,
    parse_objects,
};
#[test]
#[ignore = "requires local original mission extraction"]
fn original_globals_share_one_transaction_and_keep_authored_pause_flags() {
    let (mut p, graph) = load();
    assert!(p.global_scheduler().is_err());
    p.initialize().unwrap();
    p.host.world.configure_authored_units(&graph).unwrap();
    p.host.world.configure_authored_player(&graph).unwrap();
    p.host.world.mission_history = Some(bw_rust::mission_world::MissionHistory::fresh_attempt());
    p.host.world.controller = Some(bw_rust::mission_controls::Controller::default());
    let mut scheduler = p.global_scheduler().unwrap();
    assert_eq!(scheduler.threads.len(), 40);
    assert_eq!(scheduler.update_when_paused.len(), 1);
    for b in &p.bindings {
        if b.scope == Scope::Global {
            assert!(scheduler.threads.contains_key(&b.entity.0));
            assert_eq!(
                scheduler.update_when_paused.contains(&b.entity.0),
                b.update_when_paused
            );
        } else {
            assert!(!scheduler.threads.contains_key(&b.entity.0));
        }
    }
    let globals = p.vm.globals.clone();
    let positions = p.host.positions.clone();
    let error = scheduler
        .tick(&mut p.vm, &mut p.host, 0.0, 100_000)
        .unwrap_err();
    assert_eq!(p.vm.globals, globals);
    assert_eq!(p.host.positions, positions);
    assert_eq!(p.host.time, 0.0);
    assert_eq!(p.host.frame, 0);
    assert!(
        scheduler
            .threads
            .values()
            .all(|t| t.pc == 0 && t.status == Status::Runnable)
    );
    assert!(p.host.world.declared_victory.is_none());
    println!(
        "ORIGINAL_SHARED_GLOBAL_FRONTIER {error}; global_threads=40 pause_eligible=1 whole_tick_rollback=true unit_scripts_attached=false"
    );
    scheduler.paused = true;
    scheduler
        .tick(&mut p.vm, &mut p.host, 0.5, 100_000)
        .unwrap();
    let eligible = *scheduler.update_when_paused.first().unwrap();
    assert_eq!(
        scheduler.threads[&eligible].name,
        "Gambit_Part_1_Pause_Tutorial"
    );
    assert!(matches!(
        scheduler.threads[&eligible].status,
        Status::Sleeping(_)
    ));
    assert!(
        scheduler
            .threads
            .iter()
            .filter(|(id, _)| **id != eligible)
            .all(|(_, t)| t.pc == 0)
    );
    assert_eq!(p.host.time, 0.0);
    assert_eq!(p.host.no_pause_time, 0.5);
    assert_eq!(p.host.positions, positions);
    let instructions = scheduler.threads[&eligible].instructions;
    scheduler
        .tick(&mut p.vm, &mut p.host, 0.5, 100_000)
        .unwrap();
    assert!(scheduler.threads[&eligible].instructions > instructions);
    assert_eq!(p.host.time, 0.0);
    assert_eq!(p.host.no_pause_time, 1.0);
    println!(
        "ORIGINAL_PAUSED_GLOBAL_PASS frames=2 gameplay_clock=0 no_pause_clock=1 other_globals_not_run=true"
    );
}

#[test]
#[ignore = "requires local original mission extraction"]
fn original_winlose_uses_fresh_player_state_without_inventing_a_roster() {
    use bw_rust::mission_world::PlayerLifecycle;
    let (mut p, graph) = load();
    p.initialize().unwrap();
    p.host.world.configure_authored_units(&graph).unwrap();
    let player = p.host.world.configure_authored_player(&graph).unwrap();
    p.host.world.configure_fresh_player_lifecycle().unwrap();
    let binding = p
        .bindings
        .iter()
        .find(|b| b.script == "Gambit_Part_1_WinLose")
        .unwrap();
    let mut thread = p.thread(binding).unwrap();
    let result = thread.resume(&mut p.vm, &mut p.host, 10000);
    println!("ORIGINAL_FRESH_PLAYER_WINLOSE {result:?}");
    assert!(matches!(result.unwrap(), Status::Sleeping(_)));
    assert!(p.host.world.roster.is_empty());
    assert!(p.host.world.declared_victory.is_none());
    assert!(!p.host.world.units[&player].invulnerable);
    let lethal = p.host.world.units[&player].max_health * 2.0;
    p.host.world.damage(player, lethal).unwrap();
    assert_eq!(
        p.host.world.player_lifecycle,
        Some(PlayerLifecycle::ReplacementRequired)
    );
    p.host.frame = 1;
    let globals = p.vm.globals.clone();
    let pc = thread.pc;
    let error = thread.resume(&mut p.vm, &mut p.host, 10000).unwrap_err();
    assert!(
        error.contains("Player replacement/defeat transition not configured"),
        "{error}"
    );
    assert_eq!(p.vm.globals, globals);
    assert_eq!(thread.pc, pc);
    assert!(p.host.world.declared_victory.is_none());
    assert!(p.host.world.roster.is_empty());
    println!("ORIGINAL_PLAYER_REPLACEMENT_FRONTIER {error}");
}
#[test]
#[ignore = "requires local original mission extraction"]
fn original_minibattles_preserves_retained_but_unspawned_units() {
    use bw_rust::lua50::Value;
    let (mut p, graph) = load();
    p.initialize().unwrap();
    p.host.world.configure_authored_units(&graph).unwrap();
    p.host.world.configure_authored_player(&graph).unwrap();
    let Value::Table(table) = &p.vm.globals["Ground_Vehicle"] else {
        panic!("Vehicle table")
    };
    let Value::Object(tank) =
        p.vm.table_get(*table, &Value::String("ILheavytank04".into()))
            .unwrap()
    else {
        panic!("Tank identity")
    };
    let health = p.host.world.units[&tank].health;
    let activity = p.host.world.activity[&tank].enabled;
    let binding = p
        .bindings
        .iter()
        .find(|b| b.script == "Gambit_Part_1_MiniBattles")
        .unwrap();
    let mut thread = p.thread(binding).unwrap();
    let result = thread.resume(&mut p.vm, &mut p.host, 10000);
    println!("ORIGINAL_RETAINED_MINIBATTLES {result:?}");
    assert!(matches!(result.unwrap(), Status::Sleeping(_)));
    assert_eq!(p.host.world.units[&tank].active, Some(false));
    assert!(p.host.world.despawn_retention[&tank]);
    assert!(p.host.world.unit_list_presence(tank).unwrap());
    assert_eq!(p.host.world.units[&tank].health, health);
    assert_eq!(p.host.world.activity[&tank].enabled, activity);
    assert!(p.host.world.declared_victory.is_none());
}
#[test]
#[ignore = "requires local original mission extraction"]
fn original_capture_point_despawn_cannot_use_generic_building_cleanup() {
    let (mut p, graph) = load();
    p.initialize().unwrap();
    p.host.world.configure_authored_units(&graph).unwrap();
    assert_eq!(p.host.world.capture_points.len(), 1);
    let before = p.host.world.despawn_retention.clone();
    let binding = p
        .bindings
        .iter()
        .find(|b| b.script == "Gambit_Part_1_Gameflow_2")
        .unwrap();
    let mut thread = p.thread(binding).unwrap();
    let error = thread.resume(&mut p.vm, &mut p.host, 10000).unwrap_err();
    assert!(error.contains(
        "Gambit_Part_1_Gameflow_2 pc 6: Despawn: Capture-point lifecycle backend not configured"
    ));
    assert_eq!(thread.pc, 0);
    assert_eq!(p.host.world.despawn_retention, before);
    assert!(p.host.world.declared_victory.is_none());
}
fn load() -> (Program, ObjectGraph) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("local/extracted/files/Data/CompoundFiles");
    let mut graph = ObjectGraph::default();
    for name in ["SP_0.1_Level.xml", "SP_0.1_Level_preload.xml"] {
        graph
            .merge(parse_objects(&std::fs::read_to_string(root.join(name)).unwrap()).unwrap())
            .unwrap();
    }
    let p = Program::load(
        &graph,
        &std::fs::read(root.join("SP_0.1_Level.res")).unwrap(),
    )
    .unwrap();
    (p, graph)
}
