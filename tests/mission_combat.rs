//! Private original weapon data in an explicitly isolated flat arena, NOT mission progression.
use bw_rust::{
    ObjectGraph, ObjectId,
    mission_combat::Combat,
    mission_vm::Host,
    parse_objects,
    unit_collision::{Body, Scene},
};
fn fixture() -> (Host, Combat, ObjectId, ObjectId) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("local/extracted/files/Data/CompoundFiles");
    let mut graph = ObjectGraph::default();
    for file in ["SP_0.1_Level.xml", "SP_0.1_Level_preload.xml"] {
        graph
            .merge(parse_objects(&std::fs::read_to_string(root.join(file)).unwrap()).unwrap())
            .unwrap();
    }
    let mut host = Host::from_graph(&graph).unwrap();
    host.world.configure_rewrite(&graph).unwrap();
    host.world.rewrite.as_mut().unwrap().allow_cosmetic_gaps();
    let player = ObjectId(550030275);
    let enemy = *host
        .world
        .units
        .iter()
        .find(|(id, u)| {
            u.army != "eSolarEmpire"
                && u.army != "eNoArmy"
                && graph.objects[id].kind == "cTroop"
                && {
                    let base = bw_rust::original_scene::field(&graph.objects[id], "mBase")
                        .unwrap()
                        .parse::<u64>()
                        .unwrap();
                    bw_rust::original_scene::field(&graph.objects[&ObjectId(base)], "mUnitSubType")
                        == Some("UNIT_MAN_GRUNT")
                }
        })
        .unwrap()
        .0;
    for u in host.world.units.values_mut() {
        u.active = Some(false);
    }
    for id in [player, enemy] {
        let u = host.world.units.get_mut(&id).unwrap();
        u.active = Some(true);
        u.invulnerable = false;
    }
    for a in host.world.activity.values_mut() {
        a.enabled = Some(false);
    }
    host.world.player = Some(player);
    host.world.player_aim = Some([0., 0., 1.]);
    host.world.controller = Some(bw_rust::mission_controls::Controller::fresh_player());
    host.world.collision = Some(std::sync::Arc::new(Scene::default()));
    host.world.bodies = [(player, Body::INFANTRY), (enemy, Body::INFANTRY)]
        .into_iter()
        .collect();
    host.positions.insert(player, [0., 0., 0.]);
    host.positions.insert(enemy, [0., 0., 10.]);
    let combat = Combat::new(&host).unwrap();
    (host, combat, player, enemy)
}
#[test]
#[ignore = "requires private original assets"]
fn rifle_damage_reload_cover_invulnerability_and_retirement() {
    let (mut h, mut c, player, enemy) = fixture();
    h.world.activity.get_mut(&player).unwrap().weapons_enabled = Some(false);
    c.tick(&mut h, 1. / 60., true, false).unwrap();
    assert_eq!(c.fired, 0);
    h.world.activity.get_mut(&player).unwrap().weapons_enabled = Some(true);
    let before = h.world.units[&enemy].health;
    c.tick(&mut h, 1. / 60., true, false).unwrap();
    assert!(h.world.units[&enemy].health < before);
    assert_eq!(c.fired, 1);
    assert_eq!(c.hits, 1);
    let damage = c.shots[0].damage;
    assert!(damage > 0.);
    for _ in 0..4 {
        c.tick(&mut h, 1. / 60., true, false).unwrap();
    }
    assert_eq!(
        c.fired, 1,
        "Source cadence must prevent frame-rate-dependent firing"
    );
    c.reset();
    h.world.units.get_mut(&enemy).unwrap().invulnerable = true;
    c.tick(&mut h, 1. / 60., true, false).unwrap();
    assert_eq!(c.hits, 0);
    h.world.units.get_mut(&enemy).unwrap().invulnerable = false;
    c.reset();
    let scene = std::sync::Arc::make_mut(h.world.collision.as_mut().unwrap());
    scene
        .add_mesh(vec![
            [[-5., 0., 5.], [5., 0., 5.], [5., 5., 5.]],
            [[-5., 0., 5.], [5., 5., 5.], [-5., 5., 5.]],
        ])
        .unwrap();
    let health = h.world.units[&enemy].health;
    c.tick(&mut h, 1. / 60., true, false).unwrap();
    assert_eq!(h.world.units[&enemy].health, health);
    assert!(c.shots[0].to[2] < 6.);
    h.world.collision = Some(std::sync::Arc::new(Scene::default()));
    h.world.units.get_mut(&enemy).unwrap().health = damage;
    c.reset();
    c.tick(&mut h, 1. / 60., true, false).unwrap();
    assert_eq!(h.world.units[&enemy].health, 0.);
    assert_eq!(h.world.units[&enemy].active, Some(false));
    assert!(h.world.rewrite.as_ref().unwrap().retired.contains(&enemy));
    c.reset();
    for _ in 0..190 {
        c.tick(&mut h, 1. / 60., true, false).unwrap();
    }
    assert_eq!(
        c.fired, 30,
        "Thirty-round source clip must pause for reload"
    );
    for _ in 0..36 {
        c.tick(&mut h, 1. / 60., true, false).unwrap();
    }
    assert!(c.fired > 30);
}
#[test]
#[ignore = "requires private original assets"]
fn enemy_rifle_returns_fire_and_friendly_body_intercepts() {
    let (mut h, mut c, player, enemy) = fixture();
    let health = h.world.units[&player].health;
    h.world.activity.get_mut(&enemy).unwrap().enabled = Some(true);
    c.tick(&mut h, 1. / 60., false, false).unwrap();
    assert!(h.world.units[&player].health < health);
    assert_eq!(c.shots[0].shooter, enemy);
    c.reset();
    h.world.activity.get_mut(&enemy).unwrap().enabled = Some(false);
    let ally = *h
        .world
        .units
        .keys()
        .find(|id| **id != player && **id != enemy)
        .unwrap();
    let army = h.world.units[&player].army.clone();
    let u = h.world.units.get_mut(&ally).unwrap();
    u.army = army;
    u.active = Some(true);
    u.health = 100.;
    h.world.bodies.insert(ally, Body::INFANTRY);
    h.positions.insert(ally, [0., 0., 5.]);
    let health = h.world.units[&enemy].health;
    c.tick(&mut h, 1. / 60., true, false).unwrap();
    assert_eq!(c.shots[0].hit, Some(ally));
    assert_eq!(h.world.units[&enemy].health, health);
    assert_eq!(h.world.units[&ally].health, 100.);
}
#[test]
#[ignore = "requires private original assets"]
fn flame_weapon_cadence_reload_and_staff_bearer_fire_immunity() {
    let (mut h, mut c, player, enemy) = fixture();
    let flamer = ObjectId(550030844);
    let staff = ObjectId(550035312);
    h.world.units.get_mut(&enemy).unwrap().active = Some(false);
    for id in [flamer, staff] {
        let u = h.world.units.get_mut(&id).unwrap();
        u.active = Some(true);
        u.invulnerable = false;
        h.world.bodies.insert(id, Body::INFANTRY);
    }
    h.world.activity.get_mut(&flamer).unwrap().enabled = Some(true);
    h.world.activity.get_mut(&flamer).unwrap().weapons_enabled = Some(true);
    h.positions.insert(flamer, [0., 0., 10.]);
    h.positions.insert(player, [0., 0., 0.]);
    h.positions.insert(staff, [50., 0., 0.]);
    let before = h.world.units[&player].health;
    c.tick(&mut h, 1. / 60., false, false).unwrap();
    assert_eq!(c.fired, 1);
    assert_eq!(c.hits, 1);
    assert_eq!(c.shots[0].shooter, flamer);
    assert_eq!(c.shots[0].hit, Some(player));
    assert!(h.world.units[&player].health < before);
    c.reset();
    h.world.units.get_mut(&player).unwrap().active = Some(false);
    h.world.player = Some(staff);
    h.positions.insert(staff, [0., 0., 0.]);
    let staff_before = h.world.units[&staff].health;
    for _ in 0..6 {
        c.tick(&mut h, 1. / 60., false, false).unwrap();
    }
    assert_eq!(
        c.fired, 3,
        "Flame troop mRefireRate=0.03s must fire 3 times across 6 frames (0.10s)"
    );
    assert!(c.shots.iter().all(|s| s.damage == 0.0));
    assert_eq!(h.world.units[&staff].health, staff_before);
}
#[test]
#[ignore = "requires private original assets"]
fn tank_bazooka_staff_and_drained_staff_weapons() {
    let (mut h, mut c, player, enemy) = fixture();
    let tank = ObjectId(550030806);
    let bazooka = ObjectId(550030842);
    let staff = ObjectId(550035312);
    let drained = ObjectId(550040934);
    h.world.units.get_mut(&player).unwrap().active = Some(false);
    for id in [tank, bazooka, staff, drained, enemy] {
        let u = h.world.units.get_mut(&id).unwrap();
        u.active = Some(true);
        u.invulnerable = false;
        h.world.bodies.insert(id, Body::INFANTRY);
    }
    h.world.player = Some(staff);
    h.world.activity.get_mut(&staff).unwrap().weapons_enabled = Some(true);
    h.world.activity.get_mut(&tank).unwrap().enabled = Some(true);
    h.world.activity.get_mut(&tank).unwrap().weapons_enabled = Some(true);
    h.world.activity.get_mut(&bazooka).unwrap().enabled = Some(true);
    h.world.activity.get_mut(&bazooka).unwrap().weapons_enabled = Some(true);
    h.positions.insert(staff, [0., 0., 0.]);
    h.positions.insert(tank, [0., 0., 20.]);
    h.positions.insert(bazooka, [15., 0., 0.]);
    h.positions.insert(enemy, [0., 0., 10.]);
    h.positions.insert(drained, [50., 0., 0.]);
    let staff_hp = h.world.units[&staff].health;
    let enemy_hp = h.world.units[&enemy].health;
    c.tick(&mut h, 1. / 60., true, false).unwrap();
    assert_eq!(c.fired, 3);
    assert!(h.world.units[&enemy].health < enemy_hp);
    assert!(h.world.units[&staff].health < staff_hp);
    c.reset();
    h.world.units.get_mut(&staff).unwrap().active = Some(false);
    h.world.activity.get_mut(&tank).unwrap().enabled = Some(false);
    h.world.activity.get_mut(&bazooka).unwrap().enabled = Some(false);
    h.world.player = Some(drained);
    h.world.activity.get_mut(&drained).unwrap().weapons_enabled = Some(true);
    h.positions.insert(drained, [0., 0., 0.]);
    let enemy_hp = h.world.units[&enemy].health;
    c.tick(&mut h, 1. / 60., true, false).unwrap();
    assert_eq!(c.fired, 1);
    assert_eq!(c.hits, 0);
    assert_eq!(h.world.units[&enemy].health, enemy_hp);
}
