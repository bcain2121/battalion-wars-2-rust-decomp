use bw_rust::{
    ObjectGraph,
    frontend::Catalog,
    frontend_stack::{Command, PageStack},
    lua50::{Bootstrap, Value},
    parse_objects,
};
#[test]
fn numeric_keys_are_distinct_bounded_and_deleted() {
    let mut vm = Bootstrap::new();
    vm.tables.push(Default::default());
    vm.table_set(0, Value::Number(1.0), Value::String("numeric".into()))
        .unwrap();
    vm.table_set(0, Value::String("1".into()), Value::String("string".into()))
        .unwrap();
    assert_eq!(
        vm.table_get(0, &Value::Number(1.0)).unwrap(),
        Value::String("numeric".into())
    );
    assert_eq!(
        vm.table_get(0, &Value::String("1".into())).unwrap(),
        Value::String("string".into())
    );
    vm.table_set(0, Value::Number(-0.0), Value::Boolean(true))
        .unwrap();
    assert_eq!(
        vm.table_get(0, &Value::Number(0.0)).unwrap(),
        Value::Boolean(true)
    );
    vm.table_set(0, Value::Number(1.0), Value::Nil).unwrap();
    assert_eq!(vm.table_get(0, &Value::Number(1.0)).unwrap(), Value::Nil);
    for n in [f64::NAN, f64::INFINITY, 0.5, 9_007_199_254_740_992.0] {
        assert!(
            vm.table_set(0, Value::Number(n), Value::Boolean(true))
                .is_err()
        );
        assert!(vm.table_get(0, &Value::Number(n)).is_err())
    }
    assert!(vm.table_get(100, &Value::Number(1.0)).is_err());
    assert!(vm.table_set(0, Value::Nil, Value::Boolean(true)).is_err());
}
#[test]
fn numeric_vm_write_rolls_back_on_later_bad_opcode() {
    use bw_rust::{
        lua50::{Constant, Prototype},
        mission_vm::{Host, Status, Thread},
    };
    let mut vm = Bootstrap::new();
    vm.tables.push(Default::default());
    vm.table_set(0, Value::Number(1.0), Value::Number(7.0))
        .unwrap();
    let mut prototype = Prototype {
        upvalues: 0,
        parameters: 1,
        vararg: false,
        stack: 1,
        constants: vec![Constant::Number(1.0), Constant::Number(99.0)],
        children: vec![],
        code: vec![9 | (250 << 15) | (251 << 6), 63],
    };
    let mut thread =
        Thread::new("numeric rollback", prototype.clone(), &[Value::Table(0)]).unwrap();
    let mut host = Host::default();
    assert!(thread.resume(&mut vm, &mut host, 100).is_err());
    assert_eq!(thread.pc, 0);
    assert_eq!(
        vm.table_get(0, &Value::Number(1.0)).unwrap(),
        Value::Number(7.0)
    );
    prototype.code[1] = 27 | (1 << 15);
    let mut valid = Thread::new("numeric commit", prototype, &[Value::Table(0)]).unwrap();
    assert_eq!(
        valid.resume(&mut vm, &mut host, 100).unwrap(),
        Status::Complete(vec![])
    );
    assert_eq!(
        vm.table_get(0, &Value::Number(1.0)).unwrap(),
        Value::Number(99.0)
    );
}
#[test]
fn numeric_and_string_entries_share_limit() {
    let mut vm = Bootstrap::new();
    vm.tables.push(Default::default());
    for i in 0..9999 {
        vm.table_set(0, Value::Number(i as f64), Value::Boolean(true))
            .unwrap()
    }
    vm.table_set(0, Value::String("last".into()), Value::Boolean(true))
        .unwrap();
    assert!(
        vm.table_set(0, Value::Number(9999.0), Value::Boolean(true))
            .is_err()
    );
    vm.table_set(0, Value::Number(0.0), Value::Boolean(false))
        .unwrap();
    vm.table_set(0, Value::String("last".into()), Value::Nil)
        .unwrap();
    vm.table_set(0, Value::Number(9999.0), Value::Boolean(true))
        .unwrap();
}
#[test]
#[ignore = "requires original local frontend archive and XML"]
fn original_page_stack_executes_and_rolls_back() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("local/extracted/files/Data/CompoundFiles");
    let mut graph = ObjectGraph::default();
    for name in ["Frontend2_Level.xml", "Frontend2_Level_preload.xml"] {
        graph
            .merge(parse_objects(&std::fs::read_to_string(root.join(name)).unwrap()).unwrap())
            .unwrap()
    }
    let catalog = Catalog::load(&graph).unwrap();
    let archive = std::fs::read(root.join("Frontend2_Level.res")).unwrap();
    let mut stack = PageStack::from_archive(&archive).unwrap();
    let initial = stack.state().unwrap();
    assert_eq!(initial.top, None);
    assert!(!initial.page_done);
    let s = stack
        .apply(&catalog, Command::Push("Start".into()), 1000)
        .unwrap();
    assert_eq!(s.top.as_deref(), Some("Start"));
    assert!(s.page_done);
    for name in ["LoadCampaignData", "Save", "MainPre", "Main"] {
        let s = stack
            .apply(&catalog, Command::Push(name.into()), 1000)
            .unwrap();
        assert_eq!(s.top.as_deref(), Some(name));
        assert_eq!(s.next, s.top)
    }
    let before = stack.state().unwrap();
    assert!(
        stack
            .apply(&catalog, Command::Push("CutsceneIn".into()), 6)
            .is_err()
    );
    assert_eq!(stack.state().unwrap(), before);
    assert!(
        stack
            .apply(&catalog, Command::Push("NotARealPage".into()), 1000)
            .is_err()
    );
    assert_eq!(stack.state().unwrap(), before);
    assert_eq!(
        stack
            .apply(&catalog, Command::Pop, 1000)
            .unwrap()
            .top
            .as_deref(),
        Some("MainPre")
    );
    assert_eq!(
        stack
            .apply(&catalog, Command::Replace("Main".into()), 1000)
            .unwrap()
            .top
            .as_deref(),
        Some("Main")
    );
    assert_eq!(
        stack
            .apply(&catalog, Command::Reset("Start".into()), 1000)
            .unwrap()
            .top
            .as_deref(),
        Some("Start")
    );
    // The authored pop preserves the bottom page, rather than inventing an empty exit.
    assert_eq!(
        stack
            .apply(&catalog, Command::Pop, 1000)
            .unwrap()
            .top
            .as_deref(),
        Some("Start")
    );
    println!(
        "ORIGINAL_PAGE_STACK_PASS push_pop_replace_reset=true rollback=true lifecycle_executed=false"
    );
}
