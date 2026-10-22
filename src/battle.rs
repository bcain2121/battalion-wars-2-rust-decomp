//! First playable original-map assault. Combat/objective tuning is replacement behavior,
//! deliberately separate from recovered Lua mission state and scoring/cinematics.
use crate::{
    ObjectId, Result,
    mission_world::{MoveOrder, Navigation, World},
    unit_animation::Motion,
    unit_collision::{Other, Scene},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Team {
    Friendly,
    Enemy,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Briefing,
    Playing,
    Won,
    Lost,
}
#[derive(Clone, Default)]
pub struct Input {
    pub movement: [f64; 2],
    pub sprint: bool,
    pub fire: bool,
    pub cycle: bool,
    pub start: bool,
    pub pause: bool,
    pub hold: bool,
}
#[derive(Clone, Debug)]
pub struct Shot {
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub team: Team,
    pub hit: Option<ObjectId>,
}
#[derive(Clone)]
pub struct Battle {
    pub world: World,
    pub positions: BTreeMap<ObjectId, [f32; 3]>,
    pub teams: BTreeMap<ObjectId, Team>,
    pub player: ObjectId,
    pub phase: Phase,
    pub paused: bool,
    pub hold: bool,
    pub objective: [f32; 3],
    pub capture: f64,
    pub time: f64,
    pub target: Option<ObjectId>,
    pub shots: Vec<Shot>,
    pub facings: BTreeMap<ObjectId, [f32; 2]>,
    pub shots_fired: u64,
    pub enemy_attacks: u64,
    pub player_damage: f64,
    cooldowns: BTreeMap<ObjectId, f64>,
    poses: BTreeMap<ObjectId, f64>,
    ai_timer: f64,
}
fn distance(a: [f32; 3], b: [f32; 3]) -> f64 {
    ((a[0] - b[0]) as f64).hypot((a[2] - b[2]) as f64)
}
pub fn camera_movement(forward: f64, right: f64, yaw: f64) -> Result<[f64; 2]> {
    if !forward.is_finite() || !right.is_finite() || !yaw.is_finite() {
        return Err("Invalid movement input".into());
    }
    let length = forward.hypot(right).max(1.0);
    Ok([
        (-yaw.sin() * forward + yaw.cos() * right) / length,
        (-yaw.cos() * forward - yaw.sin() * right) / length,
    ])
}
impl Battle {
    pub fn new(
        mut world: World,
        player: ObjectId,
        teams: BTreeMap<ObjectId, Team>,
    ) -> Result<Self> {
        if teams.get(&player) != Some(&Team::Friendly) || !teams.values().any(|t| *t == Team::Enemy)
        {
            return Err("Invalid battle roster".into());
        }
        for unit in world.units.values_mut() {
            unit.active = Some(false);
        }
        for (&id, &team) in &teams {
            let unit = world.units.get_mut(&id).ok_or("Missing battle unit")?;
            if !world.bodies.contains_key(&id) {
                return Err("Unsupported battle body".into());
            }
            unit.active = Some(true);
            unit.health = unit.max_health;
            unit.invulnerable = false;
            unit.forward_speed = Some(if team == Team::Enemy { 7.0 } else { 14.0 });
            let animation = unit
                .animation
                .as_mut()
                .ok_or("Missing original unit animations")?;
            for m in [
                Motion::Idle,
                Motion::Walk,
                Motion::Run,
                Motion::Fire,
                Motion::Death,
            ] {
                if !animation.bindings.contains_key(&m) {
                    return Err("Incomplete battle animation set".into());
                }
            }
            animation.select(Motion::Idle)?;
            animation.elapsed = 0.0;
        }
        world.configure_player(
            player,
            teams
                .iter()
                .filter(|(_, t)| **t == Team::Friendly)
                .map(|(id, _)| *id)
                .collect::<BTreeSet<_>>(),
        )?;
        world.navigation = Navigation::PlannedInfantry;
        let cooldowns = teams
            .iter()
            .enumerate()
            .map(|(i, (&id, &t))| {
                (
                    id,
                    if t == Team::Enemy {
                        0.35 + i as f64 * 0.13
                    } else {
                        0.0
                    },
                )
            })
            .collect();
        Ok(Self {
            world,
            positions: BTreeMap::new(),
            teams,
            player,
            phase: Phase::Briefing,
            paused: false,
            hold: false,
            objective: [0.0; 3],
            capture: 0.0,
            time: 0.0,
            target: None,
            shots: vec![],
            facings: BTreeMap::new(),
            shots_fired: 0,
            enemy_attacks: 0,
            player_damage: 0.0,
            cooldowns,
            poses: BTreeMap::new(),
            ai_timer: 0.0,
        })
    }
    pub fn install(&mut self, scene: Scene, anchor: [f32; 3]) -> Result<()> {
        let mut placed = Vec::new();
        let mut ally = 0;
        let mut enemy = 0;
        let mut ids = vec![self.player];
        ids.extend(self.teams.keys().copied().filter(|id| *id != self.player));
        for id in ids {
            let offset = if id == self.player {
                [35.0, 0.0]
            } else if self.teams[&id] == Team::Friendly {
                ally += 1;
                [38.0, if ally % 2 == 0 { 4.0 } else { -4.0 }]
            } else {
                let offsets = [[15.0, -5.0], [12.0, 5.0], [8.0, -2.0], [8.0, 8.0]];
                let p = offsets[enemy % 4];
                enemy += 1;
                p
            };
            let body = self.world.bodies[&id];
            let pos = scene.find_free(
                [anchor[0] + offset[0], anchor[1], anchor[2] + offset[1]],
                body,
                &placed,
            )?;
            self.positions.insert(id, pos);
            placed.push(Other {
                position: pos,
                body,
            });
            self.facings.insert(
                id,
                if self.teams[&id] == Team::Enemy {
                    [1.0, 0.0]
                } else {
                    [-1.0, 0.0]
                },
            );
        }
        self.objective = scene.find_free(
            [anchor[0] + 6.0, anchor[1], anchor[2]],
            self.world.bodies[&self.player],
            &[],
        )?;
        self.world.collision = Some(Arc::new(scene));
        Ok(())
    }
    pub fn alive(&self, id: ObjectId) -> bool {
        self.world.units[&id].health > 0.0
    }
    pub fn enemies(&self) -> usize {
        self.teams
            .iter()
            .filter(|(id, t)| **t == Team::Enemy && self.alive(**id))
            .count()
    }
    pub fn eye(&self, id: ObjectId) -> Result<[f32; 3]> {
        self.world
            .collision
            .as_ref()
            .ok_or("Battle scene missing")?
            .world_point(self.positions[&id], 1.3)
    }
    fn visible(&self, a: ObjectId, b: ObjectId, range: f64) -> Result<bool> {
        Ok(distance(self.positions[&a], self.positions[&b]) <= range
            && self
                .world
                .collision
                .as_ref()
                .ok_or("Battle scene missing")?
                .line_clear(self.eye(a)?, self.eye(b)?)?)
    }
    fn nearest(&self, who: ObjectId, visible_only: bool, range: f64) -> Result<Option<ObjectId>> {
        let mut choices = Vec::new();
        for (&id, &team) in &self.teams {
            if team != self.teams[&who]
                && self.alive(id)
                && (!visible_only || self.visible(who, id, range)?)
            {
                choices.push((distance(self.positions[&who], self.positions[&id]), id));
            }
        }
        choices.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        Ok(choices.first().map(|p| p.1))
    }
    fn fire(&mut self, shooter: ObjectId, target: ObjectId) -> Result<()> {
        let from = self.eye(shooter)?;
        let to = self.eye(target)?;
        let delta = [(to[0] - from[0]) as f64, (to[2] - from[2]) as f64];
        let length = delta[0].hypot(delta[1]);
        if length < 1e-6 {
            return Ok(());
        }
        // First living cylinder on the segment receives the hit; allies block shots without friendly fire.
        let mut first = (f64::INFINITY, None);
        for &id in self.teams.keys() {
            if id == shooter || !self.alive(id) {
                continue;
            }
            let p = self.positions[&id];
            let q = [(from[0] - p[0]) as f64, (from[2] - p[2]) as f64];
            let a = length * length;
            let b = q[0] * delta[0] + q[1] * delta[1];
            let r = self.world.bodies[&id].radius;
            let disc = b * b - a * (q[0] * q[0] + q[1] * q[1] - r * r);
            if disc < 0.0 {
                continue;
            }
            let t = (-b - disc.sqrt()) / a;
            if !(0.0..=1.0).contains(&t) {
                continue;
            }
            let foot = self.world.collision.as_ref().unwrap().world_point(p, 0.0)?[1] as f64;
            let y = from[1] as f64 + (to[1] - from[1]) as f64 * t;
            if y >= foot && y <= foot + self.world.bodies[&id].height && t < first.0 {
                first = (t, Some(id));
            }
        }
        let team = self.teams[&shooter];
        let hit = first.1;
        let end = if hit.is_some() {
            std::array::from_fn(|i| from[i] + (to[i] - from[i]) * first.0 as f32)
        } else {
            to
        };
        if let Some(id) = hit
            && self.teams[&id] != team
        {
            let factor = if shooter == self.player {
                0.22
            } else if team == Team::Enemy {
                0.035
            } else {
                0.075
            };
            let before = self.world.units[&id].health;
            let damage = self.world.units[&id].max_health * factor;
            self.world.damage(id, damage)?;
            if id == self.player {
                self.player_damage += before - self.world.units[&id].health;
            }
        }
        self.facings.insert(
            shooter,
            [(delta[0] / length) as f32, (delta[1] / length) as f32],
        );
        self.shots.push(Shot {
            from,
            to: end,
            team,
            hit,
        });
        self.shots_fired += 1;
        if team == Team::Enemy {
            self.enemy_attacks += 1;
        }
        self.cooldowns.insert(
            shooter,
            if shooter == self.player {
                0.22
            } else if team == Team::Enemy {
                1.05
            } else {
                0.8
            },
        );
        self.poses.insert(shooter, 0.2);
        if let Some(a) = &mut self.world.units.get_mut(&shooter).unwrap().animation {
            a.select(Motion::Fire)?;
            a.elapsed = 0.0;
        }
        Ok(())
    }
    pub fn tick(&mut self, dt: f64, input: &Input) -> Result<()> {
        let mut staged = self.clone();
        staged.step(dt, input)?;
        *self = staged;
        Ok(())
    }
    fn step(&mut self, dt: f64, input: &Input) -> Result<()> {
        if !dt.is_finite()
            || !(0.0..=0.1).contains(&dt)
            || input.movement.iter().any(|v| !v.is_finite())
        {
            return Err("Invalid battle timestep/input".into());
        }
        self.shots.clear();
        if self.phase == Phase::Briefing {
            if input.start {
                self.phase = Phase::Playing;
            }
            return Ok(());
        }
        if self.phase != Phase::Playing {
            for id in self.teams.keys() {
                if let Some(a) = &mut self.world.units.get_mut(id).unwrap().animation {
                    a.advance(dt)?;
                }
            }
            return Ok(());
        }
        if input.pause {
            self.paused = !self.paused;
        }
        if self.paused {
            return Ok(());
        }
        if input.hold {
            self.hold = !self.hold;
        }
        self.time += dt;
        for timer in self.cooldowns.values_mut() {
            *timer = (*timer - dt).max(0.0);
        }
        for timer in self.poses.values_mut() {
            *timer = (*timer - dt).max(0.0);
        }
        let length = input.movement[0].hypot(input.movement[1]).max(1.0);
        let speed = if input.sprint { 14.0 } else { 8.0 };
        self.world.direct_velocity.insert(
            self.player,
            [
                input.movement[0] / length * speed,
                input.movement[1] / length * speed,
            ],
        );
        self.world.orders.remove(&self.player);
        self.world.routes.remove(&self.player);
        if self.target.is_none_or(|id| !self.alive(id)) {
            self.target = self.nearest(self.player, true, 45.0)?.or(self.nearest(
                self.player,
                false,
                45.0,
            )?);
        }
        if input.cycle {
            let ids = self
                .teams
                .iter()
                .filter(|(id, t)| **t == Team::Enemy && self.alive(**id))
                .map(|(id, _)| *id)
                .collect::<Vec<_>>();
            self.target = if ids.is_empty() {
                None
            } else {
                Some(
                    ids[(self
                        .target
                        .and_then(|id| ids.iter().position(|v| *v == id))
                        .map(|i| i + 1)
                        .unwrap_or(0))
                        % ids.len()],
                )
            };
        }
        self.ai_timer -= dt;
        let think = self.ai_timer <= 0.0;
        if think {
            self.ai_timer = 0.5;
        }
        let ids = self.teams.keys().copied().collect::<Vec<_>>();
        let old = self.positions.clone();
        for &id in &ids {
            if id == self.player || !self.alive(id) {
                continue;
            }
            if think || input.hold {
                if self
                    .world
                    .activity
                    .get(&id)
                    .is_some_and(|a| a.enabled == Some(false))
                {
                    self.world.orders.remove(&id);
                    self.world.routes.remove(&id);
                    continue;
                }
                let pos = self.positions[&id];
                let enemy = self.teams[&id] == Team::Enemy;
                let target = if enemy {
                    self.nearest(id, false, 32.0)?
                } else {
                    self.nearest(id, true, 30.0)?
                };
                if enemy {
                    if let Some(target) = target {
                        let visible = self.visible(id, target, 32.0)?;
                        if distance(pos, self.positions[&target]) > 18.0 || !visible {
                            let p = self.positions[&target];
                            self.world.orders.insert(
                                id,
                                MoveOrder {
                                    target: [p[0] as f64, p[2] as f64],
                                    radius: if visible { 14.0 } else { 1.5 },
                                },
                            );
                        } else {
                            self.world.orders.remove(&id);
                        }
                    }
                } else if self.hold {
                    self.world.orders.remove(&id);
                    self.world.routes.remove(&id);
                } else {
                    let p = self.positions[&self.player];
                    let side = if id.0 % 2 == 0 { 3.0 } else { -3.0 };
                    if distance(pos, p) > 6.0 {
                        self.world.orders.insert(
                            id,
                            MoveOrder {
                                target: [p[0] as f64 + 2.0, p[2] as f64 + side],
                                radius: 1.5,
                            },
                        );
                    } else {
                        self.world.orders.remove(&id);
                    }
                }
            }
        }
        self.world.advance(dt, &mut self.positions)?;
        for &id in &ids {
            if !self.alive(id) {
                continue;
            }
            let delta = [
                self.positions[&id][0] - old[&id][0],
                self.positions[&id][2] - old[&id][2],
            ];
            let moved = delta[0].hypot(delta[1]) > 0.001;
            if moved {
                let len = delta[0].hypot(delta[1]);
                self.facings.insert(id, [delta[0] / len, delta[1] / len]);
            }
            let target = if id == self.player {
                if input.fire { self.target } else { None }
            } else {
                self.nearest(
                    id,
                    true,
                    if self.teams[&id] == Team::Enemy {
                        32.0
                    } else {
                        30.0
                    },
                )?
            };
            if self.cooldowns[&id] <= 0.0
                && self
                    .world
                    .activity
                    .get(&id)
                    .is_none_or(|a| a.weapons_enabled != Some(false))
                && !(self.world.frozen && id == self.player)
                && let Some(target) = target
                && self.alive(target)
                && self.visible(id, target, if id == self.player { 45.0 } else { 32.0 })?
            {
                self.fire(id, target)?;
            }
            let motion = if moved {
                if id == self.player && !input.sprint {
                    Motion::Walk
                } else {
                    Motion::Run
                }
            } else if self.poses.get(&id).is_some_and(|v| *v > 0.0) {
                Motion::Fire
            } else {
                Motion::Idle
            };
            self.world
                .units
                .get_mut(&id)
                .unwrap()
                .animation
                .as_mut()
                .unwrap()
                .select(motion)?;
        }
        if !self.alive(self.player) {
            self.finish(Phase::Lost)?;
        } else if self.enemies() == 0 {
            if distance(self.positions[&self.player], self.objective) <= 6.0 {
                self.capture = (self.capture + dt).min(5.0);
            } else {
                self.capture = 0.0;
            }
            if self.capture >= 5.0 {
                self.finish(Phase::Won)?;
            }
        }
        Ok(())
    }
    fn finish(&mut self, phase: Phase) -> Result<()> {
        self.phase = phase;
        self.world.orders.clear();
        self.world.routes.clear();
        self.world.direct_velocity.clear();
        for &id in self.teams.keys() {
            if self.alive(id) {
                self.world
                    .units
                    .get_mut(&id)
                    .unwrap()
                    .animation
                    .as_mut()
                    .unwrap()
                    .select(Motion::Idle)?;
            }
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::mission_world::Unit;
    use crate::unit_animation::Player;
    use crate::unit_collision::Body;
    fn fixture() -> Battle {
        let mut world = World::default();
        for id in [ObjectId(1), ObjectId(2)] {
            world.units.insert(
                id,
                Unit {
                    health: 100.0,
                    max_health: 100.0,
                    invulnerable: false,
                    active: None,
                    army: id.0.to_string(),
                    forward_speed: Some(20.0),
                    animation: Some(Player {
                        bindings: [
                            Motion::Idle,
                            Motion::Walk,
                            Motion::Run,
                            Motion::Fire,
                            Motion::Death,
                        ]
                        .into_iter()
                        .map(|m| (m, "test".into()))
                        .collect(),
                        motion: Motion::Idle,
                        elapsed: 0.0,
                    }),
                },
            );
            world.bodies.insert(id, Body::INFANTRY);
        }
        let mut b = Battle::new(
            world,
            ObjectId(1),
            BTreeMap::from([(ObjectId(1), Team::Friendly), (ObjectId(2), Team::Enemy)]),
        )
        .unwrap();
        b.install(Scene::default(), [0.0; 3]).unwrap();
        b
    }
    #[test]
    fn movement_pause_and_atomic_invalid_input() {
        let mut b = fixture();
        b.tick(
            0.03,
            &Input {
                start: true,
                ..Default::default()
            },
        )
        .unwrap();
        let pos = b.positions[&b.player];
        b.tick(
            0.1,
            &Input {
                movement: [1.0, 1.0],
                ..Default::default()
            },
        )
        .unwrap();
        assert!((distance(pos, b.positions[&b.player]) - 0.8).abs() < 0.001);
        b.tick(
            0.03,
            &Input {
                pause: true,
                ..Default::default()
            },
        )
        .unwrap();
        let pos = b.positions[&b.player];
        let time = b.time;
        b.tick(
            0.1,
            &Input {
                movement: [1.0, 0.0],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(b.positions[&b.player], pos);
        assert_eq!(b.time, time);
        assert!(b.tick(f64::NAN, &Input::default()).is_err());
        assert_eq!(b.time, time);
    }
    #[test]
    fn combat_win_loss_and_restart_snapshot() {
        let initial = fixture();
        let mut b = initial.clone();
        b.tick(
            0.03,
            &Input {
                start: true,
                ..Default::default()
            },
        )
        .unwrap();
        for _ in 0..300 {
            b.tick(
                1.0 / 30.0,
                &Input {
                    fire: true,
                    ..Default::default()
                },
            )
            .unwrap();
            if b.enemies() == 0 {
                break;
            }
        }
        assert_eq!(b.enemies(), 0);
        assert!(b.enemy_attacks > 0);
        assert!(b.player_damage > 0.0);
        b.positions.insert(b.player, b.objective);
        for _ in 0..160 {
            b.tick(1.0 / 30.0, &Input::default()).unwrap();
        }
        assert_eq!(b.phase, Phase::Won);
        let mut b = initial.clone();
        b.tick(
            0.03,
            &Input {
                start: true,
                ..Default::default()
            },
        )
        .unwrap();
        for _ in 0..1600 {
            b.tick(1.0 / 30.0, &Input::default()).unwrap();
            if b.phase == Phase::Lost {
                break;
            }
        }
        assert_eq!(b.phase, Phase::Lost);
        assert_eq!(initial.phase, Phase::Briefing);
        assert_eq!(initial.world.units[&initial.player].health, 100.0);
    }
    fn wall() -> Scene {
        let mut scene = Scene::default();
        scene
            .add_mesh(vec![
                [[0.0, 0.0, -10.0], [0.0, 5.0, -10.0], [0.0, 5.0, 10.0]],
                [[0.0, 0.0, -10.0], [0.0, 5.0, 10.0], [0.0, 0.0, 10.0]],
            ])
            .unwrap();
        scene
    }
    #[test]
    fn manual_control_stops_at_cover_and_overrides_orders() {
        let mut b = fixture();
        b.world.collision = Some(Arc::new(wall()));
        b.positions.insert(b.player, [-3.0, 0.0, 0.0]);
        b.positions.insert(ObjectId(2), [3.0, 0.0, 0.0]);
        b.world.damage(ObjectId(2), 100.0).unwrap();
        b.phase = Phase::Playing;
        b.objective = [30.0, 0.0, 0.0];
        b.world.orders.insert(
            b.player,
            MoveOrder {
                target: [-20.0, 0.0],
                radius: 1.0,
            },
        );
        for _ in 0..60 {
            b.tick(
                0.1,
                &Input {
                    movement: [1.0, 0.0],
                    ..Default::default()
                },
            )
            .unwrap();
        }
        let p = b.positions[&b.player];
        assert!(p[0] <= -0.6 && p[0] > -0.7);
        assert!(
            !b.world
                .collision
                .as_ref()
                .unwrap()
                .overlaps(p, Body::INFANTRY, &[])
                .unwrap()
        );
        assert!(!b.world.orders.contains_key(&b.player));
        assert!(!b.world.routes.contains_key(&b.player));
        assert!(b.world.mesh_contacts > 0);
    }
    #[test]
    fn cover_blocks_gunfire_enemy_approaches_and_allies_intercept() {
        let mut b = fixture();
        b.world.collision = Some(Arc::new(wall()));
        b.positions.insert(b.player, [-3.0, 0.0, 0.0]);
        b.positions.insert(ObjectId(2), [3.0, 0.0, 0.0]);
        b.phase = Phase::Playing;
        b.tick(
            0.01,
            &Input {
                fire: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(b.shots_fired, 0);
        assert_eq!(b.world.units[&ObjectId(2)].health, 100.0);
        assert_eq!(b.world.orders[&ObjectId(2)].radius, 1.5);
        b.world.collision = Some(Arc::new(Scene::default()));
        let ally = ObjectId(3);
        b.world.units.insert(ally, b.world.units[&b.player].clone());
        b.world.bodies.insert(ally, Body::INFANTRY);
        b.teams.insert(ally, Team::Friendly);
        b.positions.insert(ally, [0.0, 0.0, 0.0]);
        b.fire(b.player, ObjectId(2)).unwrap();
        assert_eq!(b.shots.last().unwrap().hit, Some(ally));
        assert_eq!(b.world.units[&ObjectId(2)].health, 100.0);
        assert_eq!(b.world.units[&ally].health, 100.0);
        b.positions.insert(ally, [0.0, 0.0, 3.0]);
        b.fire(b.player, ObjectId(2)).unwrap();
        assert_eq!(b.shots.last().unwrap().hit, Some(ObjectId(2)));
        assert!(b.world.units[&ObjectId(2)].health < 100.0);
    }
    #[test]
    fn squad_hold_then_follow() {
        let base = fixture();
        let mut world = base.world;
        let ally = ObjectId(3);
        world.units.insert(ally, world.units[&ObjectId(1)].clone());
        world.bodies.insert(ally, Body::INFANTRY);
        let mut teams = base.teams;
        teams.insert(ally, Team::Friendly);
        let mut b = Battle::new(world, ObjectId(1), teams).unwrap();
        b.install(Scene::default(), [0.0; 3]).unwrap();
        b.phase = Phase::Playing;
        b.tick(
            0.03,
            &Input {
                hold: true,
                ..Default::default()
            },
        )
        .unwrap();
        let p = b.positions[&ally];
        for _ in 0..40 {
            b.tick(
                0.03,
                &Input {
                    movement: [1.0, 0.0],
                    ..Default::default()
                },
            )
            .unwrap();
        }
        assert_eq!(b.positions[&ally], p);
        assert!(distance(p, b.positions[&b.player]) > 6.0);
        assert!(b.hold);
        b.tick(
            0.1,
            &Input {
                hold: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(!b.hold);
        for _ in 0..30 {
            b.tick(0.03, &Input::default()).unwrap();
        }
        assert!(distance(p, b.positions[&ally]) > 0.1);
    }
    #[test]
    fn inactive_ai_weapon_permissions_and_player_freeze() {
        use crate::mission_world::Activity;
        let mut b = fixture();
        b.phase = Phase::Playing;
        for id in [b.player, ObjectId(2)] {
            b.world.activity.insert(
                id,
                Activity {
                    enabled: Some(false),
                    weapons_enabled: Some(false),
                },
            );
        }
        let enemy = b.positions[&ObjectId(2)];
        for _ in 0..30 {
            b.tick(
                0.1,
                &Input {
                    fire: true,
                    ..Default::default()
                },
            )
            .unwrap();
        }
        assert_eq!(b.shots_fired, 0);
        assert_eq!(b.positions[&ObjectId(2)], enemy);
        b.world
            .activity
            .get_mut(&ObjectId(2))
            .unwrap()
            .weapons_enabled = Some(true);
        b.tick(0.1, &Input::default()).unwrap();
        assert!(b.enemy_attacks > 0);
        assert_eq!(b.positions[&ObjectId(2)], enemy);
        b.world.activity.get_mut(&b.player).unwrap().weapons_enabled = Some(true);
        b.world.frozen = true;
        let p = b.positions[&b.player];
        let health = b.world.units[&ObjectId(2)].health;
        b.tick(
            0.1,
            &Input {
                fire: true,
                movement: [1.0, 0.0],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(b.positions[&b.player], p);
        assert_eq!(b.world.units[&ObjectId(2)].health, health);
    }
    #[test]
    fn camera_axes_and_diagonal_speed() {
        assert_eq!(camera_movement(1.0, 0.0, 0.0).unwrap(), [0.0, -1.0]);
        let v = camera_movement(1.0, 1.0, 0.0).unwrap();
        assert!((v[0].hypot(v[1]) - 1.0).abs() < 1e-9);
    }
}
