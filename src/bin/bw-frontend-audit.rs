//! Read-only frontend graph audit; does not execute callbacks or launch gameplay.
use bw_rust::{MAX_XML_BYTES, ObjectGraph, frontend::Catalog, parse_objects};
use std::{io::Read, process::ExitCode};
fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() > 1 {
        return Err("Usage: bw-frontend-audit [COMPOUND_FILES_DIRECTORY]".into());
    }
    let root = args
        .first()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("local/extracted/files/Data/CompoundFiles"));
    let mut graph = ObjectGraph::default();
    for name in ["Frontend2_Level.xml", "Frontend2_Level_preload.xml"] {
        let mut xml = String::new();
        std::fs::File::open(root.join(name))
            .map_err(|e| e.to_string())?
            .take(MAX_XML_BYTES as u64 + 1)
            .read_to_string(&mut xml)
            .map_err(|e| e.to_string())?;
        graph.merge(parse_objects(&xml)?)?;
    }
    let catalog = Catalog::load(&graph)?;
    let pages:Vec<_>=catalog.pages.values().map(|p|serde_json::json!({"id":p.id,"script":p.script,"widget_slots":p.children,"events":p.events})).collect();
    println!("{}",serde_json::to_string_pretty(&serde_json::json!({"page_count":pages.len(),"widget_count":catalog.widgets.len(),"pages":pages,"callbacks_executed":false,"menus_rendered":false,"mission_launched":false})).map_err(|e|e.to_string())?);
    Ok(())
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
