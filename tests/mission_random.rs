use bw_rust::{
    lua50::{Bootstrap, Constant, Prototype, Value},
    mission_random::Stream,
    mission_vm::{Host, Status, Thread},
    mission_world::{MissionHistory, World},
};
fn query(fail_after_draw: bool) -> Thread {
    let mut constants = vec![Constant::String("GetRandom".into())];
    let mut code = vec![5, 25 | (1 << 15) | (2 << 6)];
    if fail_after_draw {
        constants.push(Constant::String("GetCurrentMissionAttempted".into()));
        code.extend([5 | (1 << 6), 25 | (1 << 15) | (2 << 6)]);
    }
    code.push(27 | (2 << 15));
    Thread::new(
        "native random transaction",
        Prototype {
            upvalues: 0,
            parameters: 0,
            vararg: false,
            stack: 1,
            constants,
            children: vec![],
            code,
        },
        &[],
    )
    .unwrap()
}
#[test]
fn missing_cursor_is_not_an_invented_random_default() {
    let mut vm = Bootstrap::new();
    World::install(&mut vm).unwrap();
    let mut host = Host::default();
    let mut thread = query(false);
    assert!(
        thread
            .resume(&mut vm, &mut host, 100)
            .unwrap_err()
            .contains("Native random cursor not configured")
    );
    assert_eq!(thread.pc, 0);
    host.world.random = Some(Stream::at_cursor(0).unwrap());
    assert_eq!(
        thread.resume(&mut vm, &mut host, 100).unwrap(),
        Status::Complete(vec![Value::Number(0.74212646484375)])
    );
}
#[test]
fn failed_script_rolls_back_random_draw_and_retry_consumes_once() {
    let mut vm = Bootstrap::new();
    World::install(&mut vm).unwrap();
    let mut host = Host::default();
    host.world.random = Some(Stream::at_cursor(0).unwrap());
    let mut thread = query(true);
    assert!(
        thread
            .resume(&mut vm, &mut host, 100)
            .unwrap_err()
            .contains("history not configured")
    );
    assert_eq!(thread.pc, 0);
    assert_eq!(host.world.random.as_ref().unwrap().cursor(), 0);
    host.world.mission_history = Some(MissionHistory::fresh_attempt());
    assert_eq!(
        thread.resume(&mut vm, &mut host, 100).unwrap(),
        Status::Complete(vec![Value::Boolean(true)])
    );
    assert_eq!(host.world.random.as_ref().unwrap().cursor(), 1);
}
