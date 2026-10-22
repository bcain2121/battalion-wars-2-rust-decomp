use bw_rust::mission_spatial_removal::{
    SpatialProfile, authored_instance_flags, destroyable_virtual_1a4,
};
use bw_rust::{Object, ObjectGraph, ObjectId, parse_objects};
fn fixture() -> Object {
    parse_objects("<Instances><Object type=\"sDestroyBase\" id=\"1\"><Attribute name=\"ObjFlags\" elements=\"1\"><Item>2</Item></Attribute><Pointer name=\"ExplodeType\" elements=\"1\"><Item>0</Item></Pointer><Pointer name=\"DeadModel\" elements=\"1\"><Item>0</Item></Pointer><Attribute name=\"CollideType\" elements=\"1\"><Item>0</Item></Attribute><Attribute name=\"mDeadModelDecayTime\" elements=\"1\"><Item>0.0</Item></Attribute></Object></Instances>").unwrap().remove(0)
}
fn set(object: &mut Object, name: &str, value: &str) {
    object
        .fields
        .iter_mut()
        .find(|f| f.attributes.get("name").is_some_and(|n| n == name))
        .unwrap()
        .items = vec![value.into()];
}
#[test]
fn live_flags_are_required_only_when_native_gate_reads_them() {
    let mut p = SpatialProfile::from_object(&fixture()).unwrap();
    assert!(p.pre_query_update_required(None).is_err());
    assert!(!p.pre_query_update_required(Some(0)).unwrap());
    assert!(p.pre_query_update_required(Some(0x800)).unwrap());
    assert!(!p.pre_query_update_required(Some(0x80000)).unwrap());
    p.collide_type = 8;
    assert!(p.pre_query_update_required(None).unwrap());
}
#[test]
fn malformed_fields_and_unresolved_resources_do_not_default() {
    let original = fixture();
    for name in ["DeadModel", "CollideType", "mDeadModelDecayTime"] {
        let mut bad = original.clone();
        bad.fields
            .retain(|f| f.attributes.get("name").is_none_or(|n| n != name));
        assert!(SpatialProfile::from_object(&bad).is_err());
        let mut bad = original.clone();
        bad.fields.push(
            original
                .fields
                .iter()
                .find(|f| f.attributes.get("name").is_some_and(|n| n == name))
                .unwrap()
                .clone(),
        );
        assert!(SpatialProfile::from_object(&bad).is_err());
    }
    for (name, value) in [
        ("CollideType", "-1"),
        ("CollideType", "4294967296"),
        ("DeadModel", "+1"),
        ("DeadModel", "18446744073709551616"),
        ("mDeadModelDecayTime", "NaN"),
        ("mDeadModelDecayTime", "inf"),
    ] {
        let mut bad = original.clone();
        set(&mut bad, name, value);
        assert!(SpatialProfile::from_object(&bad).is_err());
    }
    let graph = ObjectGraph::default();
    let mut p = SpatialProfile::from_object(&original).unwrap();
    assert!(p.predicate_for_null_dead_model(&graph).is_ok());
    p.dead_model = Some(ObjectId(2));
    assert!(p.predicate_for_null_dead_model(&graph).is_err());
    p.dead_model = None;
    p.explosion = Some(ObjectId(3));
    assert!(p.predicate_for_null_dead_model(&graph).is_err());
}
#[test]
#[ignore = "requires original local mission XML"]
fn original_art_binds_real_profile_fields_without_assuming_live_flags() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("local/extracted/files/Data/CompoundFiles");
    let mut graph = ObjectGraph::default();
    for name in ["SP_0.1_Level.xml", "SP_0.1_Level_preload.xml"] {
        graph
            .merge(parse_objects(&std::fs::read_to_string(root.join(name)).unwrap()).unwrap())
            .unwrap();
    }
    let p = SpatialProfile::from_object(&graph.objects[&ObjectId(250009278)]).unwrap();
    assert_eq!(p.collide_type, 0);
    assert_eq!(p.dead_model, None);
    assert_eq!(p.dead_model_decay_time, 0.0);
    assert!(p.pre_query_update_required(None).is_err());
    for id in [250009261, 250010226, 250010165] {
        let object = &graph.objects[&ObjectId(id)];
        let flags = authored_instance_flags(object).unwrap();
        assert_eq!(flags, 0);
        assert!(!p.pre_query_update_required(Some(flags)).unwrap());
        let mut bad = object.clone();
        bad.fields.retain(|f| {
            f.attributes
                .get("name")
                .is_none_or(|n| n != "mInstanceFlags")
        });
        assert!(authored_instance_flags(&bad).is_err());
    }
    let bound = p.predicate_for_null_dead_model(&graph).unwrap();
    assert_eq!(bound.explosion_flags, Some(16));
    for flags in [None, Some(0), Some(0x400000), Some(u32::MAX)] {
        assert!(!destroyable_virtual_1a4(flags, 0x100000, Some(bound)).unwrap());
    }
}
