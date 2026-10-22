use bw_rust::{
    ObjectGraph, mission_controls::Controller, mission_program::Program, mission_vm::Status,
    parse_objects,
};
#[cfg(feature = "viewer")]
#[test]
#[ignore = "requires local original mission extraction"]
fn original_drained_weapon_reads_masked_bevy_input() {
    use bevy::prelude::*;
    use bw_rust::{controls::Controls, mission_input::Input as ScriptInput};
    let (mut p, graph) = load();
    p.initialize().unwrap();
    p.host.world.configure_authored_units(&graph).unwrap();
    p.host.world.configure_authored_player(&graph).unwrap();
    let mut controller = Controller::fresh_player();
    controller
        .call(&[
            bw_rust::lua50::Value::Enum("CONTROL_FIRE".into()),
            bw_rust::lua50::Value::Number(0.0),
        ])
        .unwrap();
    p.host.world.controller = Some(controller);
    let controls = Controls::parse(include_str!("../config/controls.json")).unwrap();
    let keys = ButtonInput::default();
    let mut mouse = ButtonInput::default();
    mouse.press(MouseButton::Left);
    let buttons = controls.mission_script_buttons(&keys, &mouse, None);
    assert!(buttons["CONTROL_FIRE"].just_pressed);
    let mut input = ScriptInput::single_player();
    input.submit(p.host.frame, buttons).unwrap();
    p.host.world.input = Some(input);
    let binding = p
        .bindings
        .iter()
        .find(|b| b.script == "Gambit_Part_1_Drained_Weapon")
        .unwrap();
    let mut thread = p.thread(binding).unwrap();
    let result = thread.resume(&mut p.vm, &mut p.host, 1000);
    println!("ORIGINAL_MASKED_INPUT_RESULT {result:?}");
    assert!(matches!(result.unwrap(), Status::Sleeping(_)));
    assert!(p.host.world.declared_victory.is_none());
}
fn load() -> (Program, ObjectGraph) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("local/extracted/files/Data/CompoundFiles");
    let mut graph = ObjectGraph::default();
    for name in ["SP_0.1_Level.xml", "SP_0.1_Level_preload.xml"] {
        graph
            .merge(parse_objects(&std::fs::read_to_string(root.join(name)).unwrap()).unwrap())
            .unwrap()
    }
    let p = Program::load(
        &graph,
        &std::fs::read(root.join("SP_0.1_Level.res")).unwrap(),
    )
    .unwrap();
    (p, graph)
}
#[test]
#[ignore = "requires local original mission extraction"]
fn original_disabled_controls_apply_atomically() {
    let (mut p, _) = load();
    p.initialize().unwrap();
    let binding = p
        .bindings
        .iter()
        .find(|b| b.script == "Gambit_Part_1_Controls_Disabled")
        .unwrap();
    let mut thread = p.thread(binding).unwrap();
    assert!(
        thread
            .resume(&mut p.vm, &mut p.host, 1000)
            .unwrap_err()
            .contains("Controller mask backend not configured")
    );
    p.host.world.controller = Some(Controller::default());
    assert!(thread.resume(&mut p.vm, &mut p.host, 10).is_err());
    assert_eq!(thread.pc, 0);
    assert_eq!(p.host.world.controller, Some(Controller::default()));
    assert!(matches!(
        thread.resume(&mut p.vm, &mut p.host, 1000).unwrap(),
        Status::Sleeping(_)
    ));
    let controller = p.host.world.controller.as_ref().unwrap();
    for item in [
        "CONTROL_TRANSFER",
        "CONTROL_HUD_TRANSFER",
        "CONTROL_MAP_TRANSFER",
        "CONTROL_NEXT_UNIT_TYPE",
        "CONTROL_NEXT_UNIT_TYPE_ALT",
        "CONTROL_PREV_UNIT_TYPE",
        "CONTROL_NEXT_UNIT",
        "CONTROL_PREV_UNIT",
        "CONTROL_TOGGLE_MAP",
        "CONTROL_GLOBAL_TOGGLE",
    ] {
        assert!(!controller.accepts(item).unwrap());
    }
    assert_eq!(controller.enabled("CONTROL_FIRE").unwrap(), None);
    assert!(p.host.world.declared_victory.is_none());
    println!(
        "ORIGINAL_CONTROLLER_MASK_PASS disabled=10 rollback=true physical_input_consumption=false"
    );
}
#[test]
#[ignore = "requires local original mission extraction"]
fn original_controller_sets_presentation_requests_transactionally() {
    use bw_rust::mission_hud::Hud;
    let (mut p, graph) = load();
    p.initialize().unwrap();
    p.host.world.configure_authored_units(&graph).unwrap();
    p.host.world.configure_authored_player(&graph).unwrap();
    let binding = p
        .bindings
        .iter()
        .find(|b| b.script == "Gambit_Part_1_Controller")
        .unwrap();
    let mut thread = p.thread(binding).unwrap();
    assert!(
        thread
            .resume(&mut p.vm, &mut p.host, 1000)
            .unwrap_err()
            .contains("HUD presentation backend not configured")
    );
    p.host.world.hud = Some(Hud::default());
    assert!(thread.resume(&mut p.vm, &mut p.host, 15).is_err());
    assert_eq!(thread.pc, 0);
    assert_eq!(p.host.world.hud, Some(Hud::default()));
    let result = thread.resume(&mut p.vm, &mut p.host, 1000);
    println!("ORIGINAL_HUD_CONTROLLER_RESULT {result:?}");
    assert!(matches!(result.unwrap(), Status::Sleeping(_)));
    let hud = p.host.world.hud.as_ref().unwrap();
    assert_eq!(hud.requested("HUD_COMMANDBAR").unwrap(), Some(false));
    assert_eq!(hud.requested("HUD_COMMANDSTACK").unwrap(), Some(false));
    assert_eq!(hud.function_indicator, Some(false));
    assert_eq!(hud.revision, 3);
    assert!(p.host.world.declared_victory.is_none());
}
