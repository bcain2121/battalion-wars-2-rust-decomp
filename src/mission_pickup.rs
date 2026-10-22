//! Original `cPickupReflected` lifecycle and collection.
//!
//! Recovered from RBWE01 `main.dol`:
//! - Lua `Spawn`/`Despawn` (0x800d4888 / 0x800e9ed8) route pickups to
//!   0x800c3c10 / 0x800c3c8c: spawn sets state 1 (active) only from state 0
//!   (unspawned); despawn always returns to state 0.
//! - Contact handler 0x800c39cc acts only in state 1. By `sPickupBase::mType`
//!   (base +0x1c): `TROOP_HEALTH` requires a troop, `VEHICLE_HEALTH` a vehicle,
//!   each with health percent below 100; collection applies a heal of
//!   `mData` (base +0x20) through the unit damage virtual (0x800c3ba4 calls
//!   vtable +0x334 with `-mData`) and enters state 2 (collected).
//!   `GOLDCOIN` requires a player-controlled unit. Weapon upgrade/ammo and
//!   `UNPICKUPABLE` do nothing on contact in that handler.
//!
//! Declared Rust policies (not recovered): contact is a horizontal distance
//! test of `body radius + CONTACT_RADIUS`; healing clamps at max health.
//! Fall/rise animation, effects, despawn timeout and auto-respawn timers are
//! not implemented: a spawned pickup stays until collected or despawned.
use crate::{ObjectGraph, ObjectId, Result, original_scene::field};
use std::{collections::BTreeMap, sync::Arc};

/// Declared Rust contact policy, in metres beyond the unit body radius.
pub const CONTACT_RADIUS: f64 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    SecondaryWeaponUpgrade,
    SecondaryWeaponAmmo,
    TroopHealth,
    VehicleHealth,
    Unpickupable,
    GoldCoin,
}
impl Kind {
    fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "PICKUP_TYPE_SECONDARY_WPN_UPGRADE" => Self::SecondaryWeaponUpgrade,
            "PICKUP_TYPE_SECONDARY_WPN_AMMO" => Self::SecondaryWeaponAmmo,
            "PICKUP_TYPE_TROOP_HEALTH" => Self::TroopHealth,
            "PICKUP_TYPE_VEHICLE_HEALTH" => Self::VehicleHealth,
            "PICKUP_TYPE_UNPICKUPABLE" => Self::Unpickupable,
            "PICKUP_TYPE_GOLDCOIN" => Self::GoldCoin,
            other => return Err(format!("Unknown pickup type {other}")),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Unspawned,
    Active,
    Collected,
}

#[derive(Clone, Debug)]
pub struct Definition {
    pub base: ObjectId,
    pub kind: Kind,
    pub data: f64,
    pub position: [f32; 3],
}

#[derive(Clone, Debug, Default)]
pub struct Pickups {
    pub defs: Arc<BTreeMap<ObjectId, Definition>>,
    pub states: BTreeMap<ObjectId, State>,
    pub collected: u64,
    pub gold_coins: u64,
}

