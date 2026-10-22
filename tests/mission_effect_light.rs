use bw_rust::{
    Object, ObjectGraph, ObjectId,
    mission_effect_light::{Light, Spec, UpdateEntry},
    parse_objects,
};

#[cfg(feature = "viewer")]
#[test]
#[ignore = "Requires privately extracted original assets"]
fn original_effect_carrier_retains_header_sphere_separately() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("local/extracted/files/Data/CompoundFiles/SP_0.1_Level.res");
    let bytes = std::fs::read(path).unwrap();
    let model = bw_rust::model::from_archive(&bytes, "SFXdummyRoot").unwrap();
    assert_eq!(model.header_sphere, [0.0, -0.35, 0.0, 0.001_484_919_9]);
    let tank = bw_rust::model::from_archive(&bytes, "VUHTNKH").unwrap();
    assert_eq!(
        tank.header_sphere,
        [0.014_415_7, 2.558_44, -0.246_145, 5.398_159_5]
    );
}

fn fixture() -> Object {
    parse_objects(
        r#"<Instances><Object type="sExplodeBase" id="7">
      <Attribute name="mEffectLightRadius"><Item>80</Item></Attribute>
      <Attribute name="mEffectLightColour"><Item>255,77,0,129</Item></Attribute>
      <Attribute name="mEffectLightDuration"><Item>10</Item></Attribute>
    </Object></Instances>"#,
    )
    .unwrap()
    .remove(0)
}

#[test]
fn strict_channel_parsing_rejects_invalid_and_ambiguous_fields() {
    let source = fixture();
    assert!(Spec::from_explosion(&source).is_ok());
    for (field, value) in [
        (0, "NaN"),
        (0, "1e300"),
        (0, "-1"),
        (1, "256,77,0,129"),
        (1, "1,2,3"),
        (1, "-1,2,3,4"),
        (2, "4294967296"),
        (2, "-1"),
        (2, "1.5"),
    ] {
        let mut bad = source.clone();
        bad.fields[field].items = vec![value.into()];
        assert!(Spec::from_explosion(&bad).is_err(), "accepted {value}");
    }
    let mut duplicate = source.clone();
    duplicate.fields.push(source.fields[0].clone());
    assert!(Spec::from_explosion(&duplicate).is_err());
    let mut missing = source.clone();
    missing.fields.pop();
    assert!(Spec::from_explosion(&missing).is_err());
    let mut wrong_kind = source;
    wrong_kind.kind = "cDestroyableObject".into();
    assert!(Spec::from_explosion(&wrong_kind).is_err());
}

#[test]
#[ignore = "Requires privately extracted original assets"]
fn original_art_initialisation_effect_light_channel() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("local/extracted/files/Data/CompoundFiles");
    let mut graph = ObjectGraph::default();
    for name in ["SP_0.1_Level.xml", "SP_0.1_Level_preload.xml"] {
        graph
            .merge(parse_objects(&std::fs::read_to_string(root.join(name)).unwrap()).unwrap())
            .unwrap();
    }
    let profile = &graph.objects[&ObjectId(250009268)];
    let mut light = Light::new(Spec::from_explosion(profile).unwrap());
    let initial = light.snapshot();
    assert_eq!(initial.radius, 80.0);
    assert_eq!(initial.colour, [255, 77, 0, 129]);
    assert_eq!(initial.remaining_updates, 10);
    // Explicit normal-update flags fixture, not a recovered live effect instance.
    let mut effect_flags = 0;
    for _ in 0..10 {
        assert_eq!(
            light.begin_effect_update(&mut effect_flags),
            UpdateEntry::RunState
        );
    }
    assert!(light.snapshot().active);
    assert_eq!(
        light.begin_effect_update(&mut effect_flags),
        UpdateEntry::RunState
    );
    assert!(!light.snapshot().active);
    // Explicit integer-cursor fixture, not recovered post-menu continuity.
    let field = |name: &str| -> &str {
        &profile
            .fields
            .iter()
            .find(|f| f.attributes.get("name").is_some_and(|n| n == name))
            .unwrap()
            .items[0]
    };
    let mut random = bw_rust::mission_random::IntegerStream::at_cursor(0).unwrap();
    let mut chain = bw_rust::mission_effect_chain::Tail::new(
        0.0, // Explicit pre-setup timer fixture, not a full native constructor.
        Some(ObjectId(field("mNextExplosionType1").parse().unwrap())),
        Some(ObjectId(field("mNextExplosionType2").parse().unwrap())),
        field("mNextExplosion1Probability").parse().unwrap(),
    )
    .unwrap();
    assert_eq!(
        chain
            .initialize_delay(&mut random, |id| {
                let selected = graph.objects.get(&id).ok_or("Missing child profile")?;
                selected
                    .fields
                    .iter()
                    .find(|f| {
                        f.attributes
                            .get("name")
                            .is_some_and(|n| n == "mNextExplosionDelay")
                    })
                    .and_then(|f| f.items.first())
                    .ok_or_else(|| "Missing child delay".to_string())?
                    .parse::<f32>()
                    .map_err(|e| e.to_string())
            })
            .unwrap(),
        Some(profile.id)
    );
    assert_eq!(chain.remaining(), 0.1);
    assert_eq!(random.cursor(), 1);
    assert_eq!(
        chain.advance(0.05, &mut random).unwrap(),
        bw_rust::mission_effect_chain::Event::NotDue
    );
    assert_eq!(
        chain.advance(0.05, &mut random).unwrap(),
        bw_rust::mission_effect_chain::Event::Selected(Some(profile.id))
    );
    assert_eq!(random.cursor(), 2);
    // This is explicitly not Kill/explosion execution: the source also requests a chain.
    assert_eq!(
        profile
            .fields
            .iter()
            .find(|f| f
                .attributes
                .get("name")
                .is_some_and(|n| n == "mNextExplosionType1"))
            .unwrap()
            .items,
        vec!["250009268"]
    );
}
