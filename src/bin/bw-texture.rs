//! Decode a supported original texture's top mip to an exclusively-created local TGA.
use bw_rust::texture::decode_named;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    process::ExitCode,
};
fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("Usage: bw-texture ARCHIVE.res TEXTURE_NAME OUTPUT.tga".into());
    }
    let mut data = Vec::new();
    const MAX: u64 = 256 * 1024 * 1024;
    File::open(&args[0])
        .map_err(|e| e.to_string())?
        .take(MAX + 1)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    if data.len() as u64 > MAX {
        return Err("Archive too large".into());
    }
    let texture = decode_named(&data, &args[1])?;
    let mut header = [0u8; 18];
    header[2] = 2;
    header[12..14].copy_from_slice(&(texture.width as u16).to_le_bytes());
    header[14..16].copy_from_slice(&(texture.height as u16).to_le_bytes());
    header[16] = 32;
    header[17] = 0x28;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[2])
        .map_err(|e| e.to_string())?;
    output.write_all(&header).map_err(|e| e.to_string())?;
    for pixel in texture.rgba.chunks_exact(4) {
        output
            .write_all(&[pixel[2], pixel[1], pixel[0], pixel[3]])
            .map_err(|e| e.to_string())?;
    }
    println!(
        "Decoded {}: {}x{} to {}",
        args[1], texture.width, texture.height, args[2]
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