impl Pickups {
    pub fn load(graph: &ObjectGraph) -> Result<Self> {
        let mut defs = BTreeMap::new();
        let mut states = BTreeMap::new();
        for o in graph
            .objects
            .values()
            .filter(|o| o.kind == "cPickupReflected")
        {
            let base_id = field(o, "mBase")
                .ok_or("Pickup missing mBase")?
                .parse::<u64>()
                .map_err(|_| "Invalid pickup base")?;
            let base = graph
                .objects
                .get(&ObjectId(base_id))
                .filter(|b| b.kind == "sPickupBase")
                .ok_or("Pickup base missing or wrong class")?;
            let kind = Kind::parse(field(base, "mType").ok_or("Pickup base missing mType")?)?;
            let data = field(base, "mData")
                .ok_or("Pickup base missing mData")?
                .parse::<i32>()
                .map_err(|_| "Invalid pickup mData")? as f64;
            let m = crate::original_scene::matrix(field(o, "Mat").ok_or("Pickup missing Mat")?)?;
            let state = match field(o, "mState") {
                Some("PICKUP_STATE_UNSPAWNED") => State::Unspawned,
                Some("PICKUP_STATE_ACTIVE") => State::Active,
                _ => return Err("Invalid pickup mState".into()),
            };
            defs.insert(
                o.id,
                Definition {
                    base: base.id,
                    kind,
                    data,
                    position: [m[12], m[13], m[14]],
                },
            );
            states.insert(o.id, state);
        }
        Ok(Self {
            defs: Arc::new(defs),
            states,
            collected: 0,
            gold_coins: 0,
        })
    }
    pub fn contains(&self, id: ObjectId) -> bool {
        self.defs.contains_key(&id)
    }
    /// 0x800c3c10: only an unspawned pickup becomes active.
    pub fn spawn(&mut self, id: ObjectId) -> Result<()> {
        let s = self.states.get_mut(&id).ok_or("Unknown pickup")?;
        if *s == State::Unspawned {
            *s = State::Active;
        }
        Ok(())
    }
    /// 0x800c3c8c: unconditional return to unspawned.
    pub fn despawn(&mut self, id: ObjectId) -> Result<()> {
        *self.states.get_mut(&id).ok_or("Unknown pickup")? = State::Unspawned;
        Ok(())
    }
    pub fn active(&self) -> impl Iterator<Item = (&ObjectId, &Definition)> {
        self.defs
            .iter()
            .filter(|(id, _)| self.states.get(id) == Some(&State::Active))
    }
}

/// What a unit touching an active pickup may receive (0x800c39cc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Effect {
    Heal(f64),
    GoldCoin,
}

/// Native eligibility. `troop`/`vehicle` classify the toucher; `percent` is its
/// integer health percentage as compared by the native handler.
pub fn effect(
    def: &Definition,
    troop: bool,
    vehicle: bool,
    percent: f64,
    player: bool,
) -> Option<Effect> {
    let hurt = percent.floor() < 100.0;
    match def.kind {
        Kind::TroopHealth if troop && hurt => Some(Effect::Heal(def.data)),
        Kind::VehicleHealth if vehicle && hurt => Some(Effect::Heal(def.data)),
        Kind::GoldCoin if player => Some(Effect::GoldCoin),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn def(kind: Kind) -> Definition {
        Definition {
            base: ObjectId(1),
            kind,
            data: 300.0,
            position: [0.0; 3],
        }
    }
    #[test]
    fn native_eligibility_rules() {
        assert_eq!(
            effect(&def(Kind::TroopHealth), true, false, 50.0, false),
            Some(Effect::Heal(300.0))
        );
        assert_eq!(
            effect(&def(Kind::TroopHealth), true, false, 100.0, true),
            None
        );
        assert_eq!(
            effect(&def(Kind::TroopHealth), false, true, 10.0, true),
            None
        );
        assert_eq!(
            effect(&def(Kind::VehicleHealth), false, true, 99.9, false),
            Some(Effect::Heal(300.0))
        );
        assert_eq!(
            effect(&def(Kind::GoldCoin), true, false, 100.0, false),
            None
        );
        assert_eq!(
            effect(&def(Kind::GoldCoin), true, false, 100.0, true),
            Some(Effect::GoldCoin)
        );
        assert_eq!(
            effect(&def(Kind::SecondaryWeaponAmmo), true, false, 1.0, true),
            None
        );
    }
    #[test]
    fn spawn_only_from_unspawned() {
        let mut p = Pickups::default();
        let mut defs = BTreeMap::new();
        defs.insert(ObjectId(5), def(Kind::TroopHealth));
        p.defs = Arc::new(defs);
        p.states.insert(ObjectId(5), State::Collected);
        p.spawn(ObjectId(5)).unwrap();
        assert_eq!(p.states[&ObjectId(5)], State::Collected);
        p.despawn(ObjectId(5)).unwrap();
        p.spawn(ObjectId(5)).unwrap();
        assert_eq!(p.states[&ObjectId(5)], State::Active);
        assert!(p.spawn(ObjectId(6)).is_err());
    }
}
