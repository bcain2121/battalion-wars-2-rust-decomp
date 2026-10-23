use bw_rust::{ObjectGraph, frontend_runtime::Runtime, level_catalog, lua50::Value, parse_objects};

fn original() -> Runtime {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("local/extracted/files/Data");
    let mut graph = ObjectGraph::default();
    for file in ["Frontend2_Level.xml", "Frontend2_Level_preload.xml"] {
        graph
            .merge(
                parse_objects(
                    &std::fs::read_to_string(root.join("CompoundFiles").join(file)).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
    }
    let archive = std::fs::read(root.join("CompoundFiles/Frontend2_Level.res")).unwrap();
    let levels =
        level_catalog::Catalog::parse(&std::fs::read_to_string(root.join("levels.xml")).unwrap())
            .unwrap();
    // Explicit test rate. Not inferred native boot/video timing.
    Runtime::load(&graph, &archive, &levels, 60.0).unwrap()
}
#[test]
#[ignore = "requires local original frontend assets"]
fn original_global_constructor_and_authored_start_event() {
    let mut runtime = original();
    assert_eq!(
        runtime.global("NumFrontendLevels"),
        Some(&Value::Number(9.0))
    );
    runtime.construct_page("Start", 100000).unwrap();
    assert!(runtime.construct_page("Start", 100000).is_err());
    assert_eq!(runtime.stack_state().unwrap().top, None);
    runtime
        .call_page_method("Start", "Open", &[], 100000)
        .unwrap();
    // Authored Open followed by its event; still no rendered page or input driver.
    runtime
        .dispatch_page_callback("Start", "eGUIEvent_OnAB", &[], 100000)
        .unwrap();
    let state = runtime.stack_state().unwrap();
    assert_eq!(state.top.as_deref(), Some("LoadCampaignData"));
    assert_eq!(state.next.as_deref(), Some("LoadCampaignData"));
}
#[test]
#[ignore = "requires local original frontend assets"]
fn original_callback_and_constructor_failures_roll_back() {
    let mut runtime = original();
    assert!(runtime.construct_page("Start", 1).is_err());
    runtime.construct_page("Start", 100000).unwrap();
    let before = runtime.stack_state().unwrap();
    assert!(
        runtime
            .dispatch_page_callback("Start", "eGUIEvent_OnAB", &[], 1)
            .is_err()
    );
    assert_eq!(runtime.stack_state().unwrap(), before);
    assert!(
        runtime
            .dispatch_page_callback("Start", "invented", &[], 100000)
            .is_err()
    );
    // Start.Open opens revConCrosshair (upvalue 5), not revConCursor (upvalue 4).
    let sprite = bw_rust::ObjectId(2144913068);
    assert_eq!(runtime.sprite_state(sprite).unwrap(), None);
    let mut completed = false;
    for budget in 1..=100 {
        let mut trial = runtime.clone();
        match trial.call_page_method("Start", "Open", &[], budget) {
            Err(_) => {
                assert_eq!(trial.sprite_state(sprite).unwrap(), None);
                assert_eq!(trial.stack_state().unwrap(), before);
            }
            Ok(values) => {
                assert!(values.is_empty());
                assert!(trial.sprite_state(sprite).unwrap().is_some());
                completed = true;
                break;
            }
        }
    }
    assert!(
        completed,
        "Original Start.Open did not complete within bounded budget"
    );
    runtime
        .call_page_method("Start", "Open", &[], 100000)
        .unwrap();
    assert_eq!(
        runtime.sprite_state(sprite).unwrap(),
        Some(bw_rust::frontend_sprites::OpenState {
            field_5c: 0.0,
            byte_70: 0
        })
    );
    assert_eq!(runtime.stack_state().unwrap(), before);
}

#[test]
#[ignore = "requires local original frontend assets"]
fn original_globe_select_script_and_orientation_matrices() {
    let mut runtime = original();
    runtime.root("GlobeSelect", 100000).unwrap();
    for continent in 1..=5 {
        runtime
            .global_call("GlobeSetTarget", &[Value::Number(continent as f64)], 100000)
            .unwrap();
        runtime
            .global_call(
                "GlobeSelectTarget",
                &[Value::Number(continent as f64)],
                100000,
            )
            .unwrap();
        runtime
            .global_call("GlobeSelectTargetUpdate", &[], 100000)
            .unwrap();
        let out = runtime
            .global_call("GlobeSelectGetOrientation", &[], 100000)
            .unwrap();
        let [Value::Table(mtx_id)] = out.as_slice() else {
            panic!("Expected 4x4 matrix table from GlobeSelectGetOrientation");
        };
        let mtx = runtime.read_matrix4(*mtx_id).unwrap();
        assert!((mtx[3][3] - 1.0).abs() < 1.0e-5);
        // Verify rows 0..3 are unit vectors
        for (r, row) in mtx.iter().enumerate().take(3) {
            let len_sq = row[0] * row[0] + row[1] * row[1] + row[2] * row[2];
            assert!(
                (len_sq - 1.0).abs() < 1.0e-3,
                "Continent {continent} row {r} len_sq={len_sq}"
            );
        }
    }
}
