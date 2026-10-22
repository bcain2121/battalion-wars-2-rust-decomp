use bw_rust::{MAX_XML_BYTES, ObjectGraph, parse_objects};
use std::{fs::File, io::Read, process::ExitCode};

fn run() -> Result<(), String> {
    let paths: Vec<_> = std::env::args_os().skip(1).collect();
    if paths.is_empty() {
        return Err("Usage: bw-rust LEVEL.xml [PRELOAD.xml ...] (uncompressed XML)".into());
    }
    let mut graph = ObjectGraph::default();
    for path in paths {
        let mut text = String::new();
        File::open(&path)
            .map_err(|e| format!("{path:?}: {e}"))?
            .take(MAX_XML_BYTES as u64 + 1)
            .read_to_string(&mut text)
            .map_err(|e| e.to_string())?;
        graph.merge(parse_objects(&text)?)?;
    }
    let summary = graph.summary()?;
    println!(
        "{}",
        serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?
    );
    if !summary.unresolved_references.is_empty() {
        return Err("Unresolved object references".into());
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
