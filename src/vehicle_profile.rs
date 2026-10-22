//! Authored ground-vehicle profile selection. This is data, not an ODE implementation.
use crate::{Object, ObjectGraph, ObjectId, Result};
use std::{collections::BTreeMap, sync::Arc};
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum Engine {
    Ode,
    Ageia,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum Override {
    Authored,
    Force(Engine),
}
#[derive(Debug, serde::Serialize)]
pub struct Profile {
    pub id: ObjectId,
    pub parameters: ObjectId,
    pub attack_speed: f64,
    pub patrol_speed: f64,
    pub reverse_speed: f64,
    pub drive_acceleration: f64,
    pub braking_acceleration: f64,
    pub max_turn_rate: f64,
    pub turn_acceleration: f64,
    pub turn_damping: f64,
    pub min_speed_to_turn: f64,
}
#[derive(Debug)]
pub struct Binding {
    pub base: ObjectId,
    pub authored_engine: Engine,
    pub engine: Engine,
    pub profile: Arc<Profile>,
    pub model: String,
}
#[derive(Debug)]
pub struct Bank {
    pub units: BTreeMap<ObjectId, Binding>,
    pub profiles: BTreeMap<ObjectId, Arc<Profile>>,
}
fn scalar<'a>(o: &'a Object, key: &str) -> Result<&'a str> {
    let mut fields = o
        .fields
        .iter()
        .filter(|f| f.attributes.get("name").is_some_and(|n| n == key));
    let f = fields
        .next()
        .ok_or_else(|| format!("Vehicle object {} missing {key}", o.id.0))?;
    if fields.next().is_some() || f.items.len() != 1 {
        return Err(format!("Invalid vehicle scalar {key}"));
    }
    Ok(&f.items[0])
}
fn reference<'a>(g: &'a ObjectGraph, o: &Object, key: &str, kind: &str) -> Result<&'a Object> {
    let value = scalar(o, key)?;
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("Invalid vehicle reference {key}"));
    }
    let id = ObjectId(value.parse().map_err(|_| "Vehicle reference overflow")?);
    let target = g
        .objects
        .get(&id)
        .ok_or_else(|| format!("Missing vehicle reference {}: {key}", id.0))?;
    if target.kind != kind {
        return Err(format!(
            "Vehicle {key} expected {kind}, found {}",
            target.kind
        ));
    }
    Ok(target)
}
fn number(o: &Object, key: &str, positive: bool) -> Result<f64> {
    let n = scalar(o, key)?
        .parse::<f64>()
        .map_err(|_| format!("Invalid vehicle number {key}"))?;
    if !n.is_finite() || n < 0.0 || (positive && n == 0.0) {
        return Err(format!("Invalid vehicle range {key}"));
    }
    Ok(n)
}
impl Bank {
    /// Native override sentinel 2 preserves authored engine; engine 1 selects the new link.
    /// Caller must state override policy. No vehicle is given an infantry movement body.
    pub fn load(graph: &ObjectGraph, policy: Override) -> Result<Self> {
        let mut bank = Self {
            units: BTreeMap::new(),
            profiles: BTreeMap::new(),
        };
        for unit in graph
            .objects
            .values()
            .filter(|o| o.kind == "cGroundVehicle")
        {
            let authored_engine = match scalar(unit, "mPhysEngine")? {
                "physEngODE" => Engine::Ode,
                "physEngAgeia" => Engine::Ageia,
                _ => return Err("Unsupported authored vehicle engine".into()),
            };
            let engine = match policy {
                Override::Authored => authored_engine,
                Override::Force(e) => e,
            };
            let base = reference(graph, unit, "mBase", "cGroundVehicleBase")?;
            let profile = reference(
                graph,
                base,
                match engine {
                    Engine::Ode => "mPhysicsBase",
                    Engine::Ageia => "mPhysicsBaseNewEngine",
                },
                "cGroundVehiclePhysicsBase",
            )?;
            if let std::collections::btree_map::Entry::Vacant(entry) =
                bank.profiles.entry(profile.id)
            {
                let parameters =
                    reference(graph, profile, "mPhysicsParams", "cReflectedPhysicsParams")?.id;
                let p = Profile {
                    id: profile.id,
                    parameters,
                    attack_speed: number(profile, "attackSpeed", true)?,
                    patrol_speed: number(profile, "patrolSpeed", true)?,
                    reverse_speed: number(profile, "reverseSpeed", true)?,
                    drive_acceleration: number(profile, "driveAccln", true)?,
                    braking_acceleration: number(profile, "brakingAccln", true)?,
                    max_turn_rate: number(profile, "maxTurnRate", true)?,
                    turn_acceleration: number(profile, "turnAccln", true)?,
                    turn_damping: number(profile, "turnDamping", false)?,
                    min_speed_to_turn: number(profile, "minSpeedToTurn", false)?,
                };
                entry.insert(Arc::new(p));
            }
            let model = scalar(
                reference(graph, base, "mpModel", "cNodeHierarchyResource")?,
                "mName",
            )?
            .to_owned();
            if model.is_empty() {
                return Err("Empty ground vehicle model name".into());
            }
            bank.units.insert(
                unit.id,
                Binding {
                    base: base.id,
                    authored_engine,
                    engine,
                    profile: bank.profiles[&profile.id].clone(),
                    model,
                },
            );
        }
        Ok(bank)
    }
}
