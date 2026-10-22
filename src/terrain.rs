//! Diagnostic terrain reconstruction. Heights/grid ordering verified by seams;
//! world scale, origin, triangle diagonals and color semantics remain provisional.
use crate::{Result, archive::chunks};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Serialize)]
pub struct Terrain {
    pub map_width: usize,
    pub map_height: usize,
    pub records: usize,
    pub seam_comparisons: usize,
    pub seam_mismatches: usize,
    pub positions: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    pub uv0: Vec<[f32; 2]>,
    pub uv1: Vec<[f32; 2]>,
    pub materials: Vec<[String; 2]>,
    pub tile_materials: Vec<usize>,
    pub indices: Vec<u32>,
}

pub fn decode(data: &[u8]) -> Result<Terrain> {
    let parts = chunks(data)?;
    let get = |tag: &[u8; 4]| -> Result<&[u8]> {
        let matches: Vec<_> = parts.iter().filter(|c| &c.tag == tag).collect();
        if matches.len() != 1 {
            return Err(format!("Expected one {:?} chunk", tag));
        }
        Ok(matches[0].payload)
    };
    let header = get(b"RRET")?;
    if header.len() != 16 {
        return Err("Unexpected terrain header".into());
    }
    let width = u32::from_le_bytes(header[..4].try_into().unwrap()) as usize;
    let height = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
    if width == 0 || height == 0 || width > 256 || height > 256 {
        return Err("Unsupported terrain dimensions".into());
    }
    let map = get(b"PAMC")?;
    let geometry = get(b"KNHC")?;
    if map.len() != width * height * 4 || geometry.len() % 2880 != 0 {
        return Err("Invalid terrain section sizes".into());
    }
    let records = geometry.len() / 2880;
    let materials = if let Some(materials) = parts.iter().find(|c| c.tag == *b"LTAM") {
        if materials.payload.len() % 48 != 0 {
            return Err("Invalid LTAM size".into());
        }
        materials
            .payload
            .chunks_exact(48)
            .map(|p| {
                let name = |bytes: &[u8]| -> Result<String> {
                    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
                    std::str::from_utf8(&bytes[..end])
                        .map(str::to_owned)
                        .map_err(|e| e.to_string())
                };
                Ok([name(&p[..16])?, name(&p[16..32])?])
            })
            .collect::<Result<Vec<_>>>()?
    } else {
        Vec::new()
    };
    let mut out = Terrain {
        map_width: width,
        map_height: height,
        records,
        seam_comparisons: 0,
        seam_mismatches: 0,
        positions: Vec::new(),
        colors: Vec::new(),
        uv0: Vec::new(),
        uv1: Vec::new(),
        materials,
        tile_materials: Vec::new(),
        indices: Vec::new(),
    };
    let mut heights: BTreeMap<(usize, usize), u16> = BTreeMap::new();
    for my in 0..height {
        for mx in 0..width {
            let m = (my * width + mx) * 4;
            let flag = u16::from_be_bytes(map[m..m + 2].try_into().unwrap());
            let index = u16::from_be_bytes(map[m + 2..m + 4].try_into().unwrap());
            if flag == 2 && index == 65535 {
                continue;
            }
            if flag != 1 || index as usize >= records {
                return Err("Unsupported terrain map entry".into());
            }
            for ty in 0..4 {
                for tx in 0..4 {
                    let offset = index as usize * 2880 + (ty * 4 + tx) * 180;
                    let tile = &geometry[offset..offset + 180];
                    let base = out.positions.len() as u32;
                    let material = u32::from_be_bytes(tile[176..180].try_into().unwrap()) as usize;
                    if !out.materials.is_empty() && material >= out.materials.len() {
                        return Err("Terrain material index out of range".into());
                    }
                    out.tile_materials.push(material);
                    let uv = |offset: usize| -> [f32; 2] {
                        [
                            u16::from_be_bytes(tile[offset..offset + 2].try_into().unwrap()) as f32
                                / 4096.0,
                            u16::from_be_bytes(tile[offset + 2..offset + 4].try_into().unwrap())
                                as f32
                                / 4096.0,
                        ]
                    };
                    let corners = [uv(96), uv(100), uv(104), uv(108)];
                    for y in 0..4 {
                        for x in 0..4 {
                            let v = y * 4 + x;
                            let h = u16::from_be_bytes(tile[v * 2..v * 2 + 2].try_into().unwrap());
                            let gx = mx * 12 + tx * 3 + x;
                            let gz = my * 12 + ty * 3 + y;
                            if let Some(old) = heights.insert((gx, gz), h) {
                                out.seam_comparisons += 1;
                                if old != h {
                                    out.seam_mismatches += 1;
                                }
                            }
                            // Diagnostic units: one grid step; height divided by 32 for readability.
                            out.positions.push([gx as f32, h as f32 / 32.0, gz as f32]);
                            let c = 32 + v * 4;
                            out.colors.push([
                                tile[c] as f32 / 255.0,
                                tile[c + 1] as f32 / 255.0,
                                tile[c + 2] as f32 / 255.0,
                                tile[c + 3] as f32 / 255.0,
                            ]);
                            let u = x as f32 / 3.0;
                            let v = y as f32 / 3.0;
                            out.uv0.push(std::array::from_fn(|i| {
                                corners[0][i] * (1.0 - u) * (1.0 - v)
                                    + corners[1][i] * u * (1.0 - v)
                                    + corners[2][i] * (1.0 - u) * v
                                    + corners[3][i] * u * v
                            }));
                            // Secondary coordinates are explicitly stored per vertex.
                            out.uv1.push(uv(112 + (y * 4 + x) * 4));
                        }
                    }
                    for y in 0..3 {
                        for x in 0..3 {
                            let a = base + (y * 4 + x) as u32;
                            out.indices
                                .extend_from_slice(&[a, a + 4, a + 1, a + 1, a + 4, a + 5]);
                        }
                    }
                }
            }
        }
    }
    if out.positions.is_empty() {
        return Err("Terrain contains no active tiles".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn chunk(t: &[u8; 4], p: &[u8]) -> Vec<u8> {
        [t.as_slice(), &(p.len() as u32).to_le_bytes(), p].concat()
    }
    fn fixture() -> Vec<u8> {
        let h = [1u32, 1, 1, 14]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
        [
            chunk(b"RRET", &h),
            chunk(b"PAMC", &[0, 1, 0, 0]),
            chunk(b"KNHC", &vec![0; 2880]),
        ]
        .concat()
    }
    #[test]
    fn flat_record() {
        let t = decode(&fixture()).unwrap();
        assert_eq!(t.positions.len(), 256);
        assert_eq!(t.indices.len(), 864);
        assert_eq!(t.seam_mismatches, 0);
        assert!(t.seam_comparisons > 0);
    }
    #[test]
    fn rejects_bad_record_reference() {
        let mut f = fixture();
        f[34] = 0xff;
        f[35] = 0xfe;
        assert!(decode(&f).is_err());
    }
    #[test]
    fn rejects_truncated_geometry() {
        let mut f = fixture();
        f.pop();
        assert!(decode(&f).is_err());
    }
}
