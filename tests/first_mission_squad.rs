#![cfg(feature = "viewer")]
use bw_rust::{
    ObjectGraph, ObjectId,
    first_mission::{Input, Session},
    lua50::Value,
    parse_objects,
};
/// Input-driven continuation of the original mission, not fabricated squad/victory state.
#[test]
#[ignore = "requires private assets and first-combat input recording from first_mission_runtime"]
fn original_route_completes_bomber_ambush_and_returns_player_control() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let data = root.join("local/extracted/files/Data/CompoundFiles");
    let mut graph = ObjectGraph::default();
    for file in ["SP_0.1_Level.xml", "SP_0.1_Level_preload.xml"] {
        graph
            .merge(parse_objects(&std::fs::read_to_string(data.join(file)).unwrap()).unwrap())
            .unwrap();
    }
    let mut session = Session::load(
        root,
        &graph,
        &std::fs::read(data.join("SP_0.1_Level.res")).unwrap(),
    )
    .unwrap();
    let inputs: Vec<Input> = serde_json::from_slice(
        &std::fs::read(root.join("local/analysis/first-combat-inputs.json"))
            .expect("Run original_intro_movement_look_pause_and_first_combat_objective first"),
    )
    .unwrap();
    let initial_frame = session.program.host.frame;
    session.begin_recording();
    for input in inputs {
        session.tick(1. / 60., input).unwrap();
    }
    assert!(session.program.host.world.objectives[&ObjectId(550032165)].1);
    let markers: Vec<_> = session
        .markers
        .iter()
        .filter(|m| m.position(&session.program.host).unwrap().is_some())
        .collect();
    assert!(!markers.iter().any(|m| m.id == ObjectId(550032163)));
    assert!(markers.iter().any(|m| m.id == ObjectId(550032491)));
    let start = session.program.host.positions[&session.program.host.world.player.unwrap()];
    let mut combat_capture = None;
    for _ in 0..7200 {
        let host = &session.program.host;
        let player = host.world.player.unwrap();
        let p = host.positions[&player];
        let aim = host.world.player_aim.unwrap();
        let all_positions = host.positions.clone();
        let headings = host.world.headings.clone();
        let releases = host.world.bombers.released;
        let frame = host.frame;
        let target = session
            .markers
            .iter()
            .filter_map(|m| m.position(host).unwrap())
            .min_by(|a, b| {
                let distance = |x: &[f32; 3]| (x[0] - p[0]).hypot(x[2] - p[2]);
                distance(a).total_cmp(&distance(b))
            })
            .unwrap_or(p);
        let dx = (target[0] - p[0]) as f64;
        let dz = (target[2] - p[2]) as f64;
        let yaw = (dx.atan2(dz) - aim[0].atan2(aim[2]) + std::f64::consts::PI)
            .rem_euclid(std::f64::consts::TAU)
            - std::f64::consts::PI;
        let result = session.tick(
            1. / 60.,
            Input {
                movement: [
                    0.,
                    if dx.hypot(dz) > 3. && yaw.abs() < 0.1 {
                        1.
                    } else {
                        0.
                    },
                ],
                look: [yaw.clamp(-0.05, 0.05), (-aim[1].asin()).clamp(-0.05, 0.05)],
                ..Default::default()
            },
        );
        if let Err(error) = result {
            assert_eq!(session.program.host.frame, frame);
            assert_eq!(session.program.host.positions, all_positions);
            assert_eq!(session.program.host.world.headings, headings);
            assert_eq!(session.program.host.world.bombers.released, releases);
            panic!(
                "Ambush blocked transactionally at {frame}: {error}; missiles={} contacts={} bombs={} impacts={}",
                session.program.host.world.missiles.launched,
                session.program.host.world.missiles.contacts,
                releases,
                session.program.host.world.bombers.impacts
            );
        }
        let world = &session.program.host.world;
        if combat_capture.is_none() && world.missiles.launched >= 4 && world.bombers.released > 0 {
            combat_capture = Some((session.program.host.frame - initial_frame) as usize);
        }
        if session.program.vm.globals.get("cutsceneBombedBegin") == Some(&Value::Number(2.))
            && !world.frozen
        {
            assert!(world.objectives[&ObjectId(550032489)].1);
            assert!((p[0] - start[0]).hypot(p[2] - start[2]) > 50.);
            assert!(world.missiles.launched > 0);
            assert!(
                world.missiles.air_contacts > 0,
                "Original defensive missiles must actually contact aircraft"
            );
            assert!(world.bombers.released > 0);
            assert!(world.bombers.impacts > 0);
            println!(
                "ORIGINAL_AMBUSH_COMPLETE time={} missiles={} contacts={} airbursts={} bomb_releases={} bomb_impacts={} player={p:?} aircraft_contacts={}",
                session.program.host.time,
                world.missiles.launched,
                world.missiles.contacts,
                world.missiles.expired,
                world.bombers.released,
                world.bombers.impacts,
                world.missiles.air_contacts
            );
            let recording = session.take_recording().unwrap();
            std::fs::write(
                root.join("local/analysis/ambush-complete-inputs.json"),
                serde_json::to_vec(&recording).unwrap(),
            )
            .unwrap();
            let n = combat_capture.expect("Must observe simultaneous missile/bomb activity");
            std::fs::write(
                root.join("local/analysis/ambush-fire-inputs.json"),
                serde_json::to_vec(&recording[..n]).unwrap(),
            )
            .unwrap();
            session.restart();
            assert_eq!(session.program.host.world.missiles.launched, 0);
            assert!(session.program.host.world.missiles.missiles.is_empty());
            assert_eq!(session.program.host.world.bombers.released, 0);
            assert!(session.program.host.world.bombers.bombs.is_empty());
            assert_eq!(session.program.host.frame, 1);
            return;
        }
    }
    panic!("Original ambush did not release the player within bounded simulation");
}
