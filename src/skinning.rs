//! Experimental CPU rest-pose/rotation-track skinning for recovery validation.
use crate::{Result, animation::Animation, model::Model};
use bevy::prelude::*;
pub fn pose(model: &Model, clip: Option<(&Animation, f32)>) -> Result<Vec<Mat4>> {
    pose_with_aim(model, clip, 0.0, 0.0)
}

pub fn pose_with_aim(
    model: &Model,
    clip: Option<(&Animation, f32)>,
    aim_yaw: f32,
    aim_pitch: f32,
) -> Result<Vec<Mat4>> {
    let mut locals = Vec::with_capacity(model.nodes.len());
    for node in &model.nodes {
        let f = node.transform;
        let mut translation = Vec3::new(f[0], f[1], f[2]);
        let mut q = Quat::from_xyzw(f[3], f[4], f[5], f[6]);
        if let Some((animation, t)) = clip
            && let Some(track) = animation.tracks.iter().find(|t| t.name == node.name)
        {
            if node.name != "BONE_CNTR_NODE"
                && let Some(p) = track.sample_position(t)
            {
                translation = Vec3::from_array(p);
            }
            if let Some(r) = track.sample_rotation(t) {
                q = Quat::from_array(r);
            }
        }
        if q.length() < 0.001 {
            return Err("Invalid bone quaternion".into());
        }
        locals.push(Mat4::from_rotation_translation(q.normalize(), translation));
    }
    let mut globals = Vec::with_capacity(model.nodes.len());
    for (i, node) in model.nodes.iter().enumerate() {
        let mut matrix = locals[i];
        let mut parent = node.parent;
        let mut steps = 0;
        while let Some(p) = parent {
            if p >= locals.len() || steps >= locals.len() {
                return Err("Invalid skin hierarchy".into());
            }
            matrix = locals[p] * matrix;
            parent = model.nodes[p].parent;
            steps += 1;
        }
        globals.push(matrix);
    }
    if aim_yaw.abs() > 1e-4 || aim_pitch.abs() > 1e-4 {
        let spine1 = model.nodes.iter().position(|n| n.name == "BONE_1_SPINE");
        let spine2 = model.nodes.iter().position(|n| n.name == "BONE_2_SPINE");
        let descends_from = |mut idx: usize, target: usize| -> bool {
            let mut steps = 0;
            while steps < model.nodes.len() {
                if idx == target {
                    return true;
                }
                let Some(p) = model.nodes[idx].parent else {
                    break;
                };
                idx = p;
                steps += 1;
            }
            false
        };
        if let Some(s1) = spine1 {
            let p1 = globals[s1].w_axis.truncate();
            let w1 = if spine2.is_some() { 0.45 } else { 1.0 };
            let rot1 = Mat4::from_translation(p1)
                * Mat4::from_rotation_y(aim_yaw * w1)
                * Mat4::from_rotation_x(aim_pitch * w1)
                * Mat4::from_translation(-p1);
            for (i, g) in globals.iter_mut().enumerate() {
                if descends_from(i, s1) {
                    *g = rot1 * *g;
                }
            }
        }
        if let Some(s2) = spine2 {
            let p2 = globals[s2].w_axis.truncate();
            let w2 = if spine1.is_some() { 0.55 } else { 1.0 };
            let rot2 = Mat4::from_translation(p2)
                * Mat4::from_rotation_y(aim_yaw * w2)
                * Mat4::from_rotation_x(aim_pitch * w2)
                * Mat4::from_translation(-p2);
            for (i, g) in globals.iter_mut().enumerate() {
                if descends_from(i, s2) {
                    *g = rot2 * *g;
                }
            }
        }
    }
    Ok(globals)
}
fn node_bind_global(model: &Model, start: usize) -> Result<Mat4> {
    let mut matrix = Mat4::IDENTITY;
    let mut current = Some(start);
    let mut steps = 0;
    while let Some(i) = current {
        if i >= model.nodes.len() || steps >= model.nodes.len() {
            return Err("Invalid skin hierarchy".into());
        }
        let node = &model.nodes[i];
        let f = node.transform;
        let translation = Vec3::new(f[0], f[1], f[2]);
        let q = Quat::from_xyzw(f[3], f[4], f[5], f[6]);
        if q.length() < 0.001 {
            return Err("Invalid bone quaternion".into());
        }
        matrix = Mat4::from_rotation_translation(q.normalize(), translation) * matrix;
        current = node.parent;
        steps += 1;
    }
    Ok(matrix)
}
pub fn deform(
    model: &Model,
    globals: &[Mat4],
    node_index: usize,
    surface_index: usize,
) -> Result<Vec<[f32; 3]>> {
    Ok(deform_positions_and_normals(model, globals, node_index, surface_index)?.0)
}

pub type DeformedSurface = (Vec<[f32; 3]>, Option<Vec<[f32; 3]>>);

