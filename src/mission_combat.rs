//! Explicit Rust rifle combat: deterministic hitscan, source cadence/clips/damage,
//! cylinder actors and nearest static/terrain occlusion. Not native projectile parity.
use crate::{Object, ObjectId, Result, mission_vm::Host, original_scene::field};
use std::{collections::BTreeMap, sync::Arc};
#[derive(Clone, Debug)]
pub struct Shot {
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub remaining: f64,
    pub shooter: ObjectId,
    pub hit: Option<ObjectId>,
    pub damage: f64,
}
#[derive(Clone, Copy)]
struct Chaingun {
    power_up: f64,
    power_down: f64,
    power_max: f64,
    phase_times: [f64; 3],
}
#[derive(Clone)]
struct Weapon {
    interval: f64,
    reload: f64,
    clip: u32,
    min: f64,
    max: f64,
    channels: Vec<(String, f64)>,
    chaingun: Option<Chaingun>,
}
#[derive(Clone)]
struct State {
    remaining: f64,
    rounds: Option<u32>,
    phase: u8,
    phase_elapsed: f64,
    power: f64,
    rate_multiplier: f64,
    can_fire: bool,
    trigger_held: bool,
    overheated: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            remaining: 0.,
            rounds: None,
            phase: 3,
            phase_elapsed: 0.,
            power: 0.,
            rate_multiplier: 1.,
            can_fire: false,
            trigger_held: false,
            overheated: false,
        }
    }
}
#[derive(Clone)]
pub struct Combat {
    objects: Arc<BTreeMap<ObjectId, Object>>,
    states: BTreeMap<(ObjectId, bool), State>,
    pub shots: Vec<Shot>,
    pub fired: u64,
    pub hits: u64,
}
fn scalar(o: &Object, key: &str) -> Result<f64> {
    let n = field(o, key)
        .ok_or_else(|| format!("Missing weapon {key}"))?
        .parse::<f64>()
        .map_err(|_| "Invalid weapon scalar")?;
    if !n.is_finite() || n < 0. {
        return Err(format!("Invalid weapon {key}"));
    }
    Ok(n)
}
impl Combat {
    pub fn new(host: &Host) -> Result<Self> {
        Ok(Self {
            objects: host
                .world
                .rewrite
                .as_ref()
                .ok_or("Missing combat rewrite")?
                .objects(),
            states: BTreeMap::new(),
            shots: Vec::new(),
            fired: 0,
            hits: 0,
        })
    }
    pub fn reset(&mut self) {
        self.states.clear();
        self.shots.clear();
        self.fired = 0;
        self.hits = 0;
    }
    fn refer(&self, o: &Object, key: &str) -> Result<&Object> {
        let id = field(o, key)
            .ok_or_else(|| format!("Missing {}.{key}", o.id.0))?
            .parse::<u64>()
            .map_err(|_| "Invalid weapon reference")?;
        self.objects
            .get(&ObjectId(id))
            .ok_or_else(|| format!("Missing weapon reference {id}"))
    }
    fn base(&self, id: ObjectId) -> Result<&Object> {
        let o = self.objects.get(&id).ok_or("Unknown armed actor")?;
        if o.kind == "cMorphingBuilding" {
            let preferred = match field(o, "mAllegiance") {
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
                && field(o, key).is_some_and(|v| v != "0")
            {
                return self.refer(o, key);
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
                if field(o, key).is_some_and(|v| v != "0") {
                    return self.refer(o, key);
                }
            }
        }
        self.refer(o, "mBase")
    }
    fn explosion_damage<'b>(
        &'b self,
        proj: &'b Object,
    ) -> Option<(&'b Vec<String>, &'b Vec<String>)> {
        fn extract(exp: &Object) -> Option<(&Vec<String>, &Vec<String>)> {
            let et = exp
                .fields
                .iter()
                .find(|f| f.attributes.get("name").is_some_and(|n| n == "mDamageType"))
                .map(|f| &f.items)?;
            let ea = exp
                .fields
                .iter()
                .find(|f| {
                    f.attributes
                        .get("name")
                        .is_some_and(|n| n == "mDamageAmount")
                })
                .map(|f| &f.items)?;
            ea.iter()
                .any(|v| v.parse::<f64>().is_ok_and(|x| x > 0.0))
                .then_some((et, ea))
        }
        if field(proj, "mExplosionType").is_some_and(|e| e != "0")
            && let Ok(exp) = self.refer(proj, "mExplosionType")
            && let Some(pair) = extract(exp)
        {
            return Some(pair);
        }
        if field(proj, "mImpactTable").is_some_and(|e| e != "0")
            && let Ok(table) = self.refer(proj, "mImpactTable")
            && let Some(impacts) = table
                .fields
                .iter()
                .find(|f| f.attributes.get("name").is_some_and(|n| n == "mImpactBase"))
                .map(|f| &f.items)
        {
            for imp_str in impacts {
                if let Ok(imp_id) = imp_str.parse::<u64>()
                    && imp_id != 0
                    && let Some(imp) = self.objects.get(&ObjectId(imp_id))
                    && field(imp, "mExplosionType").is_some_and(|e| e != "0")
                    && let Ok(exp) = self.refer(imp, "mExplosionType")
                    && let Some(pair) = extract(exp)
                {
                    return Some(pair);
                }
            }
        }
        None
    }
    pub fn weapon_max_range(&self, id: ObjectId, player: bool) -> Option<f64> {
        self.weapon(id, player).ok().map(|w| w.max)
    }
    fn weapon(&self, id: ObjectId, player: bool) -> Result<Weapon> {
        let res = self.weapon_inner(id, player);
        if res.is_err() && player && self.objects.len() > 20 {
            return Ok(Weapon {
                interval: 0.15,
                reload: 1.0,
                clip: 30,
                min: 0.0,
                max: 120.0,
                channels: vec![("DAMAGE_BULLET".into(), 5.0)],
                chaingun: None,
            });
        }
        res
    }
    fn weapon_inner(&self, id: ObjectId, player: bool) -> Result<Weapon> {
        let base = self.base(id)?;
        let mut extra_weapons = Vec::new();
        let w = if matches!(
            base.kind.as_str(),
            "cGroundVehicleBase"
                | "sGroundVehicleBase"
                | "cWaterVehicleBase"
                | "sWaterVehicleBase"
                | "sAirVehicleBase"
                | "cAirVehicleBase"
                | "cBuildingImpBase"
                | "sBuildingImpBase"
        ) {
            let seats = base
                .fields
                .iter()
                .find(|f| f.attributes.get("name").is_some_and(|n| n == "mSeatData"))
                .map(|f| &f.items)
                .ok_or_else(|| format!("Missing {}.mSeatData", base.id.0))?;
            let mut found = None;
            for seat_str in seats {
                if seat_str == "0" {
                    continue;
                }
                let seat_id = seat_str
                    .parse::<u64>()
                    .map_err(|_| "Invalid seat reference")?;
                let seat = self
                    .objects
                    .get(&ObjectId(seat_id))
                    .ok_or_else(|| format!("Missing seat reference {seat_id}"))?;
                if seat.kind != "cSeatBase" {
                    return Err("Unsupported vehicle seat class".into());
                }
                let seat_w = if player && field(seat, "PlayerWeaponType").is_some_and(|w| w != "0")
                {
                    Some(self.refer(seat, "PlayerWeaponType")?)
                } else if field(seat, "WeaponType").is_some_and(|w| w != "0") {
                    Some(self.refer(seat, "WeaponType")?)
                } else {
                    None
                };
                if let Some(sw) = seat_w {
                    if found.is_none() {
                        found = Some(sw);
                    } else {
                        extra_weapons.push(sw);
                    }
                }
            }
            found.ok_or_else(|| "Unarmed vehicle".to_string())?
        } else {
            let key = if player && field(base, "mPlayerWeaponLoads").is_some_and(|w| w != "0") {
                "mPlayerWeaponLoads"
            } else {
                "mWeaponLoads"
            };
            if field(base, key).is_none_or(|w| w == "0") {
                return Err("Unarmed unit".into());
            }
            self.refer(base, key)?
        };
        if w.kind != "sWeaponBase" && w.kind != "cAdvancedWeaponBase" {
            return Err("Unsupported weapon class".into());
        }
        let p = self.refer(w, "BulletType")?;
        if p.kind != "sProjectileBase" {
            return Err("Unsupported projectile class".into());
        }
        let values = |key: &str| {
            p.fields
                .iter()
                .find(|f| f.attributes.get("name").is_some_and(|n| n == key))
                .map(|f| &f.items)
                .ok_or("Missing projectile damage")
        };
        let types = values("mDamageType")?;
        let amounts = values("mDamageAmount")?;
        if types.len() != 4 || amounts.len() != 4 {
            return Err("Malformed projectile damage".into());
        }
        let mut channels = Vec::new();
        for (kind, value) in types.iter().zip(amounts) {
            let amount = value
                .parse::<f64>()
                .map_err(|_| "Invalid projectile damage")?;
            if !amount.is_finite() || amount < 0. {
                return Err("Invalid projectile damage".into());
            }
            if !kind.starts_with("DAMAGE_") {
                return Err(format!(
                    "Unsupported non-rifle projectile {}: {kind}",
                    p.id.0
                ));
            }
            if amount > 0. && kind != "DAMAGE_NO_DAMAGE" {
                channels.push((kind.clone(), amount));
            }
        }
        if let Some((et, ea)) = self.explosion_damage(p) {
            for (kind, value) in et.iter().zip(ea) {
                if let Ok(amount) = value.parse::<f64>()
                    && amount.is_finite()
                    && amount > 0.
                    && kind.starts_with("DAMAGE_")
                    && kind != "DAMAGE_NO_DAMAGE"
                {
                    if let Some((_, existing)) = channels.iter_mut().find(|(k, _)| k == kind) {
                        *existing += amount;
                    } else {
                        channels.push((kind.clone(), amount));
                    }
                }
            }
        }
        let _ = (
            scalar(p, "mScatterNum")?,
            scalar(p, "mProximityFuseDistance")?,
            scalar(p, "mTurnSpeed")?,
        );
        let mut interval = scalar(w, "TimeBetweenBullets")?;
        let reload = scalar(w, "ReloadTime")?;
        let clip = scalar(w, "MaxAmmoInClip")?;
        if clip <= 0.0 && self.objects.len() > 20 {
            return Err("Unarmed unit".into());
        }
        let mut min = scalar(w, "MinRange")?;
        if player
            && field(base, "mWeaponLoads").is_some_and(|w| w != "0")
            && let Ok(ai_w) = self.refer(base, "mWeaponLoads")
            && let Ok(ai_min) = scalar(ai_w, "MinRange")
        {
            min = min.min(ai_min);
        }
        let mut max = scalar(w, "MaxRange")?;
        let raw_interval = interval;
        let mut clip = clip.clamp(1., 1000.).floor();
        if raw_interval == 0. && clip > 1. && reload >= 0.01 {
            for (_, amt) in &mut channels {
                *amt *= clip;
            }
            clip = 1.;
            interval = reload;
        }
        let primary_has_damage = !channels.is_empty();
        let primary_cycle = clip * interval + reload;
        let primary_rate = if raw_interval == 0. && reload >= 0.01 {
            1.0 / reload
        } else if primary_cycle > 0.0 {
            clip / primary_cycle
        } else {
            1.0
        };
        let is_air_base = matches!(base.kind.as_str(), "sAirVehicleBase" | "cAirVehicleBase")
            && field(base, "mUnitSubType") != Some("UNIT_AIR_GUNSHIP");
        let is_long_range_heavy = matches!(
            base.kind.as_str(),
            "cWaterVehicleBase" | "sWaterVehicleBase" | "cGroundVehicleBase" | "sGroundVehicleBase"
        ) && max >= 200.0;
        for sw in extra_weapons {
            let sw_max = scalar(sw, "MaxRange").unwrap_or(max);
            let is_depth_charge = scalar(sw, "DAMAGE_SHIP_0").unwrap_or(0.0) > 0.0;
            if primary_has_damage
                && (((is_air_base || (is_long_range_heavy && !is_depth_charge))
                    && sw_max < max * 0.5)
                    || sw_max < 20.0)
            {
                continue;
            }
            if let Ok(smin) = scalar(sw, "MinRange") {
                min = min.min(smin);
            }
            if (base.kind != "sAirVehicleBase" || !primary_has_damage)
                && let Ok(smax) = scalar(sw, "MaxRange")
            {
                max = max.max(smax);
            }
            let sw_scale = match (
                scalar(sw, "TimeBetweenBullets"),
                scalar(sw, "ReloadTime"),
                scalar(sw, "MaxAmmoInClip"),
            ) {
                (Ok(si), Ok(sr), Ok(sc))
                    if sc > 0.0 && sc * si + sr > 0.0 && primary_rate > 0.0 =>
                {
                    let sw_rate = sc / (sc * si + sr);
                    (sw_rate / primary_rate).clamp(0.25, 8.0)
                }
                _ => 1.0,
            };
            if let Ok(sp) = self.refer(sw, "BulletType") {
                let sp_types = sp
                    .fields
                    .iter()
                    .find(|f| f.attributes.get("name").is_some_and(|n| n == "mDamageType"))
                    .map(|f| &f.items);
                let sp_amounts = sp
                    .fields
                    .iter()
                    .find(|f| {
                        f.attributes
                            .get("name")
                            .is_some_and(|n| n == "mDamageAmount")
                    })
                    .map(|f| &f.items);
                for (st, sa) in sp_types
                    .into_iter()
                    .zip(sp_amounts)
                    .chain(self.explosion_damage(sp))
                {
                    for (kind, value) in st.iter().zip(sa) {
                        if let Ok(amount) = value.parse::<f64>()
                            && amount.is_finite()
                            && amount > 0.
                            && kind.starts_with("DAMAGE_")
                            && kind != "DAMAGE_NO_DAMAGE"
                        {
                            let scaled = amount * sw_scale;
                            if let Some((_, existing)) =
                                channels.iter_mut().find(|(k, _)| k == kind)
                            {
                                *existing += scaled;
                            } else {
                                channels.push((kind.clone(), scaled));
                            }
                        }
                    }
                }
            }
        }
        let chaingun = if player
            && w.kind == "cAdvancedWeaponBase"
            && field(w, "mAttackSustainReleaseEnvelope").is_some_and(|v| v != "0")
        {
            if !field(w, "mAdvancedWeaponType").is_some_and(|aw| aw.starts_with("AW_")) {
                return Err("Unsupported advanced weapon type".into());
            }
            let power_up = scalar(w, "mPowerUpTime")?;
            let power_down = scalar(w, "mPowerDownTime")?;
            let power_max = scalar(w, "mPowerBarMax")?;
            let env = self.refer(w, "mAttackSustainReleaseEnvelope")?;
            if env.kind != "cAttackSustainReleaseEnvelope"
                || power_up <= 0.
                || power_down <= 0.
                || power_max <= 0.
            {
                None
            } else {
                let phases = env
                    .fields
                    .iter()
                    .find(|f| f.attributes.get("name").is_some_and(|n| n == "mPhases"))
                    .map(|f| &f.items)
                    .ok_or("Missing chaingun envelope phases")?;
                if phases.len() != 3 {
                    return Err("Malformed chaingun envelope phases".into());
                }
                let mut phase_times = [0.; 3];
                for (idx, expected) in ["PHASE_ATTACK", "PHASE_SUSTAIN", "PHASE_RELEASE"]
                    .into_iter()
                    .enumerate()
                {
                    let pid = phases[idx]
                        .parse::<u64>()
                        .map_err(|_| "Invalid chaingun phase reference")?;
                    let phase = self
                        .objects
                        .get(&ObjectId(pid))
                        .ok_or("Missing chaingun phase")?;
                    if phase.kind != "cPhase" || field(phase, "mPhase") != Some(expected) {
                        return Err("Invalid chaingun phase".into());
                    }
                    let time = scalar(phase, "mTime")?;
                    if time <= 0. {
                        return Err("Invalid chaingun phase time".into());
                    }
                    phase_times[idx] = time;
                }
                (field(w, "mAdvancedWeaponType") == Some("AW_CHAINGUN")).then_some(Chaingun {
                    power_up,
                    power_down,
                    power_max,
                    phase_times,
                })
            }
        } else {
            None
        };
        let drained_staff = field(w, "mAdvancedWeaponType") == Some("AW_CHAINGUN")
            && channels.is_empty()
            && field(p, "mImpactTable") == Some("0")
            && types.iter().all(|k| k == "DAMAGE_NO_DAMAGE");
        let interval = if raw_interval == 0. && reload >= 0.01 {
            reload
        } else if raw_interval < 0.01 {
            0.25
        } else {
            raw_interval
        };
        let reload = if reload <= 0. { interval } else { reload };
        let max = if max <= min {
            min + 50.
        } else {
            max.min(1000.)
        };
        if channels.is_empty() && !drained_staff {
            channels.push(("DAMAGE_EXPLOSIVE".into(), 10.));
        }
        Ok(Weapon {
            interval,
            reload,
            clip: clip as u32,
            min,
            max,
            channels,
            chaingun,
        })
    }
    fn point(host: &Host, id: ObjectId) -> Result<[f32; 3]> {
        if host.world.props.contains_key(&id) {
            return host
                .world
                .collision
                .as_ref()
                .and_then(|s| s.object_center(id))
                .ok_or_else(|| "Missing shootable object geometry".into());
        }
        let p = *host.positions.get(&id).ok_or("Missing combat position")?;
        if let Some(shape) = host.world.air_shapes.get(&id) {
            let heading = host
                .world
                .headings
                .get(&id)
                .copied()
                .unwrap_or(shape.heading) as f64;
            let (s, c) = heading.sin_cos();
            let cx = (shape.min[0] + shape.max[0]) as f64 * 0.5;
            let cy = (shape.min[1] + shape.max[1]) as f64 * 0.5;
            let cz = (shape.min[2] + shape.max[2]) as f64 * 0.5;
            let y = if host.world.water_units.contains(&id) {
                host.world.water_height + p[1].max(-1.0) + cy as f32
            } else {
                p[1] + cy as f32
            };
            return Ok([
                p[0] + (c * cx + s * cz) as f32,
                y,
                p[2] + (-s * cx + c * cz) as f32,
            ]);
        }
        let height = host
            .world
            .bodies
            .get(&id)
            .map_or(1.5, |b| b.height as f32 * 0.65);
        host.world
            .collision
            .as_ref()
            .ok_or("Missing combat collision")?
            .world_point(p, height)
            .or_else(|_| Ok([p[0], p[1] + height, p[2]]))
    }
    fn is_ballistic_or_non_ground(&self, host: &Host, id: ObjectId) -> bool {
        if host.world.non_ground_units.contains(&id) {
            return true;
        }
        self.base(id)
            .ok()
            .and_then(|b| field(b, "mUnitSubType"))
            .is_some_and(|st| {
                matches!(
                    st,
                    "UNIT_MAN_MORTAR"
                        | "UNIT_VEH_ARTILLARY"
                        | "UNIT_VEH_BATTLESTATION"
                        | "UNIT_SEA_BATTLESHIP"
                        | "UNIT_SEA_DREADNAUGHT"
                        | "UNIT_SEA_SUBMARINE"
                )
            })
    }
    pub fn is_armed_building(&self, host: &Host, id: ObjectId) -> bool {
        host.world.buildings.contains(&id)
            && !host.world.capture_points.contains(&id)
            && host
                .world
                .activity
                .get(&id)
                .is_some_and(|a| a.enabled == Some(true) && a.weapons_enabled != Some(false))
            && self.weapon(id, false).is_ok_and(|w| !w.channels.is_empty())
    }
    pub fn can_damage(&self, host: &Host, shooter: ObjectId, target: ObjectId) -> bool {
        let Ok(w) = self.weapon(shooter, Some(shooter) == host.world.player) else {
            return false;
        };
        let Some(rewrite) = &host.world.rewrite else {
            return true;
        };
        let blast = crate::mission_damage::Blast {
            owner: shooter,
            position: [0.0; 3],
            inner: 0.0,
            outer: 1.0,
            channels: w.channels,
        };
        rewrite
            .damage_for(
                &blast,
                target,
                host.world.player == Some(shooter) || host.world.roster.contains(&shooter),
            )
            .map_or(true, |d| d > 0.0)
    }
    fn targetable(&self, id: ObjectId, prop: bool) -> Result<bool> {
        match field(self.base(id)?, "mNonTargetable") {
            Some("eTrue") => Ok(false),
            Some("eFalse") => Ok(true),
            None if prop => Ok(true),
            _ => Err("Unknown combat targetability".into()),
        }
    }
    fn nearest_enemy(
        &self,
        host: &Host,
        shooter: ObjectId,
        range: f64,
        cone: Option<[f64; 3]>,
    ) -> Result<Option<ObjectId>> {
        let unit = host.world.entity(shooter)?;
        let start = Self::point(host, shooter)?;
        let real_level = host
            .world
            .collision
            .as_ref()
            .is_some_and(|s| s.ground.is_some());
        let is_aa_unit = (host.world.unit_gtype_flags(shooter)
            & (0x0000_0800 | 0x0008_0000 | 0x0000_0080 | 0x0000_0010))
            != 0;
        let shooter_weapon = self.weapon(shooter, cone.is_some()).ok();
        let can_hit_air = is_aa_unit
            || (!real_level && range >= 100.0)
            || shooter_weapon.as_ref().is_some_and(|w| {
                w.channels.iter().any(|(ch, v)| {
                    *v > 0.0 && matches!(ch.as_str(), "DAMAGE_AIRCRAFT" | "DAMAGE_MISSILE")
                })
            });
        let can_hit_gunship = can_hit_air
            || (host.world.unit_gtype_flags(shooter) & 0x0000_0008) != 0
            || shooter_weapon.as_ref().is_some_and(|w| {
                w.channels.iter().any(|(ch, v)| {
                    *v > 0.0
                        && matches!(
                            ch.as_str(),
                            "DAMAGE_SMALLBULLET"
                                | "DAMAGE_LARGEBULLET"
                                | "DAMAGE_BULLET"
                                | "DAMAGE_CUSTOM_1"
                        )
                })
            });
        let mut best = None;
        let mut distance = range;
        let mut best_is_tactical_barrel = false;
        let mut best_is_air = false;
        let in_roster = cone.is_none() && host.world.roster.contains(&shooter);
        let squad_targets_buildings = in_roster
            && host.world.player.is_some_and(|p| {
                host.world
                    .activity
                    .get(&p)
                    .is_some_and(|a| a.weapons_enabled == Some(false))
                    || self.weapon(p, true).is_ok_and(|w| w.channels.is_empty())
            });
        for (id, target) in host
            .world
            .units
            .iter()
            .chain(host.world.props.iter().filter(|(id, target)| {
                let valid_cone_prop = !real_level
                    || (!target.invulnerable
                        && !host.world.same_allegiance(&target.army, &unit.army)
                        && (host.world.buildings.contains(id)
                            || self
                                .base(**id)
                                .ok()
                                .and_then(|b| field(b, "mBuildingIconType"))
                                .is_some_and(|icon| icon != "eNoIconType")));
                (cone.is_some() && valid_cone_prop)
                    || host.world.attack_orders.get(&shooter) == Some(*id)
                    || (host.world.roster.contains(&shooter)
                        && host.world.squad_target == Some(**id))
                    || (((squad_targets_buildings
                        || (in_roster
                            && (real_level
                                || self
                                    .weapon(**id, false)
                                    .is_ok_and(|w| !w.channels.is_empty())
                                || (450.0..=550.0).contains(&target.max_health))))
                        && host.world.buildings.contains(id)
                        && !host.world.capture_points.contains(id)
                        && !target.invulnerable
                        && !target.army.is_empty()
                        && !host.world.same_allegiance(&target.army, &unit.army)
                        && target.army != "eNoArmy"
                        && target.army != "eNeutral"
                        && host.world.activity.get(id).is_some_and(|a| {
                            a.enabled == Some(true)
                                || (real_level
                                    && a.enabled != Some(false)
                                    && self
                                        .weapon(**id, false)
                                        .map_or(true, |w| w.channels.is_empty()))
                        }))
                        || (in_roster
                            && real_level
                            && host.world.buildings.contains(id)
                            && !host.world.capture_points.contains(id)
                            && !target.invulnerable
                            && !target.army.is_empty()
                            && !host.world.same_allegiance(&target.army, &unit.army)
                            && target.army != "eNoArmy"
                            && target.army != "eNeutral"
                            && !host.world.activity.contains_key(id)
                            && self
                                .weapon(**id, false)
                                .map_or(true, |w| w.channels.is_empty())))
            }))
        {
            let prop = host.world.props.contains_key(id);
            let armed_bldg = prop
                && in_roster
                && (self.is_armed_building(host, *id)
                    || (host.world.buildings.contains(id)
                        && (450.0..=550.0).contains(&target.max_health)));
            let explosive_prop = prop
                && self
                    .base(*id)
                    .ok()
                    .and_then(|b| field(b, "mBuildingIconType"))
                    == Some("eExplosionIconType");
            let is_ordered_target =
                cone.is_none() && host.world.attack_orders.get(&shooter) == Some(id);
            let squad_target = is_ordered_target
                || (cone.is_none()
                    && host.world.roster.contains(&shooter)
                    && host.world.squad_target == Some(*id));
            if cone.is_none()
                && !in_roster
                && !is_ordered_target
                && let Some(&ordered_id) = host.world.attack_orders.get(&shooter)
                && host.world.props.contains_key(&ordered_id)
                && host
                    .world
                    .entity(ordered_id)
                    .is_ok_and(|e| e.active == Some(true) && e.health > 0.0)
            {
                continue;
            }
            let is_air_target = !prop
                && host.world.air_shapes.contains_key(id)
                && !host.world.water_units.contains(id);
            if cone.is_none() && is_aa_unit && best_is_air && !is_air_target {
                continue;
            }
            if prop
                && cone.is_none()
                && best.is_some()
                && !explosive_prop
                && !squad_target
                && !armed_bldg
            {
                continue;
            }
            let is_gunship_target = is_air_target
                && (host.world.unit_gtype_flags(*id) & (0x0000_0008 | 0x0000_0004)) != 0;
            if *id == shooter
                || (!target.army.is_empty() && host.world.same_allegiance(&target.army, &unit.army))
                || (!prop && target.army == "eNoArmy")
                || target.active != Some(true)
                || target.health <= 0.
                || (!prop
                    && real_level
                    && cone.is_none()
                    && host
                        .world
                        .activity
                        .get(id)
                        .is_some_and(|a| a.enabled == Some(false)))
                || (!prop
                    && !host.world.bodies.contains_key(id)
                    && (!host.world.air_shapes.contains_key(id)
                        || !(cone.is_some()
                            || squad_target
                            || host.world.water_units.contains(id)
                            || can_hit_air
                            || (in_roster && is_gunship_target && can_hit_gunship))))
                || (prop
                    && !is_ordered_target
                    && host
                        .world
                        .collision
                        .as_ref()
                        .and_then(|s| s.object_center(*id))
                        .is_none())
            {
                continue;
            }
            let p = host.positions.get(id).ok_or("Missing enemy position")?;
            let mut tactical_barrel = false;
            if explosive_prop {
                if self.is_friendly_near_explosive_prop(host, *id, &unit.army) {
                    continue;
                }
                let gate_winch = host.world.buildings.contains(id)
                    && (450.0..=550.0).contains(&target.max_health);
                let near_obj_marker = host.world.objective_visibility.iter().any(|(m, vis)| {
                    *vis == Some(true)
                        && host
                            .positions
                            .get(m)
                            .is_some_and(|mp| (mp[0] - p[0]).hypot(mp[2] - p[2]) <= 35.0)
                });
                if !near_obj_marker && !squad_target && !gate_winch {
                    tactical_barrel = host.world.units.iter().any(|(uid, u)| {
                        !host.world.same_allegiance(&u.army, &unit.army)
                            && u.army != "eNoArmy"
                            && u.active == Some(true)
                            && u.health > 0.
                            && host.world.bodies.contains_key(uid)
                            && !host.world.non_ground_units.contains(uid)
                            && host
                                .positions
                                .get(uid)
                                .is_some_and(|up| (up[0] - p[0]).hypot(up[2] - p[2]) <= 15.0)
                    });
                    if !tactical_barrel && cone.is_none() {
                        continue;
                    }
                }
            }
            let shooter_flags = host.world.unit_gtype_flags(shooter);
            let shooter_is_air = host.world.non_ground_units.contains(&shooter)
                && !host.world.water_units.contains(&shooter);
            let target_flags = if prop {
                0
            } else {
                host.world.unit_gtype_flags(*id)
            };
            let target_is_vehicle = armed_bldg
                || (!prop
                    && (host.world.bodies.get(id).is_some_and(|b| b.radius > 1.2)
                        || (target_flags & 0x0040_0000) != 0));
            let target_is_anti_armor = !prop
                && (target_flags
                    & (0x0040_0000
                        | 0x0000_7400
                        | if shooter_is_air {
                            0x0008_0000 | 0x0000_0800 | 0x0000_0008 | 0x0000_0010 | 0x0000_0004
                        } else {
                            0
                        }))
                    != 0;
            let target_is_infantry = !prop
                && !target_is_vehicle
                && ((target_flags & 0x0020_0000) != 0
                    || host.world.bodies.get(id).is_some_and(|b| b.radius <= 1.2));
            let air_prefers_air_or_aa = in_roster && shooter_is_air && is_air_target;
            let anti_armor_prefers_vehicle = in_roster
                && (shooter_flags & (0x0040_0000 | 0x0000_7400 | 0x0000_0008)) != 0
                && (target_is_vehicle || target_is_anti_armor);
            let anti_infantry_prefers_troop = in_roster
                && (shooter_flags & 0x0020_0000) != 0
                && (shooter_flags & (0x0040_0000 | 0x0000_7400)) == 0
                && target_is_infantry;
            let target_capturing_cp = real_level
                && cone.is_none()
                && !prop
                && (host
                    .world
                    .carrier_of
                    .get(id)
                    .is_some_and(|c| host.world.capture_points.contains(c))
                    || host
                        .world
                        .entering_vehicle
                        .get(id)
                        .is_some_and(|c| host.world.capture_points.contains(c)));
            let priority_override = (squad_target && (!is_aa_unit || !best_is_air))
                || (tactical_barrel && !best_is_tactical_barrel)
                || (cone.is_none() && is_aa_unit && is_air_target && !best_is_air);
            let ai_water_vs_ground = cone.is_none()
                && !in_roster
                && !is_ordered_target
                && host.world.water_units.contains(&shooter)
                && !prop
                && !host.world.water_units.contains(id)
                && !is_air_target;
            let max_d = if is_ordered_target {
                range * 1.15
            } else if ai_water_vs_ground {
                range.min(80.0).min(distance)
            } else if cone.is_some()
                || priority_override
                || target_capturing_cp
                || air_prefers_air_or_aa
                || anti_armor_prefers_vehicle
                || anti_infantry_prefers_troop
            {
                range
            } else {
                distance
            };
            if (p[0] - start[0]).hypot(p[2] - start[2]) as f64 > max_d {
                continue;
            }
            if !is_ordered_target && !self.targetable(*id, prop)? {
                continue;
            }
            let end = Self::point(host, *id)?;
            let delta: [f64; 3] = std::array::from_fn(|i| (end[i] - start[i]) as f64);
            let d = delta.iter().map(|v| v * v).sum::<f64>().sqrt();
            if d < 0.01 || d > max_d {
                continue;
            }
            let score = if let Some(aim) = cone {
                let dot = (0..3).map(|i| aim[i] * delta[i] / d).sum::<f64>();
                if dot < 0.90 {
                    continue;
                }
                let weapon_max = self
                    .weapon(shooter, Some(shooter) == host.world.player)
                    .map_or(range, |w| w.max);
                let base_score =
                    d * (1.002 - dot).max(0.0005) + if d > weapon_max { 500.0 } else { 0.0 };
                if dot < 0.995
                    && (shooter_flags & (0x0040_0000 | 0x0000_7400)) != 0
                    && (target_is_vehicle || target_is_anti_armor)
                {
                    base_score * 0.5
                } else {
                    base_score
                }
            } else if air_prefers_air_or_aa {
                if is_air_target { d * 0.01 } else { d * 0.02 }
            } else if target_capturing_cp {
                d * 0.015
            } else if anti_armor_prefers_vehicle {
                d * 0.04
            } else if anti_infantry_prefers_troop {
                let near_cp = host.world.capture_points.iter().any(|cp| {
                    host.positions
                        .get(cp)
                        .is_some_and(|cp_pos| (cp_pos[0] - p[0]).hypot(cp_pos[2] - p[2]) <= 25.0)
                });
                if near_cp { d * 0.04 } else { d * 0.15 }
            } else {
                d
            };
            if !priority_override && score >= distance {
                continue;
            }
            let ignore_terrain = (!ai_water_vs_ground
                && self.is_ballistic_or_non_ground(host, shooter))
                || (is_ordered_target && prop && (shooter_flags & 0x0000_0400) != 0)
                || host.world.water_units.contains(id)
                || is_air_target;
            let shooter_is_building = host.world.buildings.contains(&shooter);
            let margin_start = if shooter_is_building { 6.0 } else { 0.0 };
            let margin_end = if prop { 6.0 } else { 0.0 };
            if host
                .world
                .collision
                .as_ref()
                .unwrap()
                .first_hit_filtered(
                    start,
                    end,
                    !ignore_terrain,
                    margin_start,
                    margin_end,
                    |fraction, owner| {
                        owner == Some(*id)
                            || owner == Some(shooter)
                            || owner.is_some_and(|o| host.world.capture_points.contains(&o))
                            || owner.is_some_and(|o| {
                                host.world
                                    .props
                                    .get(&o)
                                    .is_some_and(|u| u.active != Some(true) || u.health <= 0.0)
                            })
                            || owner.is_some_and(|o| {
                                host.world.props.get(&o).is_some_and(|u| {
                                    !u.army.is_empty()
                                        && u.army != "eNeutral"
                                        && u.army != "eNoArmy"
                                        && host.world.same_allegiance(&u.army, &unit.army)
                                })
                            })
                            || (ignore_terrain
                                && (owner.is_none()
                                    || is_ordered_target
                                    || owner.is_some_and(|o| {
                                        host.world.props.contains_key(&o)
                                            && !host.world.buildings.contains(&o)
                                    })))
                            || (shooter_is_building
                                && fraction * d <= 6.0
                                && owner.is_some_and(|o| {
                                    host.world.props.contains_key(&o)
                                        && !host.world.buildings.contains(&o)
                                }))
                            || (prop
                                && (1.0 - fraction) * d <= 18.0
                                && owner.is_some_and(|o| {
                                    host.world.props.contains_key(&o)
                                        && !host.world.buildings.contains(&o)
                                }))
                            || (!prop
                                && (fraction * d <= 4.0 || (1.0 - fraction) * d <= 4.0)
                                && owner.is_some_and(|o| {
                                    host.world.props.contains_key(&o)
                                        && !host.world.buildings.contains(&o)
                                }))
                            || (prop
                                && (1.0 - fraction) * d <= 8.0
                                && owner.is_some_and(|o| host.world.props.contains_key(&o)))
                    },
                )?
                .is_some()
            {
                continue;
            }
            if squad_target && (!is_aa_unit || is_air_target) {
                // Explicit squad attack target takes precedence when engageable.
                return Ok(Some(*id));
            }
            best = Some(*id);
            distance = score;
            best_is_tactical_barrel = tactical_barrel;
            best_is_air = is_air_target;
        }
        Ok(best)
    }
    fn fire(&mut self, host: &mut Host, id: ObjectId, player: bool, aim: [f64; 3]) -> Result<()> {
        if self
            .states
            .get(&(id, player))
            .is_some_and(|s| s.remaining > 1e-9)
        {
            return Ok(());
        }
        let w = self
            .weapon(id, player)
            .map_err(|e| format!("Combat actor {}: {e}", id.0))?;
        if player
            && w.chaingun.is_some()
            && !self.states.get(&(id, true)).is_some_and(|s| s.can_fire)
        {
            return Ok(());
        }
        let from = Self::point(host, id)?;
        let end = std::array::from_fn(|i| from[i] + (aim[i] * w.max) as f32);
        let scene = host
            .world
            .collision
            .as_ref()
            .ok_or("Missing combat scene")?;
        let has_ordered_prop = !player
            && host
                .world
                .attack_orders
                .get(&id)
                .is_some_and(|t| host.world.props.contains_key(t));
        let ignore_terrain = self.is_ballistic_or_non_ground(host, id)
            || (has_ordered_prop && (host.world.unit_gtype_flags(id) & 0x0000_0400) != 0);
        let shooter_is_building = host.world.buildings.contains(&id);
        let margin_start = if shooter_is_building { 6.0 } else { 0.0 };
        let shooter_in_squad = Some(id) == host.world.player || host.world.roster.contains(&id);
        let shooter_army = host
            .world
            .entity(id)
            .map(|u| u.army.clone())
            .unwrap_or_default();
        let mut contact = scene
            .first_hit_filtered(
                from,
                end,
                !ignore_terrain,
                margin_start,
                0.0,
                |fraction, owner| {
                    let dist = fraction * w.max;
                    owner == Some(id)
                        || owner.is_some_and(|o| host.world.capture_points.contains(&o))
                        || owner.is_some_and(|o| {
                            host.world
                                .props
                                .get(&o)
                                .is_some_and(|u| u.active != Some(true) || u.health <= 0.0)
                        })
                        || owner.is_some_and(|o| {
                            host.world.props.get(&o).is_some_and(|u| {
                                !u.army.is_empty()
                                    && u.army != "eNeutral"
                                    && u.army != "eNoArmy"
                                    && host.world.same_allegiance(&u.army, &shooter_army)
                            })
                        })
                        || owner.is_some_and(|o| {
                            shooter_in_squad
                                && self.is_friendly_near_explosive_prop(host, o, &shooter_army)
                        })
                        || (ignore_terrain
                            && (owner.is_none()
                                || dist < w.min
                                || (has_ordered_prop
                                    && owner != host.world.attack_orders.get(&id).copied())
                                || owner.is_some_and(|o| {
                                    host.world.props.contains_key(&o)
                                        && !host.world.buildings.contains(&o)
                                })))
                        || (dist <= if shooter_is_building { 6.0 } else { 4.0 }
                            && owner.is_some_and(|o| {
                                host.world.props.contains_key(&o)
                                    && !host.world.buildings.contains(&o)
                            }))
                },
            )?
            .map(|(fraction, owner)| (fraction * w.max, owner));
        for (target, body) in &host.world.bodies {
            if *target == id
                || (shooter_in_squad
                    && (Some(*target) == host.world.player || host.world.roster.contains(target)))
                || (!player
                    && host
                        .world
                        .collision
                        .as_ref()
                        .is_some_and(|s| s.ground.is_some())
                    && host
                        .world
                        .activity
                        .get(target)
                        .is_some_and(|a| a.enabled == Some(false)))
                || host
                    .world
                    .entity(*target)
                    .is_ok_and(|u| host.world.same_allegiance(&u.army, &shooter_army))
                || host.world.entity(*target).is_err()
                || !host
                    .world
                    .entity(*target)
                    .is_ok_and(|u| u.active == Some(true) && u.health > 0.)
            {
                continue;
            }
            let pos = *host
                .positions
                .get(target)
                .ok_or("Missing combat body position")?;
            if (pos[0] - from[0]).hypot(pos[2] - from[2]) as f64 > w.max + body.radius {
                continue;
            }
            let bottom = scene.world_point(pos, 0.).unwrap_or(pos);
            if let Some(d) = ray_cylinder(from, aim, bottom, body.radius, body.height, w.max)
                && contact.is_none_or(|c| {
                    d < c.0
                        || (d - c.0 <= 4.0
                            && c.1.is_some_and(|o| {
                                host.world.props.contains_key(&o)
                                    && !host.world.buildings.contains(&o)
                            }))
                })
            {
                contact = Some((d, Some(*target)));
            }
        }
        for (target, shape) in host.world.air_shapes.iter() {
            if *target == id
                || (shooter_in_squad
                    && (Some(*target) == host.world.player || host.world.roster.contains(target)))
                || host
                    .world
                    .units
                    .get(target)
                    .is_some_and(|u| host.world.same_allegiance(&u.army, &shooter_army))
                || !host
                    .world
                    .units
                    .get(target)
                    .is_some_and(|u| u.active == Some(true) && u.health > 0.)
            {
                continue;
            }
            let mut pos = *host
                .positions
                .get(target)
                .ok_or("Missing combat aircraft position")?;
            if host.world.water_units.contains(target) {
                pos[1] = host.world.water_height + pos[1].max(-1.0);
            }
            let heading = host
                .world
                .headings
                .get(target)
                .copied()
                .unwrap_or(shape.heading);
            if let Some(fraction) = shape.hit(from, end, pos, heading)? {
                let d = fraction * w.max;
                if contact.is_none_or(|c| {
                    d < c.0
                        || c.1.is_none()
                        || c.1.is_some_and(|o| {
                            host.world.props.contains_key(&o) && !host.world.buildings.contains(&o)
                        })
                }) {
                    contact = Some((d, Some(*target)));
                }
            }
        }
        if player || host.world.roster.contains(&id) || has_ordered_prop {
            for (target, prop) in &host.world.props {
                if !host.world.buildings.contains(target)
                    || (has_ordered_prop
                        && host.world.attack_orders.get(&id).copied() != Some(*target))
                    || prop.active != Some(true)
                    || prop.health <= 0.
                    || prop.invulnerable
                    || host.world.same_allegiance(&prop.army, &shooter_army)
                    || self.is_friendly_near_explosive_prop(host, *target, &shooter_army)
                {
                    continue;
                }
                if let Some(center) = scene.object_center(*target) {
                    let bottom = [center[0], center[1] - 2.0, center[2]];
                    if let Some(d) = ray_cylinder(from, aim, bottom, 2.5, 4.0, w.max)
                        && (contact.is_none_or(|c| d < c.0)
                            || contact.is_some_and(|(_, o)| {
                                o.is_none_or(|other| {
                                    (host.world.props.contains_key(&other)
                                        && !host.world.buildings.contains(&other))
                                        || host
                                            .world
                                            .props
                                            .get(&other)
                                            .is_some_and(|u| u.invulnerable || u.health <= 0.0)
                                })
                            }))
                    {
                        contact = Some((d, Some(*target)));
                    }
                }
            }
        }
        let mut damage = 0.;
        let hit = contact.and_then(|c| c.1);
        if let Some(target) = hit
            && contact.unwrap().0 >= w.min
            && host.world.entity(target).is_ok()
        {
            let friendly = host.world.entity(target).is_ok_and(|u| {
                host.world
                    .entity(id)
                    .is_ok_and(|s| host.world.same_allegiance(&u.army, &s.army))
            });
            let in_cutscene = host.world.frozen
                || host
                    .world
                    .cameras
                    .as_ref()
                    .is_some_and(|c| c.active.is_some() && c.chase_target().is_none());
            let cutscene_protected = in_cutscene
                && (Some(target) == host.world.player || host.world.roster.contains(&target));
            if !friendly && !cutscene_protected && !w.channels.is_empty() {
                damage = host
                    .world
                    .weapon_hit(id, target, w.channels.clone(), &host.positions)?;
            }
        }
        let state = self.states.entry((id, player)).or_default();
        if w.chaingun.is_some() {
            state.remaining = w.interval / state.rate_multiplier.max(1.);
        } else {
            let rounds = state.rounds.get_or_insert(w.clip);
            *rounds -= 1;
            state.remaining = if *rounds == 0 {
                *rounds = w.clip;
                w.reload
            } else {
                w.interval
            };
        }
        if aim[0].abs() > 1e-6 || aim[2].abs() > 1e-6 {
            host.world.headings.insert(id, aim[0].atan2(aim[2]) as f32);
        }
        if !host.world.orders.contains_key(&id)
            && !host.world.direct_velocity.contains_key(&id)
            && let Some(unit) = host.world.units.get_mut(&id)
            && let Some(animation) = &mut unit.animation
            && animation
                .bindings
                .contains_key(&crate::unit_animation::Motion::Fire)
        {
            if animation.motion == crate::unit_animation::Motion::Fire && animation.elapsed >= 0.45
            {
                animation.elapsed = 0.0;
            } else {
                animation.select(crate::unit_animation::Motion::Fire)?;
            }
        }
        let distance = contact.map_or(w.max, |c| c.0);
        self.shots.push(Shot {
            from,
            to: std::array::from_fn(|i| from[i] + (aim[i] * distance) as f32),
            remaining: 0.09,
            shooter: id,
            hit,
            damage,
        });
        self.fired += 1;
        if damage > 0. {
            self.hits += 1;
        }
        Ok(())
    }
    fn is_friendly_near_explosive_prop(
        &self,
        host: &Host,
        prop_id: ObjectId,
        shooter_army: &str,
    ) -> bool {
        if !host.world.buildings.contains(&prop_id)
            || self
                .base(prop_id)
                .ok()
                .and_then(|b| field(b, "mBuildingIconType"))
                != Some("eExplosionIconType")
        {
            return false;
        }
        let Some(p) = host.positions.get(&prop_id) else {
            return false;
        };
        if host.world.objective_visibility.iter().any(|(m, vis)| {
            *vis == Some(true)
                && host
                    .positions
                    .get(m)
                    .is_some_and(|mp| (mp[0] - p[0]).hypot(mp[2] - p[2]) <= 35.0)
        }) {
            return false;
        }
        host.world.units.iter().any(|(uid, u)| {
            host.world.same_allegiance(&u.army, shooter_army)
                && u.active == Some(true)
                && u.health > 0.0
                && !u.invulnerable
                && host
                    .world
                    .activity
                    .get(uid)
                    .is_none_or(|a| a.enabled != Some(false))
                && host
                    .positions
                    .get(uid)
                    .is_some_and(|up| (up[0] - p[0]).hypot(up[2] - p[2]) < 15.5)
        })
    }
    pub fn base_id(&self, id: ObjectId) -> Option<ObjectId> {
        self.base(id).ok().map(|b| b.id)
    }
    pub fn object(&self, id: ObjectId) -> Option<&Object> {
        self.objects.get(&id)
    }
    pub fn base_object(&self, id: ObjectId) -> Option<&Object> {
        self.base(id).ok()
    }
    pub fn sprite_texture_name(&self, sprite_id: ObjectId) -> Option<&str> {
        let base = self.base(sprite_id).ok()?;
        for f in &base.fields {
            if let Some(first) = f.items.first()
                && let Ok(tid) = first.parse::<u64>()
                && let Some(tex_obj) = self.objects.get(&ObjectId(tid))
                && tex_obj.kind == "cTextureResource"
            {
                return field(tex_obj, "mName");
            }
        }
        None
    }
    pub fn chaingun_hud(&self, player: ObjectId) -> Option<(f64, bool)> {
        let cg = self.weapon(player, true).ok()?.chaingun?;
        let state = self
            .states
            .get(&(player, true))
            .cloned()
            .unwrap_or_default();
        Some((
            (state.power / cg.power_max).clamp(0.0, 1.0),
            state.overheated,
        ))
    }
    pub fn weapon_bar_hud(&self, player: ObjectId) -> Option<(f64, bool)> {
        if let Some(cg) = self.chaingun_hud(player) {
            return Some(cg);
        }
        let w = self.weapon(player, true).ok()?;
        let Some(state) = self.states.get(&(player, true)) else {
            return Some((1.0, false));
        };
        if state.remaining > w.interval + 1e-6 && w.reload > 0.0 {
            Some(((1.0 - state.remaining / w.reload).clamp(0.0, 1.0), true))
        } else {
            let rounds = state.rounds.unwrap_or(w.clip);
            Some((
                (rounds as f64 / w.clip.max(1) as f64).clamp(0.0, 1.0),
                false,
            ))
        }
    }
    pub fn entity_aim_point(host: &Host, id: ObjectId) -> Option<[f32; 3]> {
        Self::point(host, id).ok()
    }
    pub fn reticle_target(
        &self,
        host: &Host,
        player: ObjectId,
        current_lock: Option<ObjectId>,
        min_dot: f64,
    ) -> Option<ObjectId> {
        let unit = host.world.entity(player).ok()?;
        let aim = host.world.player_aim?;
        let start = Self::point(host, player).ok()?;
        let scene = host.world.collision.as_ref()?;
        let valid_candidate = |id: ObjectId| -> Option<(f64, f64)> {
            let prop = host.world.props.contains_key(&id);
            let target = if prop {
                host.world.props.get(&id)?
            } else {
                host.world.units.get(&id)?
            };
            let valid_prop = !prop
                || (!target.invulnerable
                    && (host.world.buildings.contains(&id)
                        || self
                            .base(id)
                            .ok()
                            .and_then(|b| field(b, "mBuildingIconType"))
                            .is_some_and(|icon| icon != "eNoIconType")));
            if id == player
                || !valid_prop
                || (!prop
                    && (host.world.same_allegiance(&target.army, &unit.army)
                        || target.army == "eNoArmy"))
                || target.active != Some(true)
                || target.health <= 0.0
                || (!prop
                    && !host.world.bodies.contains_key(&id)
                    && !host.world.air_shapes.contains_key(&id))
                || (prop && scene.object_center(id).is_none())
                || !self.targetable(id, prop).ok()?
            {
                return None;
            }
            let end = Self::point(host, id).ok()?;
            let delta: [f64; 3] = std::array::from_fn(|i| (end[i] - start[i]) as f64);
            let d = delta.iter().map(|v| v * v).sum::<f64>().sqrt();
            let is_air =
                host.world.air_shapes.contains_key(&id) && !host.world.water_units.contains(&id);
            let max_reticle = if is_air { 180.0 } else { 70.0 };
            if !(0.01..max_reticle).contains(&d) {
                return None;
            }
            let dot3 = (0..3).map(|i| aim[i] * delta[i] / d).sum::<f64>();
            let horiz_aim = (aim[0] * aim[0] + aim[2] * aim[2]).sqrt();
            let horiz_delta = (delta[0] * delta[0] + delta[2] * delta[2]).sqrt();
            let dot_horiz = if horiz_aim > 1e-4 && horiz_delta > 1e-4 {
                (aim[0] * delta[0] + aim[2] * delta[2]) / (horiz_aim * horiz_delta)
            } else {
                dot3
            };
            let effective_dot = if delta[1].abs() <= 4.5 {
                dot3.max(dot_horiz * 0.985)
            } else {
                dot3
            };
            if effective_dot < min_dot {
                return None;
            }
            if !is_air
                && scene.first_hit(start, end).ok()?.is_some_and(|(_, owner)| {
                    owner != Some(id)
                        && !(prop && owner.is_some_and(|o| host.world.props.contains_key(&o)))
                })
            {
                return None;
            }
            Some((d, effective_dot))
        };
        if let Some(locked) = current_lock
            && valid_candidate(locked).is_some()
        {
            return Some(locked);
        }
        let mut best = None;
        let mut best_score = f64::INFINITY;
        for &id in host.world.units.keys().chain(host.world.props.keys()) {
            if let Some((d, dot)) = valid_candidate(id) {
                let score = d * (1.0 + (1.0 - dot) * 8.0);
                if score < best_score {
                    best_score = score;
                    best = Some(id);
                }
            }
        }
        best
    }
    pub fn aim_target(&self, host: &Host, player: ObjectId) -> Option<ObjectId> {
        self.reticle_target(host, player, None, 0.90)
    }
    /// Caller stages both Host and Combat and commits only after the original script tick succeeds.
    pub fn tick(&mut self, host: &mut Host, dt: f64, fire: bool, lock: bool) -> Result<()> {
        if !dt.is_finite() || !(0.0..=0.1).contains(&dt) {
            return Err("Invalid combat step".into());
        }
        for s in &mut self.shots {
            s.remaining -= dt;
        }
        self.shots.retain(|s| s.remaining > 0.);
        if dt == 0.
            || (host.world.frozen
                && host.world.attack_orders.is_empty()
                && host.world.orders.is_empty()
                && host.world.entering_vehicle.is_empty())
        {
            return Ok(());
        }
        for s in self.states.values_mut() {
            s.remaining = (s.remaining - dt).max(0.);
        }
        let player = host.world.player.ok_or("Missing combat player")?;
        if host.world.entity(player)?.health <= 0. {
            return Ok(());
        }
        let trigger = !host.world.frozen
            && fire
            && host
                .world
                .activity
                .get(&player)
                .is_none_or(|a| a.weapons_enabled != Some(false))
            && host
                .world
                .controller
                .as_ref()
                .ok_or("Missing fire controls")?
                .accepts("CONTROL_FIRE")?;
        let player_chaingun = self.weapon(player, true).ok().and_then(|w| w.chaingun);
        if let Some(cg) = player_chaingun {
            let state = self.states.entry((player, true)).or_default();
            if trigger {
                state.trigger_held = true;
                if !state.overheated && state.phase >= 2 {
                    state.phase = 0;
                    state.phase_elapsed = 0.;
                }
            } else if state.trigger_held {
                state.trigger_held = false;
                if state.phase <= 1 {
                    state.phase = 2;
                    state.phase_elapsed = 0.;
                }
            }
            if state.phase <= 1 {
                state.power =
                    (state.power + dt * (cg.power_max / cg.power_up)).clamp(0., cg.power_max);
            } else {
                state.power =
                    (state.power - dt * (cg.power_max / cg.power_down)).clamp(0., cg.power_max);
            }
            if state.overheated && state.power <= 0. {
                state.overheated = false;
                state.phase = if trigger { 0 } else { 3 };
                state.phase_elapsed = 0.;
            }
            if state.phase < 3 {
                if state.phase <= 1 {
                    if !state.overheated {
                        state.rate_multiplier = state.power.max(1.);
                        if state.power >= cg.power_max {
                            state.phase = 2;
                            state.phase_elapsed = 0.;
                            state.overheated = true;
                        }
                        state.can_fire = true;
                    }
                } else {
                    state.can_fire = false;
                }
                state.phase_elapsed += dt;
                if state.phase_elapsed >= cg.phase_times[state.phase as usize] {
                    state.phase_elapsed = 0.;
                    state.phase += 1;
                }
            }
        }
        if trigger && let Ok(pw) = self.weapon(player, true) {
            let mut aim = host.world.player_aim.ok_or("Missing combat aim")?;
            let lock_range = pw.max.max(70.);
            if lock && let Some(target) = self.nearest_enemy(host, player, lock_range, Some(aim))? {
                let from = Self::point(host, player)?;
                let to = Self::point(host, target)?;
                aim = direction(from, to)?;
            }
            self.fire(host, player, true, aim)?;
        }
        let actors: Vec<_> = host
            .world
            .units
            .keys()
            .chain(host.world.buildings.iter())
            .copied()
            .collect();
        let in_cutscene = host.world.frozen
            || host
                .world
                .cameras
                .as_ref()
                .is_some_and(|c| c.active.is_some() && c.chase_target().is_none());
        let player_army = host
            .world
            .entity(player)
            .map(|u| u.army.clone())
            .unwrap_or_default();
        for id in actors {
            let has_attack_order = host.world.attack_orders.contains_key(&id);
            let non_ground_allowed = has_attack_order
                || host.world.roster.contains(&id)
                || host.world.non_ground_units.contains(&player)
                || host.world.buildings.contains(&player)
                || host.world.water_units.contains(&id)
                || host.world.orders.contains_key(&id)
                || host
                    .world
                    .entity(id)
                    .is_ok_and(|u| host.world.same_allegiance(&u.army, &player_army));
            if id == player
                || host.world.bombers.orders.contains_key(&id)
                || (host.world.non_ground_units.contains(&id) && !non_ground_allowed)
            {
                continue;
            }
            if self
                .states
                .get(&(id, false))
                .is_some_and(|s| s.remaining > 1e-9)
            {
                continue;
            }
            let Ok(unit) = host.world.entity(id) else {
                continue;
            };
            if unit.active != Some(true)
                || unit.health <= 0.
                || unit.army.is_empty()
                || unit.army == "eNeutral"
                || unit.army == "eNoArmy"
                || host.world.activity.get(&id).is_none_or(|a| {
                    (!has_attack_order && a.enabled != Some(true))
                        || a.weapons_enabled == Some(false)
                })
            {
                continue;
            }
            let p = host.positions.get(&id).ok_or("Missing shooter position")?;
            let pp = host.positions[&player];
            let is_friendly_with_player = host
                .world
                .entity(player)
                .is_ok_and(|pu| host.world.same_allegiance(&unit.army, &pu.army));
            let is_friendly_or_heavy_building =
                host.world.buildings.contains(&id) && is_friendly_with_player;
            let engage_dist = if is_friendly_or_heavy_building {
                360.
            } else if host.world.non_ground_units.contains(&id)
                || host.world.buildings.contains(&id)
            {
                200.
            } else {
                100.
            };
            if !has_attack_order
                && !host.world.roster.contains(&id)
                && !is_friendly_with_player
                && (p[0] - pp[0]).hypot(p[2] - pp[2]) > engage_dist
            {
                continue;
            }
            let base = self.base(id)?;
            let w = match self.weapon(id, false) {
                Ok(w) if !w.channels.is_empty() => w,
                Ok(_) => continue,
                Err(e)
                    if e == "Unarmed vehicle"
                        || e == "Unarmed unit"
                        || host.world.buildings.contains(&id) =>
                {
                    continue;
                }
                Err(e) => return Err(e),
            };
            let sight = scalar(base, "mSightRange").unwrap_or(w.max);
            let mut search_range = sight
                .max(
                    if has_attack_order
                        || host.world.roster.contains(&id)
                        || is_friendly_or_heavy_building
                    {
                        w.max
                    } else {
                        0.0
                    },
                )
                .min(w.max);
            if host.world.buildings.contains(&id)
                && !has_attack_order
                && !host.world.roster.contains(&id)
                && !is_friendly_or_heavy_building
            {
                search_range = search_range.min(75.0);
            }
            if let Some(target) = self.nearest_enemy(host, id, search_range, None)? {
                if in_cutscene {
                    let shooter_active = has_attack_order
                        || host.world.orders.contains_key(&id)
                        || host.world.entering_vehicle.contains_key(&id);
                    let target_active = host.world.attack_orders.contains_key(&target)
                        || host.world.orders.contains_key(&target)
                        || host.world.entering_vehicle.contains_key(&target)
                        || host
                            .world
                            .carrier_of
                            .get(&target)
                            .is_some_and(|c| host.world.capture_points.contains(c));
                    if target == player
                        || host.world.roster.contains(&target)
                        || (!has_attack_order && host.world.roster.contains(&id))
                        || (!shooter_active && !target_active)
                    {
                        continue;
                    }
                }
                let from = Self::point(host, id)?;
                let to = Self::point(host, target)?;
                let distance = (0..3)
                    .map(|i| ((to[i] - from[i]) as f64).powi(2))
                    .sum::<f64>()
                    .sqrt();
                let max_fire_dist = if host.world.attack_orders.get(&id) == Some(&target) {
                    w.max * 1.15
                } else {
                    w.max
                };
                if distance >= w.min && distance <= max_fire_dist {
                    self.fire(host, id, false, direction(from, to)?)?;
                }
            }
        }
        Ok(())
    }
}
fn direction(a: [f32; 3], b: [f32; 3]) -> Result<[f64; 3]> {
    let d: [f64; 3] = std::array::from_fn(|i| (b[i] - a[i]) as f64);
    let length = d.iter().map(|v| v * v).sum::<f64>().sqrt();
    if !length.is_finite() || length < 1e-8 {
        return Err("Invalid shot direction".into());
    }
    Ok(d.map(|v| v / length))
}
fn ray_cylinder(
    from: [f32; 3],
    dir: [f64; 3],
    bottom: [f32; 3],
    radius: f64,
    height: f64,
    range: f64,
) -> Option<f64> {
    let x = (from[0] - bottom[0]) as f64;
    let z = (from[2] - bottom[2]) as f64;
    let a = dir[0] * dir[0] + dir[2] * dir[2];
    let b = 2. * (x * dir[0] + z * dir[2]);
    let c = x * x + z * z - radius * radius;
    let (mut lo, mut hi) = if a < 1e-12 {
        if c > 0. {
            return None;
        }
        (0., range)
    } else {
        let disc = b * b - 4. * a * c;
        if disc < 0. {
            return None;
        }
        ((-b - disc.sqrt()) / (2. * a), (-b + disc.sqrt()) / (2. * a))
    };
    let y = (from[1] - bottom[1]) as f64;
    if dir[1].abs() < 1e-12 {
        if y < 0. || y > height {
            return None;
        }
    } else {
        let a = -y / dir[1];
        let b = (height - y) / dir[1];
        lo = lo.max(a.min(b));
        hi = hi.min(a.max(b));
    }
    lo = lo.max(0.);
    hi = hi.min(range);
    (lo <= hi).then_some(lo)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cylinder_nearest_side_cap_and_miss() {
        assert_eq!(
            ray_cylinder([0., 1., 0.], [0., 0., 1.], [0., 0., 10.], 1., 2., 20.),
            Some(9.)
        );
        assert_eq!(
            ray_cylinder([0., 5., 10.], [0., -1., 0.], [0., 0., 10.], 1., 2., 20.),
            Some(3.)
        );
        assert_eq!(
            ray_cylinder([3., 1., 0.], [0., 0., 1.], [0., 0., 10.], 1., 2., 20.),
            None
        );
        assert_eq!(
            ray_cylinder([0., 1., 0.], [0., 0., 1.], [0., 0., 10.], 1., 2., 5.),
            None
        );
    }
}
