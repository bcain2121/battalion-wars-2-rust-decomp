use bw_rust::{
    Field, Object, ObjectGraph, ObjectId,
    vehicle_profile::{Bank, Engine, Override},
};
use std::collections::BTreeMap;
fn object(id: u64, kind: &str, fields: &[(&str, &str)]) -> Object {
    Object {
        id: ObjectId(id),
        kind: kind.into(),
        attributes: BTreeMap::new(),
        fields: fields
            .iter()
            .map(|(k, v)| Field {
                kind: "Attribute".into(),
                attributes: BTreeMap::from([("name".into(), (*k).into())]),
                items: vec![(*v).into()],
                original_xml: String::new(),
            })
            .collect(),
    }
}
fn fixture() -> ObjectGraph {
    let fields = [
        ("attackSpeed", "10"),
        ("patrolSpeed", "8"),
        ("reverseSpeed", "6"),
        ("driveAccln", "16"),
        ("brakingAccln", "16"),
        ("maxTurnRate", "0.25"),
        ("turnAccln", "0.1"),
        ("turnDamping", "0.25"),
        ("minSpeedToTurn", "0"),
        ("mPhysicsParams", "6"),
    ];
    let objects = [
        object(
            1,
            "cGroundVehicle",
            &[("mBase", "2"), ("mPhysEngine", "physEngODE")],
        ),
        object(
            2,
            "cGroundVehicleBase",
            &[
                ("mPhysicsBase", "3"),
                ("mPhysicsBaseNewEngine", "4"),
                ("mpModel", "5"),
            ],
        ),
        object(3, "cGroundVehiclePhysicsBase", &fields),
        object(4, "cGroundVehiclePhysicsBase", &fields),
        object(5, "cNodeHierarchyResource", &[("mName", "tank")]),
        object(6, "cReflectedPhysicsParams", &[]),
    ];
    ObjectGraph {
        objects: objects.into_iter().map(|o| (o.id, o)).collect(),
    }
}
fn set(g: &mut ObjectGraph, id: u64, name: &str, value: &str) {
    g.objects
        .get_mut(&ObjectId(id))
        .unwrap()
        .fields
        .iter_mut()
        .find(|f| f.attributes["name"] == name)
        .unwrap()
        .items[0] = value.into();
}
#[test]
fn engine_selects_correct_link_and_explicit_override() {
    let mut graph = fixture();
    set(&mut graph, 4, "attackSpeed", "11");
    let a = Bank::load(&graph, Override::Authored).unwrap();
    assert_eq!(a.units[&ObjectId(1)].profile.id, ObjectId(3));
    assert_eq!(a.units[&ObjectId(1)].profile.attack_speed, 10.0);
    let b = Bank::load(&graph, Override::Force(Engine::Ageia)).unwrap();
    assert_eq!(b.units[&ObjectId(1)].profile.id, ObjectId(4));
    assert_eq!(b.units[&ObjectId(1)].profile.attack_speed, 11.0);
    assert_eq!(b.units[&ObjectId(1)].authored_engine, Engine::Ode);
    set(&mut graph, 1, "mPhysEngine", "physEngAgeia");
    assert_eq!(
        Bank::load(&graph, Override::Authored).unwrap().units[&ObjectId(1)].engine,
        Engine::Ageia
    );
    assert_eq!(
        Bank::load(&graph, Override::Force(Engine::Ode))
            .unwrap()
            .units[&ObjectId(1)]
            .profile
            .id,
        ObjectId(3)
    );
    graph.objects.insert(
        ObjectId(7),
        object(
            7,
            "cGroundVehicle",
            &[("mBase", "2"), ("mPhysEngine", "physEngAgeia")],
        ),
    );
    let b = Bank::load(&graph, Override::Authored).unwrap();
    assert_eq!(b.profiles.len(), 1);
    assert!(std::sync::Arc::ptr_eq(
        &b.units[&ObjectId(1)].profile,
        &b.units[&ObjectId(7)].profile
    ));
}
#[test]
fn invalid_vehicle_profiles_fail_closed() {
    for (id, key, bad) in [
        (1, "mPhysEngine", "physEngMax"),
        (2, "mPhysicsBase", "999"),
        (3, "attackSpeed", "NaN"),
        (3, "driveAccln", "-1"),
        (3, "brakingAccln", "0"),
        (3, "turnDamping", "inf"),
        (5, "mName", ""),
    ] {
        let mut g = fixture();
        set(&mut g, id, key, bad);
        assert!(Bank::load(&g, Override::Authored).is_err(), "{key}: {bad}");
    }
    let mut g = fixture();
    g.objects.get_mut(&ObjectId(3)).unwrap().kind = "cWaterVehiclePhysicsBase".into();
    assert!(Bank::load(&g, Override::Authored).is_err());
    let mut g = fixture();
    let p = g.objects.get_mut(&ObjectId(3)).unwrap();
    p.fields.push(p.fields[0].clone());
    assert!(Bank::load(&g, Override::Authored).is_err());
    let mut g = fixture();
    g.objects.remove(&ObjectId(6));
    assert!(Bank::load(&g, Override::Authored).is_err());
}
#[cfg(feature = "viewer")]
mod geometry {
    use bw_rust::{
        model::{Model, Node, Surface, Vertex},
        model_geometry::{bind_transforms, physics_nodes as physics_meshes},
    };
    fn node(name: &str, parent: Option<usize>, offset: [f32; 3]) -> Node {
        let mut transform = [0.0; 11];
        transform[..3].copy_from_slice(&offset);
        transform[6] = 1.0;
        Node {
            name: name.into(),
            authored_kind: 0,
            parent,
            transform,
            scale: 1.0,
            bounds: Some(bw_rust::model::Bounds {
                min: [0.0; 3],
                max: [1.0, 1.0, 0.0],
            }),
            materials: vec![],
            material_blend_modes: vec![],
            surfaces: vec![],
            tags: vec![],
            has_skin_data: false,
            inverse_bind: vec![],
        }
    }
    fn fixture() -> Model {
        let mut child = node("PHYSICS_COLLIDE", Some(0), [0., 2., 0.]);
        child.surfaces.push(Surface {
            material: 0,
            lod: 0,
            bone_palette: vec![],
            vertices: [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]]
                .map(|position| Vertex {
                    position,
                    normal: None,
                    uv: [0.; 2],
                    matrix: None,
                })
                .into(),
            indices: vec![0, 1, 2],
        });
        Model {
            name: "fixture".into(),
            version: [0; 2],
            source: "synthetic".into(),
            header_sphere: [0.0; 4],
            nodes: vec![node("root", None, [10., 0., 0.]), child],
            hierarchy_links: vec![(0, 1)],
        }
    }
    fn body_fixture() -> Model {
        let mut m = fixture();
        m.nodes[1].bounds.as_mut().unwrap().max[2] = 1.0;
        m
    }
    fn body_node(m: &mut Model, name: &str, parent: usize, offset: [f32; 3]) -> usize {
        let index = m.nodes.len();
        let mut n = node(name, Some(parent), offset);
        n.bounds.as_mut().unwrap().max[2] = 1.0;
        m.nodes.push(n);
        m.hierarchy_links.push((parent, index));
        index
    }
    #[test]
    fn native_body_chain_obeys_link_order_not_node_order_or_bind_corners() {
        use bw_rust::vehicle_boxes::groups;
        let mut m = body_fixture();
        m.nodes[1].scale = 4.0;
        body_node(&mut m, "PHYSICS_COLLIDE02", 1, [1., 3., 0.]);
        body_node(&mut m, "PHYSICS_COLLIDE06", 0, [100., 0., 0.]);
        let g = groups(&m).unwrap();
        assert_eq!(g.chassis.len(), 2);
        assert_eq!(g.chassis[0].position, [0., 2., 0.]);
        assert_eq!(g.chassis[1].position, [1., 5., 0.]);
        assert_eq!(g.chassis[0].dimensions, [1.; 3]);
        assert_eq!(g.unselected_physics_nodes, vec![3]);
        m.hierarchy_links = vec![(0, 3), (0, 1), (1, 2)];
        let g = groups(&m).unwrap();
        assert_eq!(g.chassis.len(), 1);
        assert_eq!(g.chassis[0].node_index, 3);
        assert_eq!(g.unselected_physics_nodes, vec![1, 2]);
    }
    #[test]
    fn native_turret_and_barrel_offsets_include_selected_seat() {
        use bw_rust::vehicle_boxes::groups;
        let mut m = body_fixture();
        let seat = body_node(&mut m, "SEAT_A", 0, [0., 1., 0.]);
        let turret = body_node(&mut m, "TURRET", seat, [0., 2., 0.]);
        body_node(&mut m, "PHYSICS_COLLIDE02", turret, [0., 3., 0.]);
        let barrel = body_node(&mut m, "BARREL", turret, [0., 4., 0.]);
        body_node(&mut m, "PHYSICS_COLLIDE03", barrel, [0., 5., 0.]);
        let g = groups(&m).unwrap();
        assert_eq!(g.seat_node, Some(seat));
        assert_eq!(g.turret[0].position, [0., 6., 0.]);
        assert_eq!(g.barrel[0].position, [0., 12., 0.]);
        m.nodes[turret].name = "DECOR".into();
        m.nodes[barrel].parent = Some(seat);
        m.hierarchy_links
            .iter_mut()
            .find(|(_, c)| *c == barrel)
            .unwrap()
            .0 = seat;
        let g = groups(&m).unwrap();
        assert!(g.turret.is_empty());
        assert_eq!(g.barrel[0].position, [0., 10., 0.]);
    }
    #[test]
    fn malformed_body_metadata_and_over_capacity_fail_closed() {
        use bw_rust::vehicle_boxes::groups;
        let mut m = body_fixture();
        m.hierarchy_links.clear();
        assert!(groups(&m).is_err());
        for name in [
            "dummy_PHYSICS_COLLIDE",
            "PHYSICS_COLLIDE_extra",
            "physics_collide",
            "PHYSICS_COLLIDE999999",
        ] {
            let mut m = body_fixture();
            m.nodes[1].name = name.into();
            assert!(groups(&m).is_err());
        }
        let mut m = body_fixture();
        m.nodes[1].bounds = None;
        assert!(groups(&m).is_err());
        let mut m = body_fixture();
        m.nodes[1].bounds.as_mut().unwrap().max[2] = 0.;
        assert!(groups(&m).is_err());
        let mut m = body_fixture();
        m.nodes[1].bounds.as_mut().unwrap().min[0] = -f32::MAX;
        m.nodes[1].bounds.as_mut().unwrap().max[0] = f32::MAX;
        assert!(groups(&m).is_err());
        let mut m = body_fixture();
        for parent in 1..9 {
            body_node(&mut m, "PHYSICS_COLLIDE", parent, [0.; 3]);
        }
        assert!(groups(&m).is_err());
    }
    #[test]
    fn native_wheel_slots_preserve_holes_and_a_variant_precedence() {
        use bw_rust::vehicle_wheels::slots;
        let mut m = fixture();
        m.nodes[1].name = "L_WHEEL_A_S".into();
        m.nodes[1].scale = 4.0; // Quantized vertex scale is not applied to XOBB radius.
        m.nodes.push(node("R_INTERP_BC", Some(0), [1.0, 0.0, 3.0]));
        let s = slots(&m).unwrap();
        assert_eq!(s.len(), 22);
        assert_eq!(s[0].requested_kind, 97);
        assert!(s[0].alternate_a);
        assert_eq!(s[0].part.as_ref().unwrap().radius, 0.5);
        assert_eq!(
            s[0].part.as_ref().unwrap().bind_translation,
            [10.0, 2.0, 0.0]
        );
        assert!(s[1].part.is_none());
        assert_eq!(s[12].descriptor_offset, 0x300);
        assert_eq!(s[18].part.as_ref().unwrap().name, "R_INTERP_BC");
        m.nodes.push(node("L_WHEEL_A", Some(0), [2.0, 0.0, 0.0]));
        let s = slots(&m).unwrap();
        assert!(!s[0].alternate_a);
        assert_eq!(s[0].part.as_ref().unwrap().node_index, 3);
    }
    #[test]
    fn ambiguous_indexed_and_invalid_wheel_metadata_is_rejected() {
        use bw_rust::vehicle_wheels::slots;
        for name in [
            "L_WHEEL_A00",
            "l_wheel_a",
            "prefix_L_WHEEL_A",
            "L_WHEEL_A_extra",
        ] {
            let mut m = fixture();
            m.nodes[1].name = name.into();
            assert!(slots(&m).is_err());
        }
        let mut m = fixture();
        m.nodes[1].name = "L_WHEEL_A".into();
        m.nodes.push(node("L_WHEEL_A", Some(0), [0.0; 3]));
        assert!(slots(&m).is_err());
        m.nodes.pop();
        m.nodes[1].bounds = None;
        assert!(slots(&m).is_err());
        let mut m = fixture();
        m.nodes[1].name = "L_WHEEL_A".into();
        m.nodes[1].bounds.as_mut().unwrap().min[0] = 2.0;
        assert!(slots(&m).is_err());
        m.nodes[1].bounds.as_mut().unwrap().min[0] = f32::NAN;
        assert!(slots(&m).is_err());
        m.nodes[1].bounds = Some(bw_rust::model::Bounds {
            min: [0.0, -f32::MAX, 0.0],
            max: [1.0, f32::MAX, 1.0],
        });
        assert!(slots(&m).is_err());
    }
    #[test]
    fn physics_mesh_keeps_parent_transform_and_identity() {
        let m = fixture();
        let meshes = physics_meshes(&m).unwrap();
        assert_eq!(meshes.len(), 1);
        assert_eq!(meshes[0].node_index, 1);
        assert_eq!(meshes[0].parent, Some(0));
        assert_eq!(meshes[0].min, [10., 2., 0.]);
        assert_eq!(meshes[0].max, [11., 3., 0.]);
        assert_eq!(meshes[0].marker_triangles[0][1], [11., 2., 0.]);
        let transforms = bind_transforms(&m).unwrap();
        assert_eq!(
            transforms[1]
                .transform_point3(bevy::math::Vec3::ZERO)
                .to_array(),
            [10., 2., 0.]
        );
    }
    #[test]
    fn malformed_hierarchies_and_geometry_rejected() {
        let mut m = fixture();
        m.nodes[0].parent = Some(1);
        assert!(bind_transforms(&m).is_err());
        m.nodes[0].parent = Some(99);
        assert!(bind_transforms(&m).is_err());
        m.nodes[0].parent = None;
        m.nodes[0].transform[0] = f32::NAN;
        assert!(bind_transforms(&m).is_err());
        let mut m = fixture();
        m.nodes[1].surfaces[0].indices[0] = 99;
        assert!(physics_meshes(&m).is_err());
        let mut m = fixture();
        m.nodes[1].has_skin_data = true;
        assert!(physics_meshes(&m).is_err());
        let mut m = fixture();
        m.nodes[1].surfaces[0].vertices[0].matrix = Some(0);
        assert!(physics_meshes(&m).is_err());
        let mut m = fixture();
        m.nodes[1].surfaces.clear();
        assert!(physics_meshes(&m).is_err());
        let mut m = fixture();
        m.nodes[1].name = "visual_mesh".into();
        assert!(physics_meshes(&m).is_err());
    }
}
#[test]
#[ignore = "requires local original mission extraction"]
fn original_ground_vehicle_profiles() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("local/extracted/files/Data/CompoundFiles");
    let mut graph = ObjectGraph::default();
    for name in ["SP_0.1_Level.xml", "SP_0.1_Level_preload.xml"] {
        graph
            .merge(
                bw_rust::parse_objects(&std::fs::read_to_string(root.join(name)).unwrap()).unwrap(),
            )
            .unwrap();
    }
    let bank = Bank::load(&graph, Override::Authored).unwrap();
    assert_eq!(bank.units.len(), 30);
    assert_eq!(bank.profiles.len(), 3);
    assert!(bank.units.values().all(|b| b.engine == Engine::Ode));
    let tank = &bank.units[&ObjectId(550040155)];
    assert_eq!(tank.profile.id, ObjectId(2138048309));
    assert_eq!(tank.profile.attack_speed, 10.0);
    assert_eq!(tank.profile.patrol_speed, 10.0);
    let alternative = Bank::load(&graph, Override::Force(Engine::Ageia)).unwrap();
    assert_eq!(
        alternative.units[&ObjectId(550040155)].profile.id,
        ObjectId(50034654)
    );
    assert_eq!(
        alternative.units[&ObjectId(550040155)].profile.attack_speed,
        11.0
    );
    #[cfg(feature = "viewer")]
    {
        let bytes = std::fs::read(root.join("SP_0.1_Level.res")).unwrap();
        let names = bank
            .units
            .values()
            .map(|b| b.model.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(names.len(), 3);
        for name in names {
            let model = bw_rust::model::from_archive(&bytes, name).unwrap();
            let meshes = bw_rust::model_geometry::physics_nodes(&model).unwrap();
            assert!(!meshes.is_empty());
            let groups = bw_rust::vehicle_boxes::groups(&model).unwrap();
            let expected_boxes = match name {
                "VSHTNKH" => (1, 1, 1, 1),
                "VUBSTAH" => (1, 1, 1, 0),
                "VUHTNKH" => (2, 1, 1, 0),
                _ => panic!("Unknown original model"),
            };
            assert_eq!(
                (
                    groups.chassis.len(),
                    groups.turret.len(),
                    groups.barrel.len(),
                    groups.unselected_physics_nodes.len()
                ),
                expected_boxes
            );
            println!("ORIGINAL_BODY_GROUPS model={name} {groups:?}");
            let slots = bw_rust::vehicle_wheels::slots(&model).unwrap();
            assert_eq!(slots.len(), 22);
            assert!(slots[..12].iter().any(|s| s.part.is_some()));
            // Native assembly allows absent interpolation nodes; do not synthesize them.
            let expected = match name {
                "VSHTNKH" => (6, 0),
                "VUBSTAH" => (4, 0),
                "VUHTNKH" => (6, 4),
                _ => panic!("Unexpected original model"),
            };
            assert_eq!(
                (
                    slots[..12].iter().filter(|s| s.part.is_some()).count(),
                    slots[12..].iter().filter(|s| s.part.is_some()).count()
                ),
                expected
            );
            assert!(
                slots
                    .iter()
                    .filter_map(|s| s.part.as_ref())
                    .all(|p| p.radius > 0.0)
            );
            println!(
                "ORIGINAL_VEHICLE_WHEEL_SLOTS model={name} wheels={} interpolation={} dynamics=false",
                slots[..12].iter().filter(|s| s.part.is_some()).count(),
                slots[12..].iter().filter(|s| s.part.is_some()).count()
            );
            println!(
                "ORIGINAL_VEHICLE_MESH model={name} nodes={} triangles={}",
                meshes.len(),
                meshes
                    .iter()
                    .map(|m| m.marker_triangles.len())
                    .sum::<usize>()
            );
        }
    }
    println!("ORIGINAL_VEHICLE_PROFILE_PASS units=30 profiles=3 dynamics=false");
}
