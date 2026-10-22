//! Source-profile defensive anti-air orders with explicit Rust guided-rocket physics.
//! Not native counter-effectiveness, evasion, ricochet, animation or aerodynamic parity.
use crate::{
    Object, ObjectId, Result,
    mission_bomber::{SURFACES, items, number, object, scalar},
    mission_world::World,
};
use std::collections::BTreeMap;
#[derive(Clone, Debug)]
pub struct AirShape {
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub heading: f32,
}
impl AirShape {
    pub fn validate(&self) -> Result<()> {
        if !self.heading.is_finite()
            || (0..3).any(|i| {
                !self.min[i].is_finite() || !self.max[i].is_finite() || self.min[i] >= self.max[i]
            })
        {
            return Err("Invalid aircraft hit bounds".into());
        }
        Ok(())
    }
    /// Yaw-oriented rendered bind bounds, deliberately not native physics nodes.
    pub fn hit(
        &self,
        start: [f32; 3],
        end: [f32; 3],
        position: [f32; 3],
        heading: f32,
    ) -> Result<Option<f64>> {
        self.validate()?;
        if !heading.is_finite()
            || start
                .iter()
                .chain(&end)
                .chain(&position)
                .any(|n| !n.is_finite())
        {
            return Err("Invalid aircraft segment".into());
        }
        let (s, c) = (heading as f64).sin_cos();
        let local = |p: [f32; 3]| {
            let x = (p[0] - position[0]) as f64;
            let z = (p[2] - position[2]) as f64;
            [c * x - s * z, (p[1] - position[1]) as f64, s * x + c * z]
        };
        let a = local(start);
        let b = local(end);
        let mut lo = 0_f64;
        let mut hi = 1_f64;
        for i in 0..3 {
            let d = b[i] - a[i];
            if d.abs() < 1e-10 {
                if a[i] < (self.min[i] as f64) || a[i] > (self.max[i] as f64) {
                    return Ok(None);
                }
            } else {
                let x = (self.min[i] as f64 - a[i]) / d;
                let y = (self.max[i] as f64 - a[i]) / d;
                lo = lo.max(x.min(y));
                hi = hi.min(x.max(y));
                if lo > hi {
                    return Ok(None);
                }
            }
        }
        Ok(Some(lo))
    }
}
#[derive(Clone, Debug)]
pub struct Profile {
    pub interval: f64,
    pub reload: f64,
    pub clip: u32,
    pub min: f64,
    pub max: f64,
    pub launch: f64,
    pub acceleration: f64,
    pub thrust_delay: f64,
    pub drag: f64,
    pub turn: f64,
    pub turn_ramp: f64,
    pub lifetime: f64,
    pub damage: Vec<(String, f64)>,
    pub impacts: Vec<Option<ObjectId>>,
}
impl Profile {
    pub fn load(objects: &BTreeMap<ObjectId, Object>, owner: ObjectId) -> Result<Self> {
        let actor = objects.get(&owner).ok_or("Unknown missile unit")?;
        let base = object(objects, actor, "mBase")?;
        if actor.kind != "cTroop"
            || scalar(base, "mUnitSubType")? != "UNIT_MAN_ANTI_AIR"
            || !matches!(
                scalar(actor, "mAttackStyle")?,
                "ATTACKSTYLE_FROM_DEFENSIVE_AREA" | "ATTACKSTYLE_HUNT_AND_DESTROY"
            )
        {
            return Err("AttackTarget troop weapon/style not configured".into());
        }
        let weapon = object(objects, base, "mWeaponLoads")?;
        let bullet = object(objects, weapon, "BulletType")?;
        if weapon.kind != "sWeaponBase"
            || bullet.kind != "sProjectileBase"
            || number(weapon, "Flags")? != 480.
            || number(bullet, "mObjFlags")? != 987.
            || number(weapon, "ScatterAngle")? != 0.
            || number(bullet, "mScatterNum")? != 0.
            || number(bullet, "mProximityFuseDistance")? != 0.
        {
            return Err("Unsupported anti-air projectile mode".into());
        }
        for key in ["mSHMVel", "mSHMFactor"] {
            let values = vector(scalar(bullet, key)?)?;
            if values != [0.; 4] {
                return Err("Oscillating missile mode not configured".into());
            }
        }
        let launch = vector(scalar(weapon, "LaunchVec")?)?;
        if launch[0] != 0. || launch[1] != 0. || launch[3] != 0. {
            return Err("Unsupported missile launch vector".into());
        }
        let mut damage = Vec::new();
        let kinds = items(bullet, "mDamageType")?;
        let amounts = items(bullet, "mDamageAmount")?;
        if kinds.len() != 4 || amounts.len() != 4 {
            return Err("Invalid missile damage arrays".into());
        }
        for (kind, amount) in kinds.iter().zip(amounts) {
            let amount = amount
                .parse::<f64>()
                .map_err(|_| "Invalid missile damage")?;
            if !amount.is_finite() || amount < 0. {
                return Err("Invalid missile damage amount".into());
            }
            match kind.as_str() {
                "DAMAGE_NO_DAMAGE" if amount == 0. => {}
                "DAMAGE_IMPACT" | "DAMAGE_PRECALCULATED" => damage.push((kind.clone(), amount)),
                _ => return Err("Unsupported missile damage channel".into()),
            }
        }
        let table = object(objects, bullet, "mImpactTable")?;
        let mut impacts = Vec::new();
        for id in items(table, "mImpactBase")? {
            let id = ObjectId(id.parse().map_err(|_| "Invalid missile impact reference")?);
            let impact = objects.get(&id).ok_or("Unknown missile impact")?;
            let explosion = scalar(impact, "mExplosionType")?
                .parse::<u64>()
                .map_err(|_| "Invalid missile explosion reference")?;
            if explosion != 0
                && objects
                    .get(&ObjectId(explosion))
                    .is_none_or(|o| o.kind != "sExplodeBase")
            {
                return Err("Unknown missile explosion".into());
            }
            impacts.push((explosion != 0).then_some(ObjectId(explosion)));
        }
        let profile = Self {
            interval: number(weapon, "TimeBetweenBullets")?,
            reload: number(weapon, "ReloadTime")?,
            clip: number(weapon, "MaxAmmoInClip")? as u32,
            min: number(weapon, "MinRange")?,
            max: number(weapon, "MaxRange")?,
            launch: launch[2],
            acceleration: number(bullet, "mAccel")?,
            thrust_delay: number(bullet, "mThrustDelay")?,
            drag: number(bullet, "mDrag")?,
            turn: number(bullet, "mTurnSpeed")?,
            turn_ramp: number(bullet, "mTimeToMaxTurnSpeed")?,
            lifetime: number(bullet, "mMaxLifeTime")?,
            damage,
            impacts,
        };
        if profile.clip == 0
            || profile.clip > 100
            || profile.clip as f64 != number(weapon, "MaxAmmoInClip")?
            || profile.impacts.len() != SURFACES.len()
            || profile.min < 0.
            || profile.min >= profile.max
            || profile.max > 1000.
            || [
                profile.interval,
                profile.reload,
                profile.launch,
                profile.acceleration,
                profile.turn,
                profile.turn_ramp,
                profile.lifetime,
            ]
            .iter()
            .any(|v| *v <= 0. || *v > 1000.)
            || !(0. ..=10.).contains(&profile.drag)
            || profile.thrust_delay < 0.
            || profile.thrust_delay >= profile.lifetime
        {
            return Err("Invalid missile profile limits".into());
        }
        Ok(profile)
    }
}
fn vector(value: &str) -> Result<[f64; 4]> {
    let v = value
        .split(',')
        .map(|n| {
            n.trim()
                .parse::<f64>()
                .map_err(|_| "Invalid missile vector".to_string())
        })
        .collect::<Result<Vec<_>>>()?;
    let v: [f64; 4] = v.try_into().map_err(|_| "Invalid missile vector size")?;
    if v.iter().any(|n| !n.is_finite()) {
        return Err("Nonfinite missile vector".into());
    }
    Ok(v)
}
fn length(v: [f64; 3]) -> f64 {
    v[0].hypot(v[1]).hypot(v[2])
}
fn unit(v: [f64; 3]) -> Result<[f64; 3]> {
    let n = length(v);
    if !n.is_finite() || n < 1e-9 {
        return Err("Invalid missile direction".into());
    }
    Ok(v.map(|x| x / n))
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn steer(from: [f64; 3], to: [f64; 3], limit: f64) -> Result<[f64; 3]> {
    let dot = (0..3).map(|i| from[i] * to[i]).sum::<f64>().clamp(-1., 1.);
    let angle = dot.acos();
    if angle <= limit {
        return Ok(to);
    }
    let mut axis = cross(from, to);
    if length(axis) < 1e-9 {
        axis = cross(
            from,
            if from[0].abs() < 0.9 {
                [1., 0., 0.]
            } else {
                [0., 1., 0.]
            },
        );
    }
    let axis = unit(axis)?;
    let tangent = cross(axis, from);
    unit(std::array::from_fn(|i| {
        from[i] * limit.cos() + tangent[i] * limit.sin()
    }))
}
#[derive(Clone, Debug)]
pub struct Missile {
    pub owner: ObjectId,
    pub target: ObjectId,
    pub position: [f32; 3],
    pub direction: [f64; 3],
    pub age: f64,
    speed: f64,
    profile: Profile,
}
#[derive(Clone, Debug)]
pub struct Order {
    pub target: ObjectId,
    pub profile: Profile,
}
#[derive(Clone, Debug, Default)]
pub struct System {
    pub orders: BTreeMap<ObjectId, Order>,
    pub missiles: Vec<Missile>,
    pub launched: u64,
    pub contacts: u64,
    pub air_contacts: u64,
    pub expired: u64,
    pub damaging_hits: u64,
    weapons: BTreeMap<ObjectId, (u32, f64)>,
}
impl System {
    pub fn order(
        &mut self,
        objects: &BTreeMap<ObjectId, Object>,
        owner: ObjectId,
        target: ObjectId,
    ) -> Result<()> {
        let profile = Profile::load(objects, owner)?;
        if objects.get(&target).is_none_or(|o| o.kind != "cAirVehicle") {
            return Err("Anti-air ground target not configured".into());
        }
        self.orders.insert(owner, Order { target, profile });
        Ok(())
    }
    pub fn advance(
        &mut self,
        world: &mut World,
        positions: &mut BTreeMap<ObjectId, [f32; 3]>,
        dt: f64,
    ) -> Result<()> {
        let mut s = self.clone();
        let mut w = world.clone();
        s.advance_staged(&mut w, positions, dt)?;
        *self = s;
        *world = w;
        Ok(())
    }
    pub(crate) fn advance_staged(
        &mut self,
        world: &mut World,
        positions: &BTreeMap<ObjectId, [f32; 3]>,
        dt: f64,
    ) -> Result<()> {
        if !dt.is_finite() || !(0. ..=1.).contains(&dt) {
            return Err("Invalid missile timestep".into());
        }
        if dt == 0.
            || (self.orders.is_empty() && self.missiles.is_empty() && self.weapons.is_empty())
        {
            return Ok(());
        }
        let scene = world.collision.clone().ok_or("Missile collision missing")?;
        for (_, time) in self.weapons.values_mut() {
            *time = (*time - dt).max(0.);
        }
        let mut finished = Vec::new();
        for (owner, order) in &self.orders {
            let actor = world.entity(*owner)?;
            let target = world.entity(order.target)?;
            if actor.active != Some(true)
                || actor.health <= 0.
                || target.active != Some(true)
                || target.health <= 0.
            {
                finished.push(*owner);
                continue;
            }
            if world
                .activity
                .get(owner)
                // Forced orders bypass autonomous-AI sleep, not explicit disarming.
                .is_some_and(|a| a.weapons_enabled == Some(false))
            {
                continue;
            }
            if !world.air_shapes.contains_key(&order.target) {
                return Err("Aircraft missile bounds missing".into());
            }
            let shooter_pos = *positions
                .get(owner)
                .ok_or("Missing missile shooter position")?;
            let origin = scene
                .world_point(shooter_pos, 2.)
                .unwrap_or([shooter_pos[0], world.water_height + 2., shooter_pos[2]]);
            let destination = *positions
                .get(&order.target)
                .ok_or("Missing missile target position")?;
            let delta = std::array::from_fn(|i| (destination[i] - origin[i]) as f64);
            let distance = length(delta);
            let (rounds, time) = self
                .weapons
                .entry(*owner)
                .or_insert((order.profile.clip, 0.));
            if distance < order.profile.min || distance > order.profile.max || *time > 1e-9 {
                continue;
            }
            if self.missiles.len() >= 256 {
                return Err("Missile capacity exceeded".into());
            }
            let direction = unit(delta)?;
            world
                .headings
                .insert(*owner, direction[0].atan2(direction[2]) as f32);
            self.missiles.push(Missile {
                owner: *owner,
                target: order.target,
                position: origin,
                direction,
                age: 0.,
                speed: order.profile.launch,
                profile: order.profile.clone(),
            });
            self.launched += 1;
            *rounds -= 1;
            *time = if *rounds == 0 {
                *rounds = order.profile.clip;
                order.profile.reload
            } else {
                order.profile.interval
            };
        }
        for id in finished {
            self.orders.remove(&id);
        }
        let mut flying = Vec::new();
        for mut missile in std::mem::take(&mut self.missiles) {
            let step = dt.min((missile.profile.lifetime - missile.age).max(0.));
            if let Some(target) = world
                .units
                .get(&missile.target)
                .filter(|u| u.active == Some(true) && u.health > 0.)
            {
                let _ = target;
                let p = positions
                    .get(&missile.target)
                    .ok_or("Missing missile target position")?;
                let d = std::array::from_fn(|i| (p[i] - missile.position[i]) as f64);
                if length(d) > 1e-9 {
                    missile.direction = steer(
                        missile.direction,
                        unit(d)?,
                        missile.profile.turn
                            * (missile.age / missile.profile.turn_ramp).min(1.)
                            * step,
                    )?;
                }
            }
            let powered = (missile.age + step - missile.profile.thrust_delay)
                .max(0.)
                .min(step);
            missile.speed = (missile.speed + missile.profile.acceleration * powered)
                * (-missile.profile.drag * step).exp();
            let end = std::array::from_fn(|i| {
                missile.position[i] + (missile.direction[i] * missile.speed * step) as f32
            });
            let mut contact = scene.first_hit(missile.position, end)?;
            for (id, shape) in world.air_shapes.iter() {
                if *id == missile.owner
                    || world
                        .units
                        .get(id)
                        .is_none_or(|u| u.active != Some(true) || u.health <= 0.)
                {
                    continue;
                }
                let p = *positions
                    .get(id)
                    .ok_or("Missing aircraft collision position")?;
                if let Some(fraction) = shape.hit(
                    missile.position,
                    end,
                    p,
                    world.headings.get(id).copied().unwrap_or(shape.heading),
                )? && contact.is_none_or(|(near, _)| fraction < near)
                {
                    contact = Some((fraction, Some(*id)));
                }
            }
            missile.age += step;
            if let Some((fraction, target)) = contact {
                let point = std::array::from_fn(|i| {
                    missile.position[i] + (end[i] - missile.position[i]) * fraction as f32
                });
                let objects = world
                    .rewrite
                    .as_ref()
                    .ok_or("Missing missile rewrite")?
                    .objects();
                let index = if let Some(id) = target {
                    let base = object(
                        &objects,
                        objects.get(&id).ok_or("Unknown missile contact")?,
                        "mBase",
                    )?;
                    let surface = scalar(base, "mSurfaceType")?;
                    SURFACES
                        .iter()
                        .position(|s| *s == surface)
                        .ok_or("Unknown missile contact surface")?
                } else {
                    0
                };
                if let Some(target) = target
                    && world.entity(target).is_ok()
                {
                    let before = world.entity(target)?.health;
                    world.weapon_hit(
                        missile.owner,
                        target,
                        missile.profile.damage.clone(),
                        positions,
                    )?;
                    if world.entity(target)?.health < before {
                        self.damaging_hits += 1;
                    }
                }
                if let Some(explosion) = missile.profile.impacts[index] {
                    world.projectile_explosion(missile.owner, point, explosion, positions)?;
                }
                if target.is_some_and(|id| world.air_shapes.contains_key(&id)) {
                    self.air_contacts += 1;
                }
                self.contacts += 1;
            } else if missile.age >= missile.profile.lifetime - 1e-9 {
                if let Some(explosion) = missile.profile.impacts[10] {
                    world.projectile_explosion(missile.owner, end, explosion, positions)?;
                }
                self.expired += 1;
            } else {
                missile.position = end;
                flying.push(missile);
            }
        }
        self.missiles = flying;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aircraft_segment_rotation_and_occlusion_fraction() {
        let b = AirShape {
            min: [-4., -1., -2.],
            max: [4., 1., 2.],
            heading: 0.,
        };
        assert_eq!(
            b.hit([0., 0., -10.], [0., 0., 10.], [0.; 3], 0.).unwrap(),
            Some(0.4)
        );
        let rotated = b
            .hit(
                [0., 0., -10.],
                [0., 0., 10.],
                [0.; 3],
                std::f32::consts::FRAC_PI_2,
            )
            .unwrap()
            .unwrap();
        assert!((rotated - 0.3).abs() < 1e-6);
        assert!(
            b.hit([0., 2., -10.], [0., 2., 10.], [0.; 3], 0.)
                .unwrap()
                .is_none()
        );
        assert!(b.hit([f32::NAN, 0., 0.], [0.; 3], [0.; 3], 0.).is_err());
    }
    #[test]
    fn steering_is_bounded_even_for_opposite_heading() {
        let v = steer([0., 0., 1.], [0., 0., -1.], 0.2).unwrap();
        assert!((length(v) - 1.).abs() < 1e-9);
        assert!((v[2].acos() - 0.2).abs() < 1e-9);
    }
}
