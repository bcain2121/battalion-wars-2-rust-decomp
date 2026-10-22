use bw_rust::{
    ObjectGraph, ObjectId,
    level_catalog::Catalog,
    parse_objects,
    vehicle_profile::{Bank, Engine, Override},
};

#[test]
#[ignore = "Requires privately extracted original assets"]
fn original_catalog_selects_ageia_for_first_mission_not_authored_ode() {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("local/extracted/files/Data");
    let catalog =
        Catalog::parse(&std::fs::read_to_string(data.join("levels.xml")).unwrap()).unwrap();
    assert_eq!(catalog.entries.len(), 77);
    let entry = catalog.unique("SP_0.1").unwrap();
    assert_eq!(entry.physics_attribute, Some(1));
    assert_eq!(entry.filename, "@os:Data/CompoundFiles/SP_0.1.xml");
    assert_eq!(entry.gametype, "singleplayer");
    assert_eq!(entry.physics, Override::Force(Engine::Ageia));
    assert_eq!(
        catalog.unique("frontend2").unwrap().physics,
        Override::Force(Engine::Ode)
    );
    assert!(
        catalog
            .unique("unused.1")
            .unwrap_err()
            .contains("Ambiguous")
    );
    let mut graph = ObjectGraph::default();
    for name in ["SP_0.1_Level.xml", "SP_0.1_Level_preload.xml"] {
        graph
            .merge(
                parse_objects(
                    &std::fs::read_to_string(data.join("CompoundFiles").join(name)).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
    }
    let bank = Bank::load(&graph, entry.physics).unwrap();
    assert_eq!(bank.units.len(), 30);
    assert!(
        bank.units
            .values()
            .all(|b| b.authored_engine == Engine::Ode && b.engine == Engine::Ageia)
    );
    let tank = &bank.units[&ObjectId(550040155)];
    assert_eq!(tank.profile.id, ObjectId(50034654));
    assert_eq!(tank.profile.attack_speed, 11.0);
}
