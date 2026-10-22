use bw_rust::{
    lua50::{Bootstrap, Constant, Prototype, Value},
    mission_vm::{Host, Status, Thread},
};
fn abc(op: u32, a: u32, b: u32, c: u32) -> u32 {
    op | (a << 24) | (b << 15) | (c << 6)
}
fn abx(op: u32, a: u32, bx: u32) -> u32 {
    op | (a << 24) | (bx << 6)
}
fn prototype(
    stack: u8,
    code: Vec<u32>,
    constants: Vec<Constant>,
    children: Vec<Prototype>,
) -> Prototype {
    Prototype {
        upvalues: 0,
        parameters: 0,
        vararg: false,
        stack,
        code,
        constants,
        children,
    }
}
fn run(p: Prototype, vm: &mut Bootstrap) -> Result<Status, String> {
    Thread::new("array fixture", p, &[])?.resume(vm, &mut Host::default(), 1000)
}
#[test]
fn fixed_array_flush_boundary() {
    let mut vm = Bootstrap::new();
    let mut code = vec![abc(10, 0, 0, 0)];
    for i in 1..=32 {
        code.push(abx(1, i, i - 1))
    }
    code.extend([
        abx(31, 0, 31),
        abx(1, 1, 32),
        abx(31, 0, 32),
        abc(27, 0, 2, 0),
    ]);
    let constants = (1..=33).map(|i| Constant::Number(i as f64)).collect();
    assert_eq!(
        run(prototype(33, code, constants, vec![]), &mut vm).unwrap(),
        Status::Complete(vec![Value::Table(0)])
    );
    for i in 1..=33 {
        assert_eq!(
            vm.table_get(0, &Value::Number(i as f64)).unwrap(),
            Value::Number(i as f64)
        );
    }
}
#[test]
fn open_array_uses_actual_call_results_and_consumes_top() {
    let child = prototype(
        2,
        vec![abx(1, 0, 0), abx(1, 1, 1), abc(27, 0, 3, 0)],
        vec![Constant::Number(10.0), Constant::Number(20.0)],
        vec![],
    );
    let code = vec![
        abc(10, 0, 0, 0),
        abx(34, 1, 0),
        abc(25, 1, 1, 0),
        abx(32, 0, 32),
        abc(27, 0, 2, 0),
    ];
    let p = prototype(3, code, vec![], vec![child]);
    let mut vm = Bootstrap::new();
    run(p.clone(), &mut vm).unwrap();
    assert_eq!(
        vm.table_get(0, &Value::Number(33.0)).unwrap(),
        Value::Number(10.0)
    );
    assert_eq!(
        vm.table_get(0, &Value::Number(34.0)).unwrap(),
        Value::Number(20.0)
    );
    let mut invalid = p;
    invalid.code.insert(4, abx(32, 0, 0));
    let before = vm.numeric_tables.clone();
    assert!(
        run(invalid, &mut vm)
            .unwrap_err()
            .contains("without result top")
    );
    assert_eq!(vm.numeric_tables, before);
    assert_eq!(vm.tables.len(), 1);
}
#[test]
fn empty_open_array_and_invalid_sources() {
    let child = prototype(1, vec![abc(27, 0, 1, 0)], vec![], vec![]);
    let p = prototype(
        2,
        vec![
            abc(10, 0, 0, 0),
            abx(34, 1, 0),
            abc(25, 1, 1, 0),
            abx(32, 0, 0),
            abc(27, 0, 2, 0),
        ],
        vec![],
        vec![child],
    );
    let mut vm = Bootstrap::new();
    run(p, &mut vm).unwrap();
    assert!(vm.numeric_tables.is_empty());
    for code in [
        vec![abx(31, 0, 0)],
        vec![abc(10, 0, 0, 0), abx(31, 0, 31)],
        vec![abc(10, 0, 0, 0), abx(32, 0, 0)],
    ] {
        let count = vm.tables.len();
        assert!(run(prototype(1, code, vec![], vec![]), &mut vm).is_err());
        assert_eq!(vm.tables.len(), count)
    }
}
#[test]
fn array_capacity_failure_rolls_back_partial_writes() {
    let mut vm = Bootstrap::new();
    vm.tables.push(Default::default());
    for i in 0..9999 {
        vm.table_set(0, Value::Number(i as f64), Value::Boolean(true))
            .unwrap();
    }
    let before = vm.numeric_tables.clone();
    let mut p = prototype(
        3,
        vec![
            abx(1, 1, 0),
            abx(1, 2, 0),
            abx(31, 0, 10017),
            abc(27, 0, 1, 0),
        ],
        vec![Constant::Number(7.0)],
        vec![],
    );
    p.parameters = 1;
    let mut thread = Thread::new("array limit", p, &[Value::Table(0)]).unwrap();
    assert!(
        thread
            .resume(&mut vm, &mut Host::default(), 1000)
            .unwrap_err()
            .contains("Table entry limit")
    );
    assert_eq!(vm.numeric_tables, before);
    assert_eq!(thread.pc, 0);
}
#[test]
#[ignore = "requires original local frontend archive/XML"]
fn original_cursor_constructor_array() {
    use bw_rust::{
        ObjectGraph,
        archive::{chunks, named_payload},
        parse_objects,
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("local/extracted/files/Data/CompoundFiles");
    let mut graph = ObjectGraph::default();
    for file in ["Frontend2_Level.xml", "Frontend2_Level_preload.xml"] {
        graph
            .merge(parse_objects(&std::fs::read_to_string(root.join(file)).unwrap()).unwrap())
            .unwrap()
    }
    let archive = std::fs::read(root.join("Frontend2_Level.res")).unwrap();
    let mut scripts = std::collections::BTreeMap::new();
    for chunk in chunks(&archive).unwrap() {
        if chunk.tag == *b"PRCS" {
            let (name, bytes) = named_payload(chunk.payload).unwrap();
            scripts.insert(name.to_owned(), bw_rust::lua50::parse(bytes).unwrap());
        }
    }
    let mut vm = Bootstrap::new();
    vm.execute(
        "EntityInitialise",
        &scripts["EntityInitialise"],
        &graph,
        100000,
    )
    .unwrap();
    // Declaration only: constructor stores this engine callback; the test never executes it.
    vm.globals.insert(
        "GetCursorRoll".into(),
        Value::Native("GetCursorRoll".into()),
    );
    let Value::Table(sprites) = vm.globals["SpriteID"] else {
        panic!("missing sprite table")
    };
    for (index, sprite, y) in [
        (14, "FE_RevCon_Cursor", 20.0),
        (18, "FE_RevCon_Cursor_crosshair", 0.0),
    ] {
        // Isolated unmodified Global child, NOT execution of the Global root or Start lifecycle.
        let p = scripts["Global"].children[index].clone();
        assert_eq!(p.upvalues, 0);
        assert!(p.constants.contains(&Constant::String(sprite.into())));
        let Status::Complete(result) = run(p, &mut vm).unwrap() else {
            panic!("cursor yielded")
        };
        let [Value::Table(cursor)] = result.as_slice() else {
            panic!("cursor result")
        };
        assert_eq!(
            vm.table_get(*cursor, &Value::Number(1.0)).unwrap(),
            vm.table_get(sprites, &Value::String(sprite.into()))
                .unwrap()
        );
        let Value::Table(offset) = vm.table_get(*cursor, &Value::Number(2.0)).unwrap() else {
            panic!("offset table")
        };
        assert_eq!(
            vm.table_get(offset, &Value::Number(1.0)).unwrap(),
            Value::Number(0.0)
        );
        assert_eq!(
            vm.table_get(offset, &Value::Number(2.0)).unwrap(),
            Value::Number(y)
        );
        assert_eq!(
            vm.table_get(*cursor, &Value::Number(3.0)).unwrap(),
            Value::Native("GetCursorRoll".into())
        );
    }
    println!(
        "ORIGINAL_CURSOR_ARRAY_PASS constructors=2 lifecycle=false cursor_roll_executed=false"
    );
}
