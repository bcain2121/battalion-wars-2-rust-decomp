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

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SuspensionState {
    pub heave: f32,
    pub heave_vel: f32,
    pub pitch: f32,
    pub pitch_vel: f32,
    pub roll: f32,
    pub roll_vel: f32,
    pub wheel_spin: f32,
    pub wheel_deflections: [f32; 12],
}

impl SuspensionState {
    pub fn apply_recoil_impulse(&mut self, pitch_impulse: f32, heave_impulse: f32) {
        self.pitch_vel += pitch_impulse;
        self.heave_vel += heave_impulse;
    }

    pub fn step(
        &mut self,
        dt: f32,
        forward_speed: f32,
        target_pitch: f32,
        target_roll: f32,
        wheel_ground_deltas: &[f32; 12],
        avg_radius: f32,
    ) {
        let dt = dt.clamp(0.0, 0.05);
        let r = avg_radius.max(0.25);
        self.wheel_spin =
            (self.wheel_spin + forward_speed * dt / r).rem_euclid(std::f32::consts::TAU);

        let stiffness = 58.0f32;
        let damping = 11.5f32;
        let mean_delta = wheel_ground_deltas.iter().copied().sum::<f32>() / 12.0;
        let target_heave = mean_delta.clamp(-0.30, 0.30);

        let heave_acc = (target_heave - self.heave) * stiffness - self.heave_vel * damping;
        self.heave_vel += heave_acc * dt;
        self.heave = (self.heave + self.heave_vel * dt).clamp(-0.45, 0.45);

        let pitch_acc = (target_pitch - self.pitch) * stiffness - self.pitch_vel * damping;
        self.pitch_vel += pitch_acc * dt;
        self.pitch = (self.pitch + self.pitch_vel * dt).clamp(-0.65, 0.65);

        let roll_acc = (target_roll - self.roll) * stiffness - self.roll_vel * damping;
        self.roll_vel += roll_acc * dt;
        self.roll = (self.roll + self.roll_vel * dt).clamp(-0.65, 0.65);

        let blend = (dt * 18.0).clamp(0.0, 1.0);
        for (i, &delta) in wheel_ground_deltas.iter().enumerate() {
            let clamped = (delta - self.heave).clamp(-0.35, 0.35);
            self.wheel_deflections[i] += (clamped - self.wheel_deflections[i]) * blend;
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn step_with_physics(
        &mut self,
        dt: f32,
        forward_speed: f32,
        target_pitch: f32,
        target_roll: f32,
        wheel_ground_deltas: &[f32; 12],
        avg_radius: f32,
        physics: Option<&GroundVehiclePhysics>,
    ) {
        let dt = dt.clamp(0.0, 0.05);
        let r = avg_radius.max(0.25);
        self.wheel_spin =
            (self.wheel_spin + forward_speed * dt / r).rem_euclid(std::f32::consts::TAU);

        let stiffness = physics.map_or(58.0, |p| p.spring_stiffness.clamp(25.0, 120.0));
        let damping = physics.map_or(11.5, |p| p.spring_damping.clamp(5.0, 28.0));
        let mean_delta = wheel_ground_deltas.iter().copied().sum::<f32>() / 12.0;
        let target_heave = mean_delta.clamp(-0.30, 0.30);

        let heave_acc = (target_heave - self.heave) * stiffness - self.heave_vel * damping;
        self.heave_vel += heave_acc * dt;
        self.heave = (self.heave + self.heave_vel * dt).clamp(-0.45, 0.45);

        let pitch_acc = (target_pitch - self.pitch) * stiffness - self.pitch_vel * damping;
        self.pitch_vel += pitch_acc * dt;
        self.pitch = (self.pitch + self.pitch_vel * dt).clamp(-0.65, 0.65);

        let roll_acc = (target_roll - self.roll) * stiffness - self.roll_vel * damping;
        self.roll_vel += roll_acc * dt;
        self.roll = (self.roll + self.roll_vel * dt).clamp(-0.65, 0.65);

        let blend = (dt * 18.0).clamp(0.0, 1.0);
        for (i, &delta) in wheel_ground_deltas.iter().enumerate() {
            let clamped = (delta - self.heave).clamp(-0.35, 0.35);
            self.wheel_deflections[i] += (clamped - self.wheel_deflections[i]) * blend;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundVehiclePhysics {
    pub attack_speed: f32,
    pub reverse_speed: f32,
    pub drive_accln: f32,
    pub braking_accln: f32,
    pub max_turn_rate: f32,
    pub chassis_mass: f32,
    pub spring_stiffness: f32,
    pub spring_damping: f32,
    pub fire_impulse: f32,
    pub side_slide_grip: f32,
}

impl Default for GroundVehiclePhysics {
    fn default() -> Self {
        Self {
            attack_speed: 13.5,
            reverse_speed: 9.5,
            drive_accln: 16.0,
            braking_accln: 50.0,
            max_turn_rate: 1.2,
            chassis_mass: 60.0,
            spring_stiffness: 58.0,
            spring_damping: 11.5,
            fire_impulse: 40.0,
            side_slide_grip: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterVehiclePhysics {
    pub max_speed: f32,
    pub max_reverse_speed: f32,
    pub bob_roll_amount: f32,
    pub bob_pitch_amount: f32,
    pub bob_speed: f32,
    pub roll_on_turn: f32,
    pub pitch_on_accln: f32,
}

impl Default for WaterVehiclePhysics {
    fn default() -> Self {
        Self {
            max_speed: 10.0,
            max_reverse_speed: 5.0,
            bob_roll_amount: 0.032,
            bob_pitch_amount: 0.025,
            bob_speed: 2.5,
            roll_on_turn: 0.15,
            pitch_on_accln: 0.06,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnitCameraProfile {
    pub cam_pos: [f32; 3],
    pub cam_look_pos: [f32; 3],
    pub cam_pos_2: [f32; 3],
    pub cam_look_pos_2: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeatDisplay {
    Sitting,
    Standing,
    ToShoulders,
    HeadOnly,
    Invisible,
}

impl SeatDisplay {
    pub fn is_visible(self) -> bool {
        !matches!(self, Self::Invisible)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VehicleSeatSlot {
    pub display: SeatDisplay,
    pub is_driver: bool,
    pub has_weapon: bool,
    pub local_offset: [f32; 3],
}

fn parse_vec3_field(obj: &crate::Object, key: &str) -> Option<[f32; 3]> {
    let raw = crate::original_scene::field(obj, key)?;
    let mut parts = raw
        .split(',')
        .filter_map(|s| s.trim().parse::<f32>().ok())
        .filter(|v| v.is_finite());
    let x = parts.next()?;
    let y = parts.next()?;
    let z = parts.next()?;
    Some([x, y, z])
}

pub fn ground_vehicle_physics(
    graph: &crate::ObjectGraph,
) -> std::collections::BTreeMap<crate::ObjectId, GroundVehiclePhysics> {
    use crate::original_scene::{field, reference, resolve_base};
    let mut out = std::collections::BTreeMap::new();
    for obj in graph
        .objects
        .values()
        .filter(|o| o.kind == "cGroundVehicle")
    {
        let Some(base) = resolve_base(graph, obj) else {
            continue;
        };
        let mut phys = GroundVehiclePhysics::default();
        if let Some(pbase) = reference(graph, base, "mPhysicsBase") {
            let num = |k: &str, def: f32| -> f32 {
                field(pbase, k)
                    .and_then(|s| s.parse::<f32>().ok())
                    .filter(|v| v.is_finite() && *v > 0.0)
                    .unwrap_or(def)
            };
            phys.attack_speed = num("attackSpeed", phys.attack_speed);
            phys.reverse_speed = num("reverseSpeed", phys.reverse_speed);
            phys.drive_accln = num("driveAccln", phys.drive_accln);
            phys.braking_accln = num("brakingAccln", phys.braking_accln);
            phys.max_turn_rate = num("maxTurnRate", phys.max_turn_rate);
            if let Some(ref_params) = reference(graph, pbase, "mNewPhysParams") {
                let rnum = |k: &str, def: f32| -> f32 {
                    field(ref_params, k)
                        .and_then(|s| s.parse::<f32>().ok())
                        .filter(|v| v.is_finite() && *v > 0.0)
                        .unwrap_or(def)
                };
                phys.chassis_mass = rnum("mChassisMass", phys.chassis_mass);
                phys.spring_stiffness =
                    rnum("mSuspensionSpringStiffness", phys.spring_stiffness).clamp(25.0, 120.0);
                phys.spring_damping =
                    rnum("mSuspensionSpringDamping", phys.spring_damping).clamp(5.0, 28.0);
                phys.fire_impulse = rnum("mFireImpulse", phys.fire_impulse);
                phys.side_slide_grip = rnum("mSideSlideGripFactor", phys.side_slide_grip);
            }
        }
        out.insert(obj.id, phys);
    }
    out
}

pub fn water_vehicle_physics(
    graph: &crate::ObjectGraph,
) -> std::collections::BTreeMap<crate::ObjectId, WaterVehiclePhysics> {
    use crate::original_scene::{field, reference, resolve_base};
    let mut out = std::collections::BTreeMap::new();
    for obj in graph.objects.values().filter(|o| o.kind == "cWaterVehicle") {
        let Some(base) = resolve_base(graph, obj) else {
            continue;
        };
        let mut phys = WaterVehiclePhysics::default();
        if let Some(pbase) = reference(graph, base, "mPhysicsBase") {
            let num = |k: &str, def: f32| -> f32 {
                field(pbase, k)
                    .and_then(|s| s.parse::<f32>().ok())
                    .filter(|v| v.is_finite() && *v >= 0.0)
                    .unwrap_or(def)
            };
            phys.max_speed = num("mMaxSpeed", phys.max_speed).max(2.0);
            phys.max_reverse_speed = num("mMaxReverseSpeed", phys.max_reverse_speed).max(1.0);
            let deg_or_rad = |v: f32| -> f32 { if v > 0.35 { v.to_radians() } else { v } };
            phys.bob_roll_amount = deg_or_rad(num("mBobRollAmount", 1.8)).clamp(0.005, 0.12);
            phys.bob_pitch_amount = deg_or_rad(num("mBobPitchAmount", 1.4)).clamp(0.005, 0.10);
            phys.bob_speed = num("mBobSpeed", phys.bob_speed).clamp(0.5, 6.0);
            phys.roll_on_turn = num("mRollOnTurnCoefficient", phys.roll_on_turn).clamp(0.0, 0.5);
            phys.pitch_on_accln =
                num("mPitchOnAcclnCoefficient", phys.pitch_on_accln).clamp(0.0, 0.3);
        }
        out.insert(obj.id, phys);
    }
    out
}

pub fn unit_camera_profiles(
    graph: &crate::ObjectGraph,
) -> std::collections::BTreeMap<crate::ObjectId, UnitCameraProfile> {
    use crate::original_scene::resolve_base;
    let mut out = std::collections::BTreeMap::new();
    for obj in graph.objects.values() {
        if !matches!(
            obj.kind.as_str(),
            "cTroop" | "cGroundVehicle" | "cAirVehicle" | "cWaterVehicle"
        ) {
            continue;
        }
        let Some(base) = resolve_base(graph, obj) else {
            continue;
        };
        let Some(cam_pos) = parse_vec3_field(base, "mCamPos") else {
            continue;
        };
        let cam_look_pos = parse_vec3_field(base, "mCamLookPos").unwrap_or([0.0, 1.6, 0.0]);
        let cam_pos_2 = parse_vec3_field(base, "mCamPos2").unwrap_or(cam_pos);
        let cam_look_pos_2 = parse_vec3_field(base, "mCamLookPos2").unwrap_or(cam_look_pos);
        out.insert(
            obj.id,
            UnitCameraProfile {
                cam_pos,
                cam_look_pos,
                cam_pos_2,
                cam_look_pos_2,
            },
        );
    }
    out
}

pub fn vehicle_seat_slots(
    graph: &crate::ObjectGraph,
) -> std::collections::BTreeMap<crate::ObjectId, Vec<VehicleSeatSlot>> {
    use crate::original_scene::{field, resolve_base};
    let mut out = std::collections::BTreeMap::new();
    for obj in graph.objects.values() {
        if !matches!(
            obj.kind.as_str(),
            "cGroundVehicle" | "cAirVehicle" | "cWaterVehicle"
        ) {
            continue;
        }
        let Some(base) = resolve_base(graph, obj) else {
            continue;
        };
        let Some(seat_field) = base
            .fields
            .iter()
            .find(|f| f.attributes.get("name").map(String::as_str) == Some("mSeatData"))
        else {
            continue;
        };
        let mut slots = Vec::new();
        for (seat_idx, raw_id) in seat_field.items.iter().enumerate() {
            let Ok(sid) = raw_id.trim().parse::<u64>() else {
                continue;
            };
            if sid == 0 {
                continue;
            }
            let Some(seat_obj) = graph.objects.get(&crate::ObjectId(sid)) else {
                continue;
            };
            let display = match field(seat_obj, "DisplayType").unwrap_or("") {
                "SEATDISPLAY_SOLDER_SITTING" => SeatDisplay::Sitting,
                "SEATDISPLAY_SOLDER_STANDING" => SeatDisplay::Standing,
                "SEATDISPLAY_SOLDER_TOSHOULDERS" => SeatDisplay::ToShoulders,
                "SEATDISPLAY_SOLDER_HEADONLY" => SeatDisplay::HeadOnly,
                _ => SeatDisplay::Invisible,
            };
            let flags = field(seat_obj, "SeatFlags")
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);
            let is_driver = (flags & 1) != 0 || seat_idx == 0;
            let has_weapon = field(seat_obj, "WeaponType").is_some_and(|w| w != "0")
                || field(seat_obj, "PlayerWeaponType").is_some_and(|w| w != "0");
            let local_offset = match (seat_idx, display) {
                (0, SeatDisplay::Sitting) => [-0.45, 0.95, 0.35],
                (1, SeatDisplay::Sitting) => [0.45, 0.95, 0.35],
                (1, SeatDisplay::Standing) => [0.0, 1.35, -0.55],
                (0, SeatDisplay::ToShoulders) => [0.0, 1.55, 0.15],
                (_, SeatDisplay::ToShoulders) => [
                    if seat_idx % 2 == 0 { -0.55 } else { 0.55 },
                    1.55,
                    -0.25 * (seat_idx as f32),
                ],
                (_, SeatDisplay::HeadOnly) => [0.0, 1.75, 0.20],
                (_, SeatDisplay::Standing) => [
                    if seat_idx % 2 == 0 { -0.55 } else { 0.55 },
                    1.25,
                    -0.45 * (seat_idx as f32),
                ],
                _ => [
                    if seat_idx % 2 == 0 { -0.48 } else { 0.48 },
                    1.05,
                    0.20 - 0.55 * (seat_idx as f32),
                ],
            };
            slots.push(VehicleSeatSlot {
                display,
                is_driver,
                has_weapon,
                local_offset,
            });
        }
        if !slots.is_empty() {
            out.insert(obj.id, slots);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suspension_advances_wheel_spin_and_settles_spring_damper_pitch_and_heave() {
        let mut s = SuspensionState::default();
        s.apply_recoil_impulse(-1.2, -0.4);
        let deltas = [
            0.08, 0.08, 0.04, 0.0, -0.04, -0.08, 0.08, 0.08, 0.04, 0.0, -0.04, -0.08,
        ];
        let phys = GroundVehiclePhysics::default();
        for _ in 0..30 {
            s.step_with_physics(1.0 / 60.0, 8.0, 0.12, -0.05, &deltas, 0.5, Some(&phys));
        }
        assert!(s.wheel_spin > 0.0);
        assert!((s.pitch - 0.12).abs() < 0.06);
        assert!((s.roll - (-0.05)).abs() < 0.04);
        assert!(s.wheel_deflections[0] > s.wheel_deflections[5]);
    }

    #[test]
    fn extracts_vehicle_physics_camera_profiles_and_seat_slots_from_xml_graph() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<Instances>
  <Object type="cSeatBase" id="50">
    <Attribute name="DisplayType" type="eSeatDisplayType" elements="1">
      <Item>SEATDISPLAY_SOLDER_SITTING</Item>
    </Attribute>
    <Attribute name="SeatFlags" type="sUInt32" elements="1">
      <Item>5</Item>
    </Attribute>
    <Attribute name="WeaponType" type="cWeaponBase" elements="1">
      <Item>0</Item>
    </Attribute>
  </Object>
  <Object type="cSeatBase" id="51">
    <Attribute name="DisplayType" type="eSeatDisplayType" elements="1">
      <Item>SEATDISPLAY_SOLDER_STANDING</Item>
    </Attribute>
    <Attribute name="SeatFlags" type="sUInt32" elements="1">
      <Item>6</Item>
    </Attribute>
    <Attribute name="WeaponType" type="cWeaponBase" elements="1">
      <Item>99</Item>
    </Attribute>
  </Object>
  <Object type="cReflectedPhysicsParams" id="59">
    <Attribute name="mChassisMass" type="sFloat" elements="1"><Item>18000</Item></Attribute>
    <Attribute name="mSuspensionSpringStiffness" type="sFloat" elements="1"><Item>55.0</Item></Attribute>
    <Attribute name="mSuspensionSpringDamping" type="sFloat" elements="1"><Item>12.5</Item></Attribute>
    <Attribute name="mFireImpulse" type="sFloat" elements="1"><Item>12000</Item></Attribute>
    <Attribute name="mSideSlideGripFactor" type="sFloat" elements="1"><Item>7.2</Item></Attribute>
  </Object>
  <Object type="cGroundVehiclePhysicsBase" id="60">
    <Attribute name="attackSpeed" type="sFloat" elements="1"><Item>15.5</Item></Attribute>
    <Attribute name="reverseSpeed" type="sFloat" elements="1"><Item>8.0</Item></Attribute>
    <Attribute name="driveAccln" type="sFloat" elements="1"><Item>60</Item></Attribute>
    <Attribute name="brakingAccln" type="sFloat" elements="1"><Item>110</Item></Attribute>
    <Attribute name="maxTurnRate" type="sFloat" elements="1"><Item>2.1</Item></Attribute>
    <Pointer name="mNewPhysParams" type="cReflectedPhysicsParams" elements="1"><Item>59</Item></Pointer>
  </Object>
  <Object type="cGroundVehicleBase" id="70">
    <Attribute name="mCamPos" type="cVec3" elements="1"><Item>-5.500,3.200,8.400</Item></Attribute>
    <Attribute name="mCamLookPos" type="cVec3" elements="1"><Item>0.000,1.600,2.500</Item></Attribute>
    <Pointer name="mPhysicsBase" type="cGroundVehiclePhysicsBase" elements="1"><Item>60</Item></Pointer>
    <Pointer name="mSeatData" type="cSeatBase" elements="2">
      <Item>50</Item>
      <Item>51</Item>
    </Pointer>
  </Object>
  <Object type="cGroundVehicle" id="71">
    <Pointer name="mBase" type="cGroundVehicleBase" elements="1"><Item>70</Item></Pointer>
  </Object>
  <Object type="cWaterVehiclePhysicsBase" id="80">
    <Attribute name="mMaxSpeed" type="sFloat" elements="1"><Item>13.0</Item></Attribute>
    <Attribute name="mMaxReverseSpeed" type="sFloat" elements="1"><Item>6.5</Item></Attribute>
    <Attribute name="mBobRollAmount" type="sFloat" elements="1"><Item>4.5</Item></Attribute>
    <Attribute name="mBobPitchAmount" type="sFloat" elements="1"><Item>2.0</Item></Attribute>
    <Attribute name="mBobSpeed" type="sFloat" elements="1"><Item>0.12</Item></Attribute>
    <Attribute name="mRollOnTurn" type="sFloat" elements="1"><Item>8.0</Item></Attribute>
  </Object>
  <Object type="cWaterVehicleBase" id="81">
    <Pointer name="mPhysicsBase" type="cWaterVehiclePhysicsBase" elements="1"><Item>80</Item></Pointer>
  </Object>
  <Object type="cWaterVehicle" id="82">
    <Pointer name="mBase" type="cWaterVehicleBase" elements="1"><Item>81</Item></Pointer>
  </Object>
</Instances>"#;
        let mut graph = crate::ObjectGraph::default();
        graph
            .merge(crate::parse_objects(xml).expect("valid xml"))
            .expect("merge");
        let gphys = ground_vehicle_physics(&graph);
        let p = gphys
            .get(&crate::ObjectId(71))
            .expect("ground vehicle physics");
        assert!((p.attack_speed - 15.5).abs() < 1e-4);
        assert!((p.chassis_mass - 18000.0).abs() < 1e-4);
        assert!((p.fire_impulse - 12000.0).abs() < 1e-4);

        let wphys = water_vehicle_physics(&graph);
        let wp = wphys
            .get(&crate::ObjectId(82))
            .expect("water vehicle physics");
        assert!((wp.max_speed - 13.0).abs() < 1e-4);
        assert!((wp.bob_roll_amount - 4.5f32.to_radians()).abs() < 1e-4);

        let cams = unit_camera_profiles(&graph);
        let cam = cams.get(&crate::ObjectId(71)).expect("camera profile");
        assert!((cam.cam_pos[2] - 8.4).abs() < 1e-4);
        assert!((cam.cam_look_pos[1] - 1.6).abs() < 1e-4);

        let seats = vehicle_seat_slots(&graph);
        let v_seats = seats.get(&crate::ObjectId(71)).expect("seat slots");
        assert_eq!(v_seats.len(), 2);
        assert_eq!(v_seats[0].display, SeatDisplay::Sitting);
        assert!(v_seats[0].is_driver);
        assert!(!v_seats[0].has_weapon);
        assert_eq!(v_seats[1].display, SeatDisplay::Standing);
        assert!(v_seats[1].has_weapon);
    }
}
