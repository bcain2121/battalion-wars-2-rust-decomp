//! Native bind-pose box descriptor selection, not a running physics backend.
use crate::{
    Result,
    model::{Bounds, Model},
    model_geometry::bind_transforms,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct CollisionBox {
    pub node_index: usize,
    pub name: String,
    pub local_bounds: Bounds,
    pub dimensions: [f32; 3],
    pub position: [f32; 3],
}
#[derive(Debug, Serialize)]
pub struct Groups {
    pub chassis: Vec<CollisionBox>,
    pub turret: Vec<CollisionBox>,
    pub barrel: Vec<CollisionBox>,
    pub seat_node: Option<usize>,
    pub turret_node: Option<usize>,
    pub barrel_node: Option<usize>,
    pub unselected_physics_nodes: Vec<usize>,
}
fn role(name: &str) -> Result<Option<u16>> {
    // A deliberately narrow subset of native longest-substring classification.
    // Do not silently call a noncanonical role name absent.
    let upper = name.to_ascii_uppercase();
    for (base, kind) in [
        ("PHYSICS_COLLIDE", 31),
        ("SEAT_A", 10),
        ("TURRET", 20),
        ("GUNBARREL", 21),
        ("BARREL", 21),
    ] {
        if !upper.contains(base) {
            continue;
        }
        if let Some(suffix) = name.strip_prefix(base) {
            let digits = suffix.strip_prefix('_').unwrap_or(suffix);
            if digits.is_empty()
                || (digits.bytes().all(|b| b.is_ascii_digit()) && digits.parse::<u16>().is_ok())
            {
                return Ok(Some(kind));
            }
        }
        return Err(format!(
            "Noncanonical body-role identity unsupported: {name}"
        ));
    }
    Ok(None)
}
fn translation(model: &Model, index: usize) -> [f32; 3] {
    model.nodes[index].transform[..3].try_into().unwrap()
}
fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| a[i] + b[i])
}
/// TCNC appends children (0x803275b4); instance cloning preserves that order
/// (0x8030dd98). Root zero and one connected tree are required here.
fn hierarchy(model: &Model) -> Result<Vec<Vec<usize>>> {
    bind_transforms(model)?;
    let n = model.nodes.len();
    if n == 0 || n > 4096 || model.nodes[0].parent.is_some() || model.hierarchy_links.len() != n - 1
    {
        return Err("Unsupported body hierarchy/root".into());
    }
    let mut children = vec![vec![]; n];
    let mut seen = vec![false; n];
    seen[0] = true;
    for &(parent, child) in &model.hierarchy_links {
        if parent >= n
            || child >= n
            || parent == child
            || seen[child]
            || model.nodes[child].parent != Some(parent)
        {
            return Err("Inconsistent ordered body hierarchy".into());
        }
        seen[child] = true;
        children[parent].push(child);
    }
    if seen.contains(&false) {
        return Err("Disconnected body hierarchy".into());
    }
    Ok(children)
}
fn child(
    children: &[Vec<usize>],
    roles: &[Option<u16>],
    parent: usize,
    kind: u16,
) -> Option<usize> {
    children[parent]
        .iter()
        .copied()
        .find(|i| roles[*i] == Some(kind))
}
fn first(children: &[Vec<usize>], roles: &[Option<u16>], kind: u16) -> Option<usize> {
    let mut stack = vec![0];
    while let Some(i) = stack.pop() {
        if roles[i] == Some(kind) {
            return Some(i);
        }
        stack.extend(children[i].iter().rev().copied());
    }
    None
}
fn chain(
    model: &Model,
    children: &[Vec<usize>],
    roles: &[Option<u16>],
    parent: usize,
    origin: Option<[f32; 3]>,
) -> Result<Vec<CollisionBox>> {
    let mut out = Vec::new();
    let mut next = child(children, roles, parent, 31);
    let mut accumulated = origin;
    while let Some(i) = next {
        if out.len() == 8 {
            return Err("Native body box capacity exceeded".into());
        }
        let node = &model.nodes[i];
        let bounds = node.bounds.ok_or("Body box lacks XOBB")?;
        let dimensions = std::array::from_fn(|a| bounds.max[a] - bounds.min[a]);
        if dimensions.iter().any(|d| !d.is_finite() || *d <= 0.0) {
            return Err("Invalid body box dimensions".into());
        }
        // 0x803686a0 / 0x803686e8: local position, then previous-box offset.
        // Not a full bind transform, bbox midpoint, or vertex-scale product.
        let local = translation(model, i);
        let position = accumulated.map_or(local, |previous| add(local, previous));
        if position.iter().any(|v| !v.is_finite()) {
            return Err("Body box position overflow".into());
        }
        accumulated = Some(position);
        out.push(CollisionBox {
            node_index: i,
            name: node.name.clone(),
            local_bounds: bounds,
            dimensions,
            position,
        });
        next = child(children, roles, i, 31);
    }
    Ok(out)
}
/// Port of the box-selection portion of 0x803685e4. Chassis uses the first
/// direct child; each continuation follows another direct child, NOT siblings.
/// Seat lookup alone is recursive. Turret/barrel offsets follow native sums.
/// No rigid bodies, backend mass scaling, contacts, joints or forces are created.
pub fn groups(model: &Model) -> Result<Groups> {
    let children = hierarchy(model)?;
    let roles = model
        .nodes
        .iter()
        .map(|n| role(&n.name))
        .collect::<Result<Vec<_>>>()?;
    let chassis = chain(model, &children, &roles, 0, None)?;
    if chassis.is_empty() {
        return Err("No direct-child chassis collision chain".into());
    }
    let seat_node = first(&children, &roles, 10);
    let turret_node = seat_node.and_then(|s| child(&children, &roles, s, 20));
    let barrel_node =
        seat_node.and_then(|s| child(&children, &roles, turret_node.unwrap_or(s), 21));
    let turret = if let (Some(s), Some(t)) = (seat_node, turret_node) {
        chain(
            model,
            &children,
            &roles,
            t,
            Some(add(translation(model, t), translation(model, s))),
        )?
    } else {
        vec![]
    };
    let barrel = if let (Some(s), Some(b)) = (seat_node, barrel_node) {
        let origin = if let Some(t) = turret_node {
            add(
                translation(model, s),
                add(translation(model, b), translation(model, t)),
            )
        } else {
            add(translation(model, b), translation(model, s))
        };
        chain(model, &children, &roles, b, Some(origin))?
    } else {
        vec![]
    };
    let used = chassis
        .iter()
        .chain(&turret)
        .chain(&barrel)
        .map(|b| b.node_index)
        .collect::<std::collections::BTreeSet<_>>();
    let unselected_physics_nodes = roles
        .iter()
        .enumerate()
        .filter_map(|(i, k)| (*k == Some(31) && !used.contains(&i)).then_some(i))
        .collect();
    Ok(Groups {
        chassis,
        turret,
        barrel,
        seat_node,
        turret_node,
        barrel_node,
        unselected_physics_nodes,
    })
}
