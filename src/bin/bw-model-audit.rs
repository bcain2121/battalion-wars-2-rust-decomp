//! Read-only model structure audit. Full decoded meshes stay in ignored local/.
use bw_rust::{
    archive::{chunks, named_payload},
    model,
};
use std::{fs::File, io::Read};
fn main() {
    let mut bytes = Vec::new();
    File::open(
        std::env::args_os()
            .nth(1)
            .expect("Usage: bw-model-audit ARCHIVE.res"),
    )
    .expect("archive")
    .take(256 * 1024 * 1024 + 1)
    .read_to_end(&mut bytes)
    .unwrap();
    assert!(bytes.len() <= 256 * 1024 * 1024);
    if let Some(name) = std::env::args().nth(2) {
        let model = model::from_archive(&bytes, &name).expect("decode selected model");
        println!("{}", serde_json::to_string_pretty(&model).unwrap());
        return;
    }
    let mut results = Vec::new();
    for chunk in chunks(&bytes).unwrap() {
        if chunk.tag != *b"LDOM" {
            continue;
        }
        let (name, body) = named_payload(chunk.payload).unwrap();
        match model::parse(name, body) {
            Ok(model) => {
                results.push(serde_json::json!({"name":name,"nodes":model.nodes.len(),"surfaces":model.nodes.iter().map(|n|n.surfaces.len()).sum::<usize>(),"triangles":model.nodes.iter().flat_map(|n|&n.surfaces).map(|s|s.indices.len()/3).sum::<usize>(),"skinned_nodes":model.nodes.iter().filter(|n|n.has_skin_data).count(),"textures":model.nodes.iter().flat_map(|n|&n.materials).flat_map(|m|m.iter()).filter(|s|!s.is_empty()).collect::<std::collections::BTreeSet<_>>() }));
            }
            Err(error) => results.push(serde_json::json!({"name":name,"error":error})),
        }
    }
    println!("{}", serde_json::to_string_pretty(&results).unwrap());
}
