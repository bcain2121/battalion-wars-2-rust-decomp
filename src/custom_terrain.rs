//! Native editable heightfield input for mod sandboxes, separate from Wii decoding.
use crate::{Result, terrain::Terrain};
use serde::Deserialize;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Heightfield {
    width: usize,
    height: usize,
    heights: Vec<f32>,
    textures: [String; 2],
}
pub fn parse(json: &str) -> Result<Terrain> {
    if json.len() > 8 * 1024 * 1024 {
        return Err("Custom heightfield exceeds 8 MiB".into());
    }
    let h: Heightfield = serde_json::from_str(json).map_err(|e| e.to_string())?;
    if h.width < 4
        || h.height < 4
        || h.width > 256
        || h.height > 256
        || !(h.width - 1).is_multiple_of(3)
        || !(h.height - 1).is_multiple_of(3)
    {
        return Err("Heightfield dimensions must be 4..256 and 3n+1".into());
    }
    if h.heights.len() != h.width * h.height
        || h.heights
            .iter()
            .any(|v| !v.is_finite() || v.abs() > 10000.0)
    {
        return Err("Invalid height array".into());
    }
    if h.textures.iter().any(|s| s.is_empty() || s.len() > 32) {
        return Err("Invalid texture resource name".into());
    }
    let mut t = Terrain {
        map_width: h.width,
        map_height: h.height,
        records: 0,
        seam_comparisons: 0,
        seam_mismatches: 0,
        positions: Vec::new(),
        colors: Vec::new(),
        uv0: Vec::new(),
        uv1: Vec::new(),
        materials: vec![h.textures],
        tile_materials: Vec::new(),
        indices: Vec::new(),
    };
    for z in (0..h.height - 1).step_by(3) {
        for x in (0..h.width - 1).step_by(3) {
            let base = t.positions.len() as u32;
            t.tile_materials.push(0);
            for dz in 0..4 {
                for dx in 0..4 {
                    t.positions.push([
                        (x + dx) as f32,
                        h.heights[(z + dz) * h.width + x + dx],
                        (z + dz) as f32,
                    ]);
                    t.colors.push([0.35, 0.35, 0.35, 0.0]);
                    let uv = [dx as f32 / 3.0, dz as f32 / 3.0];
                    t.uv0.push(uv);
                    t.uv1.push(uv);
                }
            }
            for dz in 0..3 {
                for dx in 0..3 {
                    let a = base + (dz * 4 + dx) as u32;
                    t.indices
                        .extend_from_slice(&[a, a + 4, a + 1, a + 1, a + 4, a + 5]);
                }
            }
        }
    }
    Ok(t)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editable_grid() {
        let s =
            serde_json::json!({"width":4,"height":4,"heights":vec![2.0;16],"textures":["a","b"]});
        let t = parse(&s.to_string()).unwrap();
        assert_eq!(t.positions.len(), 16);
        assert_eq!(t.indices.len(), 54);
        assert_eq!(t.positions[0], [0.0, 2.0, 0.0]);
    }
    #[test]
    fn invalid_shape() {
        assert!(parse(r#"{"width":5,"height":4,"heights":[],"textures":["a","b"]}"#).is_err());
    }
}
