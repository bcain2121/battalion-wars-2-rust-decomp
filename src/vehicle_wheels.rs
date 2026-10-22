//! Bounded native wheel/interpolation slot discovery. Metadata only, not dynamics.
use crate::{
    Result,
    model::{Bounds, Model},
    model_geometry::bind_transforms,
};
use bevy::math::Vec3;

const WHEELS: [(&str, u16); 12] = [
    ("L_WHEEL_A", 97),
    ("L_WHEEL_B", 99),
    ("L_WHEEL_C", 100),
    ("L_WHEEL_D", 101),
    ("L_WHEEL_E", 102),
    ("L_WHEEL_F", 103),
    ("R_WHEEL_A", 104),
    ("R_WHEEL_B", 106),
    ("R_WHEEL_C", 107),
    ("R_WHEEL_D", 108),
    ("R_WHEEL_E", 109),
    ("R_WHEEL_F", 110),
];
const INTERPOLATION: [(&str, u16); 10] = [
    ("L_INTERP_AB", 111),
    ("L_INTERP_BC", 112),
    ("L_INTERP_CD", 113),
    ("L_INTERP_DE", 114),
    ("L_INTERP_EF", 115),
    ("R_INTERP_AB", 116),
    ("R_INTERP_BC", 117),
    ("R_INTERP_CD", 118),
    ("R_INTERP_DE", 119),
    ("R_INTERP_EF", 120),
];
#[derive(Debug, serde::Serialize)]
pub struct Part {
    pub node_index: usize,
    pub name: String,
    pub parent: Option<usize>,
    pub local_translation: [f32; 3],
    pub bind_translation: [f32; 3],
    pub local_bounds: Bounds,
    /// Native 0x80068c08..0x80068c20 / 0x80068d90..0x80068da8:
    /// abs((bbox.max.y - bbox.min.y) * 0.5), no additional vertex quantization scale.
    pub radius: f32,
}
#[derive(Debug, serde::Serialize)]
pub struct Slot {
    /// Offset in the native 0x6a8 descriptor, preserving absent slots.
    pub descriptor_offset: usize,
    pub requested_kind: u16,
    pub role: String,
    pub alternate_a: bool,
    pub part: Option<Part>,
}
/// Native lookup chooses the first matching node in a linked tree. The supported
/// subset requires unique exact names, making traversal-order differences irrelevant.
/// Indexed/case-folded variants are rejected instead of silently called absent.
fn unique(model: &Model, name: &str) -> Result<Option<usize>> {
    let mut found = None;
    for (i, node) in model.nodes.iter().enumerate() {
        let upper = node.name.to_ascii_uppercase();
        if !upper.contains(name) {
            continue;
        }
        // Native name classification tries longest substring matches after exact
        // matches (0x80348604). A_S is a distinct, longer role, not ordinary A.
        if matches!(name, "L_WHEEL_A" | "R_WHEEL_A") && upper.contains(&format!("{name}_S")) {
            continue;
        }
        if node.name != name {
            return Err(format!(
                "Noncanonical wheel identity unsupported: {}",
                node.name
            ));
        }
        if found.replace(i).is_some() {
            return Err(format!("Ambiguous wheel identity: {name}"));
        }
    }
    Ok(found)
}
/// Wheel roles at 0x804aa0d0 and interpolation roles at 0x804aa0dc, consumed by
/// 0x80068b04 and 0x80068d1c. A falls back to A_S only if its normal role is absent.
/// Auxiliary idlers, track links, body shapes and suspension remain separate.
pub fn slots(model: &Model) -> Result<Vec<Slot>> {
    let transforms = bind_transforms(model)?;
    let mut out = Vec::with_capacity(22);
    for (i, (name, kind)) in WHEELS.iter().chain(INTERPOLATION.iter()).enumerate() {
        let mut selected = unique(model, name)?;
        let mut alternate_a = false;
        if selected.is_none() && matches!(*name, "L_WHEEL_A" | "R_WHEEL_A") {
            selected = unique(model, &format!("{name}_S"))?;
            alternate_a = selected.is_some();
        }
        let part = selected
            .map(|index| -> Result<Part> {
                let node = &model.nodes[index];
                let bounds = node
                    .bounds
                    .ok_or("Wheel/interpolation node lacks XOBB bounds")?;
                if (0..3).any(|axis| {
                    !bounds.min[axis].is_finite()
                        || !bounds.max[axis].is_finite()
                        || bounds.min[axis] > bounds.max[axis]
                }) {
                    return Err("Invalid wheel/interpolation bounds".into());
                }
                let radius = ((bounds.max[1] - bounds.min[1]) * 0.5).abs();
                if !radius.is_finite() {
                    return Err("Wheel radius overflow".into());
                }
                Ok(Part {
                    radius,
                    node_index: index,
                    name: node.name.clone(),
                    parent: node.parent,
                    local_translation: node.transform[..3].try_into().unwrap(),
                    bind_translation: transforms[index].transform_point3(Vec3::ZERO).to_array(),
                    local_bounds: bounds,
                })
            })
            .transpose()?;
        out.push(Slot {
            descriptor_offset: i * 0x40,
            requested_kind: *kind,
            role: (*name).into(),
            alternate_a,
            part,
        });
    }
    Ok(out)
}
