use bw_rust::{
    frontend_environment::Environment,
    level_catalog::Catalog,
    lua50::{Bootstrap, Constant, Prototype, Value},
    mission_vm::{Host, Thread},
    mission_world::World,
};
fn environment() -> Environment {
    let catalog = Catalog::parse("<levels><level name=\"Frontend\" filename=\"@os:Data/CompoundFiles/F.xml\" gametype=\"frontend\"/></levels>").unwrap();
    Environment::new(&catalog, 60.0).unwrap()
}
#[test]
fn cold_constructor_truthiness_previous_state_and_invalid_signature() {
    let mut env = environment();
    assert!(!env.controller_errors_enabled());
    assert_eq!(
        env.call("EnableControllerErrorMessages", &[Value::Number(0.0)])
            .unwrap(),
        vec![Value::Boolean(false)]
    );
    assert!(env.controller_errors_enabled());
    assert_eq!(
        env.call(
            "EnableControllerErrorMessages",
            &[Value::String(String::new())]
        )
        .unwrap(),
        vec![Value::Boolean(true)]
    );
    assert!(env.controller_errors_enabled());
    for bad in [vec![], vec![Value::Boolean(false), Value::Nil]] {
        assert!(env.call("EnableControllerErrorMessages", &bad).is_err());
        assert!(env.controller_errors_enabled());
    }
    assert_eq!(
        env.call("EnableControllerErrorMessages", &[Value::Nil])
            .unwrap(),
        vec![Value::Boolean(true)]
    );
    assert!(!env.controller_errors_enabled());
    assert_eq!(
        env.call("EnableControllerErrorMessages", &[Value::Boolean(false)])
            .unwrap(),
        vec![Value::Boolean(false)]
    );
}
#[test]
fn later_script_failure_rolls_back_controller_policy() {
    let mut vm = Bootstrap::new();
    Environment::install(&mut vm).unwrap();
    World::install(&mut vm).unwrap();
    let mut host = Host {
        frontend: Some(environment()),
        ..Host::default()
    };
    let mut thread = Thread::new(
        "policy rollback",
        Prototype {
            upvalues: 0,
            parameters: 0,
            vararg: false,
            stack: 2,
            constants: vec![
                Constant::String("EnableControllerErrorMessages".into()),
                Constant::String("GetCurrentMissionAttempted".into()),
            ],
            children: vec![],
            code: vec![
                5,
                2 | (1 << 24) | (1 << 15),
                25 | (2 << 15) | (2 << 6),
                5 | (1 << 6),
                25 | (1 << 15) | (2 << 6),
                27 | (2 << 15),
            ],
        },
        &[],
    )
    .unwrap();
    assert!(
        thread
            .resume(&mut vm, &mut host, 100)
            .unwrap_err()
            .contains("history not configured")
    );
    assert!(!host.frontend.as_ref().unwrap().controller_errors_enabled());
    assert_eq!(thread.pc, 0);
    host.world.mission_history = Some(bw_rust::mission_world::MissionHistory::fresh_attempt());
    thread.resume(&mut vm, &mut host, 100).unwrap();
    assert!(host.frontend.as_ref().unwrap().controller_errors_enabled());
}
