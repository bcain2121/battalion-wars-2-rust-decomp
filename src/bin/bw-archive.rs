use std::{fs::File, io::Read, process::ExitCode};
fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 1 {
        return Err("Usage: bw-archive UNCOMPRESSED_RESOURCE_OR_TERRAIN".into());
    }
    let mut data = Vec::new();
    const MAX: u64 = 256 * 1024 * 1024;
    File::open(&args[0])
        .map_err(|e| e.to_string())?
        .take(MAX + 1)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    if data.len() as u64 > MAX {
        return Err("Archive size limit exceeded".into());
    }
    let result = bw_rust::archive::inventory(&data)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?
    );
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
