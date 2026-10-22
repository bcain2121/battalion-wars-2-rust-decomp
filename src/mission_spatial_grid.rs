//! Three ordered 64x64 layers from 0x8033e384. Live registration loading is separate.
use crate::{
    Result,
    mission_spatial_cell::{self, Bounds, Cell, Record, Sphere, Visit},
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub maximum_record_radius: f32,
    /// (x,z). Absence explicitly means known empty, NEVER unknown/unloaded data.
    pub cells: BTreeMap<(u8, u8), Cell>,
}
fn index(value: f32) -> Result<u8> {
    // GQR1 signed-16 store with scale zero, then native floor correction.
    // Reject quantizer saturation instead of approximating it.
    let scaled = (value - (-2048.0)) * 0.015625;
    if !scaled.is_finite() || !(-32768.0..=32767.0).contains(&scaled) {
        return Err("Spatial grid quantization outside supported range".into());
    }
    let mut index = scaled.trunc() as i32;
    if (index * 64 - 2048) as f32 > value {
        index -= 1;
    }
    Ok(index.clamp(0, 63) as u8)
}
/// Layers in stored order; z rows outermost, x columns innermost, inclusive ends.
/// Callback errors require the enclosing caller to roll back its own mutations.
pub fn visit(
    layers: &[Layer; 3],
    query: Sphere,
    mask: u32,
    mut callback: impl FnMut(&Record) -> Result<bool>,
) -> Result<Visit> {
    if !query.radius.is_finite()
        || query.radius < 0.0
        || !query.center.into_iter().all(f32::is_finite)
    {
        return Err("Invalid spatial grid query".into());
    }
    let mut total = Visit {
        callbacks: 0,
        stopped: false,
    };
    for layer in layers {
        let r = layer.maximum_record_radius;
        if !r.is_finite() || r < 0.0 {
            return Err("Invalid spatial grid layer radius".into());
        }
        if layer.cells.keys().any(|&(x, z)| x >= 64 || z >= 64) {
            return Err("Spatial grid cell outside 64x64 domain".into());
        }
        // Preserve the original two-step rounding: query bounds, then layer expansion.
        let min_x = index((query.center[0] - query.radius) - r)?;
        let max_x = index((query.center[0] + query.radius) + r)?;
        let min_z = index((query.center[2] - query.radius) - r)?;
        let max_z = index((query.center[2] + query.radius) + r)?;
        for z in min_z..=max_z {
            for x in min_x..=max_x {
                let Some(cell) = layer.cells.get(&(x, z)) else {
                    continue;
                };
                let left = f32::from(x) * 64.0 - 2048.0;
                let top = f32::from(z) * 64.0 - 2048.0;
                let result = mission_spatial_cell::visit(
                    cell,
                    Bounds {
                        min: [left, top],
                        max: [left + 64.0, top + 64.0],
                    },
                    query,
                    r,
                    mask,
                    &mut callback,
                )?;
                total.callbacks = total
                    .callbacks
                    .checked_add(result.callbacks)
                    .filter(|count| *count < 0x8000_0000)
                    .ok_or("Spatial grid callback count overflow")?;
                if result.stopped {
                    total.stopped = true;
                    return Ok(total);
                }
            }
        }
    }
    Ok(total)
}
/// Manager dispatch at 0x8033d2b8: bit 1 grid (+0x4c11c), then bit 2
/// grid (+0x28034). Higher mode bits are ignored by this native entry point.
/// Unlike the inner grid, this outer entry returns the DECODED callback count
/// even on early stop. Missing selected grids are unknown, not empty.
pub fn dispatch(
    first: Option<&[Layer; 3]>,
    second: Option<&[Layer; 3]>,
    query: Sphere,
    mask: u32,
    mode: u32,
    mut callback: impl FnMut(&Record) -> Result<bool>,
) -> Result<u32> {
    let mut count = 0_u32;
    for (bit, grid) in [(1, first), (2, second)] {
        if mode & bit == 0 {
            continue;
        }
        let grid = grid.ok_or("Selected spatial grid not configured")?;
        let result = visit(grid, query, mask, &mut callback)?;
        count = count
            .checked_add(result.callbacks)
            .filter(|n| *n < 0x8000_0000)
            .ok_or("Spatial dispatcher callback count overflow")?;
        if result.stopped {
            return Ok(count);
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mission_spatial_cell::Contents;
    fn layer() -> Layer {
        Layer {
            maximum_record_radius: 1.0,
            cells: BTreeMap::new(),
        }
    }
    fn cell(id: u64) -> Cell {
        Cell {
            mask: 1,
            contents: Contents::Leaf(vec![Record {
                identity: id,
                mask: 1,
                sphere: Sphere {
                    center: [0.0; 3],
                    radius: 1.0,
                },
            }]),
        }
    }
    #[test]
    fn dispatcher_order_duplicates_and_decoded_stop_count() {
        let mut first = [layer(), layer(), layer()];
        let mut second = first.clone();
        first[0].cells.insert((32, 32), cell(7));
        second[0].cells.insert((32, 32), cell(7));
        second[1].cells.insert((32, 32), cell(9));
        let query = Sphere {
            center: [0.0; 3],
            radius: 1.0,
        };
        let mut seen = vec![];
        let count = dispatch(Some(&first), Some(&second), query, 1, 3, |r| {
            seen.push(r.identity);
            Ok(seen.len() < 2)
        })
        .unwrap();
        assert_eq!(seen, [7, 7]);
        assert_eq!(count, 2); // Not !2: the manager decodes the grid's stop result.
        assert_eq!(
            dispatch(Some(&first), None, query, 1, 3, |_| Ok(false)).unwrap(),
            1
        );
        assert!(dispatch(Some(&first), None, query, 1, 3, |_| Ok(true)).is_err());
    }
    #[test]
    fn dispatcher_only_reads_selected_grids() {
        let mut grid = [layer(), layer(), layer()];
        grid[0].cells.insert((32, 32), cell(4));
        let query = Sphere {
            center: [0.0; 3],
            radius: 1.0,
        };
        assert_eq!(
            dispatch(None, Some(&grid), query, 1, 2, |_| Ok(true)).unwrap(),
            1
        );
        assert_eq!(
            dispatch(Some(&grid), None, query, 1, 0x81, |_| Ok(true)).unwrap(),
            1
        );
        assert_eq!(
            dispatch(
                None,
                None,
                Sphere {
                    center: [f32::NAN; 3],
                    radius: f32::NAN
                },
                1,
                4,
                |_| panic!("disabled query")
            )
            .unwrap(),
            0
        );
        assert!(dispatch(None, Some(&grid), query, 1, 1, |_| Ok(true)).is_err());
    }
    #[test]
    fn boundaries_floor_then_clamp() {
        for (v, want) in [
            (-3000.0, 0),
            (-2048.0, 0),
            (-0.001, 31),
            (0.0, 32),
            (63.999, 32),
            (64.0, 33),
            (2048.0, 63),
        ] {
            assert_eq!(index(v).unwrap(), want);
        }
        assert!(index(f32::INFINITY).is_err());
        assert!(index(f32::MAX).is_err());
    }
    #[test]
    fn layer_row_column_order_and_stop_count() {
        let mut layers = [layer(), layer(), layer()];
        for (xy, id) in [((32, 32), 4), ((31, 32), 3), ((32, 31), 2), ((31, 31), 1)] {
            layers[0].cells.insert(xy, cell(id));
        }
        layers[1].cells.insert((31, 31), cell(1));
        layers[2].cells.insert((31, 31), cell(6));
        let query = Sphere {
            center: [0.0; 3],
            radius: 1.0,
        };
        let mut seen = vec![];
        let result = visit(&layers, query, 1, |r| {
            seen.push(r.identity);
            Ok(true)
        })
        .unwrap();
        assert_eq!(seen, [1, 2, 3, 4, 1, 6]);
        assert_eq!(result.native_result(), 6);
        layers[2].maximum_record_radius = f32::NAN;
        seen.clear();
        let result = visit(&layers, query, 1, |r| {
            seen.push(r.identity);
            Ok(seen.len() < 5)
        })
        .unwrap();
        assert_eq!(result.native_result(), !5_u32);
        assert_eq!(seen, [1, 2, 3, 4, 1]);
        assert!(visit(&layers, query, 1, |_| Ok(true)).is_err());
    }
}
