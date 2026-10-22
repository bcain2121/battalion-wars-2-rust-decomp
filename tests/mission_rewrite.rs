use bw_rust::{
    ObjectGraph, ObjectId, mission_destruction::Handle, mission_world::World, parse_objects,
    unit_collision::Scene,
};
use std::{num::NonZeroU64, sync::Arc};
#[test]
#[ignore = "requires private original assets"]
fn cosmetic_policy_does_not_bypass_damage_or_leak_failed_warnings() {
    let mut data = graph();
    let id = ObjectId(550037817);
    let mut strict = World::from_graph(&data).unwrap();
    strict.configure_rewrite(&data).unwrap();
    assert!(
        strict
            .kill_rewrite(id, [0.; 3])
            .unwrap_err()
            .contains("mBoomSoundBase")
    );
    let mut playable = strict.clone();
    playable.rewrite.as_mut().unwrap().allow_cosmetic_gaps();
    playable.kill_rewrite(id, [0.; 3]).unwrap();
    assert_eq!(playable.entity(id).unwrap().health, 0.0);
    assert_eq!(playable.rewrite.as_ref().unwrap().warnings().len(), 3);
    assert!(strict.entity(id).unwrap().health > 0.0);
    assert!(strict.rewrite.as_ref().unwrap().warnings().is_empty());
    data.objects
        .get_mut(&ObjectId(550035525))
        .unwrap()
        .fields
        .iter_mut()
        .find(|f| {
            f.attributes
                .get("name")
                .is_some_and(|v| v == "mDamageAmount")
        })
        .unwrap()
        .items[0] = "1".into();
    let mut damaging = World::from_graph(&data).unwrap();
    damaging.configure_rewrite(&data).unwrap();
    damaging.rewrite.as_mut().unwrap().allow_cosmetic_gaps();
    assert!(
        damaging
            .kill_rewrite(id, [0.; 3])
            .unwrap_err()
            .contains("mDamageAmount")
    );
    assert!(damaging.entity(id).unwrap().health > 0.0);
    assert!(damaging.rewrite.as_ref().unwrap().warnings().is_empty());
}
#[test]
fn synthetic_null_explosion_retirement_is_explicit_and_idempotent() {
    let mut graph = ObjectGraph::default();
    graph.merge(parse_objects(r#"<Instances>
      <Object type="cDestroyableObject" id="1"><Pointer name="mBase"><Item>2</Item></Pointer></Object>
      <Object type="sDestroyBase" id="2">
        <Attribute name="MaxHealth"><Item>5</Item></Attribute>
        <Resource name="DeadModel"><Item>0</Item></Resource>
        <Pointer name="ExplodeType"><Item>0</Item></Pointer>
      </Object>
    </Instances>"#).unwrap()).unwrap();
    let mut world = World::from_graph(&graph).unwrap();
    let id = ObjectId(1);
    assert!(world.kill_rewrite(id, [0.; 3]).is_err());
    world.configure_rewrite(&graph).unwrap();
    assert!(world.configure_rewrite(&graph).is_err());
    world.kill_rewrite(id, [0.; 3]).unwrap();
    world.kill_rewrite(id, [0.; 3]).unwrap();
    assert_eq!(world.entity(id).unwrap().health, 0.0);
    assert_eq!(world.rewrite.as_ref().unwrap().retired.len(), 1);
    assert!(world.rewrite.as_ref().unwrap().effects().is_empty());
    assert!(world.kill_rewrite(ObjectId(999), [0.; 3]).is_err());
}
#[test]
fn capture_presence_changes_collision_and_resets_interactions_transactionally() {
    let mut graph = ObjectGraph::default();
    graph.merge(parse_objects(r#"<Instances>
      <Object type="cCapturePoint" id="1"><Pointer name="mBase"><Item>2</Item></Pointer></Object>
      <Object type="cCapturePointBase" id="2"><Attribute name="mMaxHealth"><Item>10</Item></Attribute></Object>
    </Instances>"#).unwrap()).unwrap();
    let mut world = World::from_graph(&graph).unwrap();
    world.configure_rewrite(&graph).unwrap();
    let id = ObjectId(1);
    let mut scene = Scene::default();
    scene
        .register_object_meshes(
            id,
            Handle(NonZeroU64::new(1).unwrap()),
            vec![(0, vec![[[0., 0., -2.], [0., 3., -2.], [0., 0., 2.]]])],
        )
        .unwrap();
    world.collision = Some(Arc::new(scene));
    world.set_rewrite_presence(id, true, false).unwrap();
    world.capture_sessions.get_mut(&id).unwrap().progress = 0.5;
    world
        .capture_sessions
        .get_mut(&id)
        .unwrap()
        .contenders
        .insert(ObjectId(99));
    let original = world.collision.clone().unwrap();
    let a = [-2., 1., 0.];
    let b = [2., 1., 0.];
    assert!(!original.line_clear(a, b).unwrap());
    world.set_rewrite_presence(id, false, false).unwrap();
    assert!(world.collision.as_ref().unwrap().line_clear(a, b).unwrap());
    assert!(!original.line_clear(a, b).unwrap());
    assert_eq!(world.capture_sessions[&id].progress, 0.0);
    assert!(world.capture_sessions[&id].contenders.is_empty());
    assert_eq!(world.entity(id).unwrap().health, 10.0);
    world.set_rewrite_presence(id, true, false).unwrap();
    assert!(!world.collision.as_ref().unwrap().line_clear(a, b).unwrap());
    world.collision = Some(Arc::new(Scene::default()));
    world.capture_sessions.get_mut(&id).unwrap().progress = 0.5;
    assert!(world.set_rewrite_presence(id, false, false).is_err());
    assert_eq!(world.entity(id).unwrap().active, Some(true));
    assert_eq!(world.capture_sessions[&id].progress, 0.5);
}
#[test]
fn scripted_lightning_resolves_resource_and_expires_without_fake_success() {
    let mut graph = ObjectGraph::default();
    graph.merge(parse_objects(r#"<Instances>
      <Object type="cScriptedEffectBase" id="1">
        <Pointer name="mSoundEffect"><Item>0</Item></Pointer>
        <Resource name="mTequilaEffect"><Item>2</Item></Resource>
        <Enum name="meScriptedEffectAnchorType"><Item>eEFFECT_ANCHOR_TO_CAMERA_POS_DIR</Item></Enum>
        <Attribute name="mTequilaTimeOut"><Item>0.1</Item></Attribute>
      </Object>
      <Object type="cTequilaEffectResource" id="2"><Attribute name="mName"><Item>1010_X Lightning</Item></Attribute></Object>
    </Instances>"#).unwrap()).unwrap();
    let mut runtime = bw_rust::mission_rewrite::Runtime::new(&graph);
    assert!(runtime.play_effect(ObjectId(999), 3, [0.; 3]).is_err());
    assert!(runtime.play_effect(ObjectId(1), 4, [0.; 3]).is_err());
    assert!(runtime.lightning().is_empty());
    runtime.play_effect(ObjectId(1), 3, [0.; 3]).unwrap();
    assert_eq!(runtime.lightning()[0].frame().triangles.len(), 12);
    runtime.advance(0.1).unwrap();
    assert!(runtime.lightning().is_empty());
}
fn graph() -> ObjectGraph {
    let mut graph = ObjectGraph::default();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("local/extracted/files/Data/CompoundFiles");
    for name in ["SP_0.1_Level.xml", "SP_0.1_Level_preload.xml"] {
        graph
            .merge(parse_objects(&std::fs::read_to_string(root.join(name)).unwrap()).unwrap())
            .unwrap();
    }
    graph
}
#[test]
#[ignore = "requires original local XML and script archive"]
fn original_shared_tick_executes_kills_then_rolls_back_at_next_missing_capability() {
    let graph = graph();
    let archive = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("local/extracted/files/Data/CompoundFiles/SP_0.1_Level.res"),
    )
    .unwrap();
    let mut program = bw_rust::mission_program::Program::load(&graph, &archive).unwrap();
    program.host.world.configure_rewrite(&graph).unwrap();
    program.host.world.configure_authored_units(&graph).unwrap();
    program
        .host
        .world
        .configure_authored_player(&graph)
        .unwrap();
    program.initialize().unwrap();
    let mut scheduler = program.global_scheduler().unwrap();
    let error = scheduler
        .tick(&mut program.vm, &mut program.host, 0.0, 100_000)
        .unwrap_err();
    assert!(
        error.contains("SetMusicVolume: Music PCM backend not configured"),
        "{error}"
    );
    assert!(
        program
            .host
            .world
            .rewrite
            .as_ref()
            .unwrap()
            .retired
            .is_empty()
    );
    assert!(
        program
            .host
            .world
            .rewrite
            .as_ref()
            .unwrap()
            .effects()
            .is_empty()
    );
    assert_eq!(
        program
            .host
            .world
            .entity(ObjectId(250009261))
            .unwrap()
            .health,
        10.0
    );
    assert_eq!(program.host.frame, 0);
    assert!(
        program
            .host
            .world
            .rewrite
            .as_ref()
            .unwrap()
            .lightning()
            .is_empty()
    );
}
#[test]
#[ignore = "requires original local sound bank"]
fn actual_staff_power_sample_decodes_to_non_silent_pcm() {
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("local/extracted/files/Data/CompoundFiles/SP_0.1_Level.res"),
    )
    .unwrap();
    let bank = bw_rust::sound_samples::Samples::from_archive(&bytes).unwrap();
    assert_eq!(bank.len(), 365);
    let audio = bank.decode("DE_StaffPower").unwrap();
    assert_eq!(audio.sample_rate, 22050);
    assert_eq!(audio.pcm.len(), 59648);
    assert!(!audio.looping);
    assert!(audio.pcm.iter().any(|v| *v != 0));
    assert!(audio.wav().starts_with(b"RIFF"));
    let mut voice = bw_rust::mission_effect_sound::Voice::from_profile(
        &graph(),
        &bank,
        ObjectId(550039916),
        [0.; 3],
        0.0,
    )
    .unwrap();
    let near = voice.render([0.; 3], 48000, 48000).unwrap();
    assert!(near.iter().any(|frame| frame[0] != 0.0));
    assert!(
        voice
            .render([1000., 0., 0.], 100, 48000)
            .unwrap()
            .iter()
            .all(|frame| *frame == [0.; 2])
    );
    let before = voice.render([0.; 3], 100, 48000).unwrap();
    assert!(voice.advance(f64::NAN).is_err());
    assert_eq!(voice.render([0.; 3], 100, 48000).unwrap(), before);
    for _ in 0..3 {
        voice.advance(1.0).unwrap();
    }
    assert!(voice.finished());
    assert!(
        voice
            .render([0.; 3], 100, 48000)
            .unwrap()
            .iter()
            .all(|frame| *frame == [0.; 2])
    );
}
#[cfg(feature = "viewer")]
#[test]
#[ignore = "requires original terrain/model assets; isolated vehicle motion, not full mission"]
fn real_collision_scene_and_isolated_kinematic_vehicle_motion() {
    let graph = graph();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("local/extracted/files/Data/CompoundFiles");
    let archive = std::fs::read(root.join("SP_0.1_Level.res")).unwrap();
    let terrain =
        bw_rust::terrain::decode(&std::fs::read(root.join("SP_0.1.out")).unwrap()).unwrap();
    let mut host = bw_rust::mission_vm::Host::from_graph(&graph).unwrap();
    host.world.configure_rewrite(&graph).unwrap();
    host.world.configure_authored_units(&graph).unwrap();
    let report = bw_rust::mission_rewrite_scene::configure(
        &graph,
        &archive,
        &terrain,
        bw_rust::vehicle_profile::Override::Force(bw_rust::vehicle_profile::Engine::Ageia),
        &mut host.world,
    )
    .unwrap();
    assert_eq!(report.kinematic_vehicles, 30);
    assert_eq!(report.static_registrations, 782);
    assert_eq!(report.outside_terrain.len(), 5);
    assert!(host.world.collision.as_ref().unwrap().mesh_count() > 0);
    let candidates: Vec<_> = host
        .world
        .units
        .iter()
        .filter(|(id, u)| {
            u.active == Some(true)
                && u.animation.is_none()
                && !host.world.non_ground_units.contains(id)
        })
        .map(|(id, _)| *id)
        .collect();
    for id in candidates {
        let body = host.world.bodies[&id];
        let Ok(start) =
            host.world
                .collision
                .as_ref()
                .unwrap()
                .find_free(host.positions[&id], body, &[])
        else {
            continue;
        };
        for velocity in [[1., 0.], [-1., 0.], [0., 1.], [0., -1.]] {
            let mut trial = host.clone();
            trial.world.units.retain(|other, _| *other == id);
            trial.positions.insert(id, start);
            trial.world.direct_velocity.insert(id, velocity);
            let mut scheduler = bw_rust::mission_vm::Scheduler::default();
            let mut vm = bw_rust::lua50::Bootstrap::new();
            scheduler.tick(&mut vm, &mut trial, 0.1, 1000).unwrap();
            let end = trial.positions[&id];
            if (end[0] - start[0]).hypot(end[2] - start[2]) > 0.01 {
                return;
            }
        }
    }
    panic!("No tested ground vehicle could move in the real collision scene");
}
#[test]
#[ignore = "requires original local XML"]
fn actual_art_lights_retire_and_continue_as_bounded_rust_chains() {
    let graph = graph();
    let mut world = World::from_graph(&graph).unwrap();
    assert!(world.kill_rewrite(ObjectId(250009261), [0.; 3]).is_err());
    world.configure_rewrite(&graph).unwrap();
    for id in [250009261, 250010226, 250010165] {
        world.kill_rewrite(ObjectId(id), [1., 2., 3.]).unwrap();
        assert_eq!(world.entity(ObjectId(id)).unwrap().health, 0.);
        assert_eq!(world.entity(ObjectId(id)).unwrap().active, Some(false));
    }
    let runtime = world.rewrite.as_mut().unwrap();
    assert_eq!(runtime.effects().len(), 3);
    runtime.retire(ObjectId(250009261), [0.; 3]).unwrap();
    assert_eq!(runtime.effects().len(), 3);
    for _ in 0..40 {
        runtime.advance(0.25).unwrap();
        assert!(!runtime.effects().is_empty());
        assert!(runtime.effects().len() <= 6);
        assert!(runtime.effects().iter().all(|e| e.position == [1., 2., 3.]));
    }
    let mut partitioned = runtime.clone();
    let mut combined = runtime.clone();
    for _ in 0..30 {
        partitioned.advance(1.0 / 60.0).unwrap();
    }
    combined.advance(0.5).unwrap();
    assert_eq!(partitioned.effects().len(), combined.effects().len());
    for (a, b) in partitioned.effects().iter().zip(combined.effects()) {
        assert_eq!(a.light, b.light);
        assert_eq!(a.position, b.position);
    }
    let before = runtime.effects().len();
    assert!(runtime.advance(f64::NAN).is_err());
    assert_eq!(runtime.effects().len(), before);
}
#[test]
#[ignore = "original XML with synthetic registered collision mesh"]
fn retirement_removes_owned_geometry_and_isolates_snapshots() {
    let graph = graph();
    let id = ObjectId(250009261);
    let owner = Handle(NonZeroU64::new(1).unwrap());
    let mut scene = Scene::default();
    scene
        .register_object_meshes(
            id,
            owner,
            vec![(0, vec![[[0., 0., 0.], [0., 2., 0.], [0., 0., 2.]]])],
        )
        .unwrap();
    let original = Arc::new(scene);
    let mut world = World::from_graph(&graph).unwrap();
    world.configure_rewrite(&graph).unwrap();
    world.collision = Some(original.clone());
    world.kill_rewrite(id, [0.; 3]).unwrap();
    assert_eq!(world.collision.as_ref().unwrap().collision_owner(id), None);
    assert_eq!(original.collision_owner(id), Some(owner));
    assert_eq!(world.rewrite.as_ref().unwrap().effects().len(), 1);
}
#[test]
#[ignore = "requires original local XML"]
fn missing_collision_registration_rolls_back_health_and_effect_creation() {
    let graph = graph();
    let id = ObjectId(250009261);
    let mut world = World::from_graph(&graph).unwrap();
    world.configure_rewrite(&graph).unwrap();
    world.collision = Some(Arc::new(Scene::default()));
    let health = world.entity(id).unwrap().health;
    assert!(
        world
            .kill_rewrite(id, [0.; 3])
            .unwrap_err()
            .contains("registration")
    );
    assert_eq!(world.entity(id).unwrap().health, health);
    assert!(world.rewrite.as_ref().unwrap().retired.is_empty());
    assert!(world.rewrite.as_ref().unwrap().effects().is_empty());
}
