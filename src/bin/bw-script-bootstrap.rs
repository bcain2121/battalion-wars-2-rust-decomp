//! Capability-limited bootstrap audit, not a restored mission runtime.
use bw_rust::{
    ObjectGraph,
    archive::{chunks, named_payload},
    lua50::{self, Bootstrap, Value},
    parse_objects,
};
use std::{collections::BTreeMap, fs};
fn main() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("local/extracted/files/Data/CompoundFiles");
    let mut graph = ObjectGraph::default();
    for name in ["SP_0.1_Level.xml", "SP_0.1_Level_preload.xml"] {
        graph
            .merge(parse_objects(&fs::read_to_string(root.join(name)).unwrap()).unwrap())
            .unwrap();
    }
    let archive = fs::read(root.join("SP_0.1_Level.res")).unwrap();
    let mut scripts = BTreeMap::new();
    for c in chunks(&archive).unwrap() {
        if c.tag == *b"PRCS" {
            let (n, b) = named_payload(c.payload).unwrap();
            assert!(
                scripts
                    .insert(n.to_string(), lua50::parse(b).unwrap())
                    .is_none()
            );
        }
    }
    let mut vm = Bootstrap::new();
    for (name, p) in &scripts {
        vm.execute(name, p, &graph, 100000)
            .unwrap_or_else(|e| panic!("Bootstrap {name}: {e}"));
    }
    let tables = vm
        .globals
        .iter()
        .filter_map(|(n, v)| {
            if let Value::Table(i) = v {
                Some((n.clone(), vm.tables[*i].len()))
            } else {
                None
            }
        })
        .collect::<BTreeMap<_, _>>();
    let functions = vm
        .globals
        .iter()
        .filter_map(|(n, v)| matches!(v, Value::Function { .. }).then_some(n.clone()))
        .collect::<Vec<_>>();
    println!("{}",serde_json::to_string_pretty(&serde_json::json!({"scope":"Root/bootstrap chunks only. No mission bodies or scheduler executed.","host_policy":"RegisterReflectionId returns a validated symbolic ObjectId, not a claim about original pointer ABI. All other calls fail closed.","scripts":scripts.len(),"instructions_executed":vm.instructions,"registration_calls":vm.registrations,"tables":tables,"functions_registered":functions})).unwrap());
}
