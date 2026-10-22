//! Recover authored startup bindings, execute every initializer, report real first-resume blockers.
fn audit_source_node_kinds(archive: &[u8]) -> bw_rust::Result<serde_json::Value> {
    use bw_rust::model_node_kind::{Catalog, SpatialDispatch};
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("local/extracted/sys/main.dol");
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let catalog = Catalog::from_rbwe01_dol(&bytes)?;
    let model = bw_rust::model::from_archive(archive, "SFXdummyRoot")?;
    let mut nodes = Vec::new();
    let mut dispatched = 0;
    for (index, node) in model.nodes.iter().enumerate() {
        let resolved = catalog.resolve(&node.name, node.authored_kind)?;
        let dispatch = resolved.spatial_dispatch();
        dispatched += usize::from(dispatch != SpatialDispatch::Ignored);
        nodes.push(serde_json::json!({"node":index,"name":node.name,"authored_kind":node.authored_kind,"resolved_kind":resolved.kind,"flag":resolved.flag,"source":format!("{:?}",resolved.source),"dispatch":format!("{dispatch:?}")}));
    }
    Ok(
        serde_json::json!({"available":true,"model":model.name,"static_catalog_rows":catalog.static_tag_count(),"nodes":nodes,"source_volume_dispatch_nodes":dispatched,"source_model_only":true,"native_spatial_cleanup_complete":false}),
    )
}
fn audit_art_spatial_profile(graph: &ObjectGraph) -> bw_rust::Result<serde_json::Value> {
    use bw_rust::mission_spatial_removal::{
        SpatialProfile, authored_instance_flags, destroyable_virtual_1a4,
    };
    let base = graph
        .objects
        .get(&bw_rust::ObjectId(250009278))
        .ok_or("Missing Art base")?;
    let profile = SpatialProfile::from_object(base)?;
    let predicate = profile.predicate_for_null_dead_model(graph)?;
    let instances = [250009261,250010226,250010165].into_iter().map(|id| {
        let object = graph.objects.get(&bw_rust::ObjectId(id)).ok_or("Missing Art instance")?;
        let flags = authored_instance_flags(object)?;
        Ok(serde_json::json!({"object":id,"authored_instance_flags":flags,"pre_query_update_at_authored_state":profile.pre_query_update_required(Some(flags))?}))
    }).collect::<bw_rust::Result<Vec<_>>>()?;
    Ok(
        serde_json::json!({"base":250009278u64,"collide_type":profile.collide_type,"dead_model":profile.dead_model,"dead_model_decay_time":profile.dead_model_decay_time,"explosion_flags":predicate.explosion_flags,"authored_instances":instances,"live_flags_12c_configured":false,"pre_query_gate_error":profile.pre_query_update_required(None).err(),"dead_predicate_with_model_bit":destroyable_virtual_1a4(Some(0x00400000),0x00100000,Some(predicate))?,"source_profile_only":true,"kill_backend_installed":false}),
    )
}
fn audit_vehicles(
    graph: &ObjectGraph,
    _archive: &[u8],
    entry: &bw_rust::level_catalog::Entry,
) -> serde_json::Value {
    use bw_rust::vehicle_profile::Bank;
    let bank = Bank::load(graph, entry.physics).expect("Level-selected ground vehicle profiles");
    let names = bank
        .units
        .values()
        .map(|b| b.model.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    #[cfg(feature = "viewer")]
    let geometry = Some(
        names
            .iter()
            .map(|name| {
                let model =
                    bw_rust::model::from_archive(_archive, name).expect("Ground vehicle model");
                let nodes =
                    bw_rust::model_geometry::physics_nodes(&model).expect("Authored physics nodes");
                let wheel_slots=bw_rust::vehicle_wheels::slots(&model).expect("Authored wheel/interpolation slots");
                let box_groups=bw_rust::vehicle_boxes::groups(&model).expect("Native bind-pose box groups");
                serde_json::json!({"model":name,"header_sphere":model.header_sphere,"physics_nodes":nodes,"wheel_slots":wheel_slots,"box_groups":box_groups,"auxiliary_idlers_configured":false})
            })
            .collect::<Vec<_>>(),
    );
    #[cfg(not(feature = "viewer"))]
    let geometry: Option<Vec<serde_json::Value>> = None;
    serde_json::json!({"units":bank.units.len(),"profiles":bank.profiles.values().map(|p|p.as_ref()).collect::<Vec<_>>(),"models":names,"geometry":geometry,"level_entry":entry,"engine_override":"original_levels_xml","dynamics_configured":false})
}
use bw_rust::{
    ObjectGraph,
    mission_program::{Program, Scope},
    parse_objects,
};
fn main() {
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    let simulation_ticks = if args.len() >= 2 && args[args.len() - 2] == "--ticks" {
        let ticks = args
            .pop()
            .unwrap()
            .parse::<usize>()
            .expect("Integer tick count");
        args.pop();
        assert!(ticks <= 60000, "Tick limit exceeded");
        ticks
    } else {
        0
    };
    let unit_scripts = args.last().is_some_and(|arg| arg == "--unit-scripts");
    if unit_scripts {
        args.pop();
    }
    let allow_cosmetic_gaps = args
        .last()
        .is_some_and(|arg| arg == "--allow-cosmetic-gaps");
    if allow_cosmetic_gaps {
        args.pop();
    }
    let rewrite = args.last().is_some_and(|arg| arg == "--rust-rewrite");
    if rewrite {
        args.pop();
    }
    let (configured, fresh_attempt) = match args.as_slice() {
        [] => (false, false),
        [flag] if matches!(flag.as_str(), "--authored-startup" | "--shared-startup") => {
            (true, false)
        }
        [flag, history]
            if matches!(flag.as_str(), "--authored-startup" | "--shared-startup")
                && history == "--fresh-attempt" =>
        {
            (true, true)
        }
        _ => panic!(
            "Usage: bw-mission-audit [--authored-startup|--shared-startup [--fresh-attempt] [--rust-rewrite] [--allow-cosmetic-gaps] [--unit-scripts] [--ticks N]]"
        ),
    };
    assert!(
        !rewrite || configured,
        "--rust-rewrite requires a startup audit mode"
    );
    assert!(
        !allow_cosmetic_gaps || rewrite,
        "Cosmetic policy requires --rust-rewrite"
    );
    let shared = args.first().is_some_and(|flag| flag == "--shared-startup");
    assert!(
        !unit_scripts || (shared && rewrite),
        "Unit scripts require shared rewrite mode"
    );
    assert!(
        simulation_ticks == 0 || (shared && configured),
        "Simulation ticks require configured shared mode"
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("local/extracted/files/Data/CompoundFiles");
    let mut graph = ObjectGraph::default();
    for name in ["SP_0.1_Level.xml", "SP_0.1_Level_preload.xml"] {
        graph
            .merge(
                parse_objects(
                    &std::fs::read_to_string(root.join(name)).expect("Local original XML"),
                )
                .expect("Parse original XML"),
            )
            .expect("Merge");
    }
    let archive = std::fs::read(root.join("SP_0.1_Level.res")).expect("Local original archive");
    let mut program = Program::load(&graph, &archive).expect("Authored script bindings");
    if rewrite {
        program
            .host
            .world
            .configure_rewrite(&graph)
            .expect("Rust-owned runtime");
        if allow_cosmetic_gaps {
            program
                .host
                .world
                .rewrite
                .as_mut()
                .unwrap()
                .allow_cosmetic_gaps();
        }
    }
    if fresh_attempt {
        program.host.world.mission_history =
            Some(bw_rust::mission_world::MissionHistory::fresh_attempt());
    }
    if configured {
        program.host.world.controller = Some(bw_rust::mission_controls::Controller::fresh_player());
        program.host.world.input = Some(bw_rust::mission_input::Input::single_player());
        program.host.world.hud = Some(bw_rust::mission_hud::Hud::default());
        program.host.world.waypoints = Some(std::sync::Arc::new(
            bw_rust::mission_waypoints::Bank::load(&graph).expect("Authored unit waypoints"),
        ));
        program
            .host
            .world
            .configure_authored_units(&graph)
            .expect("Authored unit states");
        program
            .host
            .world
            .configure_authored_player(&graph)
            .expect("Authored player-camera target");
        program
            .host
            .world
            .configure_fresh_player_lifecycle()
            .expect("Fresh player lifecycle");
        program.host.world.cameras = Some(bw_rust::mission_camera::Rig::new(std::sync::Arc::new(
            bw_rust::mission_camera::Bank::load(&graph).expect("Authored cameras"),
        )));
        let bank =
            bw_rust::mission_music::Bank::load(root.parent().unwrap().parent().unwrap(), &graph)
                .expect("Original stereo music");
        program.host.world.music = Some(bw_rust::mission_music::Transport::new(
            std::sync::Arc::new(bank),
        ));
    }
    if configured {
        let files = root.parent().unwrap().parent().unwrap();
        let language = bw_rust::localization::Language::parse(
            &std::fs::read(files.join("Data/Strings/SP_0.1English.str")).expect("Original STR"),
        )
        .expect("STR parse");
        program.host.world.messages =
            Some(bw_rust::mission_messages::Queue::new(std::sync::Arc::new(
                bw_rust::mission_audio::Bank::load(files, &language).expect("Original voices"),
            )));
        program
            .host
            .world
            .messages
            .as_mut()
            .unwrap()
            .configure_voiced_playback(
                std::sync::Arc::new(language),
                program.host.known_objects.clone(),
            );
    }
    let vehicle_assets = configured.then(|| {
        let catalog = bw_rust::level_catalog::Catalog::parse(
            &std::fs::read_to_string(root.parent().unwrap().join("levels.xml"))
                .expect("Original level catalog"),
        )
        .expect("Parse original level catalog");
        audit_vehicles(
            &graph,
            &archive,
            catalog.unique("SP_0.1").expect("Unique first mission"),
        )
    });
    let rewrite_scene: Option<serde_json::Value> = if rewrite {
        #[cfg(feature = "viewer")]
        {
            let terrain = bw_rust::terrain::decode(
                &std::fs::read(root.join("SP_0.1.out")).expect("Original terrain"),
            )
            .expect("Terrain decode");
            let catalog = bw_rust::level_catalog::Catalog::parse(
                &std::fs::read_to_string(root.parent().unwrap().join("levels.xml")).unwrap(),
            )
            .unwrap();
            let report = bw_rust::mission_rewrite_scene::configure(
                &graph,
                &archive,
                &terrain,
                catalog.unique("SP_0.1").unwrap().physics,
                &mut program.host.world,
            )
            .expect("Rust-owned collision scene");
            Some(serde_json::to_value(report).unwrap())
        }
        #[cfg(not(feature = "viewer"))]
        {
            panic!("--rust-rewrite requires --features viewer");
        }
    } else {
        None
    };
    let initial_player = program.host.world.player;
    let initialized = program.initialize().expect("All authored initializers");
    let mut simulation = None;
    let shared_tick = shared.then(|| {
        let mut scheduler = if unit_scripts { program.mission_scheduler() } else { program.global_scheduler() }.expect("Authored scheduler");
        let idle = || bw_rust::mission_input::ITEMS.iter().map(|name| ((*name).to_string(), bw_rust::mission_input::Button::default())).collect();
        let result = if configured { scheduler.tick_with_input(&mut program.vm, &mut program.host, 0.0, idle(), 100_000) } else { scheduler.tick(&mut program.vm, &mut program.host, 0.0, 100_000) };
        if result.is_ok() && simulation_ticks > 0 {
            let mut completed = 0;
            let mut error = None;
            for _ in 0..simulation_ticks {
                match scheduler.tick_with_input(&mut program.vm, &mut program.host, 1.0/60.0, idle(), 100_000) {
                    Ok(_) => completed += 1,
                    Err(e) => { error = Some(e); break; }
                }
            }
            simulation = Some(serde_json::json!({"requested":simulation_ticks,"completed":completed,"error":error}));
        }
        serde_json::json!({"committed":result.is_ok(),"error":result.err(),"threads":scheduler.threads.len(),"pause_eligible":scheduler.update_when_paused,"unit_scripts_attached":unit_scripts,"player_frozen":program.host.world.frozen,"chase_target":program.host.world.cameras.as_ref().and_then(|c|c.chase_target()),"intro_script_complete":program.bindings.iter().find(|b|b.script=="Gambit_Part_1_Cutscene_INTRO").and_then(|b|scheduler.threads.get(&b.entity.0)).is_some_and(|t|matches!(t.status,bw_rust::mission_vm::Status::Complete(_))),"game_time":program.host.time,"no_pause_time":program.host.no_pause_time,"frame":program.host.frame})
    });
    let probes = if shared {
        Vec::new()
    } else {
        program.probe_globals().expect("Global probes")
    };
    let scope = if shared && unit_scripts {
        "Original global/unit mission simulation, not full mission playability"
    } else if shared {
        "Shared authored global startup tick, not a complete mission"
    } else {
        "Authored startup discovery and isolated capability probes, not a running mission"
    };
    println!("{}",serde_json::to_string_pretty(&serde_json::json!({"scope":scope,"simulation":simulation,"cosmetic_warnings":program.host.world.rewrite.as_ref().map(|r|r.warnings()),"rejected_commands":program.host.world.rewrite.as_ref().map(|r|&r.rejected_commands),"placement_corrections":program.host.world.rewrite.as_ref().map(|r|&r.placement_corrections),"allow_cosmetic_gaps":allow_cosmetic_gaps,"rewrite_scene":rewrite_scene,"rust_owned_rewrite":rewrite,"collision_scene_loaded":program.host.world.collision.is_some(),"shared_tick":shared_tick,"initializers_complete":initialized,"global_scripts":program.bindings.iter().filter(|b|b.scope==Scope::Global).count(),"unit_scripts":program.bindings.iter().filter(|b|b.scope==Scope::Unit).count(),"captured_cells":program.vm.upvalues.len(),"closures":program.vm.closures.len(),"bindings":program.bindings,"probes":probes,"full_mission":false,"ordering_policy":"Stable object ID, original ordering unverified","activation_policy":if configured{"Authored AI state and player-camera target; no inferred squad ownership"}else{"Not inferred from placement"},"authored_startup_support":configured,"explicit_unsaved_fresh_attempt":fresh_attempt,"initial_player":initial_player,"current_player":program.host.world.player,"fresh_player_lifecycle":program.host.world.player_lifecycle.is_some(),"squad_roster_recovered":false,"retention_flags_configured":program.host.world.despawn_retention.len(),"native_list_filters_complete":false,"random_cursor":program.host.world.random.as_ref().map(|r|r.cursor()),"hardware_audio":false,"ground_vehicle_assets":vehicle_assets,"art_spatial_profile":audit_art_spatial_profile(&graph).unwrap_or_else(|error|serde_json::json!({"error":error})),"source_node_kinds":audit_source_node_kinds(&archive).unwrap_or_else(|error|serde_json::json!({"available":false,"error":error}))})).unwrap());
}
