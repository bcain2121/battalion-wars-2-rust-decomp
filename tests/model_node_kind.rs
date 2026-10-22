use bw_rust::model_node_kind::{Catalog, Source, SpatialDispatch};

#[test]
#[ignore = "Requires original archive and original DOL"]
fn original_art_carrier_has_no_volume_dispatch_nodes() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let dol = std::fs::read(root.join("local/extracted/sys/main.dol")).unwrap();
    let catalog = Catalog::from_rbwe01_dol(&dol).unwrap();
    assert_eq!(catalog.static_tag_count(), 250);
    // The earlier 162-row research snapshot was partial; never use it as a full catalog.
    let bytes =
        std::fs::read(root.join("local/extracted/files/Data/CompoundFiles/SP_0.1_Level.res"))
            .unwrap();
    let model = bw_rust::model::from_archive(&bytes, "SFXdummyRoot").unwrap();
    assert_eq!(model.nodes.len(), 2);
    assert_eq!(model.nodes[0].name, "ROOT");
    assert_eq!(model.nodes[1].name, "EFFECT_A");
    assert_eq!(model.hierarchy_links, vec![(0, 1)]);
    let a = catalog
        .resolve(&model.nodes[0].name, model.nodes[0].authored_kind)
        .unwrap();
    let b = catalog
        .resolve(&model.nodes[1].name, model.nodes[1].authored_kind)
        .unwrap();
    assert_eq!(
        (model.nodes[0].authored_kind, model.nodes[1].authored_kind),
        (0, 0)
    );
    assert_eq!((a.kind, a.source), (0, Source::AuthoredHeader));
    assert_eq!((b.kind, b.source), (62, Source::NameTag));
    assert_eq!(a.spatial_dispatch(), SpatialDispatch::Ignored);
    assert_eq!(b.spatial_dispatch(), SpatialDispatch::Ignored);
    // This does NOT omit the surrounding spatial query, dirty marking, lights or cleanup.
}
