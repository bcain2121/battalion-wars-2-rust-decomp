//! Execute two original initializer bodies under the narrow in-memory host.
use bw_rust::{
    ObjectGraph,
    archive::{chunks, named_payload},
    lua50::{self, Bootstrap, Value},
    mission_vm::{Host, Status, Thread},
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
            scripts.insert(n.to_string(), lua50::parse(b).unwrap());
        }
    }
    let mut vm = Bootstrap::new();
    for (n, p) in &scripts {
        vm.execute(n, p, &graph, 100000).unwrap();
    }
    let mut host = Host::from_graph(&graph).unwrap();
    host.install(&mut vm);
    let mut report = Vec::new();
    for name in ["ArtVariables", "Gambit_Part_1_Global_Variables"] {
        let mut thread =
            Thread::new(name, scripts[name].children[0].clone(), &[Value::Nil]).unwrap();
        assert!(matches!(
            thread.resume(&mut vm, &mut host, 100000).unwrap(),
            Status::Complete(_)
        ));
        report.push(
            serde_json::json!({"name":name,"instructions":thread.instructions,"complete":true}),
        );
    }
    let globals = vm
        .globals
        .iter()
        .filter_map(|(k, v)| {
            if let Value::Number(n) = v {
                Some((k.clone(), *n))
            } else {
                None
            }
        })
        .collect::<BTreeMap<_, _>>();
    println!("{}",serde_json::to_string_pretty(&serde_json::json!({"scope":"Two original initializer bodies only, not a playable mission","initializers":report,"numeric_globals_count":globals.len(),"debug_events":host.debug.len(),"bounds_message_configured":host.bounds_message.is_some(),"logical_time":host.time,"warning":"Host contracts are reimplementation adapters. Scheduler order and full Lua behavior are not original-runtime validated."})).unwrap());
}
