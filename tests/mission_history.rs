use bw_rust::{
    lua50::{Bootstrap, Constant, Prototype, Value},
    mission_vm::{Host, Status, Thread},
    mission_world::{MissionHistory, World},
};
fn query(name: &str) -> Thread {
    Thread::new(
        "profile query",
        Prototype {
            upvalues: 0,
            parameters: 0,
            vararg: false,
            stack: 1,
            constants: vec![Constant::String(name.into())],
            children: vec![],
            code: vec![5, 25 | (1 << 15) | (2 << 6), 27 | (2 << 15)],
        },
        &[],
    )
    .unwrap()
}
#[test]
fn queries_require_explicit_history_and_preserve_separate_bits() {
    let mut vm = Bootstrap::new();
    World::install(&mut vm).unwrap();
    let mut host = Host::default();
    let mut thread = query("GetCurrentMissionCompleted");
    assert!(
        thread
            .resume(&mut vm, &mut host, 100)
            .unwrap_err()
            .contains("history not configured")
    );
    assert_eq!(thread.pc, 0);
    for attempted in [false, true] {
        for completed in [false, true] {
            host.world.mission_history = Some(MissionHistory {
                attempted,
                completed,
            });
            for (name, want) in [
                ("GetCurrentMissionAttempted", attempted),
                ("GetCurrentMissionCompleted", completed),
                ("GetTutorialMode", true),
            ] {
                assert_eq!(
                    query(name).resume(&mut vm, &mut host, 100).unwrap(),
                    Status::Complete(vec![Value::Boolean(want)])
                );
            }
            assert!(host.world.declared_victory.is_none());
        }
    }
}
#[test]
fn fresh_attempt_does_not_complete_mission() {
    assert_eq!(
        MissionHistory::fresh_attempt(),
        MissionHistory {
            attempted: true,
            completed: false
        }
    );
}
#[test]
#[ignore = "requires original local mission extraction"]
fn original_gameflow_reaches_next_frontier_with_explicit_fresh_profile() {
    use bw_rust::{ObjectGraph, mission_program::Program, parse_objects};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("local/extracted/files/Data/CompoundFiles");
    let mut graph = ObjectGraph::default();
    for name in ["SP_0.1_Level.xml", "SP_0.1_Level_preload.xml"] {
        graph
            .merge(parse_objects(&std::fs::read_to_string(root.join(name)).unwrap()).unwrap())
            .unwrap()
    }
    let mut program = Program::load(
        &graph,
        &std::fs::read(root.join("SP_0.1_Level.res")).unwrap(),
    )
    .unwrap();
    program.initialize().unwrap();
    let binding = program
        .bindings
        .iter()
        .find(|b| b.script == "Gambit_Part_1_Gameflow")
        .unwrap();
    let mut thread = program.thread(binding).unwrap();
    assert!(
        thread
            .resume(&mut program.vm, &mut program.host, 10000)
            .unwrap_err()
            .contains("history not configured")
    );
    program.host.world.mission_history = Some(MissionHistory::fresh_attempt());
    let before_globals = program.vm.globals.clone();
    let before_units: Vec<_> = program
        .host
        .world
        .units
        .iter()
        .map(|(id, u)| (*id, u.active))
        .collect();
    let result = thread.resume(&mut program.vm, &mut program.host, 10000);
    println!("FRESH_PROFILE_GAMEFLOW_RESULT {result:?}");
    assert!(program.host.world.declared_victory.is_none());
    let error = result.unwrap_err();
    assert!(error.contains("Gambit_Part_1_Gameflow pc 153: GetPlayerUnit"));
    assert!(program.host.world.despawn_retention.is_empty());
    assert_eq!(thread.pc, 0);
    assert_eq!(program.vm.globals, before_globals);
    assert_eq!(
        program
            .host
            .world
            .units
            .iter()
            .map(|(id, u)| (*id, u.active))
            .collect::<Vec<_>>(),
        before_units
    );
}
