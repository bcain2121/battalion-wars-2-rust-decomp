//! Original profiles in deliberately isolated arenas; not mission-progression evidence.
#![cfg(feature = "viewer")]
use bw_rust::{
    ObjectGraph, ObjectId, first_mission::Session, mission_missile::System, parse_objects,
    unit_collision::Scene,
};
use std::sync::Arc;
fn fixture() -> (Session, ObjectGraph, ObjectId, ObjectId) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let data = root.join("local/extracted/files/Data/CompoundFiles");
    let mut graph = ObjectGraph::default();
    for file in ["SP_0.1_Level.xml", "SP_0.1_Level_preload.xml"] {
        graph
            .merge(parse_objects(&std::fs::read_to_string(data.join(file)).unwrap()).unwrap())
            .unwrap();
    }
    let mut s = Session::load(
        root,
        &graph,
        &std::fs::read(data.join("SP_0.1_Level.res")).unwrap(),
    )
    .unwrap();
    let owner = ObjectId(550035415);
    let target = ObjectId(550032485);
    let h = &mut s.program.host;
    h.world.units.retain(|id, _| *id == owner || *id == target);
    h.world.props.clear();
    for u in h.world.units.values_mut() {
        u.active = Some(true);
        u.invulnerable = false;
        u.health = 100000.;
        u.max_health = 100000.;
    }
    h.world.collision = Some(Arc::new(Scene::default()));
    h.positions.insert(owner, [0., 0., 0.]);
    h.positions.insert(target, [0., 30., 70.]);
    h.world.headings.insert(target, 0.);
    assert_eq!(
        h.world.activity[&owner].enabled,
        Some(false),
        "Original AA starts with autonomous AI disabled"
    );
    (s, graph, owner, target)
}
#[test]
#[ignore = "private original assets; isolated ordnance arena"]
fn forced_anti_air_hits_reloads_and_obeys_weapon_disable_and_invulnerability() {
    let (mut s, g, owner, target) = fixture();
    let h = &mut s.program.host;
    let mut sys = System::default();
    sys.order(&g.objects, owner, target).unwrap();
    let mut previous = None;
    let mut protected_health = None;
    for frame in 0..1200 {
        let before = sys.launched;
        sys.order(&g.objects, owner, target).unwrap();
        sys.advance(&mut h.world, &mut h.positions, 1. / 60.)
            .unwrap();
        if sys.launched > before {
            assert_eq!(sys.launched, before + 1);
            if let Some(last) = previous {
                assert!(frame - last >= if sys.launched == 5 { 240 } else { 12 });
            }
            previous = Some(frame);
        }
        if sys.air_contacts > 0 && protected_health.is_none() {
            protected_health = Some(h.world.units[&target].health);
            h.world.units.get_mut(&target).unwrap().invulnerable = true;
        }
        if sys.launched >= 8 && sys.contacts >= 8 {
            break;
        }
    }
    assert!(sys.air_contacts >= 8);
    assert!(sys.damaging_hits > 0);
    assert!(protected_health.unwrap() < 100000.);
    assert_eq!(Some(h.world.units[&target].health), protected_health);
    h.world.activity.get_mut(&owner).unwrap().weapons_enabled = Some(false);
    let fired = sys.launched;
    for _ in 0..300 {
        sys.advance(&mut h.world, &mut h.positions, 1. / 60.)
            .unwrap();
    }
    assert_eq!(sys.launched, fired);
    h.world.activity.get_mut(&owner).unwrap().weapons_enabled = Some(true);
    sys.advance(&mut h.world, &mut h.positions, 1. / 60.)
        .unwrap();
    assert!(sys.launched > fired);
    let fired = sys.launched;
    let position = h.positions.clone();
    let health = h.world.units[&target].health;
    assert!(
        sys.advance(&mut h.world, &mut h.positions, f64::NAN)
            .is_err()
    );
    assert_eq!(sys.launched, fired);
    assert_eq!(h.positions, position);
    assert_eq!(h.world.units[&target].health, health);
    let mut broken = g.objects.clone();
    broken
        .get_mut(&ObjectId(50034030))
        .unwrap()
        .fields
        .iter_mut()
        .find(|f| f.attributes.get("name").is_some_and(|v| v == "mObjFlags"))
        .unwrap()
        .items[0] = "988".into();
    assert!(sys.order(&broken, owner, target).is_err());
    assert_eq!(sys.orders[&owner].target, target);
    println!(
        "AA_ARENA launches={} air_contacts={} damaging_contacts={} protected_health={:?}",
        sys.launched, sys.air_contacts, sys.damaging_hits, protected_health
    );
}
#[test]
#[ignore = "private original assets; isolated ordnance arena"]
fn missiles_respect_cover_and_expire_with_airburst_after_target_despawns() {
    let (mut s, g, owner, target) = fixture();
    let h = &mut s.program.host;
    let mut sys = System::default();
    let scene = Arc::make_mut(h.world.collision.as_mut().unwrap());
    scene
        .add_mesh(vec![
            [[-100., -10., 15.], [100., -10., 15.], [100., 100., 15.]],
            [[-100., -10., 15.], [100., 100., 15.], [-100., 100., 15.]],
        ])
        .unwrap();
    sys.order(&g.objects, owner, target).unwrap();
    for _ in 0..180 {
        sys.advance(&mut h.world, &mut h.positions, 1. / 60.)
            .unwrap();
    }
    assert!(sys.contacts > 0);
    assert_eq!(sys.air_contacts, 0);
    assert_eq!(h.world.units[&target].health, 100000.);
    let (mut s, g, owner, target) = fixture();
    let h = &mut s.program.host;
    let mut sys = System::default();
    sys.order(&g.objects, owner, target).unwrap();
    sys.advance(&mut h.world, &mut h.positions, 1. / 60.)
        .unwrap();
    assert_eq!(sys.launched, 1);
    h.world.units.get_mut(&target).unwrap().active = Some(false);
    for _ in 0..190 {
        sys.advance(&mut h.world, &mut h.positions, 1. / 60.)
            .unwrap();
    }
    assert_eq!(sys.expired, 1);
    assert!(sys.missiles.is_empty());
    assert_eq!(sys.launched, 1);
    assert_eq!(h.world.units[&target].health, 100000.);
}
#[test]
#[ignore = "private original assets"]
fn scripted_ground_facing_reset_never_shortcuts_position() {
    let (mut s, _, owner, _) = fixture();
    let h = &mut s.program.host;
    h.world.player = Some(owner);
    let before = h.positions.clone();
    assert!(
        h.world
            .teleport_scripted(&mut h.positions, owner, 0., 0., 90.)
            .unwrap()
    );
    assert_eq!(h.positions, before);
    assert!((h.world.player_aim.unwrap()[0] - 1.).abs() < 1e-6);
    let heading = h.world.headings[&owner];
    assert!(
        h.world
            .teleport_scripted(&mut h.positions, owner, 1., 0., 90.)
            .is_err()
    );
    assert_eq!(h.positions, before);
    assert_eq!(h.world.headings[&owner], heading);
}
