use bw_rust::{ObjectGraph, ObjectId, frontend::Catalog, parse_objects};
fn fixture() -> String {
    r#"<Instances>
<Object id="1" type="cGUIPage"><Pointer name="mpScript" elements="1"><Item>2</Item></Pointer><Pointer name="mWidgetArray" elements="3"><Item>4</Item><Item>0</Item><Item>4</Item></Pointer></Object>
<Object id="2" type="cInitialisationScriptEntity"><Resource name="mpScript" elements="1"><Item>3</Item></Resource></Object>
<Object id="3" type="cGameScriptResource"><Attribute name="mName"><Item>Start</Item></Attribute></Object>
<Object id="4" type="cGUICustomWidget"><Pointer name="mWidgetArray" elements="1"><Item>0</Item></Pointer></Object>
</Instances>"#.into()
}
fn load(xml: &str) -> Result<Catalog, String> {
    let mut graph = ObjectGraph::default();
    graph.merge(parse_objects(xml)?)?;
    Catalog::load(&graph)
}
#[test]
fn shared_widgets_and_null_slots_preserved() {
    let c = load(&fixture()).unwrap();
    assert_eq!(c.widgets.len(), 1);
    assert_eq!(
        c.pages["Start"].children,
        vec![Some(ObjectId(4)), None, Some(ObjectId(4))]
    );
}
#[test]
fn missing_widget_and_cycle_rejected() {
    assert!(load(&fixture().replace("<Item>4</Item>", "<Item>99</Item>")).is_err());
    let xml = fixture().replace(
        "elements=\"1\"><Item>0</Item>",
        "elements=\"1\"><Item>4</Item>",
    );
    assert!(load(&xml).unwrap_err().contains("cycle"));
}
#[test]
fn bad_counts_and_script_types_rejected() {
    assert!(load(&fixture().replace("elements=\"3\"", "elements=\"2\"")).is_err());
    assert!(load(&fixture().replace("cGameScriptResource", "cTextureResource")).is_err());
}
#[test]
#[ignore = "requires original local frontend XML"]
fn original_pages_and_start_event() {
    let mut graph = ObjectGraph::default();
    for name in ["Frontend2_Level.xml", "Frontend2_Level_preload.xml"] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("local/extracted/files/Data/CompoundFiles")
            .join(name);
        graph
            .merge(parse_objects(&std::fs::read_to_string(path).unwrap()).unwrap())
            .unwrap();
    }
    let c = Catalog::load(&graph).unwrap();
    assert_eq!(c.pages.len(), 82);
    assert_eq!(c.pages["Start"].id, ObjectId(2144911942));
    assert!(
        c.pages["Start"]
            .events
            .iter()
            .flatten()
            .any(|e| e.signal == "eGUIEvent_OnAB" && e.callback.as_deref() == Some("gotoMain"))
    );
    assert_eq!(c.pages["Main"].id, ObjectId(2144911973));
    println!("pages={} widgets={}", c.pages.len(), c.widgets.len());
}
