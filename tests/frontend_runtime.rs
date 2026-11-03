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

#[test]
#[ignore = "requires local original frontend assets"]
fn original_all_shared_scripts_and_all_82_pages_and_page_stack_flows() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("local/extracted/files/Data");
    let global_strings: Vec<String> = bw_rust::localization::Language::parse(
        &std::fs::read(root.join("Strings/globeng.str")).unwrap(),
    )
    .unwrap()
    .messages
    .into_iter()
    .map(|e| e.text)
    .collect();
    let frontend_strings: Vec<String> = bw_rust::localization::Language::parse(
        &std::fs::read(root.join("Strings/frontend2English.str")).unwrap(),
    )
    .unwrap()
    .messages
    .into_iter()
    .map(|e| e.text)
    .collect();

    let mut runtime = original();
    runtime.set_string_tables(global_strings, frontend_strings);
    runtime.init_shared_scripts(200_000).unwrap();

    let page_count = runtime.construct_all_pages(200_000).unwrap();
    assert_eq!(page_count, 82);

    // 1. Step from Logo -> StartPre -> Start
    runtime.reset_page_stack("Logo", 100_000).unwrap();
    let mut visited = Vec::new();
    for _ in 0..400 {
        if let Some(active) = runtime.step_page_stack(1.0 / 60.0, 200_000).unwrap() {
            if visited.last() != Some(&active) {
                visited.push(active.clone());
            }
            if active == "Start" {
                break;
            }
        }
    }
    assert!(
        visited.contains(&"Logo".to_string())
            && visited.contains(&"StartPre".to_string())
            && visited.contains(&"Start".to_string()),
        "Expected Logo -> StartPre -> Start, got {visited:?}"
    );

    // 2. Dispatch Start AB press -> LoadCampaignData -> Save
    runtime
        .dispatch_page_callback("Start", "eGUIEvent_OnAB", &[], 200_000)
        .unwrap();
    for _ in 0..30 {
        if let Some(active) = runtime.step_page_stack(1.0 / 60.0, 200_000).unwrap() {
            if visited.last() != Some(&active) {
                visited.push(active.clone());
            }
            if active == "Save" {
                break;
            }
        }
    }
    assert!(
        visited.contains(&"Save".to_string()),
        "Expected transition to Save, got {visited:?}"
    );

    // 3. Unlock solar campaign levels and step MapPre -> Map -> MapPost -> CampaignS -> CampaignSInfo -> CutsceneIn -> CampaignSLoading -> RunLevel
    if let Some(env) = runtime.env_mut() {
        env.levels_done = 5;
        env.levels_attempted_or_done = 5;
        env.sp_save_valid = true;
    }
    runtime.set_global("LevelsDone", Value::Number(5.0));
    runtime.set_global("SelectedCampaign", Value::Number(1.0));
    runtime.reset_page_stack("MapPre", 200_000).unwrap();
    for _ in 0..250 {
        if let Some(active) = runtime.step_page_stack(1.0 / 60.0, 200_000).unwrap() {
            if visited.last() != Some(&active) {
                visited.push(active.clone());
            }
            if active == "Map" {
                break;
            }
        }
    }
    assert!(
        visited.contains(&"Map".to_string()),
        "Expected MapPre -> Map, got {visited:?}"
    );

    // Select Campaign 1 (Solar Empire) on Map -> MapPost -> CampaignS
    runtime
        .call_page_method(
            "Map",
            "campaignButtonActivated",
            &[Value::Number(1.0)],
            200_000,
        )
        .unwrap();
    let mut bezier_dots_drawn = 0;
    for _ in 0..300 {
        if let Some(active) = runtime.step_page_stack(1.0 / 60.0, 200_000).unwrap() {
            if visited.last() != Some(&active) {
                visited.push(active.clone());
            }
            if active == "CampaignS" {
                bezier_dots_drawn = runtime.drawn_sprites().len();
                break;
            }
        }
    }
    assert!(
        visited.contains(&"MapPost".to_string()) && visited.contains(&"CampaignS".to_string()),
        "Expected Map -> MapPost -> CampaignS, got {visited:?}"
    );
    assert!(
        bezier_dots_drawn > 0,
        "Expected CampaignS.renderBeziers to draw Bezier link sprites"
    );

    // Hover Level 1 (missionTTOn) -> openLevelInfo -> CampaignSInfo -> playCampaignLevel -> CutsceneIn -> CampaignSLoading -> RunLevel
    runtime
        .call_page_method("CampaignS", "missionTTOn", &[Value::Number(1.0)], 200_000)
        .unwrap();
    runtime
        .call_page_method("CampaignS", "openLevelInfo", &[], 200_000)
        .unwrap();
    let active = runtime.step_page_stack(1.0 / 60.0, 200_000).unwrap();
    assert_eq!(active.as_deref(), Some("CampaignSInfo"));
    runtime
        .call_page_method("CampaignSInfo", "playCampaignLevel", &[], 200_000)
        .unwrap();
    for _ in 0..200 {
        let active = runtime.step_page_stack(1.0 / 60.0, 200_000).unwrap();
        if let Some(p) = &active
            && visited.last() != Some(p)
        {
            visited.push(p.clone());
        }
        if runtime.env().and_then(|e| e.launched_level).is_some() {
            break;
        }
    }
    assert!(
        visited.contains(&"CutsceneIn".to_string())
            && visited.contains(&"CampaignSLoading".to_string()),
        "Expected CutsceneIn -> CampaignSLoading, got {visited:?}"
    );
    // Campaign Mission 1 is index 10 in levels.xml (1 + NumFrontendLevels where NumFrontendLevels == 9)
    assert_eq!(runtime.env().and_then(|e| e.launched_level), Some(10));

    // 4. Test Multiplayer page-stack flow (Gametype -> Levels -> ChooseArmy -> MPLoading)
    runtime.set_global("FrontendDone", Value::Boolean(false));
    runtime.reset_page_stack("Gametype", 200_000).unwrap();
    let active = runtime.step_page_stack(1.0 / 60.0, 200_000).unwrap();
    assert_eq!(active.as_deref(), Some("Gametype"));
    runtime
        .call_page_method("Gametype", "gotoLevels", &[Value::Number(2.0)], 200_000)
        .unwrap();
    for _ in 0..120 {
        if let Some(p) = runtime.step_page_stack(1.0 / 60.0, 200_000).unwrap() {
            if visited.last() != Some(&p) {
                visited.push(p.clone());
            }
            if p == "Levels" {
                break;
            }
        }
    }
    assert!(
        visited.contains(&"Levels".to_string()),
        "Expected Gametype -> Levels, got {visited:?}"
    );
    runtime
        .call_page_method("Levels", "selectLevel", &[Value::Number(1.0)], 200_000)
        .unwrap();
    let chosen_name_id = runtime
        .call_page_method("Levels", "getChosenLevelName", &[], 200_000)
        .unwrap();
    let chosen_name = runtime
        .global_call("GetString", &chosen_name_id, 10_000)
        .unwrap();
    assert_eq!(chosen_name, vec![Value::String("STORM THE BEACHES".into())]);

    // 5. Test Victory and Defeat pages Open/Update/Close cleanly
    for outcome_page in ["Victory", "Defeat"] {
        runtime.reset_page_stack(outcome_page, 200_000).unwrap();
        let active = runtime.step_page_stack(1.0 / 60.0, 200_000).unwrap();
        assert_eq!(active.as_deref(), Some(outcome_page));
    }

    // 6. Verify GuiScene canvas evaluation across authored pages
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
    let gui_scene = bw_rust::frontend_gui::GuiScene::from_graph(&graph);
    assert!(!gui_scene.textures.is_empty());
    assert!(!gui_scene.sprite_bases.is_empty());
    for page_name in [
        "Start",
        "Save",
        "Map",
        "CampaignS",
        "Gametype",
        "Levels",
        "Victory",
        "Defeat",
    ] {
        runtime.reset_page_stack(page_name, 200_000).unwrap();
        let active = runtime.step_page_stack(1.0 / 60.0, 200_000).unwrap();
        assert_eq!(active.as_deref(), Some(page_name));
        let quads = gui_scene
            .evaluate_active_page_canvas(&graph, &mut runtime)
            .unwrap();
        assert!(
            !quads.is_empty(),
            "Expected non-empty GuiCanvasQuad list for page {page_name}"
        );
    }
}
