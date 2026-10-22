//! Narrow Rust scripted-aircraft adapter: horizontal GoToArea at source cruise speed,
//! maintaining authored altitude. No dogfighting, water movement or player flight controls.
use crate::{ObjectGraph, ObjectId, Result, original_scene::field};
use std::collections::BTreeMap;
/// Rust aircraft placement: planar coordinates + heading in degrees. The optional
/// native clearance-search parameter is currently supported only at zero.
/// Native aircraft placement uses terrain height plus flight-data cruise HAG.
pub fn teleport(
    objects: &BTreeMap<ObjectId, crate::Object>,
    scene: &crate::unit_collision::Scene,
    id: ObjectId,
    x: f64,
    z: f64,
    degrees: f64,
) -> Result<([f32; 3], f32)> {
    if [x, z, degrees].iter().any(|v| !v.is_finite())
        || !(x as f32).is_finite()
        || !(z as f32).is_finite()
    {
        return Err("Invalid aircraft teleport coordinates/heading".into());
    }
    let mut object = objects.get(&id).ok_or("Unknown teleport aircraft")?;
    if object.kind != "cAirVehicle" {
        return Err("Teleport for non-aircraft is not configured".into());
    }
    for key in ["mBase", "mPhysicsBase", "mFlightMode"] {
        let next = field(object, key)
            .ok_or("Missing aircraft teleport profile")?
            .parse::<u64>()
            .map_err(|_| "Invalid aircraft teleport reference")?;
        object = objects
            .get(&ObjectId(next))
            .ok_or("Unknown aircraft teleport profile")?;
    }
    if object.kind != "cFlightData" {
        return Err("Unsupported aircraft teleport flight mode".into());
    }
    let height = field(object, "mCruiseHAG")
        .ok_or("Missing cruise altitude")?
        .parse::<f32>()
        .map_err(|_| "Invalid cruise altitude")?;
    if !height.is_finite() || height <= 0. || height > 1000. {
        return Err("Invalid cruise altitude".into());
    }
    let position = scene
        .world_point([x as f32, 0., z as f32], height)
        .or_else(|e| {
            if scene.ground.is_some() {
                Ok([x as f32, height, z as f32])
            } else {
                Err(e)
            }
        })?;
    Ok((position, degrees.rem_euclid(360.).to_radians() as f32))
}
pub fn cruise_speeds(graph: &ObjectGraph) -> Result<BTreeMap<ObjectId, f64>> {
    let mut speeds = BTreeMap::new();
    for object in graph.objects.values().filter(|o| o.kind == "cAirVehicle") {
        let mut current = object;
        for key in ["mBase", "mPhysicsBase", "mFlightMode"] {
            let id = field(current, key)
                .ok_or_else(|| format!("Missing aircraft {}.{key}", current.id.0))?
                .parse::<u64>()
                .map_err(|_| "Invalid aircraft reference")?;
            current = graph
                .objects
                .get(&ObjectId(id))
                .ok_or("Unknown aircraft reference")?;
        }
        if current.kind != "cFlightData" {
            return Err("Unsupported flight profile".into());
        }
        let speed = field(current, "mCruiseSpeed")
            .ok_or("Missing aircraft cruise speed")?
            .parse::<f64>()
            .map_err(|_| "Invalid aircraft cruise speed")?;
        if !speed.is_finite() || speed <= 0. || speed > 1000. {
            return Err("Invalid aircraft cruise speed".into());
        }
        speeds.insert(object.id, speed);
    }
    Ok(speeds)
}
pub fn cruise_altitudes(graph: &ObjectGraph) -> Result<BTreeMap<ObjectId, f32>> {
    let mut altitudes = BTreeMap::new();
    for object in graph.objects.values().filter(|o| o.kind == "cAirVehicle") {
        let mut current = object;
        for key in ["mBase", "mPhysicsBase", "mFlightMode"] {
            let id = field(current, key)
                .ok_or_else(|| format!("Missing aircraft {}.{key}", current.id.0))?
                .parse::<u64>()
                .map_err(|_| "Invalid aircraft reference")?;
            current = graph
                .objects
                .get(&ObjectId(id))
                .ok_or("Unknown aircraft reference")?;
        }
        if current.kind != "cFlightData" {
            return Err("Unsupported flight profile".into());
        }
        let height = field(current, "mCruiseHAG")
            .ok_or("Missing cruise altitude")?
            .parse::<f32>()
            .map_err(|_| "Invalid cruise altitude")?;
        if !height.is_finite() || height <= 0. || height > 1000. {
            return Err("Invalid cruise altitude".into());
        }
        altitudes.insert(object.id, height);
    }
    Ok(altitudes)
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightProfile {
    pub min_speed: f64,
    pub cruise_speed: f64,
    pub max_speed: f64,
    pub strafe_speed: f64,
    pub up_speed: f64,
    pub down_speed: f64,
    pub turn_rate: f64,
    pub min_hag: f32,
    pub cruise_hag: f32,
    pub max_hag: f32,
}
pub fn flight_profiles(graph: &ObjectGraph) -> Result<BTreeMap<ObjectId, FlightProfile>> {
    let mut profiles = BTreeMap::new();
    for object in graph.objects.values().filter(|o| o.kind == "cAirVehicle") {
        let mut current = object;
        for key in ["mBase", "mPhysicsBase", "mFlightMode"] {
            let id = field(current, key)
                .ok_or_else(|| format!("Missing aircraft {}.{key}", current.id.0))?
                .parse::<u64>()
                .map_err(|_| "Invalid aircraft reference")?;
            current = graph
                .objects
                .get(&ObjectId(id))
                .ok_or("Unknown aircraft reference")?;
        }
        if current.kind != "cFlightData" {
            return Err("Unsupported flight profile".into());
        }
        let num = |k: &str, default: f64| -> f64 {
            field(current, k)
                .and_then(|s| s.parse::<f64>().ok())
                .filter(|v| v.is_finite() && *v >= 0.0)
                .unwrap_or(default)
        };
        let cruise_speed = num("mCruiseSpeed", 20.0).max(1.0);
        let min_speed = num("mForwardMinSpeed", 0.0).min(cruise_speed);
        let max_speed = num("mForwardMaxSpeed", cruise_speed).max(cruise_speed);
        let strafe_speed = num("mStrafeTopSpeed", cruise_speed * 0.5).max(4.0);
        let up_speed = num("mUpTopSpeed", 12.0).clamp(1.0, 60.0);
        let down_speed = num("mDownTopSpeed", 12.0).clamp(1.0, 60.0);
        let turn_rate = num("mMaxRotnRate", 1.2).clamp(0.2, 6.0);
        let cruise_hag = num("mCruiseHAG", 20.0) as f32;
        let min_hag = (num("mMinHAG", cruise_hag as f64 * 0.5) as f32)
            .max(1.0)
            .min(cruise_hag);
        let max_hag = (num("mMaxHAG", cruise_hag as f64 * 1.5) as f32).max(cruise_hag);
        profiles.insert(
            object.id,
            FlightProfile {
                min_speed,
                cruise_speed,
                max_speed,
                strafe_speed,
                up_speed,
                down_speed,
                turn_rate,
                min_hag,
                cruise_hag,
                max_hag,
            },
        );
    }
    Ok(profiles)
}
pub fn step(
    position: &mut [f32; 3],
    target: [f64; 2],
    radius: f64,
    speed: f64,
    dt: f64,
) -> Result<bool> {
    if position.iter().any(|x| !x.is_finite())
        || target.iter().any(|x| !x.is_finite())
        || !radius.is_finite()
        || radius < 0.
        || !speed.is_finite()
        || speed <= 0.
        || !dt.is_finite()
        || !(0.0..=1.0).contains(&dt)
    {
        return Err("Invalid scripted flight step".into());
    }
    let dx = target[0] - position[0] as f64;
    let dz = target[1] - position[2] as f64;
    let distance = dx.hypot(dz);
    if distance <= radius {
        return Ok(true);
    }
    let travel = (speed * dt).min(distance - radius);
    position[0] += (dx / distance * travel) as f32;
    position[2] += (dz / distance * travel) as f32;
    Ok(travel >= distance - radius - 1e-6)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_flight_preserves_altitude() {
        let mut p = [0., 100., 0.];
        assert!(!step(&mut p, [0., 50.], 5., 30., 1.).unwrap());
        assert_eq!(p, [0., 100., 30.]);
        assert!(step(&mut p, [0., 50.], 5., 30., 1.).unwrap());
        assert_eq!(p, [0., 100., 45.]);
        let before = p;
        assert!(step(&mut p, [0., f64::NAN], 5., 30., 1.).is_err());
        assert_eq!(p, before);
    }
}