pub fn deform_positions_and_normals(
    model: &Model,
    globals: &[Mat4],
    node_index: usize,
    surface_index: usize,
) -> Result<DeformedSurface> {
    let node = model.nodes.get(node_index).ok_or("Missing skin node")?;
    let surface = node
        .surfaces
        .get(surface_index)
        .ok_or("Missing skin surface")?;
    if !node.has_skin_data {
        let positions = surface.vertices.iter().map(|v| v.position).collect();
        let normals = surface
            .vertices
            .iter()
            .map(|v| v.normal)
            .collect::<Option<Vec<[f32; 3]>>>();
        return Ok((positions, normals));
    }
    if node.scale <= 0.0
        || node.inverse_bind.len() != model.nodes.len()
        || globals.len() != model.nodes.len()
    {
        return Err("Invalid skin matrix table".into());
    }
    // Precompute palette matrices once per surface instead of per vertex.
    let inverse_node = node_bind_global(model, node_index)?.inverse();
    let transforms = globals
        .iter()
        .zip(&node.inverse_bind)
        .map(|(g, b)| {
            let m = inverse_node * *g * Mat4::from_cols_array(b);
            let n = Mat3::from_mat4(m);
            (m, n)
        })
        .collect::<Vec<_>>();
    let inv_scale = 1.0 / node.scale;
    let mut positions = Vec::with_capacity(surface.vertices.len());
    let has_all_normals = surface.vertices.iter().all(|v| v.normal.is_some());
    let mut normals = if has_all_normals {
        Some(Vec::with_capacity(surface.vertices.len()))
    } else {
        None
    };
    for v in &surface.vertices {
        let index = v.matrix.ok_or("Skinned vertex lacks matrix selector")? as usize;
        if !index.is_multiple_of(3) {
            return Err("Invalid GX matrix selector".into());
        }
        let bone = *surface
            .bone_palette
            .get(index / 3)
            .ok_or("GX palette selector out of range")?;
        let &(matrix, norm_mat) = transforms.get(bone).ok_or("Bone index out of range")?;
        let result = matrix.transform_point3(Vec3::from_array(v.position) * inv_scale);
        if !result.is_finite() {
            return Err("Nonfinite skinned vertex".into());
        }
        positions.push(result.to_array());
        if let (Some(norm_out), Some(n)) = (&mut normals, v.normal) {
            let rn = (norm_mat * Vec3::from_array(n)).normalize_or_zero();
            norm_out.push(rn.to_array());
        }
    }
    Ok((positions, normals))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Model, Node, Surface, Vertex};

    fn node(name: &str, parent: Option<usize>, y: f32) -> Node {
        Node {
            name: name.to_string(),
            authored_kind: 0,
            transform: [0.0, y, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 0.0],
            parent,
            scale: 1.0,
            bounds: None,
            materials: Vec::new(),
            material_blend_modes: Vec::new(),
            surfaces: Vec::new(),
            tags: Vec::new(),
            has_skin_data: false,
            inverse_bind: Vec::new(),
        }
    }

    #[test]
    fn pose_with_aim_twists_spine_and_hands_while_keeping_feet_planted() {
        let mut root = node("BONE_PELVIS", None, 1.0);
        root.has_skin_data = true;
        root.inverse_bind = vec![Mat4::IDENTITY.to_cols_array(); 5];
        root.surfaces.push(Surface {
            material: 0,
            vertices: vec![Vertex {
                position: [0.0, 2.0, 0.0],
                normal: Some([0.0, 0.0, 1.0]),
                uv: [0.0, 0.0],
                matrix: Some(6),
            }],
            indices: vec![0, 0, 0],
            lod: 0,
            bone_palette: vec![0, 1, 2, 3, 4],
        });
        let model = Model {
            name: "test_grunt".into(),
            version: [1, 0],
            source: String::new(),
            header_sphere: [0.0; 4],
            nodes: vec![
                root,
                node("BONE_1_SPINE", Some(0), 0.5),
                node("BONE_2_SPINE", Some(1), 0.5),
                node("BONE_R_HAND", Some(2), 0.6),
                node("BONE_L_FOOT", Some(0), -1.0),
            ],
            hierarchy_links: Vec::new(),
        };
        let rest = pose(&model, None).unwrap();
        let aimed = pose_with_aim(&model, None, 0.4, -0.3).unwrap();
        assert_eq!(rest[0], aimed[0], "pelvis should remain unchanged");
        assert_eq!(rest[4], aimed[4], "left foot should remain planted");
        assert_ne!(rest[1], aimed[1], "spine 1 should rotate with aim");
        assert_ne!(rest[2], aimed[2], "spine 2 should rotate with aim");
        assert_ne!(rest[3], aimed[3], "hand should inherit spine aim rotation");
        let (pos_rest, norm_rest) = deform_positions_and_normals(&model, &rest, 0, 0).unwrap();
        let (pos_aim, norm_aim) = deform_positions_and_normals(&model, &aimed, 0, 0).unwrap();
        assert_ne!(
            pos_rest, pos_aim,
            "skinned vertex position should reflect aim"
        );
        assert_ne!(
            norm_rest.unwrap()[0],
            norm_aim.unwrap()[0],
            "skinned vertex normal should rotate with spine aim"
        );
    }
}
