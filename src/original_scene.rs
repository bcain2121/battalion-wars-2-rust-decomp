//! Authored object placement, deliberately independent of mission activation rules.
use crate::{Object, ObjectGraph, ObjectId, Result};
use serde::Serialize;
pub fn field<'a>(object: &'a Object, name: &str) -> Option<&'a str> {
    object
        .fields
        .iter()
        .find(|f| f.attributes.get("name").is_some_and(|n| n == name))
        .and_then(|f| f.items.first())
        .map(String::as_str)
}
fn reference<'a>(graph: &'a ObjectGraph, object: &Object, name: &str) -> Option<&'a Object> {
    graph
        .objects
        .get(&ObjectId(field(object, name)?.parse().ok()?))
}
pub fn resolve_base<'a>(graph: &'a ObjectGraph, object: &Object) -> Option<&'a Object> {
    if let Some(base) = reference(graph, object, "mBase") {
        return Some(base);
    }
    if object.kind == "cMorphingBuilding" {
        let preferred = match field(object, "mAllegiance") {
            Some("eSolarEmpire") => Some("mBaseSE"),
            Some("eAngloIsles") => Some("mBaseAG"),
            Some("eWesternFrontier") => Some("mBaseWF"),
            Some("eTundranTerritories") | Some("eTundran") => Some("mBaseTT"),
            Some("eXylvania") | Some("eXylvanian") => Some("mBaseXY"),
            Some("eUnderWorld") | Some("eUnderworld") => Some("mBaseUW"),
            Some("eNeutral") | Some("eNoArmy") => Some("mBaseNeutral"),
            _ => None,
        };
        if let Some(key) = preferred
            && let Some(base) = reference(graph, object, key)
        {
            return Some(base);
        }
        for key in [
            "mBaseSE",
            "mBaseAG",
            "mBaseWF",
            "mBaseTT",
            "mBaseXY",
            "mBaseUW",
            "mBaseNeutral",
        ] {
            if let Some(base) = reference(graph, object, key) {
                return Some(base);
            }
        }
    }
    None
}
#[derive(Debug, Serialize)]
pub struct Placement {
    pub id: ObjectId,
    pub kind: String,
    pub model: String,
    pub matrix: [f32; 16],
    pub stick_to_floor: bool,
    pub next_link_object: Option<ObjectId>,
    pub link_offset: [f32; 3],
    pub attachments: Vec<(String, String)>,
}
#[derive(Debug, Serialize, Default)]
pub struct Scene {
    pub placements: Vec<Placement>,
    pub omitted: Vec<(ObjectId, String)>,
}
pub fn matrix(text: &str) -> Result<[f32; 16]> {
    let values = text
        .split(',')
        .map(|s| s.trim().parse::<f32>().map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>>>()?;
    let out: [f32; 16] = values.try_into().map_err(|_| "Expected 16 matrix values")?;
    if out.iter().any(|v| !v.is_finite())
        || out[3] != 0.0
        || out[7] != 0.0
        || out[11] != 0.0
        || out[15] != 1.0
    {
        return Err("Invalid affine placement matrix".into());
    }
    Ok(out)
}
fn vector4_xyz(text: &str) -> Result<[f32; 3]> {
    let values = text
        .split(',')
        .map(|s| s.trim().parse::<f32>().map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>>>()?;
    let out: [f32; 4] = values.try_into().map_err(|_| "Expected 4 vector4 values")?;
    if out.iter().any(|v| !v.is_finite()) {
        return Err("Invalid LinkOffset vector4".into());
    }
    Ok([out[0], out[1], out[2]])
}
pub fn load(graph: &ObjectGraph) -> Result<Scene> {
    let mut scene = Scene::default();
    for object in graph.objects.values() {
        let Some(raw) = field(object, "Mat") else {
            continue;
        };
        let Some(base) = resolve_base(graph, object) else {
            continue;
        };
        let model = ["mBAN_Model", "mpModel", "Model", "model", "mModel"]
            .iter()
            .find_map(|key| reference(graph, base, key))
            .filter(|o| o.kind == "cNodeHierarchyResource")
            .and_then(|o| field(o, "mName"));
        let Some(model) = model else {
            scene.omitted.push((
                object.id,
                format!(
                    "{}: no supported primary model (clusters are not expanded)",
                    object.kind
                ),
            ));
            continue;
        };
        let mut attachments = Vec::new();
        if object.kind == "cTroop" {
            for (key, bone) in [
                ("mWeaponModel", "BONE_R_HAND"),
                ("mHelmetModel", "BONE_HELMET"),
                ("mBkPackModel", "BONE_BCK_MISC"),
            ] {
                if let Some(name) = reference(graph, base, key).and_then(|o| field(o, "mName")) {
                    attachments.push((bone.into(), name.into()));
                }
            }
        }
        let mut mat = matrix(raw).map_err(|e| format!("Object {}: {e}", object.id.0))?;
        let mut next_link_object = None;
        let mut link_offset = [0.0; 3];
        if object.kind == "cDestroyableObject" {
            if 999.99 < mat[13] && mat[13] < 1000.01 {
                mat[13] = 0.0;
            }
            if let Some(raw_link) = field(object, "NextLinkObject") {
                let target = raw_link
                    .trim()
                    .parse::<u64>()
                    .map_err(|_| format!("Object {}: Invalid NextLinkObject", object.id.0))?;
                if target != 0 && target != object.id.0 {
                    next_link_object = Some(ObjectId(target));
                }
            }
            if let Some(raw_offset) = field(object, "LinkOffset") {
                link_offset =
                    vector4_xyz(raw_offset).map_err(|e| format!("Object {}: {e}", object.id.0))?;
            }
        }
        scene.placements.push(Placement {
            id: object.id,
            kind: object.kind.clone(),
            model: model.into(),
            matrix: mat,
            stick_to_floor: field(object, "mStickToFloor") == Some("eTrue")
                || field(object, "mLockToSurface") == Some("eTrue")
                || matches!(object.kind.as_str(), "cTroop" | "cGroundVehicle"),
            next_link_object,
            link_offset,
            attachments,
        });
    }
    Ok(scene)
}
impl Scene {
    pub fn resolve_placements(
        &self,
        ground: &crate::skirmish::Ground,
        model_bounds: &std::collections::BTreeMap<String, crate::model::Bounds>,
    ) -> Result<(Vec<[f32; 16]>, std::collections::BTreeSet<ObjectId>)> {
        use std::collections::{BTreeMap, BTreeSet};
        let index_by_id: BTreeMap<ObjectId, usize> = self
            .placements
            .iter()
            .enumerate()
            .map(|(idx, p)| (p.id, idx))
            .collect();
        let mut matrices: Vec<[f32; 16]> = self.placements.iter().map(|p| p.matrix).collect();
        let mut outside_terrain = BTreeSet::new();
        let mut prev_link: BTreeMap<ObjectId, ObjectId> = BTreeMap::new();
        for (idx, placement) in self.placements.iter().enumerate() {
            if placement.stick_to_floor {
                let [x, z] = source_to_grid(matrices[idx][12], matrices[idx][14]);
                if let Some(height) = ground.height(x, z) {
                    matrices[idx][13] += height * 2.0;
                } else {
                    outside_terrain.insert(placement.id);
                }
            }
            if let Some(next_id) = placement.next_link_object {
                let Some(&next_idx) = index_by_id.get(&next_id) else {
                    return Err(format!(
                        "Object {}: missing NextLinkObject {}",
                        placement.id.0, next_id.0
                    ));
                };
                if next_id == placement.id || self.placements[next_idx].kind != "cDestroyableObject"
                {
                    return Err(format!(
                        "Object {}: invalid NextLinkObject {}",
                        placement.id.0, next_id.0
                    ));
                }
                prev_link.insert(next_id, placement.id);
            }
        }
        for placement in &self.placements {
            let mut cursor = placement.next_link_object;
            let mut steps = 0usize;
            while let Some(next_id) = cursor {
                steps += 1;
                if next_id == placement.id || steps > self.placements.len() {
                    return Err(format!("Object {}: NextLinkObject cycle", placement.id.0));
                }
                cursor = self.placements[index_by_id[&next_id]].next_link_object;
            }
        }
        for placement in &self.placements {
            if placement.kind != "cDestroyableObject"
                || placement.next_link_object.is_some()
                || !prev_link.contains_key(&placement.id)
            {
                continue;
            }
            let mut cur_id_opt = prev_link.get(&placement.id).copied();
            let mut steps = 0usize;
            while let Some(cur_id) = cur_id_opt {
                steps += 1;
                if steps > self.placements.len() {
                    return Err(format!("Object {}: PrevLinkObject cycle", placement.id.0));
                }
                let cur_idx = index_by_id[&cur_id];
                let next_id = self.placements[cur_idx]
                    .next_link_object
                    .ok_or("Missing NextLinkObject in chain")?;
                let next_idx = index_by_id[&next_id];
                let cur_bounds = model_bounds
                    .get(&self.placements[cur_idx].model)
                    .ok_or_else(|| {
                        format!(
                            "Missing hierarchy bounds for {}",
                            self.placements[cur_idx].model
                        )
                    })?;
                let next_bounds = model_bounds
                    .get(&self.placements[next_idx].model)
                    .ok_or_else(|| {
                        format!(
                            "Missing hierarchy bounds for {}",
                            self.placements[next_idx].model
                        )
                    })?;
                let next_mat = matrices[next_idx];
                snap_link_matrix(
                    &mut matrices[cur_idx],
                    &next_mat,
                    self.placements[cur_idx].link_offset,
                    cur_bounds,
                    next_bounds,
                )?;
                cur_id_opt = prev_link.get(&cur_id).copied();
            }
        }
        Ok((matrices, outside_terrain))
    }
}
fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn neg3(v: [f32; 3]) -> [f32; 3] {
    [-v[0], -v[1], -v[2]]
}
fn snap_link_matrix(
    cur: &mut [f32; 16],
    next: &[f32; 16],
    link_offset: [f32; 3],
    cur_bounds: &crate::model::Bounds,
    next_bounds: &crate::model::Bounds,
) -> Result<()> {
    let cur_row2 = [cur[8], cur[9], cur[10]];
    let next_row0 = [next[0], next[1], next[2]];
    let next_row1 = [next[4], next[5], next[6]];
    let next_row2 = [next[8], next[9], next[10]];
    let f29 = dot3(cur_row2, next_row2);
    let f31 = dot3(cur_row2, next_row0);
    let (new_row0, new_row1, new_row2, rot_deg) = if f29.abs() > f31.abs() {
        if f29 > 0.0 {
            (next_row0, next_row1, next_row2, 0_i32)
        } else {
            (neg3(next_row0), next_row1, neg3(next_row2), 180_i32)
        }
    } else if f31 > 0.0 {
        (neg3(next_row2), next_row1, next_row0, 90_i32)
    } else {
        (next_row2, next_row1, neg3(next_row0), 270_i32)
    };
    cur[0..3].copy_from_slice(&new_row0);
    cur[4..7].copy_from_slice(&new_row1);
    cur[8..11].copy_from_slice(&new_row2);
    let dx = next[12] - cur[12];
    let dz = next[14] - cur[14];
    let len = dx.hypot(dz);
    let delta = if len > 0.0 {
        [dx / len, 0.0, dz / len]
    } else {
        [0.0, 0.0, 0.0]
    };
    let d_z = dot3(delta, next_row2);
    let d_x = dot3(delta, next_row0);
    let dir_deg = if d_z.abs() > d_x.abs() {
        if d_z > 0.0 { 0_i32 } else { 180_i32 }
    } else if d_x > 0.0 {
        90_i32
    } else {
        270_i32
    };
    let rel_deg = (dir_deg + 360 - rot_deg) % 360;
    let f29 = match rel_deg {
        180 => cur_bounds.min[2],
        90 => cur_bounds.max[0],
        270 => cur_bounds.min[0],
        _ => cur_bounds.max[2],
    } * (1023.0_f32 / 1024.0_f32);
    let f31 = f29.abs();
    let mut offset = [0.0_f32, 0.0_f32, 0.0_f32];
    match dir_deg {
        0 => offset[2] = next_bounds.min[2] - f31,
        180 => offset[2] = next_bounds.max[2] + f31,
        90 => offset[0] = next_bounds.min[0] - f31,
        270 => offset[0] = next_bounds.max[0] + f31,
        _ => {}
    }
    offset[0] += link_offset[0];
    offset[1] += link_offset[1];
    offset[2] += link_offset[2];
    let world_offset = [
        offset[0] * next_row0[0] + offset[1] * next_row1[0] + offset[2] * next_row2[0],
        offset[0] * next_row0[1] + offset[1] * next_row1[1] + offset[2] * next_row2[1],
        offset[0] * next_row0[2] + offset[1] * next_row1[2] + offset[2] * next_row2[2],
    ];
    cur[12] = next[12] + world_offset[0];
    cur[13] = next[13] + world_offset[1];
    cur[14] = next[14] + world_offset[2];
    if cur.iter().any(|v| !v.is_finite()) {
        return Err("Nonfinite linked placement matrix".into());
    }
    Ok(())
}
/// Community-correlated source coordinates. Validate against collision/original runtime next.
pub fn source_to_grid(x: f32, z: f32) -> [f32; 2] {
    [(x + 2048.0) * 3.0 / 16.0, (z + 2048.0) * 3.0 / 16.0]
}
pub fn grid_to_source(p: [f32; 3]) -> [f32; 3] {
    [
        p[0] * 16.0 / 3.0 - 2048.0,
        p[1] * 2.0,
        p[2] * 16.0 / 3.0 - 2048.0,
    ]
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coordinates_roundtrip() {
        let p = grid_to_source([348.0, 10.0, 612.0]);
        assert_eq!(source_to_grid(p[0], p[2]), [348.0, 612.0]);
        assert_eq!(p[1], 20.0);
    }
    #[test]
    fn matrix_validates() {
        assert!(matrix("1,0,0,0,0,1,0,0,0,0,1,0,4,5,6,1").is_ok());
        assert!(matrix("1,2").is_err());
        assert!(matrix("1,0,0,0,0,1,0,0,0,0,1,0,NaN,5,6,1").is_err());
    }
    #[test]
    fn destroyable_object_link_chain_propagates_tail_height_and_edge_snapping() {
        let terrain = crate::terrain::Terrain {
            map_width: 0,
            map_height: 0,
            records: 0,
            seam_comparisons: 0,
            seam_mismatches: 0,
            positions: vec![
                [384.0, 20.0, 384.0],
                [385.0, 20.0, 384.0],
                [384.0, 20.0, 385.0],
                [385.0, 20.0, 385.0],
                [386.0, 28.0, 384.0],
                [387.0, 28.0, 384.0],
                [386.0, 28.0, 385.0],
                [387.0, 28.0, 385.0],
            ],
            colors: Vec::new(),
            uv0: Vec::new(),
            uv1: Vec::new(),
            materials: Vec::new(),
            tile_materials: Vec::new(),
            indices: vec![0, 1, 2, 1, 3, 2, 4, 5, 6, 5, 7, 6],
        };
        let ground = crate::skirmish::Ground::from_terrain(&terrain);
        let mid_pos = grid_to_source([384.5, 0.0, 384.5]);
        let end_pos = grid_to_source([386.5, 0.0, 384.5]);
        let scene = Scene {
            placements: vec![
                Placement {
                    id: ObjectId(10),
                    kind: "cDestroyableObject".into(),
                    model: "bridge_mid".into(),
                    matrix: [
                        -1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, mid_pos[0],
                        0.0, mid_pos[2], 1.0,
                    ],
                    stick_to_floor: true,
                    next_link_object: Some(ObjectId(11)),
                    link_offset: [0.0, 0.0, 0.0],
                    attachments: Vec::new(),
                },
                Placement {
                    id: ObjectId(11),
                    kind: "cDestroyableObject".into(),
                    model: "bridge_end".into(),
                    matrix: [
                        -1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, end_pos[0],
                        0.0, end_pos[2], 1.0,
                    ],
                    stick_to_floor: true,
                    next_link_object: None,
                    link_offset: [0.0, 0.0, 0.0],
                    attachments: Vec::new(),
                },
            ],
            omitted: Vec::new(),
        };
        let mut bounds = std::collections::BTreeMap::new();
        bounds.insert(
            "bridge_mid".into(),
            crate::model::Bounds {
                min: [-13.0, -10.0, -5.0],
                max: [13.0, 13.0, 5.0],
            },
        );
        bounds.insert(
            "bridge_end".into(),
            crate::model::Bounds {
                min: [-9.5, -10.0, -5.0],
                max: [9.5, 0.0, 5.0],
            },
        );
        let (resolved, outside) = scene.resolve_placements(&ground, &bounds).unwrap();
        assert!(outside.is_empty());
        assert!((resolved[1][13] - 56.0).abs() < 1e-4);
        assert!((resolved[0][13] - 56.0).abs() < 1e-4);
        let expected_x = end_pos[0] - (9.5 + 13.0 * (1023.0 / 1024.0));
        assert!((resolved[0][12] - expected_x).abs() < 1e-3);
    }
}
