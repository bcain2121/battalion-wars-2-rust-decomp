//! Experimental CPU rest-pose/rotation-track skinning for recovery validation.
use crate::{Result, animation::Animation, model::Model};
use bevy::prelude::*;
pub fn pose(model: &Model, clip: Option<(&Animation, f32)>) -> Result<Vec<Mat4>> {
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
    let node = model.nodes.get(node_index).ok_or("Missing skin node")?;
    let surface = node
        .surfaces
        .get(surface_index)
        .ok_or("Missing skin surface")?;
    if !node.has_skin_data {
        return Ok(surface.vertices.iter().map(|v| v.position).collect());
    }
    if node.scale <= 0.0
        || node.inverse_bind.len() != model.nodes.len()
        || globals.len() != model.nodes.len()
    {
        return Err("Invalid skin matrix table".into());
    }
    // Mesh entity already has this node's rest transform. Remove it from CPU vertices.
    let inverse_node = node_bind_global(model, node_index)?.inverse();
    let transforms = globals
        .iter()
        .zip(&node.inverse_bind)
        .map(|(g, b)| inverse_node * *g * Mat4::from_cols_array(b))
        .collect::<Vec<_>>();
    surface
        .vertices
        .iter()
        .map(|v| {
            let index = v.matrix.ok_or("Skinned vertex lacks matrix selector")? as usize;
            if !index.is_multiple_of(3) {
                return Err("Invalid GX matrix selector".into());
            }
            let bone = *surface
                .bone_palette
                .get(index / 3)
                .ok_or("GX palette selector out of range")?;
            let matrix = transforms.get(bone).ok_or("Bone index out of range")?;
            let result = matrix.transform_point3(Vec3::from_array(v.position) / node.scale);
            if !result.is_finite() {
                return Err("Nonfinite skinned vertex".into());
            }
            Ok(result.to_array())
        })
        .collect()
}
