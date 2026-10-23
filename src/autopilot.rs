//! Deterministic diagnostic input driver for original missions.
//!
//! The autopilot only produces ordinary player [`Input`] frames: it never edits
//! world state, script globals or progression. It walks toward the nearest
//! script-visible objective marker, fights nearby hostiles with the normal
//! lock/fire controls, recruits nearby friendlies and acknowledges pause
//! tutorials. It exists to exercise original scripts across levels and expose
//! the next unsupported engine boundary; it is not a player-quality AI and its
//! routes are not evidence of authored difficulty or balance.
use crate::{
    ObjectId,
    first_mission::{Input, Session},
    navigation, unit_collision,
};
use std::collections::VecDeque;

#[derive(Clone, Debug, Default)]
pub struct Autopilot {
    route: VecDeque<[f64; 2]>,
    planned_goal: Option<[f64; 2]>,
    last_plan: u64,
    frame: u64,
    paused_frames: u32,
    last_pos: Option<[f32; 3]>,
    stuck_frames: u32,
    /// Position sampled every 180 frames, and consecutive no-progress windows.
    progress_anchor: Option<[f32; 3]>,
    blocked_windows: u32,
    strafe: f64,
    unstick_frames: u32,
    /// (target, health, frames without damage progress) while firing.
    attack_progress: Option<(ObjectId, f64, u32)>,
    engaged_enemy: Option<ObjectId>,
    reached_marker: Option<ObjectId>,
    active_pickup: Option<ObjectId>,
}

/// What the autopilot decided to pursue on a frame (diagnostic reporting).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Goal {
    Idle,
    Frozen,
    Paused,
    Marker(ObjectId),
    Enemy(ObjectId),
}

