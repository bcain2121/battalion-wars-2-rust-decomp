//! Shared bind-space transforms and authored physics-node geometry. No hull inference from inertia.
use crate::{
    Result,
    model::{Model, Node},
};
use bevy::math::{Mat4, Quat, Vec3};
fn local(node: &Node) -> Result<Mat4> {
    let f = &node.transform;
    if f[..7].iter().any(|v| !v.is_finite()) {
        return Err("Nonfinite model transform".into());
    }
    let q = Quat::from_xyzw(f[3], f[4], f[5], f[6]);
    let length = q.length();
    if !length.is_finite() || length < 0.0001 {
        return Err("Invalid model quaternion".into());
    }
    Ok(Mat4::from_rotation_translation(
        q / length,
        Vec3::new(f[0], f[1], f[2]),
    ))
}
pub fn bind_transforms(model: &Model) -> Result<Vec<Mat4>> {
    if model.nodes.len() > 4096 {
        return Err("Model transform node budget".into());
    }
    let mut out = Vec::with_capacity(model.nodes.len());
    for (index, node) in model.nodes.iter().enumerate() {
        let mut transform = local(node)?;
        let mut parent = node.parent;
        let mut depth = 0;
        while let Some(p) = parent {
            depth += 1;
            if depth > model.nodes.len() || p == index {
                return Err("Model transform ancestry cycle".into());
            }
            let ancestor = model
                .nodes
                .get(p)
                .ok_or("Model transform parent out of range")?;
            transform = local(ancestor)? * transform;
            parent = ancestor.parent;
        }
        if !transform.is_finite() {
            return Err("Model transform overflow".into());
        }
        out.push(transform);
    }
    Ok(out)
}
pub fn hierarchy_bounds(model: &Model) -> Result<crate::model::Bounds> {
    if model.nodes.is_empty() {
        return Err("Model has no nodes for hierarchy bounds".into());
    }
    let transforms = bind_transforms(model)?;
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for (index, node) in model.nodes.iter().enumerate() {
        let bounds = node.bounds.unwrap_or(crate::model::Bounds {
            min: [-1.0, -1.0, -1.0],
            max: [1.0, 1.0, 1.0],
        });
        if (0..3).any(|axis| {
            !bounds.min[axis].is_finite()
                || !bounds.max[axis].is_finite()
                || bounds.min[axis] > bounds.max[axis]
        }) {
            return Err("Invalid model node bounds".into());
        }
        for bits in 0..8 {
            let p = Vec3::from_array(std::array::from_fn(|axis| {
                if bits & (1 << axis) == 0 {
                    bounds.min[axis]
                } else {
                    bounds.max[axis]
                }
            }));
            let p = transforms[index].transform_point3(p);
            if !p.is_finite() {
                return Err("Model hierarchy bounds transform overflow".into());
            }
            min = min.min(p);
            max = max.max(p);
        }
    }
    Ok(crate::model::Bounds {
        min: min.to_array(),
        max: max.to_array(),
    })
}
#[derive(Debug, serde::Serialize)]
pub struct PhysicsNode {
    pub node_index: usize,
    pub name: String,
    pub parent: Option<usize>,
    pub marker_triangles: Vec<[[f32; 3]; 3]>,
    pub local_bounds: crate::model::Bounds,
    pub box_corners: [[f32; 3]; 8],
    pub min: [f32; 3],
    pub max: [f32; 3],
}
/// Original PHYSICS_COLLIDE nodes, including articulated children at bind pose.
/// Their visible marker triangles are NOT closed collision hulls. Preserve the
/// separate XOBB bounds and hierarchy; applying them to a dynamics engine is separate.
pub fn physics_nodes(model: &Model) -> Result<Vec<PhysicsNode>> {
    let transforms = bind_transforms(model)?;
    let mut out = Vec::new();
    let mut total = 0;
    for (index, node) in model
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.name.starts_with("PHYSICS_COLLIDE"))
    {
        if node.has_skin_data {
            return Err("Skinned physics geometry unsupported".into());
        }
        let bounds = node
            .bounds
            .ok_or("Physics node lacks authored XOBB bounds")?;
        if (0..3).any(|axis| {
            !bounds.min[axis].is_finite()
                || !bounds.max[axis].is_finite()
                || bounds.min[axis] > bounds.max[axis]
        }) {
            return Err("Invalid physics node bounds".into());
        }
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        let mut box_corners = [[0.0; 3]; 8];
        for (bits, corner) in box_corners.iter_mut().enumerate() {
            let p = Vec3::from_array(std::array::from_fn(|axis| {
                if bits & (1 << axis) == 0 {
                    bounds.min[axis]
                } else {
                    bounds.max[axis]
                }
            }));
            let p = transforms[index].transform_point3(p);
            if !p.is_finite() {
                return Err("Physics bounds transform overflow".into());
            }
            min = min.min(p);
            max = max.max(p);
            *corner = p.to_array();
        }
        let mut triangles = Vec::new();
        for surface in &node.surfaces {
            if surface.lod != 0 || !surface.bone_palette.is_empty() {
                return Err("Unsupported physics surface LOD/palette".into());
            }
            if !surface.indices.len().is_multiple_of(3) {
                return Err("Incomplete physics triangle".into());
            }
            for indices in surface.indices.chunks_exact(3) {
                total += 1;
                if total > 1_000_000 {
                    return Err("Physics triangle budget".into());
                }
                let mut triangle = [[0.0; 3]; 3];
                for (dest, id) in triangle.iter_mut().zip(indices) {
                    let vertex = surface
                        .vertices
                        .get(*id as usize)
                        .ok_or("Physics vertex index out of range")?;
                    if vertex.matrix.is_some() {
                        return Err("Matrix-weighted physics vertex unsupported".into());
                    }
                    let p = transforms[index].transform_point3(Vec3::from_array(vertex.position));
                    if !p.is_finite() {
                        return Err("Nonfinite physics vertex".into());
                    }
                    *dest = p.to_array();
                }
                triangles.push(triangle);
            }
        }
        if triangles.is_empty() {
            return Err(format!("Empty physics geometry {}", node.name));
        }
        out.push(PhysicsNode {
            node_index: index,
            name: node.name.clone(),
            parent: node.parent,
            marker_triangles: triangles,
            local_bounds: bounds,
            box_corners,
            min: min.to_array(),
            max: max.to_array(),
        });
    }
    if out.is_empty() {
        return Err("Model has no authored PHYSICS_COLLIDE meshes".into());
    }
    Ok(out)
}
