#![cfg(feature = "viewer")]
use bw_rust::{
    ObjectGraph, ObjectId, first_mission::Session, mission_bomber::System, parse_objects,
};
fn session() -> (Session, ObjectGraph) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let data = root.join("local/extracted/files/Data/CompoundFiles");
    let mut graph = ObjectGraph::default();
    for f in ["SP_0.1_Level.xml", "SP_0.1_Level_preload.xml"] {
        graph
            .merge(parse_objects(&std::fs::read_to_string(data.join(f)).unwrap()).unwrap())
            .unwrap();
    }
    let session = Session::load(
        root,
        &graph,
        &std::fs::read(data.join("SP_0.1_Level.res")).unwrap(),
    )
    .unwrap();
    (session, graph)
}
#[test]
#[ignore = "private original assets; isolated ordnance fixture, not mission completion"]
fn source_bomber_teleport_and_guided_impacts_are_real() {
    let (mut s, graph) = session();
    let h = &mut s.program.host;
    let bomber = ObjectId(550032485);
    let target = ObjectId(550032385);
    h.world.set_rewrite_presence(bomber, true, false).unwrap();
    h.world
        .teleport_air(&mut h.positions, bomber, 18., 151.8, 158.2)
        .unwrap();
    let expected = h
        .world
        .collision
        .as_ref()
        .unwrap()
        .world_point([18., 0., 151.8], 40.)
        .unwrap();
    assert_eq!(h.positions[&bomber], expected);
    assert!((h.world.headings[&bomber] - 158.2_f32.to_radians()).abs() < 1e-5);
    let before = h.positions.clone();
    let headings = h.world.headings.clone();
    assert!(
        h.world
            .teleport_air(&mut h.positions, bomber, f64::NAN, 0., 0.)
            .is_err()
    );
    assert_eq!(h.positions, before);
    assert_eq!(h.world.headings, headings);
    // Isolate the original weapon and geometry from scripts/other combat actors.
    h.world.units.retain(|id, _| *id == bomber);
    h.world.props.retain(|id, _| *id == target);
    let unit = h.world.props.get_mut(&target).unwrap();
    unit.invulnerable = false;
    unit.health = 100000.;
    unit.max_health = 100000.;
    unit.active = Some(true);
    let objects = graph.objects;
    let mut system = System::default();
    system.order(&objects, bomber, target).unwrap();
    let mut last_release = None;
    let mut health_after_first = None;
    for frame in 0..2400 {
        let before = system.released;
        // Reissuing an order must not refill the clip or bypass reload.
        system.order(&objects, bomber, target).unwrap();
        system
            .advance(&mut h.world, &mut h.positions, 1. / 60.)
            .unwrap();
        if system.released > before {
            assert_eq!(system.released, before + 1);
            if let Some(previous) = last_release {
                assert!(frame - previous >= if system.released == 7 { 599 } else { 14 });
            }
            last_release = Some(frame);
        }
        if system.impacts > 0 && health_after_first.is_none() {
            health_after_first = Some(h.world.props[&target].health);
            h.world.props.get_mut(&target).unwrap().invulnerable = true;
        }
        if system.released >= 7 && system.impacts >= 6 {
            break;
        }
    }
    assert!(system.released >= 7, "Clip must reload and fire again");
    assert_eq!(
        Some(h.world.props[&target].health),
        health_after_first,
        "Invulnerability must stop later blast damage"
    );
    let before = h.positions.clone();
    let released = system.released;
    let impacts = system.impacts;
    let health = h.world.props[&target].health;
    assert!(
        system
            .advance(&mut h.world, &mut h.positions, f64::NAN)
            .is_err()
    );
    assert_eq!(h.positions, before);
    assert_eq!(system.released, released);
    assert_eq!(system.impacts, impacts);
    assert_eq!(h.world.props[&target].health, health);
    let mut malformed = objects.clone();
    malformed
        .get_mut(&ObjectId(50034048))
        .unwrap()
        .fields
        .iter_mut()
        .find(|f| {
            f.attributes
                .get("name")
                .is_some_and(|n| n == "mDamageAmount")
        })
        .unwrap()
        .items[2] = "NaN".into();
    assert!(system.order(&malformed, bomber, target).is_err());
    assert_eq!(system.orders[&bomber].target, target);
    println!(
        "BOMBER_FIXTURE released={} impacts={} target_health={} position={:?}",
        system.released, system.impacts, h.world.props[&target].health, h.positions[&bomber]
    );
    assert!(system.released > 0);
    assert!(system.impacts > 0);
    assert!(h.world.props[&target].health < 100000.);
}
