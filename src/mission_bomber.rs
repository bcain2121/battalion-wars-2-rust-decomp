//! Explicit Rust forced-bomber attack policy, not native flight/projectile parity.
//! Primary guided bombs only: cruise approach, source drop distance/cadence/clip,
//! bounded guided descent under 30 world-units/s² gravity, real collision and damage.
use crate::{Object, ObjectId, Result, mission_world::World, original_scene::field};
use std::collections::BTreeMap;
#[derive(Clone, Debug)]
pub struct Profile {
    pub speed: f64,
    pub height: f64,
    pub rise: f64,
    pub descend: f64,
    pub turn: f64,
    pub drop_distance: f64,
    pub min_range: f64,
    pub max_range: f64,
    pub fire_angle: f64,
    pub launch_speed: f64,
    pub interval: f64,
    pub reload: f64,
    pub clip: u32,
    pub channels: Vec<(String, f64)>,
    pub impacts: Vec<Option<ObjectId>>,
}
pub(crate) fn scalar<'a>(o: &'a Object, key: &str) -> Result<&'a str> {
    let [value] = items(o, key)? else {
        return Err(format!("Invalid scalar bomber {key}"));
    };
    Ok(value)
}
pub(crate) fn number(o: &Object, key: &str) -> Result<f64> {
    let value = scalar(o, key)?
        .parse::<f64>()
        .map_err(|_| format!("Invalid bomber {key}"))?;
    if !value.is_finite() {
        return Err(format!("Nonfinite bomber {key}"));
    }
    Ok(value)
}
pub(crate) fn object<'a>(
    objects: &'a BTreeMap<ObjectId, Object>,
    o: &Object,
    key: &str,
) -> Result<&'a Object> {
    let id = scalar(o, key)?
        .parse::<u64>()
        .map_err(|_| "Invalid bomber reference")?;
    objects
        .get(&ObjectId(id))
        .ok_or_else(|| format!("Unknown bomber {key} reference"))
}
pub(crate) fn items<'a>(o: &'a Object, key: &str) -> Result<&'a [String]> {
    let matches: Vec<_> = o
        .fields
        .iter()
        .filter(|f| f.attributes.get("name").is_some_and(|s| s == key))
        .collect();
    let [f] = matches.as_slice() else {
        return Err(format!("Missing/ambiguous bomber {key}"));
    };
    Ok(&f.items)
}
impl Profile {
    pub fn load(objects: &BTreeMap<ObjectId, Object>, id: ObjectId) -> Result<Self> {
        let actor = objects.get(&id).ok_or("Unknown attacking bomber")?;
        if actor.kind != "cAirVehicle" {
            return Err("AttackTarget currently requires an aircraft bomber".into());
        }
        let base = object(objects, actor, "mBase")?;
        if field(base, "mUnitSubType") != Some("UNIT_AIR_BOMBER") {
            return Err("AttackTarget aircraft class unsupported".into());
        }
        let physics = object(objects, base, "mPhysicsBase")?;
        let flight = object(objects, physics, "mFlightMode")?;
        let seats = items(base, "mSeatData")?;
        let seat = objects
            .get(&ObjectId(
                seats
                    .first()
                    .ok_or("Missing bomber primary seat")?
                    .parse()
                    .map_err(|_| "Invalid primary seat")?,
            ))
            .ok_or("Unknown primary seat")?;
        let weapon = object(objects, seat, "WeaponType")?;
        let bullet = object(objects, weapon, "BulletType")?;
        if weapon.kind != "sWeaponBase"
            || bullet.kind != "sProjectileBase"
            || flight.kind != "cFlightData"
        {
            return Err("Unsupported bomber profile types".into());
        }
        if !matches!(number(weapon, "Flags")? as u32, 96 | 4160 | 4192 | 4288 | 32854)
            || !matches!(number(bullet, "mObjFlags")? as u32, 140 | 396 | 971 | 262540)
            || number(weapon, "ScatterAngle")? != 0.
            || number(bullet, "mScatterNum")? != 0.
            || number(bullet, "mProximityFuseDistance")? != 0.
        {
            return Err("Unsupported bomber weapon/projectile mode".into());
        }
        let table = object(objects, bullet, "mImpactTable")?;
        let mut impacts = Vec::new();
        for id in items(table, "mImpactBase")? {
            let impact = objects
                .get(&ObjectId(
                    id.parse().map_err(|_| "Invalid bomb impact reference")?,
                ))
                .ok_or("Missing bomb impact")?;
            let id = field(impact, "mExplosionType")
                .ok_or("Missing impact explosion")?
                .parse::<u64>()
                .map_err(|_| "Invalid impact explosion")?;
            if id != 0
                && objects
                    .get(&ObjectId(id))
                    .is_none_or(|e| e.kind != "sExplodeBase")
            {
                return Err("Unknown impact explosion".into());
            }
            impacts.push((id != 0).then_some(ObjectId(id)));
        }
        let kinds = items(bullet, "mDamageType")?;
        let amounts = items(bullet, "mDamageAmount")?;
        if kinds.len() != amounts.len() || kinds.is_empty() {
            return Err("Invalid bomb damage arrays".into());
        }
        let mut channels = Vec::new();
        for (kind, value) in kinds.iter().zip(amounts) {
            let amount = value.parse::<f64>().map_err(|_| "Invalid bomb damage")?;
            if !amount.is_finite() || amount < 0. {
                return Err("Invalid bomb damage".into());
            }
            if kind == "DAMAGE_NO_DAMAGE" {
                if amount != 0. {
                    return Err("Contradictory bomb damage".into());
                }
            } else if matches!(
                kind.as_str(),
                "DAMAGE_IMPACT"
                    | "DAMAGE_CUSTOM_1"
                    | "DAMAGE_CUSTOM_2"
                    | "DAMAGE_PRECALCULATED"
            ) {
                channels.push((kind.clone(), amount));
            } else {
                return Err("Unsupported bomb damage channel".into());
            }
        }
        let launch = scalar(weapon, "LaunchVec")?
            .split(',')
            .map(|v| {
                v.trim()
                    .parse::<f64>()
                    .map_err(|_| "Invalid bomb launch vector")
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let [x, y, speed, w] = launch.as_slice() else {
            return Err("Invalid bomb launch vector size".into());
        };
        if *x != 0.
            || !(-10.0..=10.0).contains(y)
            || *w != 0.
            || !speed.is_finite()
            || *speed < 0.
            || *speed > 1000.
        {
            return Err("Unsupported bomb launch vector".into());
        }
        let result = Self {
            speed: number(flight, "mCruiseSpeed")?,
            height: number(flight, "mCruiseHAG")?,
            rise: number(flight, "mUpTopSpeed")?,
            descend: number(flight, "mDownTopSpeed")?,
            turn: number(flight, "mMaxRotnRate")?,
            drop_distance: number(bullet, "mBombDropDistance")?,
            min_range: number(weapon, "MinRange")?,
            max_range: number(weapon, "MaxRange")?,
            fire_angle: number(weapon, "MinFireAngle")?,
            launch_speed: *speed,
            interval: number(weapon, "TimeBetweenBullets")?,
            reload: number(weapon, "ReloadTime")?,
            clip: number(weapon, "MaxAmmoInClip")? as u32,
            channels,
            impacts,
        };
        if !(0. < result.speed
            && result.speed <= 1000.
            && [result.height, result.rise, result.descend]
                .iter()
                .all(|v| *v > 0. && *v <= 1000.)
            && 0. < result.turn
            && result.turn <= 10.
            && 0. < result.drop_distance
            && result.drop_distance <= result.max_range
            && result.min_range >= 0.
            && result.min_range < result.max_range
            && result.max_range <= 1000.
            && result.fire_angle > 0.
            && result.fire_angle <= std::f64::consts::PI
            && 0. < result.interval
            && result.interval <= 60.
            && 0. < result.reload
            && result.reload <= 120.
            && result.clip > 0
            && result.clip <= 100
            && number(weapon, "MaxAmmoInClip")? == result.clip as f64)
        {
            return Err("Invalid bomber limits".into());
        }
        if result.impacts.len() != SURFACES.len() {
            return Err("Invalid impact surface table size".into());
        }
        Ok(result)
    }
}
pub(crate) const SURFACES: [&str; 27] = [
    "STONE",
    "WOOD",
    "METAL",
    "FLESH",
    "WATER",
    "GRAVEL",
    "SAND",
    "SNOW",
    "MUD",
    "GRASS",
    "AIRBURST",
    "ROAD",
    "CLOTH",
    "UNDERWATER",
    "PATH",
    "FOLIAGE",
    "CORNFIELD",
    "METAL_WOOD_HYBRID",
    "ARMOUR",
    "ST_SWORD",
    "CHAINMAIL",
    "SPECIALCASE3RDSTRIKE",
    "SPECIALCASEBLOCKBREAKER",
    "XYLVANIAN_STONE",
    "ICE",
    "CONCRETE",
    "NEROCYTE",
];
#[derive(Clone, Debug)]
pub struct Order {
    pub target: ObjectId,
    pub profile: Profile,
    pub remaining: f64,
    pub rounds: u32,
}
#[derive(Clone, Debug)]
pub struct Bomb {
    pub owner: ObjectId,
    pub target: ObjectId,
    pub position: [f32; 3],
    destination: [f32; 3],
    fall_speed: f64,
    age: f64,
    profile: Profile,
}
#[derive(Clone, Debug, Default)]
pub struct System {
    pub orders: BTreeMap<ObjectId, Order>,
    pub bombs: Vec<Bomb>,
    pub released: u64,
    pub impacts: u64,
    cooldowns: BTreeMap<ObjectId, (u32, f64)>,
}
impl System {
    pub fn order(
        &mut self,
        objects: &BTreeMap<ObjectId, Object>,
        owner: ObjectId,
        target: ObjectId,
    ) -> Result<()> {
        let profile = Profile::load(objects, owner)?;
        if !objects.get(&target).is_some_and(|o| {
            matches!(
                o.kind.as_str(),
                "cBuilding"
                    | "cDestroyableObject"
                    | "cMorphingBuilding"
                    | "cGroundVehicle"
                    | "cTroop"
                    | "cWaterVehicle"
            )
        }) {
            return Err("Bomber direct unit targeting not configured".into());
        }
        let (rounds, remaining) = self
            .cooldowns
            .get(&owner)
            .copied()
            .unwrap_or((profile.clip, 0.));
        self.orders.insert(
            owner,
            Order {
                target,
                rounds,
                profile,
                remaining,
            },
        );
        Ok(())
    }
    pub fn advance(
        &mut self,
        world: &mut World,
        positions: &mut BTreeMap<ObjectId, [f32; 3]>,
        dt: f64,
    ) -> Result<()> {
        let mut system = self.clone();
        let mut staged = world.clone();
        let mut moved = positions.clone();
        system.advance_staged(&mut staged, &mut moved, dt)?;
        *self = system;
        *world = staged;
        *positions = moved;
        Ok(())
    }
    pub(crate) fn advance_staged(
        &mut self,
        world: &mut World,
        positions: &mut BTreeMap<ObjectId, [f32; 3]>,
        dt: f64,
    ) -> Result<()> {
        if !dt.is_finite() || !(0.0..=1.0).contains(&dt) {
            return Err("Invalid bomber timestep".into());
        }
        if dt == 0.
            || (self.orders.is_empty() && self.bombs.is_empty() && self.cooldowns.is_empty())
        {
            return Ok(());
        }
        let scene = world.collision.clone().ok_or("Bomber collision missing")?;
        for (_, time) in self.cooldowns.values_mut() {
            *time = (*time - dt).max(0.);
        }
        let mut finished = Vec::new();
        for (owner, order) in &mut self.orders {
            if let Some((rounds, remaining)) = self.cooldowns.get(owner) {
                order.rounds = *rounds;
                order.remaining = *remaining;
            }
            let actor = world.units.get(owner).ok_or("Missing attacking aircraft")?;
            if actor.active != Some(true) || actor.health <= 0. {
                finished.push(*owner);
                continue;
            }
            let target = world.entity(order.target)?;
            if target.active != Some(true) || target.health <= 0. {
                finished.push(*owner);
                continue;
            }
            if world
                .activity
                .get(owner)
                .is_some_and(|a| a.enabled == Some(false))
            {
                continue;
            }
            let target_pos = *positions
                .get(&order.target)
                .ok_or("Missing bomber target position")?;
            let destination = scene.object_center(order.target).unwrap_or_else(|| {
                scene
                    .world_point(target_pos, 0.)
                    .unwrap_or([target_pos[0], world.water_height, target_pos[2]])
            });
            let p = positions.get_mut(owner).ok_or("Missing bomber position")?;
            let dx = (destination[0] - p[0]) as f64;
            let dz = (destination[2] - p[2]) as f64;
            let current = *world
                .headings
                .get(owner)
                .ok_or("Bomber heading not configured")? as f64;
            let desired = dx.atan2(dz);
            let delta = (desired - current + std::f64::consts::PI)
                .rem_euclid(std::f64::consts::TAU)
                - std::f64::consts::PI;
            let heading = current + delta.clamp(-order.profile.turn * dt, order.profile.turn * dt);
            world.headings.insert(*owner, heading as f32);
            p[0] += (heading.sin() * order.profile.speed * dt) as f32;
            p[2] += (heading.cos() * order.profile.speed * dt) as f32;
            let desired_height = scene
                .world_point([p[0], 0., p[2]], order.profile.height as f32)
                .map(|pt| pt[1])
                .unwrap_or(world.water_height + order.profile.height as f32);
            p[1] += (desired_height - p[1]).clamp(
                (-order.profile.descend * dt) as f32,
                (order.profile.rise * dt) as f32,
            );
            let distance = ((destination[0] - p[0]) as f64)
                .hypot((destination[2] - p[2]) as f64)
                .hypot((destination[1] - p[1]) as f64);
            if dx.hypot(dz) <= order.profile.drop_distance
                && distance >= order.profile.min_range
                && distance <= order.profile.max_range
                && world
                    .activity
                    .get(owner)
                    .is_none_or(|a| a.weapons_enabled != Some(false))
                && (delta - delta.clamp(-order.profile.turn * dt, order.profile.turn * dt)).abs()
                    < order.profile.fire_angle
                && order.remaining <= 1e-9
            {
                if self.bombs.len() >= 128 {
                    return Err("Bomber projectile capacity exceeded".into());
                }
                self.bombs.push(Bomb {
                    owner: *owner,
                    target: order.target,
                    position: *p,
                    destination,
                    fall_speed: 0.,
                    age: 0.,
                    profile: order.profile.clone(),
                });
                self.released += 1;
                order.rounds -= 1;
                order.remaining = if order.rounds == 0 {
                    order.rounds = order.profile.clip;
                    order.profile.reload
                } else {
                    order.profile.interval
                };
            }
        }
        for (owner, order) in &self.orders {
            self.cooldowns
                .insert(*owner, (order.rounds, order.remaining));
        }
        for owner in finished {
            self.orders.remove(&owner);
        }
        let mut flying = Vec::new();
        for mut bomb in std::mem::take(&mut self.bombs) {
            bomb.age += dt;
            if bomb.age > 30. {
                return Err("Guided bomb exceeded bounded flight time".into());
            }
            let start = bomb.position;
            let height = (start[1] - bomb.destination[1]).max(0.) as f64;
            let time =
                ((bomb.fall_speed * bomb.fall_speed + 60. * height).sqrt() - bomb.fall_speed) / 30.;
            let horizontal = ((bomb.destination[0] - start[0]) as f64)
                .hypot((bomb.destination[2] - start[2]) as f64);
            let fraction = if horizontal < 1e-6 {
                1.
            } else {
                (dt / time.max(dt))
                    .min((bomb.profile.speed + bomb.profile.launch_speed) * dt / horizontal)
            };
            let mut end = start;
            for i in [0, 2] {
                end[i] += ((bomb.destination[i] - start[i]) as f64 * fraction) as f32;
            }
            end[1] -= (bomb.fall_speed * dt + 15. * dt * dt) as f32;
            bomb.fall_speed += 30. * dt;
            let contact = world
                .collision
                .as_ref()
                .ok_or("Missing bomb collision")?
                .first_hit(start, end)?;
            if let Some((fraction, target)) = contact {
                let point =
                    std::array::from_fn(|i| start[i] + (end[i] - start[i]) * fraction as f32);
                let objects = world
                    .rewrite
                    .as_ref()
                    .ok_or("Missing bomb runtime")?
                    .objects();
                let index = if let Some(target) = target {
                    let base = object(
                        &objects,
                        objects.get(&target).ok_or("Unknown bomb contact")?,
                        "mBase",
                    )?;
                    let surface =
                        field(base, "mSurfaceType").ok_or("Missing bomb contact surface")?;
                    SURFACES
                        .iter()
                        .position(|s| *s == surface)
                        .ok_or("Unknown bomb contact surface")?
                } else {
                    // Declared Rust land-collision policy, not recovered terrain-material parity.
                    // World geometry without an owner uses the source STONE impact response.
                    0
                };
                if let Some(target) = target
                    && world.entity(target).is_ok()
                {
                    world.weapon_hit(
                        bomb.owner,
                        target,
                        bomb.profile.channels.clone(),
                        positions,
                    )?;
                }
                if let Some( explosion ) = bomb.profile.impacts[index] {
                    world.projectile_explosion(bomb.owner, point, explosion, positions)?;
                }
                if world.frozen
                    && let Some(order) = self.orders.get(&bomb.owner)
                    && world.buildings.contains(&order.target)
                    && world.entity(order.target).is_ok_and(|e| e.health > 0.0)
                {
                    world.kill_rewrite_in_scene(order.target, positions)?;
                }
                self.impacts += 1;
            } else {
                bomb.position = end;
                flying.push(bomb);
            }
        }
        self.bombs = flying;
        Ok(())
    }
}
