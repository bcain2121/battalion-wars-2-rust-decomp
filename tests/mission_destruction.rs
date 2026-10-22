use bw_rust::{ObjectGraph, ObjectId, mission_destruction::Profile, parse_objects};
#[test]
#[ignore = "requires original local mission assets"]
fn original_art_profile_retains_explosion_obligation() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("local/extracted/files/Data/CompoundFiles");
    let mut graph = ObjectGraph::default();
    for file in ["SP_0.1_Level.xml", "SP_0.1_Level_preload.xml"] {
        graph
            .merge(parse_objects(&std::fs::read_to_string(root.join(file)).unwrap()).unwrap())
            .unwrap();
    }
    for id in [250009261, 250010226, 250010165] {
        let object = &graph.objects[&ObjectId(id)];
        assert_eq!(object.kind, "cDestroyableObject");
        assert!(object.fields.iter().any(|f| f.items == ["250009278"]));
    }
    let base = &graph.objects[&ObjectId(250009278)];
    let p = Profile::from_object(base).unwrap();
    assert_eq!(p.flags, 8194);
    assert_eq!(p.explosion, Some(ObjectId(250009268)));
    assert_eq!(graph.objects[&p.explosion.unwrap()].kind, "sExplodeBase");
    // Authored cover references are not silently converted to an empty live attachment list.
    let covers = base
        .fields
        .iter()
        .find(|f| {
            f.attributes
                .get("name")
                .is_some_and(|n| n == "mCoverPointBase")
        })
        .unwrap();
    assert_eq!(covers.items.len(), 4);
    assert!(covers.items.iter().all(|id| id != "0"));
}
#[test]
fn profile_rejects_missing_duplicate_and_invalid_fields() {
    let xml = "<Instances><Object type=\"sDestroyBase\" id=\"1\"><Attribute name=\"ObjFlags\" elements=\"1\"><Item>2</Item></Attribute><Pointer name=\"ExplodeType\" elements=\"1\"><Item>0</Item></Pointer></Object></Instances>";
    let graph = parse_objects(xml).unwrap();
    let base = graph.into_iter().next().unwrap();
    assert_eq!(Profile::from_object(&base).unwrap().explosion, None);
    let mut bad = base.clone();
    bad.fields.pop();
    assert!(Profile::from_object(&bad).is_err());
    let mut bad = base.clone();
    bad.fields.push(base.fields[0].clone());
    assert!(Profile::from_object(&bad).is_err());
    for value in ["-1", "+2", "1.0", "4294967296", ""] {
        let mut bad = base.clone();
        bad.fields[0].items[0] = value.into();
        assert!(Profile::from_object(&bad).is_err());
    }
}
