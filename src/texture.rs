//! Initial Nintendo CMPR decoder: 8x8 macrotiles containing four 4x4 blocks.
//! RGB interpolation is an initial BC1-style approximation, not GPU parity proof.
use crate::Result;

fn rgb565(value: u16) -> [u8; 4] {
    let r = ((value >> 11) & 31) as u8;
    let g = ((value >> 5) & 63) as u8;
    let b = (value & 31) as u8;
    [
        (r << 3) | (r >> 2),
        (g << 2) | (g >> 4),
        (b << 3) | (b >> 2),
        255,
    ]
}

pub fn decode_cmpr(data: &[u8], width: usize, height: usize) -> Result<Vec<u8>> {
    if width == 0 || height == 0 || width > 4096 || height > 4096 {
        return Err("Unsupported texture dimensions".into());
    }
    let expected = width.div_ceil(8) * height.div_ceil(8) * 32;
    if data.len() != expected {
        return Err("CMPR byte count mismatch".into());
    }
    let mut out = vec![0; width * height * 4];
    let mut offset = 0;
    for ty in (0..height).step_by(8) {
        for tx in (0..width).step_by(8) {
            for sub in 0..4 {
                let bytes = &data[offset..offset + 8];
                offset += 8;
                let c0 = u16::from_be_bytes([bytes[0], bytes[1]]);
                let c1 = u16::from_be_bytes([bytes[2], bytes[3]]);
                let mut colors = [rgb565(c0), rgb565(c1), [0, 0, 0, 255], [0, 0, 0, 255]];
                for channel in [0, 1, 2] {
                    let a = colors[0][channel] as u16;
                    let b = colors[1][channel] as u16;
                    if c0 > c1 {
                        colors[2][channel] = ((2 * a + b) / 3) as u8;
                        colors[3][channel] = ((a + 2 * b) / 3) as u8;
                    } else {
                        colors[2][channel] = ((a + b) / 2) as u8;
                        colors[3][channel] = colors[2][channel];
                    }
                }
                if c0 <= c1 {
                    colors[3][3] = 0;
                }
                for y in 0..4 {
                    for x in 0..4 {
                        let px = tx + (sub % 2) * 4 + x;
                        let py = ty + (sub / 2) * 4 + y;
                        if px < width && py < height {
                            let index = (bytes[4 + y] >> (6 - 2 * x)) & 3;
                            let dest = (py * width + px) * 4;
                            out[dest..dest + 4].copy_from_slice(&colors[index as usize]);
                        }
                    }
                }
            }
        }
    }
    Ok(out)
}

