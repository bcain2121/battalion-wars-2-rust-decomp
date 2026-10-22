//! Opt-in local-disc integration checks. No proprietary fixtures in the repository.
use bw_rust::{
    animation,
    archive::{chunks, named_payload},
    model,
};
fn archive() -> Vec<u8> {
    std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/local/extracted/files/Data/CompoundFiles/SP_0.1_Level.res"
    ))
    .expect("Local first-mission archive required")
}
#[test]
#[ignore = "requires local disc extraction"]
fn original_models() {
    let archive = archive();
    let mut count = 0;
    let mut triangles = 0;
    for c in chunks(&archive).unwrap() {
        if c.tag != *b"LDOM" {
            continue;
        }
        let (n, b) = named_payload(c.payload).unwrap();
        let m = model::parse(n, b).unwrap();
        triangles += m
            .nodes
            .iter()
            .flat_map(|n| &n.surfaces)
            .map(|s| s.indices.len() / 3)
            .sum::<usize>();
        count += 1;
    }
    assert_eq!(count, 116);
    assert!(triangles > 200000);
    println!("models={count} triangles_including_degenerate={triangles}");
}
#[test]
#[ignore = "requires local disc extraction"]
fn original_animation_tables() {
    let archive = archive();
    let mut clips = 0;
    let mut rotations = 0;
    let mut tails = 0;
    for c in chunks(&archive).unwrap() {
        if c.tag != *b"MINA" {
            continue;
        }
        let (n, b) = named_payload(c.payload).unwrap();
        let a = animation::parse(n, b).unwrap();
        rotations += a
            .tracks
            .iter()
            .map(|t| t.packed_rotations.len())
            .sum::<usize>();
        tails += usize::from(!a.trailing_data.is_empty());
        clips += 1;
    }
    assert!(clips > 50);
    assert_eq!(rotations, 245350);
    println!(
        "animation_clips={clips} rotation_samples={rotations} clips_with_uninterpreted_trailing_data={tails}"
    );
}
#[test]
#[ignore = "requires local disc extraction"]
fn original_script_bootstrap() {
    use bw_rust::{
        ObjectGraph,
        lua50::{self, Bootstrap, Value},
        parse_objects,
    };
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("local/extracted/files/Data/CompoundFiles");
    let mut graph = ObjectGraph::default();
    for name in ["SP_0.1_Level.xml", "SP_0.1_Level_preload.xml"] {
        graph
            .merge(parse_objects(&std::fs::read_to_string(dir.join(name)).unwrap()).unwrap())
            .unwrap();
    }
    let archive = archive();
    let mut vm = Bootstrap::new();
    let mut count = 0;
    let mut entity = None;
    for c in chunks(&archive).unwrap() {
        if c.tag != *b"PRCS" {
            continue;
        }
        let (n, b) = named_payload(c.payload).unwrap();
        let p = lua50::parse(b).unwrap();
        vm.execute(n, &p, &graph, 100000).unwrap();
        if n == "EntityInitialise" {
            entity = Some(p);
        }
        count += 1;
    }
    assert_eq!(count, 54);
    assert_eq!(vm.registrations, 741);
    assert_eq!(vm.tables.len(), 42);
    assert_eq!(
        vm.globals
            .values()
            .filter(|v| matches!(v, Value::Function { .. }))
            .count(),
        53
    );
    let tables = vm.tables.clone();
    vm.execute("EntityInitialise", &entity.unwrap(), &graph, 100000)
        .unwrap();
    assert_eq!(
        vm.tables, tables,
        "Initialization must retain existing tables and object handles"
    );
}
#[cfg(feature = "viewer")]
#[test]
#[ignore = "requires local disc extraction"]
fn original_skin_bind_pose() {
    let archive = archive();
    let mut models = 0;
    let mut vertices = 0;
    let mut max_error = 0.0_f32;
    for c in chunks(&archive).unwrap() {
        if c.tag != *b"LDOM" {
            continue;
        }
        let (n, b) = named_payload(c.payload).unwrap();
        let m = model::parse(n, b).unwrap();
        if !m.nodes.iter().any(|n| n.has_skin_data) {
            continue;
        }
        let globals = bw_rust::skinning::pose(&m, None).unwrap();
        models += 1;
        for (ni, node) in m.nodes.iter().enumerate() {
            if !node.has_skin_data {
                continue;
            }
            for (si, surface) in node.surfaces.iter().enumerate() {
                let decoded = bw_rust::skinning::deform(&m, &globals, ni, si).unwrap();
                for (a, b) in decoded.iter().zip(&surface.vertices) {
                    let err = a
                        .iter()
                        .zip(b.position)
                        .map(|(a, b)| (a - b).abs())
                        .fold(0.0_f32, f32::max);
                    max_error = max_error.max(err);
                    vertices += 1;
                }
            }
        }
    }
    println!("skinned_models={models} vertices={vertices} max_bind_pose_error={max_error}");
    assert!(models > 0);
    assert!(max_error < 0.001);
}
