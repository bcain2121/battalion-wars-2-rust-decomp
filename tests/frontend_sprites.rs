use bw_rust::{
    ObjectGraph, ObjectId,
    frontend_sprites::{OpenState, Sprites},
    lua50::{Bootstrap, Value},
    parse_objects,
};
fn fixture() -> (Sprites, Bootstrap) {
    let mut graph = ObjectGraph::default();
    graph.merge(parse_objects(r#"<Instances>
      <Object type="cSprite" id="99"/>
      <Object type="cScriptSprite" id="10"><Pointer name="mSprite"><Item>99</Item></Pointer></Object>
      <Object type="cScriptSprite" id="11"><Pointer name="mSprite"><Item>99</Item></Pointer></Object>
      <Object type="cScriptSprite" id="12"><Pointer name="mSprite"><Item>0</Item></Pointer></Object>
    </Instances>"#).unwrap()).unwrap();
    let mut vm = Bootstrap::new();
    for id in [10, 11, 12] {
        let table = vm.tables.len();
        vm.tables.push(Default::default());
        vm.table_set(table, Value::Number(1.0), Value::Object(ObjectId(id)))
            .unwrap();
    }
    (Sprites::from_graph(&graph).unwrap(), vm)
}
#[test]
fn shared_sprite_identity_close_is_table_local_and_reopen_works() {
    let (mut sprites, mut vm) = fixture();
    let key = Value::String("objectPtr".into());
    assert_eq!(sprites.state(ObjectId(99)).unwrap(), None);
    sprites.open(&mut vm, &[Value::Table(0)]).unwrap();
    sprites.open(&mut vm, &[Value::Table(1)]).unwrap();
    let identity = vm.table_get(0, &key).unwrap();
    assert!(matches!(identity, Value::LightUserdata(h) if h != 0 && h != 99 && h != 10));
    assert_eq!(vm.table_get(1, &key).unwrap(), identity);
    let state = Some(OpenState {
        field_5c: 0.0,
        byte_70: 0,
    });
    assert_eq!(sprites.state(ObjectId(99)).unwrap(), state);
    sprites.close(&mut vm, &[Value::Table(0)]).unwrap();
    assert_eq!(vm.table_get(0, &key).unwrap(), Value::LightUserdata(0));
    assert_eq!(vm.table_get(1, &key).unwrap(), identity);
    assert_eq!(sprites.state(ObjectId(99)).unwrap(), state);
    sprites.open(&mut vm, &[Value::Table(0)]).unwrap();
    assert_eq!(vm.table_get(0, &key).unwrap(), identity);
}
#[test]
fn known_null_and_unresolved_descriptors_are_distinct() {
    let (mut sprites, mut vm) = fixture();
    sprites.open(&mut vm, &[Value::Table(2)]).unwrap();
    assert_eq!(
        vm.table_get(2, &Value::String("objectPtr".into())).unwrap(),
        Value::Nil
    );
    for value in [
        Value::Object(ObjectId(999)),
        Value::Number(1.5),
        Value::String("unresolved".into()),
    ] {
        vm.table_set(2, Value::Number(1.0), value).unwrap();
        assert!(sprites.open(&mut vm, &[Value::Table(2)]).is_err());
        assert_eq!(
            vm.table_get(2, &Value::String("objectPtr".into())).unwrap(),
            Value::Nil
        );
        assert_eq!(sprites.state(ObjectId(99)).unwrap(), None);
    }
    assert!(sprites.open(&mut vm, &[Value::Table(999)]).is_err());
    assert!(sprites.close(&mut vm, &[]).is_err());
}
#[test]
fn clones_do_not_share_mutable_sprite_state() {
    let (sprites, vm) = fixture();
    let mut changed = sprites.clone();
    let mut changed_vm = vm.clone();
    changed.open(&mut changed_vm, &[Value::Table(0)]).unwrap();
    assert_eq!(sprites.state(ObjectId(99)).unwrap(), None);
    assert_eq!(
        vm.table_get(0, &Value::String("objectPtr".into())).unwrap(),
        Value::Nil
    );
}