#[derive(Debug, Clone)]
pub struct DecodedTexture {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

/// Nintendo RGBA8: 4x4 tiles, AR plane followed by GB plane.
pub fn decode_rgba8(data: &[u8], width: usize, height: usize) -> Result<Vec<u8>> {
    if width == 0 || height == 0 || width > 4096 || height > 4096 {
        return Err("Unsupported texture dimensions".into());
    }
    if data.len() != width.div_ceil(4) * height.div_ceil(4) * 64 {
        return Err("RGBA8 byte count mismatch".into());
    }
    let mut rgba = vec![0; width * height * 4];
    let mut offset = 0;
    for ty in (0..height).step_by(4) {
        for tx in (0..width).step_by(4) {
            let tile = &data[offset..offset + 64];
            offset += 64;
            for y in 0..4 {
                for x in 0..4 {
                    if tx + x < width && ty + y < height {
                        let v = (y * 4 + x) * 2;
                        let dst = ((ty + y) * width + tx + x) * 4;
                        rgba[dst..dst + 4].copy_from_slice(&[
                            tile[v + 1],
                            tile[32 + v],
                            tile[32 + v + 1],
                            tile[v],
                        ]);
                    }
                }
            }
        }
    }
    Ok(rgba)
}

/// GX intensity and indexed formats. Layout/bit expansion cross-checked against
/// LagoLunatic's MIT texlib; see docs/references/texlib-MIT.txt.
#[derive(Clone, Copy, Debug)]
pub enum TiledFormat {
    I4,
    I8,
    Ia4,
    Ia8,
    C4,
    C8,
}
pub fn decode_tiled(
    data: &[u8],
    width: usize,
    height: usize,
    format: TiledFormat,
    palette: &[u8],
) -> Result<Vec<u8>> {
    use TiledFormat::*;
    if width == 0 || height == 0 || width > 4096 || height > 4096 {
        return Err("Unsupported texture dimensions".into());
    }
    let indexed = matches!(format, C4 | C8);
    if indexed && (palette.is_empty() || palette.len() > 512 || !palette.len().is_multiple_of(2)) {
        return Err("Invalid RGB5A3 palette".into());
    }
    if !indexed && !palette.is_empty() {
        return Err("Unexpected texture palette".into());
    }
    let (tw, th, bits) = match format {
        I4 | C4 => (8, 8, 4),
        I8 | Ia4 | C8 => (8, 4, 8),
        Ia8 => (4, 4, 16),
    };
    let tile_bytes = tw * th * bits / 8;
    if data.len() != width.div_ceil(tw) * height.div_ceil(th) * tile_bytes {
        return Err("Tiled texture byte count mismatch".into());
    }
    let mut out = vec![0; width * height * 4];
    let mut base = 0;
    for ty in (0..height).step_by(th) {
        for tx in (0..width).step_by(tw) {
            for y in 0..th {
                for x in 0..tw {
                    let i = y * tw + x;
                    let value = match bits {
                        4 => (data[base + i / 2] >> if i % 2 == 0 { 4 } else { 0 }) as usize & 15,
                        8 => data[base + i] as usize,
                        _ => u16::from_be_bytes([data[base + i * 2], data[base + i * 2 + 1]])
                            as usize,
                    };
                    if tx + x >= width || ty + y >= height {
                        continue;
                    }
                    let rgba = match format {
                        I4 => [value as u8 * 17; 4],
                        I8 => [value as u8; 4],
                        Ia4 => {
                            let intensity = (value as u8 & 15) * 17;
                            [intensity, intensity, intensity, (value as u8 >> 4) * 17]
                        }
                        Ia8 => {
                            let intensity = value as u8;
                            [intensity, intensity, intensity, (value >> 8) as u8]
                        }
                        C4 | C8 => {
                            let p = palette
                                .get(value * 2..value * 2 + 2)
                                .ok_or("Palette index outside table")?;
                            let v = u16::from_be_bytes([p[0], p[1]]);
                            if v & 0x8000 != 0 {
                                let five = |s| {
                                    let c = ((v >> s) & 31u16) as u8;
                                    (c << 3) | (c >> 2)
                                };
                                [five(10), five(5), five(0), 255]
                            } else {
                                let a = ((v >> 12) & 7) as u8;
                                [
                                    ((v >> 8) & 15) as u8 * 17,
                                    ((v >> 4) & 15) as u8 * 17,
                                    (v & 15) as u8 * 17,
                                    (a << 5) | (a << 2) | (a >> 1),
                                ]
                            }
                        }
                    };
                    let dest = ((ty + y) * width + tx + x) * 4;
                    out[dest..dest + 4].copy_from_slice(&rgba);
                }
            }
            base += tile_bytes;
        }
    }
    Ok(out)
}
pub fn decode_named(archive: &[u8], wanted: &str) -> Result<DecodedTexture> {
    use crate::archive::{chunks, named_payload};
    let mut result = None;
    for chunk in chunks(archive)? {
        if chunk.tag != *b"RXET" {
            continue;
        }
        let (_, body) = named_payload(chunk.payload)?;
        for group in chunks(body)? {
            if group.tag != *b"FTBG" {
                continue;
            }
            let entries = group.payload.get(4..).ok_or("Missing texture count")?;
            for texture in chunks(entries)? {
                if texture.tag != *b"DXTG" {
                    continue;
                }
                let p = texture.payload;
                if p.len() < 120 {
                    return Err("Short texture header".into());
                }
                let end = p[..32].iter().position(|b| *b == 0).unwrap_or(32);
                let name = std::str::from_utf8(&p[..end]).map_err(|e| e.to_string())?;
                if !name.eq_ignore_ascii_case(wanted) {
                    continue;
                }
                if result.is_some() {
                    return Err("Ambiguous case-insensitive texture name".into());
                }
                let width = u32::from_be_bytes(p[32..36].try_into().unwrap()) as usize;
                let height = u32::from_be_bytes(p[36..40].try_into().unwrap()) as usize;
                let mips = chunks(&p[112..])?;
                let indexed = matches!(&p[52..56], b"\0\0\x34P" | b"\0\0\x38P");
                let palette = if indexed {
                    let palette = mips.first().ok_or("Missing palette")?;
                    if palette.tag != *b" LAP" {
                        return Err("Indexed texture has no palette".into());
                    }
                    palette.payload
                } else {
                    &[]
                };
                let mip = mips.get(usize::from(indexed)).ok_or("Missing mip")?;
                if mip.tag != *b" PIM" {
                    return Err(format!("Unsupported palette/mip layout for {name}"));
                }
                let rgba = match &p[52..56] {
                    b"1TXD" => decode_cmpr(mip.payload, width, height)?,
                    b"8R8A" => decode_rgba8(mip.payload, width, height)?,
                    b"\0\0\x34P" => {
                        decode_tiled(mip.payload, width, height, TiledFormat::C4, palette)?
                    }
                    b"\0\0\x38P" => {
                        decode_tiled(mip.payload, width, height, TiledFormat::C8, palette)?
                    }
                    b"\0\0\x34I" => {
                        decode_tiled(mip.payload, width, height, TiledFormat::I4, palette)?
                    }
                    b"\0\0\x38I" => {
                        decode_tiled(mip.payload, width, height, TiledFormat::I8, palette)?
                    }
                    b"\0\x34AI" => {
                        decode_tiled(mip.payload, width, height, TiledFormat::Ia4, palette)?
                    }
                    b"\0\x38AI" => {
                        decode_tiled(mip.payload, width, height, TiledFormat::Ia8, palette)?
                    }
                    _ => return Err(format!("Unsupported format for {name}")),
                };
                result = Some(DecodedTexture {
                    width,
                    height,
                    rgba,
                });
            }
        }
    }
    result.ok_or_else(|| format!("Texture not found: {wanted}"))
}

pub fn decode_btf_first(btf: &[u8]) -> Result<DecodedTexture> {
    use crate::archive::chunks;
    for group in chunks(btf)? {
        if group.tag != *b"FTBG" {
            continue;
        }
        let entries = group.payload.get(4..).ok_or("Missing texture count")?;
        for texture in chunks(entries)? {
            if texture.tag != *b"DXTG" {
                continue;
            }
            let p = texture.payload;
            if p.len() < 120 {
                return Err("Short texture header".into());
            }
            let end = p[..32].iter().position(|b| *b == 0).unwrap_or(32);
            let name = std::str::from_utf8(&p[..end]).map_err(|e| e.to_string())?;
            let width = u32::from_be_bytes(p[32..36].try_into().unwrap()) as usize;
            let height = u32::from_be_bytes(p[36..40].try_into().unwrap()) as usize;
            let mips = chunks(&p[112..])?;
            let indexed = matches!(&p[52..56], b"\0\0\x34P" | b"\0\0\x38P");
            let palette = if indexed {
                let palette = mips.first().ok_or("Missing palette")?;
                if palette.tag != *b" LAP" {
                    return Err("Indexed texture has no palette".into());
                }
                palette.payload
            } else {
                &[]
            };
            let mip = mips.get(usize::from(indexed)).ok_or("Missing mip")?;
            if mip.tag != *b" PIM" {
                return Err(format!("Unsupported palette/mip layout for {name}"));
            }
            let rgba = match &p[52..56] {
                b"1TXD" => decode_cmpr(mip.payload, width, height)?,
                b"8R8A" => decode_rgba8(mip.payload, width, height)?,
                b"\0\0\x34P" => decode_tiled(mip.payload, width, height, TiledFormat::C4, palette)?,
                b"\0\0\x38P" => decode_tiled(mip.payload, width, height, TiledFormat::C8, palette)?,
                b"\0\0\x34I" => decode_tiled(mip.payload, width, height, TiledFormat::I4, palette)?,
                b"\0\0\x38I" => decode_tiled(mip.payload, width, height, TiledFormat::I8, palette)?,
                b"\0\x34AI" => decode_tiled(mip.payload, width, height, TiledFormat::Ia4, palette)?,
                b"\0\x38AI" => decode_tiled(mip.payload, width, height, TiledFormat::Ia8, palette)?,
                _ => return Err(format!("Unsupported format for {name}")),
            };
            return Ok(DecodedTexture {
                width,
                height,
                rgba,
            });
        }
    }
    Err("No DXTG texture found in BTF".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn intensity_tiles_and_alpha() {
        let mut data = vec![0; 64];
        data[0] = 0xf1;
        data[32] = 0x82;
        let pixels = decode_tiled(&data, 9, 1, TiledFormat::I4, &[]).unwrap();
        assert_eq!(&pixels[..8], &[255, 255, 255, 255, 17, 17, 17, 17]);
        assert_eq!(&pixels[32..36], &[136; 4]);
        let pixels = decode_tiled(&[0xa3; 32], 1, 1, TiledFormat::Ia4, &[]).unwrap();
        assert_eq!(pixels, vec![51, 51, 51, 170]);
        let pixels = decode_tiled(&[0x40, 0x80].repeat(16), 1, 1, TiledFormat::Ia8, &[]).unwrap();
        assert_eq!(pixels, vec![128, 128, 128, 64]);
    }
    #[test]
    fn palette_rgb5a3_and_bounds() {
        let palette = [0xfc, 0x00, 0x30, 0xf0];
        let mut data = vec![0; 32];
        data[0] = 0x01;
        let pixels = decode_tiled(&data, 2, 1, TiledFormat::C4, &palette).unwrap();
        assert_eq!(pixels, vec![255, 0, 0, 255, 0, 255, 0, 109]);
        data[0] = 0x20;
        assert!(decode_tiled(&data, 2, 1, TiledFormat::C4, &palette).is_err());
        assert!(decode_tiled(&data[..31], 2, 1, TiledFormat::C8, &palette).is_err());
        assert!(decode_tiled(&data, 0, 1, TiledFormat::I8, &[]).is_err());
    }
    #[test]
    fn indexed_and_intensity_eight_bit_tile_order() {
        let mut data = vec![0; 64];
        data[32] = 1;
        let palette = [0x80, 0x00, 0xff, 0xff];
        let pixels = decode_tiled(&data, 9, 1, TiledFormat::C8, &palette).unwrap();
        assert_eq!(&pixels[..4], &[0, 0, 0, 255]);
        assert_eq!(&pixels[32..36], &[255; 4]);
        let pixels = decode_tiled(&data, 9, 1, TiledFormat::I8, &[]).unwrap();
        assert_eq!(&pixels[32..36], &[1; 4]);
        assert!(decode_tiled(&data, 9, 1, TiledFormat::I8, &palette).is_err());
        assert!(decode_tiled(&data, 9, 1, TiledFormat::C8, &[0]).is_err());
        // No implicit zero-filling for truncated original texture payloads.
        assert!(decode_tiled(&[0; 16], 8, 4, TiledFormat::C4, &palette).is_err());
    }
    #[test]
    fn rgba8_planes_and_subtiles() {
        let mut data = vec![0; 128];
        for i in 0..16 {
            data[i * 2] = 255;
            data[i * 2 + 1] = 120;
            data[32 + i * 2] = 50;
            data[32 + i * 2 + 1] = 30;
        }
        let rgba = decode_rgba8(&data, 8, 4).unwrap();
        assert_eq!(&rgba[..4], &[120, 50, 30, 255]);
        assert_eq!(&rgba[16..20], &[0, 0, 0, 0]);
        assert!(decode_rgba8(&data[..127], 8, 4).is_err());
    }
    #[test]
    fn endpoints_and_subtile_order() {
        let colors = [0xf800u16, 0x07e0, 0x001f, 0xffff];
        let mut data = Vec::new();
        for color in colors {
            data.extend(color.to_be_bytes());
            data.extend([0; 6]);
        }
        let rgba = decode_cmpr(&data, 8, 8).unwrap();
        for (pixel, expected) in [
            (0, [255, 0, 0, 255]),
            (4, [0, 255, 0, 255]),
            (32, [0, 0, 255, 255]),
            (36, [255, 255, 255, 255]),
        ] {
            assert_eq!(&rgba[pixel * 4..pixel * 4 + 4], &expected);
        }
    }
    #[test]
    fn transparent_index() {
        let data = [0, 0, 255, 255, 255, 255, 255, 255].repeat(4);
        let rgba = decode_cmpr(&data, 8, 8).unwrap();
        assert!(rgba.chunks_exact(4).all(|p| p[3] == 0));
    }
    #[test]
    fn rejects_invalid_dimensions_or_size() {
        assert!(decode_cmpr(&[], 0, 8).is_err());
        assert!(decode_cmpr(&[], 8, 8).is_err());
    }
}