fn yaw_delta(desired: f64, current: f64) -> f64 {
    (desired - current + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
        - std::f64::consts::PI
}

impl Autopilot {
    pub fn new() -> Self {
        Self::default()
    }

    fn hostile(s: &Session, player: ObjectId, id: ObjectId) -> bool {
        let w = &s.program.host.world;
        let Ok(me) = w.entity(player) else {
            return false;
        };
        let Ok(t) = w.entity(id) else {
            return false;
        };
        t.active == Some(true)
            && t.health > 0.0
            && !t.invulnerable
            && !t.army.is_empty()
            && t.army != "eNoArmy"
            && t.army != "eNeutral"
            && !w.same_allegiance(&t.army, &me.army)
    }

    /// Nearest visible marker position and its marker id.
    fn marker(s: &Session, pos: [f32; 3]) -> Option<(ObjectId, ObjectId, [f32; 3])> {
        let w = &s.program.host.world;
        let player_army = w
            .player
            .and_then(|p| w.entity(p).ok().map(|u| u.army.as_str()))
            .unwrap_or("");
        let mut best = None;
        let mut best_score = (true, true, true, true, true, true, f32::INFINITY);
        for m in &s.markers {
            if let Ok(Some(p)) = m.position(&s.program.host) {
                let target = m.effective_target(&s.program.host);
                let is_completed_parent = m
                    .parent
                    .is_some_and(|pid| w.objectives.get(&pid) == Some(&(true, true)));
                let is_hidden_parent = m
                    .parent
                    .is_some_and(|pid| w.objectives.get(&pid).is_some_and(|(vis, _)| !*vis));
                let is_dead_target = target != m.id
                    && w.entity(target)
                        .is_ok_and(|e| e.health <= 0.0 || e.active == Some(false));
                let is_own_squad_unit =
                    target != m.id && (w.player == Some(target) || w.roster.contains(&target));
                let is_passive_ally = target != m.id
                    && w.units.get(&target).is_some_and(|u| {
                        w.same_allegiance(&u.army, player_army)
                            && (u.army != player_army || w.enemy_targetable_only.contains(&target))
                    });
                let is_garrison_bldg = target != m.id
                    && w.garrisonable_buildings.contains(&target)
                    && w.props.get(&target).is_some_and(|b| {
                        b.army == "eNeutral"
                            || (w.same_allegiance(&b.army, player_army)
                                && w.passengers
                                    .get(&target)
                                    .is_some_and(|p| p.iter().any(|id| w.roster.contains(id))))
                    });
                let is_defended_bldg = target != m.id
                    && w.buildings.contains(&target)
                    && !w.capture_points.contains(&target)
                    && w.props.get(&target).is_some_and(|b| {
                        !b.invulnerable && w.same_allegiance(&b.army, player_army)
                    });
                let raw_d = (p[0] - pos[0]).hypot(p[2] - pos[2]);
                let d = if is_defended_bldg
                    && raw_d <= 260.0
                    && let Some(player) = w.player
                    && s.program.host.world.units.keys().any(|&uid| {
                        Self::hostile(s, player, uid)
                            && s.program
                                .host
                                .positions
                                .get(&uid)
                                .is_some_and(|up| (up[0] - p[0]).hypot(up[2] - p[2]) <= 95.0)
                    }) {
                    raw_d * 0.25
                } else {
                    raw_d
                };
                let score = (
                    is_completed_parent,
                    is_hidden_parent,
                    is_dead_target,
                    is_own_squad_unit,
                    is_passive_ally,
                    is_garrison_bldg,
                    d,
                );
                if score < best_score {
                    best_score = score;
                    best = Some((m.id, target, p));
                }
            }
        }
        if best.is_none() {
            let mut best_d = f32::INFINITY;
            for m in &s.markers {
                let Some(parent) = m.parent else {
                    continue;
                };
                if w.objectives.get(&parent) != Some(&(true, false)) {
                    continue;
                }
                let target = m.effective_target(&s.program.host);
                if !w.capture_points.contains(&target)
                    || w.props
                        .get(&target)
                        .is_none_or(|cp| cp.active != Some(true))
                    || w.capture_point_state
                        .get(&target)
                        .is_some_and(|cp| w.same_allegiance(&cp.owner, player_army))
                {
                    continue;
                }
                if let Some(&p) = s.program.host.positions.get(&target) {
                    let d = (p[0] - pos[0]).hypot(p[2] - pos[2]);
                    if d < best_d {
                        best_d = d;
                        best = Some((m.id, target, p));
                    }
                }
            }
        }
        best
    }

    fn nearest_enemy(
        s: &Session,
        player: ObjectId,
        pos: [f32; 3],
        range: f32,
        allow_ground_from_water: bool,
    ) -> Option<ObjectId> {
        let h = &s.program.host;
        let player_is_water = h.world.water_units.contains(&player);
        let player_is_vehicle = h.world.non_ground_units.contains(&player)
            || h.world
                .bodies
                .get(&player)
                .is_some_and(|b| b.radius > unit_collision::Body::INFANTRY.radius);
        let player_is_anti_armor = (h.world.unit_gtype_flags(player)
            & (0x0040_0000 | 0x0000_7400 | 0x0000_0008 | 0x0000_0002 | 0x0000_0001))
            != 0;
        let player_pos = h.positions.get(&player).copied().unwrap_or(pos);
        let my_army = h
            .world
            .entity(player)
            .map(|u| u.army.clone())
            .unwrap_or_default();
        let mut best = None;
        let mut best_score = f32::INFINITY;
        for id in h.world.units.keys().chain(h.world.buildings.iter()) {
            let is_bldg = h.world.buildings.contains(id);
            if *id == player
                || !Self::hostile(s, player, *id)
                || (is_bldg && !s.combat.is_armed_building(h, *id))
                || (!is_bldg
                    && !h.world.bodies.contains_key(id)
                    && !h.world.water_units.contains(id))
                || (!is_bldg
                    && h.world
                        .carrier_of
                        .get(id)
                        .is_some_and(|c| !h.world.capture_points.contains(c)))
                || (player_is_water
                    && !allow_ground_from_water
                    && !h.world.water_units.contains(id))
            {
                continue;
            }
            let pos_opt = if is_bldg {
                crate::mission_combat::Combat::entity_aim_point(h, *id)
                    .or_else(|| h.positions.get(id).copied())
            } else {
                h.positions.get(id).copied()
            };
            if let Some(p) = pos_opt {
                let d = (p[0] - pos[0]).hypot(p[2] - pos[2]);
                let d_player = (p[0] - player_pos[0]).hypot(p[2] - player_pos[2]);
                let defending = (pos[0] - player_pos[0]).hypot(pos[2] - player_pos[2]) > 1.0;
                if (if defending {
                    d >= range && d_player > 40.0
                } else {
                    d >= range && d_player >= range
                }) || (is_bldg
                    && !player_is_water
                    && !h.world.non_ground_units.contains(&player)
                    && d > 75.0
                    && d_player > 75.0)
                {
                    continue;
                }
                let cp_target = (!is_bldg)
                    .then(|| {
                        h.world
                            .carrier_of
                            .get(id)
                            .copied()
                            .or_else(|| h.world.entering_vehicle.get(id).copied())
                    })
                    .flatten();
                let (is_capturing, is_critical_capture) = cp_target
                    .filter(|c| h.world.capture_points.contains(c))
                    .and_then(|c| h.world.capture_point_state.get(&c))
                    .map(|st| {
                        let ours_or_neutral =
                            st.owner == "eNeutral" || h.world.same_allegiance(&st.owner, &my_army);
                        let critical =
                            ours_or_neutral && (st.owner == "eNeutral" || st.height < 65.0);
                        (ours_or_neutral, critical)
                    })
                    .unwrap_or((false, false));
                let player_is_air = h.world.non_ground_units.contains(&player) && !player_is_water;
                let is_anti_armor = !is_bldg
                    && (h.world.unit_gtype_flags(*id)
                        & (0x0040_0000
                            | 0x0000_7400
                            | if player_is_air {
                                0x0008_0000 | 0x0000_0800 | 0x0000_0080
                            } else {
                                0
                            }))
                        != 0;
                let enemy_is_heavy_veh =
                    is_bldg || h.world.bodies.get(id).is_some_and(|b| b.radius > 1.2);
                let enemy_is_combat_veh =
                    !is_bldg && h.world.bodies.get(id).is_some_and(|b| b.radius > 1.2);
                if player_is_vehicle
                    && !allow_ground_from_water
                    && !s.markers.is_empty()
                    && !is_capturing
                    && !is_anti_armor
                    && !enemy_is_heavy_veh
                    && d > 35.0
                    && d_player > 35.0
                {
                    continue;
                }
                let enemy_is_artillery =
                    !is_bldg && (h.world.unit_gtype_flags(*id) & (0x0000_1000 | 0x0000_0200)) != 0;
                let ref_d = if defending { d } else { d_player.min(d) };
                let score = if is_critical_capture {
                    d * 0.005
                } else if defending && enemy_is_artillery {
                    ref_d * 0.01
                } else if is_capturing {
                    d * 0.015
                } else if player_is_anti_armor && enemy_is_combat_veh {
                    ref_d * 0.03
                } else if player_is_vehicle && is_anti_armor {
                    (if defending { d } else { d_player }) * 0.04
                } else if player_is_anti_armor && enemy_is_heavy_veh {
                    d * 0.08
                } else if !player_is_vehicle && !player_is_anti_armor && enemy_is_heavy_veh {
                    d + 50.0
                } else {
                    d
                };
                if score < best_score {
                    best_score = score;
                    best = Some(*id);
                }
            }
        }
        best
    }

    fn nearest_air_enemy(
        s: &Session,
        player: ObjectId,
        pos: [f32; 3],
        range: f32,
    ) -> Option<ObjectId> {
        let h = &s.program.host;
        let mut best = None;
        let mut best_score = range;
        for id in h.world.units.keys() {
            if *id == player
                || !Self::hostile(s, player, *id)
                || !h.world.air_shapes.contains_key(id)
                || h.world.water_units.contains(id)
            {
                continue;
            }
            if let Some(p) = h.positions.get(id) {
                let d = (p[0] - pos[0]).hypot(p[2] - pos[2]);
                if d >= range {
                    continue;
                }
                let is_transport = (h.world.unit_gtype_flags(*id) & 0x0000_0004) != 0;
                let has_cargo = h.world.carrier_of.values().any(|&c| c == *id)
                    || h.world.entering_vehicle.values().any(|&c| c == *id);
                let is_objective = s.markers.iter().any(|m| m.effective_target(h) == *id);
                if is_transport && !has_cargo && !is_objective {
                    continue;
                }
                let score = if !is_transport && d <= 120.0 {
                    d * 0.25
                } else if is_objective && has_cargo {
                    d * 0.5
                } else if is_objective {
                    d * 0.75
                } else {
                    d
                };
                if score < best_score {
                    best_score = score;
                    best = Some(*id);
                }
            }
        }
        best
    }

    fn plan_water_towards(
        scene: &unit_collision::Scene,
        water_height: f32,
        start: [f32; 3],
        goal: [f64; 2],
        radius: f64,
    ) -> VecDeque<[f64; 2]> {
        use std::cmp::Ordering;
        use std::collections::{BinaryHeap, HashMap};

        const STEP: f64 = 16.0;
        let sx = (start[0] as f64 / STEP).round() as i32;
        let sz = (start[2] as f64 / STEP).round() as i32;
        let is_water = |ix: i32, iz: i32| -> bool {
            if ix == sx && iz == sz {
                return true;
            }
            let wx = (ix as f64 * STEP) as f32;
            let wz = (iz as f64 * STEP) as f32;
            let Some(wy) = scene.terrain_height(wx, wz) else {
                return false;
            };
            if wy > water_height + 4.0 {
                return false;
            }
            for (ox, oz) in [(6.0f32, 0.0f32), (-6.0, 0.0), (0.0, 6.0), (0.0, -6.0)] {
                if scene
                    .terrain_height(wx + ox, wz + oz)
                    .is_some_and(|y| y > water_height + 5.8)
                {
                    return false;
                }
            }
            true
        };
        let dist_goal = |ix: i32, iz: i32| -> f64 {
            (ix as f64 * STEP - goal[0]).hypot(iz as f64 * STEP - goal[1])
        };

        #[derive(Clone, Copy)]
        struct Node {
            f: f64,
            g: f64,
            c: (i32, i32),
        }
        impl PartialEq for Node {
            fn eq(&self, other: &Self) -> bool {
                self.f.to_bits() == other.f.to_bits() && self.c == other.c
            }
        }
        impl Eq for Node {}
        impl PartialOrd for Node {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }
        impl Ord for Node {
            fn cmp(&self, other: &Self) -> Ordering {
                other
                    .f
                    .total_cmp(&self.f)
                    .then_with(|| other.c.cmp(&self.c))
            }
        }

        let mut open = BinaryHeap::new();
        let mut g_score: HashMap<(i32, i32), f64> = HashMap::new();
        let mut parent: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
        let h0 = dist_goal(sx, sz);
        g_score.insert((sx, sz), 0.0);
        open.push(Node {
            f: h0,
            g: 0.0,
            c: (sx, sz),
        });
        let mut best_cell = (sx, sz);
        let mut best_h = h0;
        let mut expanded = 0usize;

        while let Some(cur) = open.pop() {
            if cur.g > g_score.get(&cur.c).copied().unwrap_or(f64::INFINITY) {
                continue;
            }
            let h = dist_goal(cur.c.0, cur.c.1);
            if h < best_h {
                best_h = h;
                best_cell = cur.c;
            }
            if h <= radius.max(STEP) {
                best_cell = cur.c;
                break;
            }
            expanded += 1;
            if expanded >= 4096 {
                break;
            }
            for (dx, dz) in [
                (1, 0),
                (-1, 0),
                (0, 1),
                (0, -1),
                (1, 1),
                (1, -1),
                (-1, 1),
                (-1, -1),
            ] {
                let nc = (cur.c.0 + dx, cur.c.1 + dz);
                if (nc.0 - sx).abs() > 256 || (nc.1 - sz).abs() > 256 {
                    continue;
                }
                if !is_water(nc.0, nc.1) {
                    continue;
                }
                if dx != 0
                    && dz != 0
                    && (!is_water(cur.c.0 + dx, cur.c.1) || !is_water(cur.c.0, cur.c.1 + dz))
                {
                    continue;
                }
                let step_cost = if dx != 0 && dz != 0 {
                    STEP * std::f64::consts::SQRT_2
                } else {
                    STEP
                };
                let ng = cur.g + step_cost;
                if ng + 1e-6 < g_score.get(&nc).copied().unwrap_or(f64::INFINITY) {
                    g_score.insert(nc, ng);
                    parent.insert(nc, cur.c);
                    open.push(Node {
                        f: ng + dist_goal(nc.0, nc.1),
                        g: ng,
                        c: nc,
                    });
                }
            }
        }

        let mut pts = Vec::new();
        let mut cur = best_cell;
        while cur != (sx, sz) {
            pts.push([cur.0 as f64 * STEP, cur.1 as f64 * STEP]);
            let Some(&p) = parent.get(&cur) else {
                break;
            };
            cur = p;
        }
        pts.reverse();
        if best_h <= radius.max(STEP) {
            pts.push(goal);
        }
        pts.into()
    }

    /// Produce the next input frame for `s`.
    pub fn step(&mut self, s: &Session) -> (Input, Goal) {
        self.frame += 1;
        let mut input = Input::default();
        let h = &s.program.host;
        let paused = h.world.tutorial.as_ref().is_some_and(|t| t.paused);
        if paused {
            self.paused_frames += 1;
            input.confirm = self.paused_frames >= 125 && self.paused_frames.is_multiple_of(4);
            return (input, Goal::Paused);
        }
        self.paused_frames = 0;
        let Some(player) = h.world.player else {
            return (input, Goal::Idle);
        };
        let in_cutscene = h.world.frozen
            || h.world
                .cameras
                .as_ref()
                .is_some_and(|c| c.active.is_some() && c.chase_target().is_none());
        if in_cutscene {
            self.route.clear();
            self.planned_goal = None;
            return (input, Goal::Frozen);
        }
        let Some(&pos) = h.positions.get(&player) else {
            return (input, Goal::Idle);
        };
        let Some(aim) = h.world.player_aim else {
            return (input, Goal::Idle);
        };
        let marker = Self::marker(s, pos);
        if let Some((mid, _, mp)) = marker {
            if (pos[0] - mp[0]).hypot(pos[2] - mp[2]) <= 16.0 {
                self.reached_marker = Some(mid);
            } else if self.reached_marker != Some(mid) {
                self.reached_marker = None;
            }
        } else {
            self.reached_marker = None;
        }
        let player_gtype = h.world.unit_gtype_flags(player);
        let is_recon = (player_gtype & 0x0002_0000) != 0;
        let player_is_vehicle = h
            .world
            .bodies
            .get(&player)
            .is_some_and(|b| b.radius > unit_collision::Body::INFANTRY.radius);
        let player_is_anti_armor = (player_gtype
            & (0x0040_0000 | 0x0000_7400 | 0x0000_0008 | 0x0000_0002 | 0x0000_0001))
            != 0;
        let pickup_goal = (!h.world.non_ground_units.contains(&player))
            .then(|| {
                let u = h.world.units.get(&player)?;
                let hp_thresh = if is_recon {
                    0.85
                } else if player_is_vehicle {
                    0.65
                } else {
                    0.45
                };
                if u.health >= u.max_health * hp_thresh {
                    return None;
                }
                let pickups = h.world.pickups.as_ref()?;
                let want = if player_is_vehicle {
                    crate::mission_pickup::Kind::VehicleHealth
                } else {
                    crate::mission_pickup::Kind::TroopHealth
                };
                let max_d = if is_recon {
                    55.0
                } else if player_is_vehicle {
                    if u.health < u.max_health * 0.45 {
                        165.0
                    } else {
                        85.0
                    }
                } else {
                    25.0
                };
                let scene = h.world.collision.as_ref();
                let p_body = h
                    .world
                    .bodies
                    .get(&player)
                    .copied()
                    .unwrap_or(unit_collision::Body::INFANTRY);
                pickups
                    .defs
                    .iter()
                    .filter(|(pid, def)| {
                        if def.kind != want {
                            return false;
                        }
                        match pickups.states.get(pid) {
                            Some(crate::mission_pickup::State::Active) => true,
                            Some(crate::mission_pickup::State::Unspawned) if player_is_vehicle => {
                                h.world.crushable_props.iter().any(|tid| {
                                    h.world.props.get(tid).is_some_and(|p| {
                                        p.active == Some(true) && p.health > 0.0 && !p.invulnerable
                                    }) && scene
                                        .and_then(|sc| sc.object_center(*tid))
                                        .or_else(|| h.positions.get(tid).copied())
                                        .is_some_and(|tp| {
                                            (tp[0] - def.position[0]).hypot(tp[2] - def.position[2])
                                                <= 3.0
                                        })
                                })
                            }
                            _ => false,
                        }
                    })
                    .filter_map(|(pid, def)| {
                        let d = (def.position[0] - pos[0]).hypot(def.position[2] - pos[2]);
                        let keep = self.active_pickup == Some(*pid);
                        let limit = if keep { max_d + 25.0 } else { max_d };
                        if d > limit {
                            return None;
                        }
                        let is_unspawned_tent = pickups.states.get(pid)
                            == Some(&crate::mission_pickup::State::Unspawned);
                        let path_d = if d > 15.0
                            && let Some(sc) = scene
                        {
                            let limits = if limit > 75.0 {
                                navigation::Limits {
                                    expansions: 2048,
                                    probes: 65536,
                                    extent: 192,
                                }
                            } else {
                                navigation::Limits {
                                    expansions: 1024,
                                    probes: 16384,
                                    extent: 96,
                                }
                            };
                            let plan_start = sc
                                .find_free_near(
                                    pos,
                                    p_body,
                                    &[],
                                    (p_body.radius * 2.0 + 1.0).min(8.0),
                                )
                                .unwrap_or(pos);
                            let goal_r = if is_unspawned_tent {
                                (p_body.radius + 4.5).max(5.5)
                            } else {
                                (p_body.radius + 0.8).max(1.5)
                            };
                            let plan = navigation::plan_towards(
                                sc,
                                plan_start,
                                [def.position[0] as f64, def.position[2] as f64],
                                goal_r,
                                p_body,
                                &[],
                                limits,
                            )
                            .ok()?;
                            if plan.status != navigation::Status::Found {
                                return None;
                            }
                            let mut prev = [pos[0] as f64, pos[2] as f64];
                            let mut len = 0.0f32;
                            for &pt in &plan.points {
                                len += ((pt[0] - prev[0]).hypot(pt[1] - prev[1])) as f32;
                                prev = pt;
                            }
                            if len > limit * 1.25 {
                                return None;
                            }
                            len
                        } else {
                            d
                        };
                        let score = if keep { path_d - 15.0 } else { path_d };
                        Some((score, *pid, def.position))
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0))
                    .map(|(_, pid, p)| (pid, p))
            })
            .flatten();
        self.active_pickup = pickup_goal.map(|(pid, _)| pid);
        let player_is_air_unit =
            h.world.non_ground_units.contains(&player) && !h.world.water_units.contains(&player);
        // Marker attached to a live hostile or destructible non-friendly objective structure:
        // that target is the objective.
        let marker_enemy = marker
            .map(|(mid, target, _)| (mid, target))
            .and_then(|(mid, t)| {
                let attached_destructible_prop = mid != t
                    && !h.world.capture_points.contains(&t)
                    && !h.world.garrisonable_buildings.contains(&t)
                    && h.world.props.get(&t).is_some_and(|p| {
                        p.active == Some(true)
                            && p.health > 0.0
                            && !p.invulnerable
                            && !h
                                .world
                                .entity(player)
                                .is_ok_and(|me| h.world.same_allegiance(&p.army, &me.army))
                    });
                ((Self::hostile(s, player, t)
                    || attached_destructible_prop
                    || (!h.world.buildings.contains(&t)
                        && h.world.props.get(&t).is_some_and(|p| {
                            p.active == Some(true) && p.health > 0.0 && !p.invulnerable
                        })))
                    && (!player_is_air_unit || s.combat.can_damage(h, player, t)))
                .then_some(t)
            });
        // Otherwise a vulnerable building right next to an unattached marker
        // (destroy-the-barrier style objectives), an inactive POW unit marker,
        // or remaining hostiles in a cluster whose primary marker target was
        // already destroyed.
        let marker_enemy = marker_enemy.or_else(|| {
            let (mid, target, mp) = marker?;
            let dead_target_hostile = mid != target
                && h.world.entity(target).is_ok_and(|e| {
                    (e.health <= 0.0
                        || (player_is_air_unit && !s.combat.can_damage(h, player, target)))
                        && !e.army.is_empty()
                        && e.army != "eNoArmy"
                        && e.army != "eNeutral"
                        && h.world
                            .entity(player)
                            .is_ok_and(|me| !h.world.same_allegiance(&e.army, &me.army))
                });
            let inactive_pow_target = mid != target
                && !h.world.enemy_targetable_only.contains(&target)
                && h.world.units.get(&target).is_some_and(|u| {
                    u.active == Some(true)
                        && u.health > 0.0
                        && h.world
                            .activity
                            .get(&target)
                            .is_some_and(|a| a.enabled == Some(false))
                        && h.world
                            .entity(player)
                            .is_ok_and(|me| h.world.same_allegiance(&u.army, &me.army))
                });
            if mid != target && !dead_target_hostile && !inactive_pow_target {
                return None;
            }
            let search_r = if dead_target_hostile { 80.0 } else { 25.0 };
            let scene = h.world.collision.as_ref()?;
            let bldg =
                h.world
                    .props
                    .iter()
                    .filter(|(id, p)| {
                        h.world.buildings.contains(id)
                            && !h.world.capture_points.contains(id)
                            && p.active == Some(true)
                            && p.health > 0.0
                            && !p.invulnerable
                            && (!player_is_air_unit || s.combat.can_damage(h, player, **id))
                            && (!(is_recon && mid == target) || p.max_health <= 500.0)
                            && (Self::hostile(s, player, **id)
                                || ((mid == target || inactive_pow_target)
                                    && p.max_health <= 500.0
                                    && !h.world.entity(player).is_ok_and(|me| {
                                        h.world.same_allegiance(&p.army, &me.army)
                                    })))
                    })
                    .filter_map(|(id, p)| {
                        let c = scene.object_center(*id)?;
                        let d = (c[0] - mp[0]).hypot(c[2] - mp[2]);
                        let r = if !dead_target_hostile && p.max_health <= 500.0 {
                            38.0
                        } else {
                            search_r
                        };
                        (d <= r).then_some((d, *id))
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0))
                    .map(|(_, id)| id);
            bldg.or_else(|| {
                if (mid == target || inactive_pow_target)
                    && (pos[0] - mp[0]).hypot(pos[2] - mp[2]) <= 45.0
                {
                    h.world
                        .props
                        .iter()
                        .filter(|(id, p)| {
                            !h.world.buildings.contains(id)
                                && !h.world.capture_points.contains(id)
                                && p.active == Some(true)
                                && p.health > 0.0
                                && !p.invulnerable
                                && p.max_health
                                    <= if inactive_pow_target
                                        || (player_is_vehicle
                                            && !h.world.non_ground_units.contains(&player))
                                    {
                                        450.0
                                    } else {
                                        50.0
                                    }
                                && !h
                                    .world
                                    .entity(player)
                                    .is_ok_and(|me| h.world.same_allegiance(&p.army, &me.army))
                        })
                        .filter_map(|(id, _)| {
                            let c = scene.object_center(*id)?;
                            let d = (c[0] - mp[0]).hypot(c[2] - mp[2]);
                            (d <= 22.0).then_some((d, *id))
                        })
                        .min_by(|a, b| a.0.total_cmp(&b.0))
                        .map(|(_, id)| id)
                } else {
                    None
                }
            })
            .or_else(|| {
                dead_target_hostile
                    .then(|| Self::nearest_enemy(s, player, mp, search_r, true))
                    .flatten()
            })
        });
        // A route that has made no progress for several windows is probably
        // blocked by a destructible structure (gates, barriers, explosive dumps, fences).
        let blocker = (self.blocked_windows >= 2
            && !h.world.non_ground_units.contains(&player)
            && !h.world.buildings.contains(&player))
        .then(|| {
            let scene = h.world.collision.as_ref()?;
            let bldg = h
                .world
                .props
                .iter()
                .filter(|(id, p)| {
                    h.world.buildings.contains(id)
                        && !h.world.capture_points.contains(id)
                        && p.active == Some(true)
                        && p.health > 0.0
                        && !p.invulnerable
                        && !h
                            .world
                            .entity(player)
                            .is_ok_and(|me| h.world.same_allegiance(&p.army, &me.army))
                        && (Self::hostile(s, player, **id) || p.max_health <= 500.0)
                })
                .filter_map(|(id, _)| {
                    let c = scene.object_center(*id)?;
                    let d = (c[0] - pos[0]).hypot(c[2] - pos[2]);
                    (d <= 25.0).then_some((d, *id))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, id)| id);
            bldg.or_else(|| {
                h.world
                    .props
                    .iter()
                    .filter(|(id, p)| {
                        !h.world.buildings.contains(id)
                            && !h.world.capture_points.contains(id)
                            && p.active == Some(true)
                            && p.health > 0.0
                            && !p.invulnerable
                            && p.max_health <= 1200.0
                            && !h
                                .world
                                .entity(player)
                                .is_ok_and(|me| h.world.same_allegiance(&p.army, &me.army))
                    })
                    .filter_map(|(id, _)| {
                        let c = scene.object_center(*id)?;
                        let d = (c[0] - pos[0]).hypot(c[2] - pos[2]);
                        (d <= 15.0).then_some((d, *id))
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0))
                    .map(|(_, id)| id)
            })
        })
        .flatten();
        let marker_is_friendly_defend = marker.is_some_and(|(_, target, _)| {
            h.world.props.get(&target).is_some_and(|p| {
                h.world
                    .entity(player)
                    .is_ok_and(|me| h.world.same_allegiance(&p.army, &me.army))
            })
        });
        let gate_winch =
            if !marker_is_friendly_defend && marker_enemy.is_none() && blocker.is_none() {
                let is_small_gate = |gid: &ObjectId, gp: &crate::mission_world::Unit| {
                    !h.world.buildings.contains(gid) && (gp.max_health - 5000.0).abs() <= 500.0
                };
                let is_big_gate = |gid: &ObjectId, gp: &crate::mission_world::Unit| {
                    h.world.buildings.contains(gid)
                        && gp.max_health >= 10000.0
                        && (gp.army.is_empty() || gp.army == "eNeutral" || gp.army == "eNoArmy")
                };
                h.world
                    .props
                    .iter()
                    .filter(|(id, p)| {
                        h.world.buildings.contains(id)
                            && !s.combat.is_armed_building(h, **id)
                            && p.active == Some(true)
                            && p.health > 0.0
                            && !p.invulnerable
                            && ((450.0..=550.0).contains(&p.max_health)
                                || (2900.0..=3100.0).contains(&p.max_health))
                            && !p.army.is_empty()
                            && p.army != "eNeutral"
                            && p.army != "eNoArmy"
                            && !h
                                .world
                                .entity(player)
                                .is_ok_and(|me| h.world.same_allegiance(&p.army, &me.army))
                    })
                    .filter_map(|(id, _)| {
                        let wp = h.positions.get(id)?;
                        let near_active_gate = h.world.props.iter().any(|(gid, gp)| {
                            (is_small_gate(gid, gp) || is_big_gate(gid, gp))
                                && gp.active == Some(true)
                                && gp.health > 0.0
                                && h.positions.get(gid).is_some_and(|gpos| {
                                    (gpos[0] - wp[0]).hypot(gpos[2] - wp[2]) <= 32.0
                                })
                        });
                        let row_already_opened = h.world.props.iter().any(|(gid, gp)| {
                            is_small_gate(gid, gp)
                                && gp.health <= 0.0
                                && h.positions
                                    .get(gid)
                                    .is_some_and(|gpos| (gpos[2] - wp[2]).abs() <= 60.0)
                        });
                        if near_active_gate && !row_already_opened {
                            let d = (wp[0] - pos[0]).hypot(wp[2] - pos[2]);
                            (d <= 350.0).then_some((d, *id))
                        } else {
                            None
                        }
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0))
                    .map(|(_, id)| id)
            } else {
                None
            };
        let marker_enemy = marker_enemy.or(blocker).or(gate_winch);
        let roster_marker_count = s
            .markers
            .iter()
            .filter(|m| {
                let mt = m.effective_target(h);
                mt != m.id && h.world.roster.contains(&mt)
            })
            .count();
        let marker_friendly_unit = marker
            .and_then(|(mid, target, mp)| {
                if target != mid && target != player {
                    if h.world.roster.contains(&target)
                        && (h.world.water_units.contains(&player) || roster_marker_count >= 2)
                    {
                        None
                    } else {
                        Some(target)
                    }
                } else {
                    h.world
                        .units
                        .keys()
                        .copied()
                        .filter(|&id| {
                            id != player
                                && !h.world.roster.contains(&id)
                                && !h.world.enemy_targetable_only.contains(&id)
                                && (h.world.non_ground_units.contains(&id)
                                    || h.world.bodies.get(&id).is_some_and(|b| {
                                        b.radius > unit_collision::Body::INFANTRY.radius
                                    }))
                        })
                        .find(|id| {
                            h.positions
                                .get(id)
                                .is_some_and(|up| (up[0] - mp[0]).hypot(up[2] - mp[2]) <= 12.0)
                        })
                }
            })
            .filter(|t| {
                !h.world.enemy_targetable_only.contains(t)
                    && (h.world.bodies.contains_key(t) || h.world.non_ground_units.contains(t))
                    && h.world
                        .activity
                        .get(t)
                        .is_none_or(|a| a.enabled != Some(false))
                    && h.world.units.get(t).is_some_and(|u| {
                        u.active == Some(true)
                            && u.health > 0.0
                            && h.world.entity(player).is_ok_and(|me| u.army == me.army)
                    })
            });
        let can_capture = (!player_is_vehicle && !h.world.non_ground_units.contains(&player))
            || h.world.roster.iter().any(|id| {
                h.world
                    .bodies
                    .get(id)
                    .is_some_and(|b| b.radius <= unit_collision::Body::INFANTRY.radius)
                    && h.world
                        .units
                        .get(id)
                        .is_some_and(|u| u.active == Some(true) && u.health > 0.0)
                    && h.positions.get(id).is_some_and(|ip| {
                        (ip[0] - pos[0]).hypot(ip[2] - pos[2]) <= 150.0
                            || marker.is_some_and(|(_, _, mp)| {
                                (ip[0] - mp[0]).hypot(ip[2] - mp[2]) <= 150.0
                            })
                    })
            });
        let marker_capture = can_capture
            .then(|| {
                marker
                    .map(|(_, target, _)| target)
                    .filter(|t| {
                        h.world.capture_points.contains(t)
                            && h.world
                                .props
                                .get(t)
                                .is_some_and(|cp| cp.active == Some(true))
                            && h.world.capture_point_state.get(t).is_some_and(|st| {
                                h.world.entity(player).is_ok_and(|me| {
                                    !h.world.same_allegiance(&st.owner, &me.army)
                                        || st.height < 100.0
                                })
                            })
                    })
                    .or_else(|| {
                        let (_, target, _) = marker?;
                        let cp_id = h
                            .world
                            .installations
                            .iter()
                            .find(|&(si_id, si)| {
                                *si_id == target || si.main_building == Some(target)
                            })
                            .and_then(|(_, si)| si.capture_point)?;
                        h.world
                            .props
                            .get(&cp_id)
                            .is_some_and(|cp| cp.active == Some(true))
                            .then_some(cp_id)
                            .filter(|cp_id| {
                                h.world.capture_point_state.get(cp_id).is_some_and(|st| {
                                    h.world.entity(player).is_ok_and(|me| {
                                        !h.world.same_allegiance(&st.owner, &me.army)
                                    })
                                })
                            })
                    })
                    .or_else(|| {
                        if is_recon {
                            return None;
                        }
                        let (_, _, mp) = marker?;
                        h.world
                            .capture_points
                            .iter()
                            .copied()
                            .filter(|cp_id| {
                                h.world
                                    .props
                                    .get(cp_id)
                                    .is_some_and(|cp| cp.active == Some(true))
                                    && h.world.capture_point_state.get(cp_id).is_some_and(|st| {
                                        h.world.entity(player).is_ok_and(|me| {
                                            !h.world.same_allegiance(&st.owner, &me.army)
                                                || st.height < 100.0
                                        })
                                    })
                            })
                            .find(|cp_id| {
                                h.positions
                                    .get(cp_id)
                                    .is_some_and(|cp| (cp[0] - mp[0]).hypot(cp[2] - mp[2]) <= 25.0)
                            })
                    })
            })
            .flatten();
        let marker_near_friendly_building = marker.is_some_and(|(mid, target, mp)| {
            ((mid == target || h.world.buildings.contains(&target))
                && h.world.buildings.iter().any(|bid| {
                    !h.world.capture_points.contains(bid)
                        && h.world.props.get(bid).is_some_and(|b| {
                            !b.invulnerable
                                && h.world
                                    .entity(player)
                                    .is_ok_and(|me| h.world.same_allegiance(&b.army, &me.army))
                        })
                        && h.positions
                            .get(bid)
                            .is_some_and(|bp| (bp[0] - mp[0]).hypot(bp[2] - mp[2]) <= 20.0)
                }))
                || ((mid == target || h.world.units.contains_key(&target))
                    && h.world.units.iter().any(|(&uid, u)| {
                        uid != player
                            && u.active == Some(true)
                            && u.health > 0.0
                            && (h.world.unit_gtype_flags(uid) & 0x0000_0400) != 0
                            && h.world
                                .entity(player)
                                .is_ok_and(|me| h.world.same_allegiance(&u.army, &me.army))
                            && h.positions
                                .get(&uid)
                                .is_some_and(|up| (up[0] - mp[0]).hypot(up[2] - mp[2]) <= 25.0)
                    }))
        });
        let defend_dist = if marker_near_friendly_building {
            280.0
        } else {
            160.0
        };
        let defending_cp = marker.is_some_and(|(mid, target, mp)| {
            (mp[0] - pos[0]).hypot(mp[2] - pos[2]) <= defend_dist
                && !(is_recon
                    && mid == target
                    && self.reached_marker != Some(mid)
                    && (mp[0] - pos[0]).hypot(mp[2] - pos[2]) > 45.0)
                && (marker_near_friendly_building
                    || (h.world.capture_points.contains(&target)
                        && h.world.capture_point_state.get(&target).is_some_and(|st| {
                            h.world
                                .entity(player)
                                .is_ok_and(|me| h.world.same_allegiance(&st.owner, &me.army))
                        }))
                    || h.world
                        .installations
                        .iter()
                        .find(|&(si_id, si)| *si_id == target || si.main_building == Some(target))
                        .and_then(|(_, si)| si.capture_point)
                        .is_some_and(|cp_id| {
                            h.world.capture_point_state.get(&cp_id).is_some_and(|st| {
                                h.world
                                    .entity(player)
                                    .is_ok_and(|me| h.world.same_allegiance(&st.owner, &me.army))
                            })
                        })
                    || (mid == target
                        && h.world.capture_points.iter().any(|cp_id| {
                            h.world
                                .props
                                .get(cp_id)
                                .is_some_and(|cp| cp.active == Some(true))
                                && h.world.capture_point_state.get(cp_id).is_some_and(|st| {
                                    h.world.entity(player).is_ok_and(|me| {
                                        h.world.same_allegiance(&st.owner, &me.army)
                                    })
                                })
                                && h.positions
                                    .get(cp_id)
                                    .is_some_and(|cp| (cp[0] - mp[0]).hypot(cp[2] - mp[2]) <= 25.0)
                        }))
                    || (h.world.buildings.contains(&target)
                        && !h.world.capture_points.contains(&target)
                        && h.world.props.get(&target).is_some_and(|b| {
                            !b.invulnerable
                                && h.world
                                    .entity(player)
                                    .is_ok_and(|me| h.world.same_allegiance(&b.army, &me.army))
                        }))
                    || (marker_friendly_unit.is_none()
                        && h.world.units.get(&target).is_some_and(|u| {
                            h.world
                                .entity(player)
                                .is_ok_and(|me| h.world.same_allegiance(&u.army, &me.army))
                        })))
        });
        let at_recon_objective = is_recon
            && !defending_cp
            && marker_capture.is_none()
            && marker_friendly_unit.is_none()
            && marker_enemy.is_none()
            && marker.is_some_and(|(mid, target, _)| {
                self.reached_marker == Some(mid)
                    && (!h.world.buildings.contains(&target)
                        || h.world.capture_points.contains(&target))
            });
        let search_origin = marker
            .filter(|_| defending_cp || at_recon_objective)
            .map_or(pos, |(_, _, mp)| mp);
        let player_is_building = h.world.buildings.contains(&player);
        let is_anti_air = player_is_building
            || (player_gtype & (0x0000_0800 | 0x0008_0000 | 0x0000_0080 | 0x0000_0010)) != 0;
        let acquire_range = if h.world.water_units.contains(&player) || player_is_building {
            260.0
        } else if marker_near_friendly_building && h.world.non_ground_units.contains(&player) {
            250.0
        } else if defending_cp {
            210.0
        } else if h.world.non_ground_units.contains(&player) {
            if marker.is_some() && marker_enemy.is_none() {
                75.0
            } else {
                210.0
            }
        } else if at_recon_objective {
            35.0
        } else if is_anti_air
            && h.world.roster.len() <= 2
            && marker.is_some()
            && marker_enemy.is_none()
        {
            28.0
        } else if player_is_vehicle && (player_gtype & 0x0000_0400) != 0 {
            195.0
        } else if player_is_vehicle && !is_recon {
            80.0
        } else {
            65.0
        };
        let enemy = Self::nearest_enemy(
            s,
            player,
            search_origin,
            acquire_range,
            defending_cp || at_recon_objective,
        )
        .or_else(|| {
            let prev = self.engaged_enemy?;
            let player_is_water = h.world.water_units.contains(&player);
            let prev_is_harmless_inf = player_is_vehicle
                && !is_recon
                && !defending_cp
                && marker.is_some()
                && !s.combat.is_armed_building(h, prev)
                && !h.world.bodies.get(&prev).is_some_and(|b| b.radius > 1.2)
                && (h.world.unit_gtype_flags(prev) & (0x0040_0000 | 0x0000_7400)) == 0;
            let leash = if at_recon_objective {
                acquire_range
            } else if prev_is_harmless_inf {
                40.0
            } else {
                acquire_range + 30.0
            };
            if prev != player
                && Self::hostile(s, player, prev)
                && (h.world.bodies.contains_key(&prev)
                    || h.world.water_units.contains(&prev)
                    || s.combat.is_armed_building(h, prev))
                && (!player_is_water || defending_cp || h.world.water_units.contains(&prev))
                && crate::mission_combat::Combat::entity_aim_point(h, prev)
                    .or_else(|| h.positions.get(&prev).copied())
                    .is_some_and(|p| {
                        (p[0] - search_origin[0]).hypot(p[2] - search_origin[2]) <= leash
                    })
            {
                Some(prev)
            } else {
                None
            }
        })
        .filter(|e| {
            !(is_recon
                && marker.is_some()
                && marker_enemy.is_none()
                && (h.world.buildings.contains(e)
                    || h.world.bodies.get(e).is_some_and(|b| b.radius > 1.2)))
        });
        self.engaged_enemy = enemy;
        let player_is_air =
            h.world.non_ground_units.contains(&player) && !h.world.water_units.contains(&player);
        let player_is_bomber = (player_gtype & 0x0000_0002) != 0;
        let can_engage_air =
            !player_is_bomber && (is_anti_air || !player_is_vehicle || player_is_air);
        let air_enemy = if can_engage_air
            && (is_anti_air
                || player_is_air
                || defending_cp
                || (enemy.is_none()
                    && marker_enemy.is_none()
                    && marker_friendly_unit.is_none()
                    && marker_capture.is_none()))
        {
            Self::nearest_air_enemy(
                s,
                player,
                search_origin,
                if is_anti_air || (player_is_air && !defending_cp) {
                    450.0
                } else if defending_cp && player_is_air && !marker_near_friendly_building {
                    75.0
                } else {
                    250.0
                },
            )
        } else {
            None
        };
        let base_fire_range = if is_anti_air {
            195.0
        } else if h.world.non_ground_units.contains(&player) {
            s.combat
                .weapon_max_range(player, true)
                .map(|m| (m - 5.0).max(55.0))
                .unwrap_or(155.0)
        } else if player_is_building {
            s.combat
                .weapon_max_range(player, true)
                .map(|m| (m - 2.0).max(55.0))
                .unwrap_or(80.0)
        } else if (player_is_vehicle && !is_recon) || player_is_anti_armor {
            s.combat
                .weapon_max_range(player, true)
                .map(|m| (m - 1.0).max(75.0))
                .unwrap_or(75.0)
        } else {
            55.0
        };
        let (goal_kind, mut goal, mut radius, attack): (Goal, [f64; 2], f64, Option<ObjectId>) =
            match (enemy, marker_enemy, marker) {
                _ if pickup_goal.is_some_and(|(_, pp)| {
                    let pd = (pp[0] - pos[0]).hypot(pp[2] - pos[2]);
                    let hp_frac = h
                        .world
                        .units
                        .get(&player)
                        .map_or(1.0, |u| u.health / u.max_health.max(1.0));
                    let urgent_threat = player_is_vehicle
                        && !is_recon
                        && pd > 30.0
                        && hp_frac >= 0.45
                        && enemy.is_some_and(|e| {
                            let eg = h.world.unit_gtype_flags(e);
                            let e_aa = (eg & (0x0040_0000 | 0x0000_7400)) != 0
                                || ((eg & 0x0002_0000) == 0
                                    && h.world.bodies.get(&e).is_some_and(|b| b.radius > 1.2));
                            let ed = h
                                .positions
                                .get(&e)
                                .map_or(f32::INFINITY, |ep| (ep[0] - pos[0]).hypot(ep[2] - pos[2]));
                            e_aa && ed <= 35.0
                        });
                    let close_marker_enemy = hp_frac >= 0.30
                        && marker_enemy.is_some_and(|me| {
                            crate::mission_combat::Combat::entity_aim_point(h, me)
                                .or_else(|| h.positions.get(&me).copied())
                                .is_some_and(|mp| (mp[0] - pos[0]).hypot(mp[2] - pos[2]) <= 75.0)
                        });
                    let close_marker_capture = marker_capture.is_some()
                        && pd > 45.0
                        && marker
                            .is_some_and(|(_, _, mp)| (mp[0] - pos[0]).hypot(mp[2] - pos[2]) < pd);
                    let urgent_vehicle_heal =
                        player_is_vehicle && !is_recon && !urgent_threat && hp_frac < 0.60;
                    !urgent_threat
                        && !close_marker_enemy
                        && !close_marker_capture
                        && (is_recon || enemy.is_none() || urgent_vehicle_heal)
                        && !(is_recon
                            && !defending_cp
                            && marker_enemy.is_none()
                            && marker.is_some_and(|(_, _, mp)| {
                                (mp[0] - pp[0]).hypot(mp[2] - pp[2]) <= 50.0
                            }))
                }) =>
                {
                    let (pid, pp) = pickup_goal.unwrap();
                    (
                        Goal::Marker(pid),
                        [pp[0] as f64, pp[2] as f64],
                        0.5,
                        enemy.or(marker_enemy),
                    )
                }
                (_, _, Some((mid, _, p))) if marker_friendly_unit.is_some() => {
                    (Goal::Marker(mid), [p[0] as f64, p[2] as f64], 8.0, None)
                }
                (enemy, None, Some((mid, target, p)))
                    if is_recon
                        && !defending_cp
                        && marker_capture.is_none()
                        && marker_friendly_unit.is_none()
                        && (!h.world.buildings.contains(&target)
                            || h.world.capture_points.contains(&target))
                        && (self.reached_marker != Some(mid)
                            || enemy.is_none_or(|e| {
                                h.positions
                                    .get(&e)
                                    .is_none_or(|ep| (ep[0] - p[0]).hypot(ep[2] - p[2]) > 35.0)
                            })) =>
                {
                    (Goal::Marker(mid), [p[0] as f64, p[2] as f64], 0.02, enemy)
                }
                _ if (is_anti_air || player_is_air || defending_cp)
                    && marker_enemy.is_none_or(|me| {
                        h.world.air_shapes.contains_key(&me)
                            || air_enemy.is_some_and(|ae| {
                                h.positions.get(&ae).is_some_and(|aep| {
                                    (aep[0] - pos[0]).hypot(aep[2] - pos[2]) <= 110.0
                                        || marker.is_some_and(|(_, _, mp)| {
                                            (aep[0] - mp[0]).hypot(aep[2] - mp[2]) <= 110.0
                                        })
                                })
                            })
                    })
                    && air_enemy.is_some_and(|ae| {
                        let air_leash = if marker_near_friendly_building && player_is_air {
                            250.0
                        } else {
                            85.0
                        };
                        is_anti_air
                            || (player_is_air && !defending_cp)
                            || enemy.is_none()
                            || h.positions.get(&ae).is_some_and(|aep| {
                                (aep[0] - pos[0]).hypot(aep[2] - pos[2]) <= air_leash
                                    || marker.is_some_and(|(_, _, mp)| {
                                        (aep[0] - mp[0]).hypot(aep[2] - mp[2]) <= air_leash
                                    })
                            })
                    })
                    && enemy.is_none_or(|e| {
                        let capturing_cp = h
                            .world
                            .carrier_of
                            .get(&e)
                            .is_some_and(|c| h.world.capture_points.contains(c))
                            || h.world
                                .entering_vehicle
                                .get(&e)
                                .is_some_and(|c| h.world.capture_points.contains(c))
                            || (!is_anti_air
                                && h.world.capture_points.iter().any(|cp| {
                                    h.positions.get(cp).is_some_and(|cpp| {
                                        h.positions.get(&e).is_some_and(|ep| {
                                            (cpp[0] - ep[0]).hypot(cpp[2] - ep[2]) <= 35.0
                                        })
                                    })
                                }));
                        let urgent_aa = player_is_vehicle
                            && !player_is_air
                            && (h.world.unit_gtype_flags(e) & (0x0040_0000 | 0x0000_7400)) != 0
                            && h.positions
                                .get(&e)
                                .is_some_and(|ep| (ep[0] - pos[0]).hypot(ep[2] - pos[2]) <= 45.0);
                        let defending_artillery = marker_near_friendly_building
                            && !h.world.buildings.contains(&e)
                            && (h.world.unit_gtype_flags(e) & (0x0000_1000 | 0x0000_0200)) != 0;
                        !capturing_cp
                            && !urgent_aa
                            && !defending_artillery
                            && (is_anti_air
                                || (player_is_air
                                    && !defending_cp
                                    && !marker_near_friendly_building)
                                || (player_is_air
                                    && marker_near_friendly_building
                                    && air_enemy.is_some_and(|ae| {
                                        h.positions.get(&ae).is_some_and(|aep| {
                                            marker.is_some_and(|(_, _, mp)| {
                                                let d_ae = (aep[0] - mp[0]).hypot(aep[2] - mp[2]);
                                                let d_e = h
                                                    .positions
                                                    .get(&e)
                                                    .map_or(f32::INFINITY, |ep| {
                                                        (ep[0] - mp[0]).hypot(ep[2] - mp[2])
                                                    });
                                                d_ae <= 150.0 && d_ae < d_e
                                            })
                                        })
                                    }))
                                || h.positions.get(&e).is_none_or(|ep| {
                                    (ep[0] - pos[0]).hypot(ep[2] - pos[2]) > 55.0
                                        && marker.is_none_or(|(_, _, mp)| {
                                            (ep[0] - mp[0]).hypot(ep[2] - mp[2]) > 85.0
                                        })
                                }))
                    }) =>
                {
                    let e = marker_enemy
                        .filter(|me| h.world.air_shapes.contains_key(me))
                        .or(air_enemy)
                        .unwrap();
                    let p = h.positions[&e];
                    if defending_cp
                        && !h.world.non_ground_units.contains(&player)
                        && let Some((mid, _, mp)) = marker
                        && (enemy.is_some()
                            || ((p[0] - mp[0]) as f64).hypot((p[2] - mp[2]) as f64) <= 45.0)
                    {
                        let dx = (p[0] - mp[0]) as f64;
                        let dz = (p[2] - mp[2]) as f64;
                        let d_mp = dx.hypot(dz);
                        if d_mp > 25.0 {
                            (
                                Goal::Marker(mid),
                                [
                                    mp[0] as f64 + dx / d_mp * 20.0,
                                    mp[2] as f64 + dz / d_mp * 20.0,
                                ],
                                8.0,
                                Some(e),
                            )
                        } else {
                            (
                                Goal::Marker(mid),
                                [mp[0] as f64, mp[2] as f64],
                                12.0,
                                Some(e),
                            )
                        }
                    } else {
                        let r = if is_anti_air {
                            95.0
                        } else if player_is_air {
                            65.0
                        } else {
                            35.0
                        };
                        (Goal::Enemy(e), [p[0] as f64, p[2] as f64], r, Some(e))
                    }
                }
                (Some(e), _, _)
                    if !defending_cp
                        && h.world.bodies.get(&player).is_some_and(|b| b.radius <= 0.8)
                        && marker_capture.is_some_and(|cp_id| {
                            h.positions.get(&cp_id).is_some_and(|mp| {
                                (mp[0] - pos[0]).hypot(mp[2] - pos[2]) <= 85.0
                                    && h.positions.get(&e).is_some_and(|ep| {
                                        (ep[0] - mp[0]).hypot(ep[2] - mp[2]) <= 50.0
                                    })
                            })
                        }) =>
                {
                    let cp_id = marker_capture.unwrap();
                    let mp = h.positions[&cp_id];
                    (
                        Goal::Marker(marker.map_or(cp_id, |(mid, _, _)| mid)),
                        [mp[0] as f64, mp[2] as f64],
                        8.0,
                        Some(e),
                    )
                }
                (_, Some(t), Some((mid, target, p)))
                    if enemy.is_none_or(|e| {
                        let td = h
                            .positions
                            .get(&t)
                            .map_or(f32::INFINITY, |tp| (tp[0] - pos[0]).hypot(tp[2] - pos[2]));
                        let max_td = if h.world.water_units.contains(&player) || player_is_bomber {
                            260.0
                        } else if player_is_air && h.world.air_shapes.contains_key(&t) {
                            600.0
                        } else {
                            75.0
                        };
                        let any_aa_threat = h
                            .world
                            .units
                            .keys()
                            .chain(h.world.buildings.iter())
                            .any(|&uid| {
                                Self::hostile(s, player, uid)
                                    && (s.combat.is_armed_building(h, uid)
                                        || h.world.bodies.get(&uid).is_some_and(|b| b.radius > 1.2)
                                        || (h.world.unit_gtype_flags(uid)
                                            & (0x0040_0000 | 0x0000_7400))
                                            != 0)
                                    && h.positions.get(&uid).is_some_and(|ep| {
                                        (ep[0] - pos[0]).hypot(ep[2] - pos[2]) <= 50.0
                                    })
                            });
                        let direct_bldg_obj = t == target
                            && h.world.buildings.contains(&t)
                            && !s.combat.is_armed_building(h, t)
                            && h.world
                                .props
                                .get(&t)
                                .is_some_and(|p| p.max_health <= 4000.0)
                            && td <= 65.0
                            && !any_aa_threat;
                        let clear_radius = if h.world.buildings.contains(&t)
                            && !s.combat.is_armed_building(h, t)
                            && h.world.props.get(&t).is_some_and(|p| p.max_health > 4000.0)
                        {
                            55.0
                        } else {
                            25.0
                        };
                        td <= max_td
                            && (h.world.water_units.contains(&player)
                                || player_is_bomber
                                || (player_is_air && h.world.air_shapes.contains_key(&t))
                                || direct_bldg_obj
                                || (gate_winch.is_none()
                                    && h.positions.get(&e).is_none_or(|ep| {
                                        (ep[0] - pos[0]).hypot(ep[2] - pos[2]) > clear_radius
                                    })))
                    }) =>
                {
                    let from_y = crate::mission_combat::Combat::entity_aim_point(h, player)
                        .map_or(pos[1], |fp| fp[1]);
                    let gp = crate::mission_combat::Combat::entity_aim_point(h, t)
                        .or_else(|| h.positions.get(&t).copied())
                        .unwrap_or(p);
                    let dy = (gp[1] - from_y) as f64;
                    let r_3d = if h.world.water_units.contains(&player) {
                        if h.world.water_units.contains(&t) {
                            85.0
                        } else {
                            95.0
                        }
                    } else if h.world.flight_profiles.contains_key(&player) {
                        if h.world.air_shapes.contains_key(&t) {
                            (base_fire_range - 10.0).clamp(40.0, 65.0)
                        } else if player_is_bomber {
                            (base_fire_range - 15.0).clamp(35.0, 50.0)
                        } else {
                            (base_fire_range - 10.0).clamp(40.0, 75.0)
                        }
                    } else if h.world.air_shapes.contains_key(&t) {
                        if is_anti_air { 95.0 } else { 35.0 }
                    } else if player_is_vehicle && (player_gtype & 0x200) != 0 {
                        60.0
                    } else if player_is_vehicle && (player_gtype & 0x0000_0400) != 0 {
                        (base_fire_range - 25.0).clamp(45.0, 165.0)
                    } else if player_is_vehicle && !is_recon && s.combat.is_armed_building(h, t) {
                        (base_fire_range - 2.5).max(55.0)
                    } else if player_is_vehicle
                        && !is_recon
                        && (h.world.buildings.contains(&t)
                            || h.world.bodies.get(&t).is_some_and(|b| b.radius > 1.2))
                    {
                        45.0
                    } else if h.world.buildings.contains(&t)
                        || h.world.bodies.get(&t).is_some_and(|b| b.radius > 1.2)
                    {
                        26.0
                    } else {
                        14.0
                    };
                    let r = (r_3d * r_3d - dy * dy).max(14.0 * 14.0).sqrt().min(r_3d);
                    (Goal::Marker(mid), [gp[0] as f64, gp[2] as f64], r, Some(t))
                }
                (Some(e), _, _) => {
                    let from_y = crate::mission_combat::Combat::entity_aim_point(h, player)
                        .map_or(pos[1], |fp| fp[1]);
                    let p = crate::mission_combat::Combat::entity_aim_point(h, e)
                        .or_else(|| h.positions.get(&e).copied())
                        .unwrap_or(pos);
                    let dy = (p[1] - from_y) as f64;
                    let cp_leash = if marker_near_friendly_building && player_is_air {
                        245.0
                    } else {
                        85.0
                    };
                    if defending_cp
                        && (!h.world.non_ground_units.contains(&player)
                            || (player_gtype & 0x0000_0008) != 0)
                        && (h.world.unit_gtype_flags(e) & 0x200) == 0
                        && let Some((mid, target, mp)) = marker
                    {
                        let near_bldg = marker_near_friendly_building
                            || (h.world.buildings.contains(&target)
                                && !h.world.capture_points.contains(&target));
                        let near_other_marker = s.markers.iter().any(|m| {
                            m.id != mid
                                && m.position(h)
                                    .ok()
                                    .flatten()
                                    .or_else(|| h.positions.get(&m.effective_target(h)).copied())
                                    .is_some_and(|mp2| (p[0] - mp2[0]).hypot(p[2] - mp2[2]) <= 65.0)
                        });
                        let dx = (p[0] - mp[0]) as f64;
                        let dz = (p[2] - mp[2]) as f64;
                        let d_mp = dx.hypot(dz);
                        if d_mp > cp_leash {
                            if near_other_marker {
                                (Goal::Enemy(e), [p[0] as f64, p[2] as f64], 25.0, Some(e))
                            } else {
                                let r = if near_bldg { 25.0 } else { 6.0 };
                                (Goal::Marker(mid), [mp[0] as f64, mp[2] as f64], r, Some(e))
                            }
                        } else if !player_is_air && d_mp > if near_bldg { 28.0 } else { 18.0 } {
                            let step = if near_bldg {
                                24.0
                            } else if player_is_vehicle || player_is_air {
                                15.0
                            } else {
                                10.0
                            };
                            let r = if near_bldg { 8.0 } else { 4.0 };
                            (
                                Goal::Enemy(e),
                                [
                                    mp[0] as f64 + dx / d_mp * step,
                                    mp[2] as f64 + dz / d_mp * step,
                                ],
                                r,
                                Some(e),
                            )
                        } else {
                            let r = if player_is_air {
                                75.0
                            } else if h.world.buildings.contains(&e)
                                || h.world.bodies.get(&e).is_some_and(|b| b.radius > 1.2)
                            {
                                26.0
                            } else {
                                12.0
                            };
                            (Goal::Enemy(e), [p[0] as f64, p[2] as f64], r, Some(e))
                        }
                    } else {
                        let eg = h.world.unit_gtype_flags(e);
                        let r_3d = if h.world.water_units.contains(&player) {
                            if h.world.water_units.contains(&e) {
                                85.0
                            } else {
                                95.0
                            }
                        } else if h.world.flight_profiles.contains_key(&player) {
                            if h.world.air_shapes.contains_key(&e) {
                                (base_fire_range - 10.0).clamp(40.0, 65.0)
                            } else if player_is_bomber {
                                (base_fire_range - 15.0).clamp(35.0, 50.0)
                            } else {
                                (base_fire_range - 10.0).clamp(40.0, 75.0)
                            }
                        } else if player_is_vehicle && (player_gtype & 0x200) != 0 {
                            60.0
                        } else if player_is_vehicle && (player_gtype & 0x0000_0400) != 0 {
                            (base_fire_range - 25.0).clamp(45.0, 165.0)
                        } else if player_is_vehicle && !is_recon && s.combat.is_armed_building(h, e)
                        {
                            (base_fire_range - 2.5).max(55.0)
                        } else if player_is_vehicle
                            && !is_recon
                            && (eg & 0x0040_0000) != 0
                            && h.world.bodies.get(&e).is_some_and(|b| b.radius <= 1.2)
                        {
                            (base_fire_range - 15.0).clamp(45.0, 64.0)
                        } else if player_is_vehicle
                            && !is_recon
                            && (h.world.buildings.contains(&e)
                                || h.world.bodies.get(&e).is_some_and(|b| b.radius > 1.2)
                                || (eg & 0x0000_7400) != 0)
                        {
                            45.0
                        } else if player_is_vehicle && !is_recon {
                            30.0
                        } else if h.world.buildings.contains(&e)
                            || h.world.bodies.get(&e).is_some_and(|b| b.radius > 1.2)
                        {
                            26.0
                        } else {
                            12.0
                        };
                        let r = (r_3d * r_3d - dy * dy).max(12.0 * 12.0).sqrt().min(r_3d);
                        (Goal::Enemy(e), [p[0] as f64, p[2] as f64], r, Some(e))
                    }
                }
                (None, None, Some((mid, target, mp))) if defending_cp && air_enemy.is_some() => {
                    let e = air_enemy.unwrap();
                    let p = h.positions[&e];
                    if !is_anti_air
                        && !h.world.non_ground_units.contains(&player)
                        && (p[0] - mp[0]).hypot(p[2] - mp[2]) > 80.0
                    {
                        let r = if marker_near_friendly_building
                            || (h.world.buildings.contains(&target)
                                && !h.world.capture_points.contains(&target))
                        {
                            25.0
                        } else {
                            18.0
                        };
                        (Goal::Marker(mid), [mp[0] as f64, mp[2] as f64], r, Some(e))
                    } else {
                        let r = if is_anti_air {
                            95.0
                        } else if player_is_air {
                            65.0
                        } else {
                            35.0
                        };
                        (Goal::Enemy(e), [p[0] as f64, p[2] as f64], r, Some(e))
                    }
                }
                (None, None, _) if let Some(cp_id) = marker_capture => {
                    let cp_p = h.positions[&cp_id];
                    (
                        Goal::Marker(marker.map_or(cp_id, |(mid, _, _)| mid)),
                        [cp_p[0] as f64, cp_p[2] as f64],
                        8.0,
                        air_enemy,
                    )
                }
                (None, None, Some((mid, target, p))) => {
                    let r = if marker_friendly_unit.is_some() {
                        8.0
                    } else if (h.world.buildings.contains(&target)
                        && !h.world.capture_points.contains(&target))
                        || marker_near_friendly_building
                    {
                        if defending_cp { 25.0 } else { 15.0 }
                    } else if defending_cp {
                        6.0
                    } else {
                        0.02
                    };
                    (Goal::Marker(mid), [p[0] as f64, p[2] as f64], r, air_enemy)
                }
                (None, None, None) if let Some(e) = air_enemy => {
                    let p = h.positions[&e];
                    let r = if is_anti_air {
                        95.0
                    } else if player_is_air {
                        65.0
                    } else {
                        35.0
                    };
                    (Goal::Enemy(e), [p[0] as f64, p[2] as f64], r, Some(e))
                }
                _ => return (input, Goal::Idle),
            };
        let mut los_blocked = false;
        let player_is_flight_combat =
            h.world.flight_profiles.contains_key(&player) && !player_is_bomber;
        if let Some(t) = attack
            && (!h.world.non_ground_units.contains(&player) || player_is_flight_combat)
            && !h.world.air_shapes.contains_key(&t)
        {
            let goal_is_attack_target = goal_kind == Goal::Enemy(t)
                || (marker_enemy == Some(t)
                    && self.active_pickup.is_none()
                    && marker_capture.is_none());
            if goal_is_attack_target
                && !defending_cp
                && (player_gtype & 0x200) == 0
                && self
                    .attack_progress
                    .is_some_and(|(at, _, n)| at == t && n >= 210)
            {
                radius = radius.min(if player_is_flight_combat { 18.0 } else { 8.0 });
                if self
                    .attack_progress
                    .is_some_and(|(at, _, n)| at == t && n >= 240)
                {
                    los_blocked = true;
                    radius = radius.min(if player_is_flight_combat { 14.0 } else { 4.0 });
                }
            }
            if let Some(scene) = h.world.collision.as_ref()
                && let Some(tp) = crate::mission_combat::Combat::entity_aim_point(h, t)
            {
                let from =
                    crate::mission_combat::Combat::entity_aim_point(h, player).unwrap_or(pos);
                let ray_d = ((tp[0] - from[0]).powi(2)
                    + (tp[1] - from[1]).powi(2)
                    + (tp[2] - from[2]).powi(2))
                .sqrt() as f64;
                let is_ballistic = (player_gtype & (0x200 | 0x0000_0400)) != 0;
                let prop_t = h.world.props.contains_key(&t);
                if !is_ballistic
                    && goal_is_attack_target
                    && scene
                        .first_hit_filtered(
                            from,
                            tp,
                            true,
                            0.0,
                            if prop_t { 6.0 } else { 0.0 },
                            |fraction, owner| {
                                owner == Some(t)
                                    || owner == Some(player)
                                    || owner.is_some_and(|o| h.world.capture_points.contains(&o))
                                    || owner.is_some_and(|o| {
                                        h.world.props.get(&o).is_some_and(|u| {
                                            u.active != Some(true) || u.health <= 0.0
                                        })
                                    })
                                    || owner.is_some_and(|o| {
                                        h.world.props.get(&o).is_some_and(|u| {
                                            !u.army.is_empty()
                                                && u.army != "eNeutral"
                                                && u.army != "eNoArmy"
                                                && h.world.entity(player).is_ok_and(|me| {
                                                    h.world.same_allegiance(&u.army, &me.army)
                                                })
                                        })
                                    })
                                    || (!prop_t
                                        && (fraction * ray_d <= 4.0
                                            || (1.0 - fraction) * ray_d <= 4.0)
                                        && owner.is_some_and(|o| {
                                            h.world.props.contains_key(&o)
                                                && !h.world.buildings.contains(&o)
                                        }))
                                    || (prop_t
                                        && (1.0 - fraction) * ray_d <= 18.0
                                        && owner.is_some_and(|o| {
                                            h.world.props.contains_key(&o)
                                                && !h.world.buildings.contains(&o)
                                        }))
                            },
                        )
                        .ok()
                        .flatten()
                        .is_some()
                {
                    los_blocked = true;
                    if defending_cp
                        && !prop_t
                        && !player_is_flight_combat
                        && (h.world.unit_gtype_flags(t) & 0x200) == 0
                        && let Some((mid, target, mp)) = marker
                    {
                        let near_other_marker = s.markers.iter().any(|m| {
                            m.id != mid
                                && m.position(h)
                                    .ok()
                                    .flatten()
                                    .or_else(|| h.positions.get(&m.effective_target(h)).copied())
                                    .is_some_and(|mp2| {
                                        (tp[0] - mp2[0]).hypot(tp[2] - mp2[2]) <= 65.0
                                    })
                        });
                        let near_bldg = marker_near_friendly_building
                            || (h.world.buildings.contains(&target)
                                && !h.world.capture_points.contains(&target));
                        let dx = (tp[0] - mp[0]) as f64;
                        let dz = (tp[2] - mp[2]) as f64;
                        let d_mp = dx.hypot(dz);
                        if near_other_marker {
                            goal = [tp[0] as f64, tp[2] as f64];
                            radius = radius.min(18.0);
                        } else if !player_is_vehicle && d_mp > 38.0 {
                            goal = [mp[0] as f64, mp[2] as f64];
                            radius = if near_bldg { 25.0 } else { 12.0 };
                        } else if d_mp > if near_bldg { 28.0 } else { 16.0 } {
                            let step = if near_bldg {
                                24.0
                            } else if player_is_vehicle {
                                15.0
                            } else {
                                6.0
                            };
                            goal = [
                                mp[0] as f64 + dx / d_mp * step,
                                mp[2] as f64 + dz / d_mp * step,
                            ];
                            radius = if near_bldg { 8.0 } else { 4.0 };
                        } else {
                            goal = [tp[0] as f64, tp[2] as f64];
                            radius = radius.min(16.0);
                        }
                    } else {
                        goal = [tp[0] as f64, tp[2] as f64];
                        let clear_r = if player_is_flight_combat
                            || (defending_cp && !prop_t)
                            || h.world.buildings.contains(&t)
                        {
                            18.0
                        } else {
                            0.5
                        };
                        radius = radius.min(clear_r);
                    }
                }
            }
        }
        let squad_capturing = h.world.roster.iter().any(|id| {
            h.world.entering_vehicle.contains_key(id)
                || h.world
                    .carrier_of
                    .get(id)
                    .is_some_and(|c| h.world.capture_points.contains(c))
        });
        let squad_busy = h.world.squad_target.is_some_and(|st| {
            h.world
                .entity(st)
                .is_ok_and(|u| u.active == Some(true) && u.health > 0.0 && !u.invulnerable)
        }) || squad_capturing;
        let has_unrecruited = h.world.units.iter().any(|(id, u)| {
            *id != player
                && !h.world.roster.contains(id)
                && !h.world.enemy_targetable_only.contains(id)
                && (h.world.bodies.contains_key(id)
                    || h.world.flight_profiles.contains_key(id)
                    || h.world.water_units.contains(id))
                && h.world.water_units.contains(id) == h.world.water_units.contains(&player)
                && h.world.entity(player).is_ok_and(|me| u.army == me.army)
                && u.active == Some(true)
                && u.health > 0.0
                && h.world
                    .activity
                    .get(id)
                    .is_none_or(|a| a.enabled != Some(false))
                && h.positions.get(id).is_some_and(|q| {
                    let range = if h.world.water_units.contains(id) {
                        150.0
                    } else if h.world.non_ground_units.contains(id) {
                        60.0
                    } else {
                        65.0
                    };
                    (pos[0] - q[0]).hypot(pos[2] - q[2]) <= range
                })
        });
        let far_attached_marker = marker.is_some_and(|(mid, target, mp)| {
            mid != target && (mp[0] - pos[0]).hypot(mp[2] - pos[2]) > 85.0
        });
        let needs_recall = enemy.is_none()
            && (marker_enemy.is_none() || far_attached_marker)
            && !squad_busy
            && h.world.roster.iter().any(|id| {
                *id != player
                    && !h.world.squad_follow.contains(id)
                    && h.world.can_squad_follow(*id, player)
                    && h.world
                        .units
                        .get(id)
                        .is_some_and(|u| u.active == Some(true) && u.health > 0.0)
                    && h.world
                        .activity
                        .get(id)
                        .is_none_or(|a| a.enabled != Some(false))
            });
        if !squad_busy
            && ((has_unrecruited && self.frame.is_multiple_of(30))
                || (needs_recall
                    && self
                        .frame
                        .is_multiple_of(if far_attached_marker { 60 } else { 240 })))
        {
            input.recruit = true;
        }
        let player_is_building = h.world.buildings.contains(&player);
        let dist = (goal[0] - pos[0] as f64).hypot(goal[1] - pos[2] as f64);
        if self.frame.is_multiple_of(180) {
            let moved = self
                .progress_anchor
                .map_or(f32::INFINITY, |a| (a[0] - pos[0]).hypot(a[2] - pos[2]));
            if !player_is_building && moved < 2.0 && dist > radius {
                self.blocked_windows += 1;
            } else {
                self.blocked_windows = 0;
            }
            self.progress_anchor = Some(pos);
        }
        // Stuck detection: perturb with strafing/backing up and force a replan.
        if let Some(lp) = self.last_pos {
            if !player_is_building && (lp[0] - pos[0]).hypot(lp[2] - pos[2]) < 0.02 && dist > radius
            {
                self.stuck_frames += 1;
            } else {
                self.stuck_frames = 0;
            }
        }
        self.last_pos = Some(pos);
        if self.stuck_frames > 45 {
            self.strafe = if self.strafe > 0.0 { -1.0 } else { 1.0 };
            self.unstick_frames = 25;
            let stuck_pop = if h.world.water_units.contains(&player) {
                20.0
            } else {
                3.0
            };
            if self
                .route
                .front()
                .is_some_and(|pt| (pt[0] - pos[0] as f64).hypot(pt[1] - pos[2] as f64) <= stuck_pop)
            {
                self.route.pop_front();
            } else if self.frame - self.last_plan >= 120 {
                self.planned_goal = None;
            }
            self.stuck_frames = 0;
        }
        let player_body = h
            .world
            .bodies
            .get(&player)
            .copied()
            .unwrap_or(unit_collision::Body::INFANTRY);
        let player_is_water = h.world.water_units.contains(&player);
        let replan = !player_is_building
            && dist > radius
            && (!h.world.non_ground_units.contains(&player) || player_is_water)
            && (self
                .planned_goal
                .is_none_or(|g| (g[0] - goal[0]).hypot(g[1] - goal[1]) > 10.0)
                || (self.route.is_empty() && self.frame - self.last_plan >= 120)
                || (self.blocked_windows > 0 && self.frame - self.last_plan >= 600));
        if replan && let Some(scene) = h.world.collision.as_ref() {
            if player_is_water {
                self.route =
                    Self::plan_water_towards(scene, h.world.water_height, pos, goal, radius);
                self.planned_goal = Some(goal);
                self.last_plan = self.frame;
            } else {
                let others: Vec<_> = h
                    .world
                    .units
                    .iter()
                    .filter(|(id, u)| {
                        **id != player
                            && !h.world.roster.contains(id)
                            && u.active == Some(true)
                            && u.health > 0.0
                            && !h.world.non_ground_units.contains(id)
                            && h.world.bodies.contains_key(id)
                            && h.positions.contains_key(id)
                    })
                    .map(|(id, _)| unit_collision::Other {
                        position: h.positions[id],
                        body: h.world.bodies[id],
                    })
                    .collect();
                let avoid_plan = (is_recon
                    && dist > 250.0
                    && marker_enemy.is_none()
                    && !defending_cp)
                    .then(|| {
                        let my_army = h
                            .world
                            .entity(player)
                            .map(|u| u.army.clone())
                            .unwrap_or_default();
                        let threat_others: Vec<_> = h
                            .world
                            .units
                            .iter()
                            .filter(|(id, u)| {
                                **id != player
                                    && !h.world.roster.contains(id)
                                    && (u.active == Some(true)
                                        || h.world.despawn_retention.get(id) == Some(&true))
                                    && u.health > 0.0
                                    && !h.world.non_ground_units.contains(id)
                                    && h.world.bodies.contains_key(id)
                                    && h.positions.contains_key(id)
                            })
                            .map(|(id, u)| {
                                let p = h.positions[id];
                                let mut body = h.world.bodies[id];
                                let is_hostile = !u.invulnerable
                                    && !u.army.is_empty()
                                    && u.army != "eNoArmy"
                                    && u.army != "eNeutral"
                                    && !h.world.same_allegiance(&u.army, &my_army);
                                if is_hostile {
                                    let dp = (p[0] - pos[0]).hypot(p[2] - pos[2]);
                                    let dg = (p[0] as f64 - goal[0]).hypot(p[2] as f64 - goal[1]);
                                    if dp > 45.0 && dg > 45.0 {
                                        body.radius = body.radius.max(42.0);
                                    }
                                }
                                unit_collision::Other { position: p, body }
                            })
                            .collect();
                        let start = scene
                            .find_free_near(
                                pos,
                                player_body,
                                &others,
                                (player_body.radius * 2.0 + 1.0).min(8.0),
                            )
                            .unwrap_or(pos);
                        navigation::plan_towards(
                            scene,
                            start,
                            goal,
                            radius,
                            player_body,
                            &threat_others,
                            navigation::Limits {
                                expansions: 18000,
                                probes: 500000,
                                extent: 384,
                            },
                        )
                        .ok()
                        .filter(|p| p.status == navigation::Status::Found && !p.points.is_empty())
                    })
                    .flatten();
                let plan_start = scene
                    .find_free_near(
                        pos,
                        player_body,
                        &others,
                        (player_body.radius * 2.0 + 1.0).min(8.0),
                    )
                    .unwrap_or(pos);
                if let Some(plan) = avoid_plan.or_else(|| {
                    navigation::plan_towards(
                        scene,
                        plan_start,
                        goal,
                        radius,
                        player_body,
                        &others,
                        navigation::Limits {
                            expansions: 18000,
                            probes: 500000,
                            extent: 384,
                        },
                    )
                    .ok()
                    .filter(|p| {
                        p.status == navigation::Status::Found
                            || p.points.last().is_some_and(|last| {
                                (last[0] - pos[0] as f64).hypot(last[1] - pos[2] as f64) >= 4.0
                            })
                    })
                }) {
                    self.route = plan.points.into();
                } else {
                    self.route.clear();
                }
                self.planned_goal = Some(goal);
                self.last_plan = self.frame;
            }
        } else if (h.world.non_ground_units.contains(&player) && !player_is_water)
            || player_is_building
        {
            self.route.clear();
        }
        let pop_radius = if player_is_water { 14.0 } else { 1.5 };
        while let Some(&pt) = self.route.front() {
            if (pt[0] - pos[0] as f64).hypot(pt[1] - pos[2] as f64) < pop_radius {
                self.route.pop_front();
            } else {
                break;
            }
        }
        let current_yaw = aim[0].atan2(aim[2]);
        let current_pitch = aim[1].clamp(-1.0, 1.0).asin();
        let attack_point =
            attack.and_then(|t| crate::mission_combat::Combat::entity_aim_point(h, t));
        let fire_range = if !h.world.non_ground_units.contains(&player)
            && attack.is_some_and(|t| h.world.air_shapes.contains_key(&t))
        {
            195.0
        } else {
            base_fire_range
        };
        let from_aim = crate::mission_combat::Combat::entity_aim_point(h, player).unwrap_or(pos);
        let firing = attack_point.is_some_and(|p| {
            ((p[0] - from_aim[0]) as f64)
                .hypot((p[1] - from_aim[1]) as f64)
                .hypot((p[2] - from_aim[2]) as f64)
                <= fire_range
        });
        let battlestation_fighter_transfer = (player_gtype & 0x0000_0400) != 0
            && (h.world.attack_orders.get(&player).is_some_and(|t| {
                h.world.props.get(t).is_some_and(|p| {
                    p.active == Some(true) && p.health > 0.0 && p.max_health >= 20000.0
                })
            }) || marker_enemy.is_some_and(|t| {
                h.world.props.get(&t).is_some_and(|p| {
                    p.active == Some(true) && p.health > 0.0 && p.max_health >= 20000.0
                })
            }) || s.markers.iter().any(|m| {
                let t = m.effective_target(h);
                h.world.props.get(&t).is_some_and(|p| {
                    p.active == Some(true) && p.health > 0.0 && p.max_health >= 20000.0
                })
            }))
            && h.world.roster.iter().any(|&id| {
                id != player
                    && (h.world.unit_gtype_flags(id) & 0x0000_0010) != 0
                    && h.world
                        .units
                        .get(&id)
                        .is_some_and(|u| u.active == Some(true) && u.health > 0.0)
                    && h.world
                        .activity
                        .get(&id)
                        .is_none_or(|a| a.enabled != Some(false))
            });
        let gunship_in_roster = !player_is_vehicle
            && !h.world.non_ground_units.contains(&player)
            && marker_friendly_unit.is_none()
            && marker_capture.is_none()
            && h.world.roster.iter().any(|&id| {
                id != player
                    && (h.world.unit_gtype_flags(id) & 0x0000_0008) != 0
                    && h.world
                        .units
                        .get(&id)
                        .is_some_and(|u| u.active == Some(true) && u.health > 0.0)
                    && h.world
                        .activity
                        .get(&id)
                        .is_none_or(|a| a.enabled != Some(false))
            });
        let blocked_air_transfer = (!h.world.flight_profiles.contains_key(&player)
            && ((self.blocked_windows >= 2 && !defending_cp)
                || gunship_in_roster
                || battlestation_fighter_transfer)
            && (blocker.is_none() || battlestation_fighter_transfer || gunship_in_roster)
            && marker_capture.is_none()
            && h.world
                .controller
                .as_ref()
                .is_some_and(|c| c.accepts("CONTROL_TRANSFER").unwrap_or(false)))
        .then(|| {
            let max_transfer_d = if battlestation_fighter_transfer || gunship_in_roster {
                800.0
            } else {
                45.0
            };
            h.world
                .roster
                .iter()
                .copied()
                .filter(|&id| {
                    id != player
                        && h.world.flight_profiles.contains_key(&id)
                        && (!battlestation_fighter_transfer
                            || (h.world.unit_gtype_flags(id) & 0x0000_0010) != 0)
                        && (!(defending_cp || gunship_in_roster)
                            || (h.world.unit_gtype_flags(id) & 0x0000_0008) != 0)
                        && h.world
                            .units
                            .get(&id)
                            .is_some_and(|u| u.active == Some(true) && u.health > 0.0)
                        && h.world
                            .activity
                            .get(&id)
                            .is_none_or(|a| a.enabled != Some(false))
                })
                .filter_map(|id| {
                    let tp = *h.positions.get(&id)?;
                    let d = (tp[0] - pos[0]).hypot(tp[2] - pos[2]);
                    (d <= max_transfer_d).then_some((id, d))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(id, _)| id)
        })
        .flatten();
        let transfer_target = marker_friendly_unit
            .filter(|t| {
                attack.is_none()
                    && h.world
                        .controller
                        .as_ref()
                        .is_some_and(|c| c.accepts("CONTROL_TRANSFER").unwrap_or(false))
                    && h.positions
                        .get(t)
                        .is_some_and(|tp| (tp[0] - pos[0]).hypot(tp[2] - pos[2]) <= 35.0)
            })
            .or(blocked_air_transfer);
        let capture_target = marker_capture.filter(|cp_id| {
            attack.is_none()
                && transfer_target.is_none()
                && h.positions
                    .get(cp_id)
                    .is_some_and(|cp| (cp[0] - pos[0]).hypot(cp[2] - pos[2]) <= 30.0)
        });
        let (desired_yaw, desired_pitch) = if let (true, Some(tp)) =
            (firing && blocked_air_transfer.is_none(), attack_point)
        {
            let from = crate::mission_combat::Combat::entity_aim_point(h, player).unwrap_or(pos);
            let dx = (tp[0] - from[0]) as f64;
            let dz = (tp[2] - from[2]) as f64;
            let dy = (tp[1] - from[1]) as f64;
            (dx.atan2(dz), dy.atan2(dx.hypot(dz)))
        } else if let Some(tid) = transfer_target.or(capture_target)
            && let Some(&tp) = h.positions.get(&tid)
        {
            let dx = (tp[0] - pos[0]) as f64;
            let dz = (tp[2] - pos[2]) as f64;
            (dx.atan2(dz), 0.0)
        } else {
            let next = self.route.front().copied().unwrap_or(goal);
            (
                (next[0] - pos[0] as f64).atan2(next[1] - pos[2] as f64),
                0.0,
            )
        };
        let raw_dyaw = yaw_delta(desired_yaw, current_yaw);
        let dyaw = raw_dyaw.clamp(-0.08, 0.08);
        input.look = [dyaw, (desired_pitch - current_pitch).clamp(-0.08, 0.08)];
        let has_friendly_battlestation = h.world.roster.iter().any(|&id| {
            id != player
                && (h.world.unit_gtype_flags(id) & 0x0000_0400) != 0
                && h.world
                    .units
                    .get(&id)
                    .is_some_and(|u| u.active == Some(true) && u.health > 0.0)
        });
        let non_ground_to_ground_transfer = h.world.non_ground_units.contains(&player)
            && h.world
                .controller
                .as_ref()
                .is_some_and(|c| c.accepts("CONTROL_TRANSFER").unwrap_or(false))
            && (marker_capture.is_some()
                || ((player_gtype & 0x0000_0010) != 0
                    && (!has_friendly_battlestation
                        || (enemy.is_none() && marker_enemy.is_none()))
                    && air_enemy.is_none()
                    && marker.is_some_and(|(_, t, _)| !h.world.air_shapes.contains_key(&t))))
            && h.world.roster.iter().any(|&id| {
                id != player
                    && !h.world.non_ground_units.contains(&id)
                    && h.world
                        .units
                        .get(&id)
                        .is_some_and(|u| u.active == Some(true) && u.health > 0.0)
            });
        if transfer_target.is_some() {
            input.lock = true;
            if raw_dyaw.abs() < 0.06
                && (battlestation_fighter_transfer || self.frame.is_multiple_of(15))
            {
                input.transfer = true;
                self.blocked_windows = 0;
                self.route.clear();
                self.planned_goal = None;
            }
        } else if non_ground_to_ground_transfer && self.frame.is_multiple_of(15) {
            input.transfer = true;
            self.blocked_windows = 0;
            self.route.clear();
            self.planned_goal = None;
        }
        let has_empty_garrison = !squad_capturing
            && h.world.garrisonable_buildings.iter().any(|b_id| {
                !h.world.capture_points.contains(b_id)
                    && h.world
                        .passengers
                        .get(b_id)
                        .is_none_or(|p| p.iter().all(|id| id.0 == 0))
                    && h.world.entering_vehicle.values().all(|&c| c != *b_id)
                    && h.positions.get(b_id).is_some_and(|bp| {
                        (bp[0] - pos[0]).hypot(bp[2] - pos[2]) <= 60.0
                            && s.markers.iter().any(|m| {
                                m.effective_target(h) == *b_id
                                    || (defending_cp
                                        && m.position(h).ok().flatten().is_some_and(|mp| {
                                            (bp[0] - mp[0]).hypot(bp[2] - mp[2]) <= 35.0
                                        }))
                            })
                    })
                    && h.world.props.get(b_id).is_some_and(|b| {
                        b.active == Some(true)
                            && b.health > 0.0
                            && !b.invulnerable
                            && (b.army == "eNeutral"
                                || h.world
                                    .entity(player)
                                    .is_ok_and(|me| h.world.same_allegiance(&b.army, &me.army)))
                    })
            })
            && h.world.roster.iter().any(|&id| {
                id != player
                    && !h.world.carrier_of.contains_key(&id)
                    && !h.world.entering_vehicle.contains_key(&id)
                    && h.world
                        .bodies
                        .get(&id)
                        .is_some_and(|b| b.radius == unit_collision::Body::INFANTRY.radius)
                    && h.world
                        .units
                        .get(&id)
                        .is_some_and(|u| u.active == Some(true) && u.health > 0.0)
            });
        if (capture_target.is_some() && raw_dyaw.abs() < 0.2 && self.frame.is_multiple_of(60))
            || (has_empty_garrison && self.frame.is_multiple_of(30))
        {
            input.send = true;
        }
        let is_flight = h.world.flight_profiles.contains_key(&player);
        if !player_is_building && dist > radius && (!is_flight || dyaw.abs() < 0.079) {
            let mut throttle = if radius < 0.1 && dist < radius + 0.5 {
                ((dist - radius) / 0.5).clamp(0.05, 1.0)
            } else {
                1.0
            };
            if player_body.radius <= 0.8 && !h.world.non_ground_units.contains(&player) {
                let player_speed = h
                    .world
                    .units
                    .get(&player)
                    .and_then(|u| u.forward_speed)
                    .unwrap_or(20.0);
                let mut lagging_vehicle = false;
                let mut min_veh_speed = player_speed;
                for &fid in &h.world.squad_follow {
                    if fid != player
                        && h.world
                            .units
                            .get(&fid)
                            .is_some_and(|u| u.active == Some(true) && u.health > 0.0)
                        && h.world.bodies.get(&fid).is_some_and(|b| b.radius > 0.8)
                        && let Some(&fp) = h.positions.get(&fid)
                    {
                        let d_veh = (fp[0] - pos[0]).hypot(fp[2] - pos[2]);
                        if d_veh <= 65.0 {
                            if let Some(vs) = h.world.units[&fid].forward_speed {
                                min_veh_speed = min_veh_speed.min(vs);
                            }
                            if d_veh > 28.0 && enemy.is_none() {
                                lagging_vehicle = true;
                            }
                        }
                    }
                }
                if lagging_vehicle {
                    throttle = throttle.min(0.5);
                } else if min_veh_speed < player_speed {
                    throttle = throttle.min((min_veh_speed / player_speed).clamp(0.3, 1.0));
                }
            }
            if is_flight {
                input.movement = [0.0, throttle];
                if los_blocked && dist <= 25.0 {
                    let s = if self.strafe == 0.0 { 1.0 } else { self.strafe };
                    input.movement = [s, 0.4];
                } else if firing {
                    let next = self.route.front().copied().unwrap_or(goal);
                    let move_yaw = (next[0] - pos[0] as f64).atan2(next[1] - pos[2] as f64);
                    let rel = yaw_delta(move_yaw, current_yaw + dyaw);
                    input.movement = [-rel.sin() * throttle, rel.cos() * throttle];
                }
            } else if los_blocked && dist <= 8.0 && !player_is_water {
                let s = if self.strafe == 0.0 { 1.0 } else { self.strafe };
                input.movement = [s, 0.3];
            } else {
                let next = self.route.front().copied().unwrap_or(goal);
                let move_yaw = (next[0] - pos[0] as f64).atan2(next[1] - pos[2] as f64);
                let rel = yaw_delta(move_yaw, current_yaw + dyaw);
                input.movement = [-rel.sin() * throttle, rel.cos() * throttle];
            }
        } else if firing
            && (h.world.water_units.contains(&player) || is_flight)
            && attack_point.is_some_and(|tp| {
                let d_xz = ((tp[0] - pos[0]) as f64).hypot((tp[2] - pos[2]) as f64);
                if is_flight {
                    let from_y = crate::mission_combat::Combat::entity_aim_point(h, player)
                        .map_or(pos[1], |fp| fp[1]);
                    d_xz.hypot((tp[1] - from_y) as f64) < 35.0
                } else {
                    d_xz < 34.0
                }
            })
        {
            input.movement = [0.0, -1.0];
        }
        if self.unstick_frames > 0 && !player_is_building {
            self.unstick_frames -= 1;
            let back = if h.world.water_units.contains(&player) {
                0.4
            } else {
                -0.6
            };
            input.movement = [self.strafe, back];
        }
        // If sustained fire makes little progress (wrong weapon for the target),
        // command the squad to focus fire and then cycle control to the next squad unit.
        if firing
            && let Some(t) = attack
            && !h.world.air_shapes.contains_key(&t)
        {
            let hp = h.world.entity(t).map(|u| u.health).unwrap_or(0.0);
            self.attack_progress = match self.attack_progress {
                Some((id, initial_hp, n)) if id == t && initial_hp - hp < 400.0 => {
                    Some((t, initial_hp, n + 1))
                }
                _ => Some((t, hp, 0)),
            };
            let is_heavy_or_building = h.world.buildings.contains(&t)
                || h.world.bodies.get(&t).is_some_and(|b| b.radius > 0.8);
            let is_ground_combat_veh = player_is_vehicle
                && !is_anti_air
                && !is_recon
                && (player_gtype & 0x0000_0400) == 0
                && !h.world.water_units.contains(&player);
            let cannot_damage = !s.combat.can_damage(h, player, t);
            let can_transfer_to_damaging_ground = cannot_damage
                && h.world.roster.iter().any(|&rid| {
                    rid != player
                        && !h.world.non_ground_units.contains(&rid)
                        && h.world
                            .units
                            .get(&rid)
                            .is_some_and(|u| u.active == Some(true) && u.health > 0.0)
                        && (!defending_cp
                            || h.positions
                                .get(&rid)
                                .is_some_and(|rp| (rp[0] - pos[0]).hypot(rp[2] - pos[2]) <= 65.0))
                        && s.combat.can_damage(h, rid, t)
                });
            if (self
                .attack_progress
                .is_some_and(|(_, _, n)| n > 0 && n % 120 == 0)
                || (cannot_damage && raw_dyaw.abs() < 0.25)
                || (!is_ground_combat_veh
                    && (is_anti_air
                        || is_heavy_or_building
                        || h.world.water_units.contains(&player))
                    && raw_dyaw.abs() < 0.25
                    && self.frame.is_multiple_of(60)
                    && (!h.world.non_ground_units.contains(&t)
                        || h.world.water_units.contains(&t))))
                && !squad_capturing
                && !defending_cp
            {
                // Ask the squad to attack what we are aiming at.
                input.send = true;
            }
            let max_stall = if can_transfer_to_damaging_ground {
                15
            } else if is_anti_air && is_heavy_or_building && air_enemy.is_none() {
                90
            } else {
                360
            };
            if (can_transfer_to_damaging_ground
                || (!defending_cp
                    && !is_recon
                    && !h.world.non_ground_units.contains(&player)
                    && (!player_is_vehicle || is_anti_air)))
                && !player_is_building
                && !h.world.water_units.contains(&player)
                && self.attack_progress.is_some_and(|(_, _, n)| n >= max_stall)
            {
                input.transfer = true;
                self.attack_progress = None;
                self.route.clear();
                self.planned_goal = None;
            }
        }
        if firing && raw_dyaw.abs() < 0.2 {
            input.lock = true;
            input.fire = if let Some((heat, overheated)) = s.combat.chaingun_hud(player) {
                !overheated && heat < 0.85
            } else {
                true
            };
        }
        (input, goal_kind)
    }
}
