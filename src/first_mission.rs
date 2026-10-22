//! Playable-first SP_0.1 session. Original initializers and all bound scripts execute.
//! Presentation gaps are explicit; gameplay errors still roll back the complete tick.
use crate::{
    ObjectGraph, Result,
    lua50::Bootstrap,
    mission_input::{Button, ITEMS},
    mission_program::Program,
    mission_vm::{Host, Scheduler},
};
use std::{collections::BTreeMap, path::Path, sync::Arc};
#[derive(Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub movement: [f64; 2],
    pub look: [f64; 2],
    pub recruit: bool,
    #[serde(default)]
    pub send: bool,
    #[serde(default)]
    pub dismiss: bool,
    #[serde(default)]
    pub transfer: bool,
    #[serde(default)]
    pub load_vehicle: bool,
    #[serde(default)]
    pub unload_vehicle: bool,
    #[serde(default)]
    pub next_unit: bool,
    #[serde(default)]
    pub prev_unit: bool,
    #[serde(default)]
    pub next_unit_type: bool,
    #[serde(default)]
    pub prev_unit_type: bool,
    #[serde(default)]
    pub global_toggle: bool,
    #[serde(default)]
    pub toggle_map: bool,
    #[serde(default)]
    pub pause: bool,
    #[serde(default)]
    pub skip_cutscene_alt: bool,
    pub fire: bool,
    pub lock: bool,
    pub confirm: bool,
}
pub struct Session {
    pub program: Program,
    pub scheduler: Scheduler,
    pub scene_report: crate::mission_rewrite_scene::Report,
    pub markers: Vec<crate::mission_guidance::Marker>,
    pub combat: crate::mission_combat::Combat,
    last_input: Input,
    recording: Option<Vec<Input>>,
    initial: (Bootstrap, Host, Scheduler),
}
impl Session {
    pub fn load(root: &Path, graph: &ObjectGraph, archive: &[u8]) -> Result<Self> {
        Self::load_level(root, "SP_0.1", graph, archive)
    }
    pub fn load_level(
        root: &Path,
        level_name: &str,
        graph: &ObjectGraph,
        archive: &[u8],
    ) -> Result<Self> {
        let files = root.join("local/extracted/files");
        let data = files.join("Data/CompoundFiles");
        let mut program = Program::load(graph, archive)?;
        let world = &mut program.host.world;
        world.configure_rewrite(graph)?;
        world.rewrite.as_mut().unwrap().allow_cosmetic_gaps();
        world.mission_history = Some(crate::mission_world::MissionHistory::fresh_attempt());
        world.controller = Some(crate::mission_controls::Controller::fresh_player());
        world.input = Some(crate::mission_input::Input::single_player());
        world.hud = Some(crate::mission_hud::Hud::default());
        world.waypoints = Some(Arc::new(crate::mission_waypoints::Bank::load(graph)?));
        world.configure_authored_units(graph)?;
        world.configure_authored_player(graph)?;
        world.configure_fresh_player_lifecycle()?;
        world.cameras = Some(crate::mission_camera::Rig::new(Arc::new(
            crate::mission_camera::Bank::load(graph)?,
        )));
        world.music = Some(crate::mission_music::Transport::new(Arc::new(
            crate::mission_music::Bank::load(&files, graph)?,
        )));
        let language = Arc::new(crate::localization::Language::parse(
            &std::fs::read(files.join(format!("Data/Strings/{level_name}English.str")))
                .map_err(|e| e.to_string())?,
        )?);
        let mut messages = crate::mission_messages::Queue::new(Arc::new(
            crate::mission_audio::Bank::load(&files, &language)?,
        ));
        messages.configure_voiced_playback(language, program.host.known_objects.clone());
        world.secondary_messages = Some(messages.clone());
        world.messages = Some(messages);
        let terrain = crate::terrain::decode(
            &std::fs::read(data.join(format!("{level_name}.out"))).map_err(|e| e.to_string())?,
        )?;
        let catalog = crate::level_catalog::Catalog::parse(
            &std::fs::read_to_string(files.join("Data/levels.xml")).map_err(|e| e.to_string())?,
        )?;
        let scene_report = crate::mission_rewrite_scene::configure(
            graph,
            archive,
            &terrain,
            catalog.unique(level_name)?.physics,
            world,
        )?;
        program.initialize()?;
        let mut scheduler = program.mission_scheduler()?;
        scheduler.tick_with_input(
            &mut program.vm,
            &mut program.host,
            0.0,
            Self::idle_buttons(),
            100_000,
        )?;
        let initial = (program.vm.clone(), program.host.clone(), scheduler.clone());
        let combat = crate::mission_combat::Combat::new(&program.host)?;
        Ok(Self {
            combat,
            last_input: Input::default(),
            recording: None,
            program,
            scheduler,
            scene_report,
            markers: crate::mission_guidance::load(graph)?,
            initial,
        })
    }
    pub fn idle_buttons() -> BTreeMap<String, Button> {
        ITEMS
            .iter()
            .map(|s| ((*s).into(), Button::default()))
            .collect()
    }
    pub fn begin_recording(&mut self) {
        self.recording = Some(Vec::new());
    }
    pub fn take_recording(&mut self) -> Option<Vec<Input>> {
        self.recording.take()
    }
    pub fn restart(&mut self) {
        self.recording = None;
        self.program.vm = self.initial.0.clone();
        self.program.host = self.initial.1.clone();
        self.scheduler = self.initial.2.clone();
        self.combat.reset();
        self.last_input = Input::default();
    }
    pub fn tick(&mut self, dt: f64, input: Input) -> Result<()> {
        if !dt.is_finite()
            || !(0.0..=0.1).contains(&dt)
            || input
                .movement
                .iter()
                .chain(&input.look)
                .any(|v| !v.is_finite())
        {
            return Err("Invalid mission input frame".into());
        }
        let mut host = self.program.host.clone();
        let world = &mut host.world;
        let player = world.player.ok_or("Missing mission player")?;
        world.direct_velocity.remove(&player);
        let paused = world.tutorial.as_ref().is_some_and(|t| t.paused);
        if !world.frozen
            && !paused
            && world
                .entity(player)
                .is_ok_and(|u| u.active == Some(true) && u.health > 0.)
        {
            let aim = world.player_aim.ok_or("Missing mission aim")?;
            let yaw = aim[0].atan2(aim[2]) + input.look[0];
            let pitch = (aim[1].clamp(-1.0, 1.0).asin() + input.look[1]).clamp(-1.3, 1.3);
            let aim = [
                yaw.sin() * pitch.cos(),
                pitch.sin(),
                yaw.cos() * pitch.cos(),
            ];
            world.set_player_aim(aim)?;
            let magnitude = input.movement[0].hypot(input.movement[1]).max(1.0);
            let forward = input.movement[1] / magnitude;
            let right = input.movement[0] / magnitude;
            let speed = world
                .units
                .get(&player)
                .and_then(|u| u.forward_speed)
                .unwrap_or(0.0);
            if let Some(fp) = world.flight_profiles.get(&player).copied() {
                if forward != 0.0 || right != 0.0 {
                    let fwd_v = if forward >= 0.0 {
                        (fp.cruise_speed + (fp.max_speed - fp.cruise_speed) * forward) * forward
                    } else if fp.min_speed > 0.0 {
                        fp.min_speed
                    } else {
                        forward * fp.cruise_speed
                    };
                    let str_v = right * fp.strafe_speed;
                    world.direct_velocity.insert(
                        player,
                        [
                            yaw.sin() * fwd_v - yaw.cos() * str_v,
                            yaw.cos() * fwd_v + yaw.sin() * str_v,
                        ],
                    );
                }
            } else if speed > 0.0 && (forward != 0.0 || right != 0.0) {
                world.direct_velocity.insert(
                    player,
                    [
                        (yaw.sin() * forward - yaw.cos() * right) * speed,
                        (yaw.cos() * forward + yaw.sin() * right) * speed,
                    ],
                );
            }
            if ((input.load_vehicle && !self.last_input.load_vehicle)
                || (input.unload_vehicle && !self.last_input.unload_vehicle))
                && let Some(sub) = world.submarines.get_mut(&player)
            {
                sub.submerged = !sub.submerged;
            }
            if input.recruit
                && world
                    .controller
                    .as_ref()
                    .ok_or("Missing controller")?
                    .accepts("CONTROL_CALL")?
            {
                world.recall_squad()?;
                let p = host.positions[&player];
                let player_army = world
                    .entity(player)
                    .map(|u| u.army.clone())
                    .unwrap_or_default();
                let candidates: Vec<_> = world
                    .units
                    .iter()
                    .filter(|(id, u)| {
                        **id != player
                            && !world.enemy_targetable_only.contains(id)
                            && (world.bodies.contains_key(id)
                                || world.flight_profiles.contains_key(id)
                                || world.water_units.contains(id))
                            && world.water_units.contains(id)
                                == world.water_units.contains(&player)
                            && u.army == player_army
                            && u.active == Some(true)
                            && u.health > 0.0
                            && world
                                .activity
                                .get(id)
                                .is_none_or(|a| a.enabled != Some(false))
                    })
                    .filter_map(|(id, _)| {
                        let range = if world.water_units.contains(id) {
                            150.0
                        } else if world.non_ground_units.contains(id) {
                            60.0
                        } else if world.collision.as_ref().is_some_and(|s| s.ground.is_some()) {
                            65.0
                        } else {
                            30.0
                        };
                        host.positions
                            .get(id)
                            .filter(|q| (p[0] - q[0]).hypot(p[2] - q[2]) <= range)
                            .map(|_| *id)
                    })
                    .collect();
                for id in candidates {
                    world.recruit(id, &host.positions)?;
                }
            }
            if input.send
                && world
                    .controller
                    .as_ref()
                    .ok_or("Missing controller")?
                    .accepts("CONTROL_SEND")?
            {
                world.send_squad(&host.positions)?;
            }
            if input.dismiss
                && world
                    .controller
                    .as_ref()
                    .ok_or("Missing controller")?
                    .accepts("CONTROL_DISMISS")?
            {
                world.dismiss_squad()?;
            }
            if input.transfer
                && !self.last_input.transfer
                && world
                    .controller
                    .as_ref()
                    .ok_or("Missing controller")?
                    .accepts("CONTROL_TRANSFER")?
            {
                let mut candidates: Vec<crate::ObjectId> = world
                    .roster
                    .iter()
                    .copied()
                    .filter(|&id| {
                        !world.enemy_targetable_only.contains(&id)
                            && (world.bodies.contains_key(&id)
                                || world.non_ground_units.contains(&id)
                                || world.buildings.contains(&id))
                            && world
                                .entity(id)
                                .is_ok_and(|u| u.active == Some(true) && u.health > 0.0)
                            && world
                                .activity
                                .get(&id)
                                .is_none_or(|a| a.enabled != Some(false))
                    })
                    .collect();
                let p = host.positions[&player];
                let player_army = world
                    .entity(player)
                    .map(|u| u.army.clone())
                    .unwrap_or_default();
                for (&id, u) in &world.units {
                    if id != player
                        && !candidates.contains(&id)
                        && !world.enemy_targetable_only.contains(&id)
                        && (world.bodies.contains_key(&id) || world.non_ground_units.contains(&id))
                        && u.army == player_army
                        && u.active == Some(true)
                        && u.health > 0.0
                        && world
                            .activity
                            .get(&id)
                            .is_none_or(|a| a.enabled != Some(false))
                        && host
                            .positions
                            .get(&id)
                            .is_some_and(|q| (p[0] - q[0]).hypot(p[2] - q[2]) <= 45.0)
                    {
                        candidates.push(id);
                    }
                }
                candidates.sort();
                if candidates.len() > 1 {
                    let aimed_target = if input.lock
                        && let Some(aim) = world.player_aim
                    {
                        let aim_h = aim[0].hypot(aim[2]);
                        let dir = if aim_h > 0.01 {
                            [aim[0] / aim_h, aim[2] / aim_h]
                        } else {
                            [0.0, 1.0]
                        };
                        candidates
                            .iter()
                            .copied()
                            .filter(|&id| id != player)
                            .filter_map(|id| {
                                let q = host.positions.get(&id)?;
                                let dx = (q[0] - p[0]) as f64;
                                let dz = (q[2] - p[2]) as f64;
                                let d = dx.hypot(dz);
                                let max_d = if world.roster.contains(&id)
                                    || world.flight_profiles.contains_key(&id)
                                {
                                    600.0
                                } else {
                                    45.0
                                };
                                if !(0.05..=max_d).contains(&d) {
                                    return None;
                                }
                                let cos = (dx * dir[0] + dz * dir[1]) / d;
                                let bonus = if self.markers.iter().any(|m| {
                                    world
                                        .objective_attachment
                                        .get(&m.id)
                                        .copied()
                                        .map(|t| if t.0 == 0 { m.id } else { t })
                                        .unwrap_or(m.target)
                                        == id
                                }) {
                                    0.15
                                } else {
                                    0.0
                                };
                                (cos >= 0.70).then_some((cos + bonus, id))
                            })
                            .max_by(|a, b| a.0.total_cmp(&b.0))
                            .map(|(_, id)| id)
                    } else {
                        None
                    };
                    let pos = candidates.iter().position(|&id| id == player);
                    let idx = pos.map_or(0, |i| (i + 1) % candidates.len());
                    let fallback = if world.non_ground_units.contains(&player) {
                        candidates
                            .iter()
                            .copied()
                            .find(|id| !world.non_ground_units.contains(id))
                            .unwrap_or(candidates[idx])
                    } else {
                        let start = pos.unwrap_or(0);
                        (1..=candidates.len())
                            .map(|step| candidates[(start + step) % candidates.len()])
                            .find(|id| !world.non_ground_units.contains(id))
                            .unwrap_or(candidates[idx])
                    };
                    let target = aimed_target.unwrap_or(fallback);
                    if target != player {
                        let was_non_ground = world.non_ground_units.contains(&player);
                        let now_ground = !world.non_ground_units.contains(&target);
                        world.player = Some(target);
                        world.roster.insert(target);
                        let act = world.activity.entry(target).or_default();
                        act.enabled = Some(true);
                        act.weapons_enabled = Some(true);
                        world.squad_follow.remove(&target);
                        if world.can_squad_follow(player, target) {
                            world.squad_follow.insert(player);
                        } else {
                            world.squad_follow.remove(&player);
                        }
                        if was_non_ground && now_ground {
                            let roster_ids: Vec<_> = world.roster.iter().copied().collect();
                            for rid in roster_ids {
                                if rid != target
                                    && world.can_squad_follow(rid, target)
                                    && world
                                        .units
                                        .get(&rid)
                                        .is_some_and(|u| u.active == Some(true) && u.health > 0.0)
                                    && !world.carrier_of.contains_key(&rid)
                                    && !world.orders.contains_key(&rid)
                                {
                                    world.squad_follow.insert(rid);
                                }
                            }
                        }
                        world.orders.remove(&target);
                        world.follow_unit_orders.remove(&target);
                        for (&fid, ftarget) in world.follow_unit_orders.iter_mut() {
                            if *ftarget == player && fid != target {
                                *ftarget = target;
                            }
                        }
                        world.attack_orders.remove(&target);
                        world.following.remove(&target);
                        world.routes.remove(&target);
                        world.direct_velocity.remove(&player);
                        world.direct_velocity.remove(&target);
                        if let Some(view) = &mut world.player_view {
                            view.target = target;
                            view.cut_revision = view.cut_revision.saturating_add(1);
                        }
                        if world.player_lifecycle.is_some() {
                            world.player_lifecycle =
                                Some(crate::mission_world::PlayerLifecycle::FreshAlive(target));
                        }
                        if let Some(rig) = &mut world.cameras {
                            rig.retarget_chase(target);
                        }
                    }
                }
            }
        }
        let mut combat = self.combat.clone();
        if !paused {
            combat.tick(&mut host, dt, input.fire, input.lock)?;
            host.world.replace_dead_player(&host.positions);
            if !host.world.frozen {
                host.world.advance_pickups(&host.positions)?;
            }
        } else {
            if let Some(messages) = &mut host.world.messages {
                messages.advance(dt)?;
            }
            if let Some(music) = &mut host.world.music {
                music.advance(dt)?;
            }
        }
        let edge = |f: fn(&Input) -> bool| {
            let now = f(&input);
            let prev = f(&self.last_input);
            Button {
                held: now,
                just_pressed: now && !prev,
                just_released: !now && prev,
            }
        };
        let mut buttons = Self::idle_buttons();
        for (item, b) in [
            ("CONTROL_CALL", edge(|i| i.recruit)),
            ("CONTROL_LOCAL_CALL", edge(|i| i.recruit)),
            ("CONTROL_SEND", edge(|i| i.send)),
            ("CONTROL_DISMISS", edge(|i| i.dismiss)),
            ("CONTROL_FIRE", edge(|i| i.fire)),
            ("CONTROL_GLOBAL_TOGGLE", edge(|i| i.global_toggle)),
            ("CONTROL_HUD_TRANSFER", edge(|i| i.transfer)),
            ("CONTROL_MAP_TRANSFER", edge(|i| i.transfer)),
            ("CONTROL_TRANSFER", edge(|i| i.transfer)),
            ("CONTROL_LOAD_VEHICLE", edge(|i| i.load_vehicle)),
            ("CONTROL_UNLOAD_VEHICLE", edge(|i| i.unload_vehicle)),
            ("CONTROL_NEXT_UNIT", edge(|i| i.next_unit)),
            ("CONTROL_PREV_UNIT", edge(|i| i.prev_unit)),
            ("CONTROL_NEXT_UNIT_TYPE", edge(|i| i.next_unit_type)),
            ("CONTROL_NEXT_UNIT_TYPE_ALT", edge(|i| i.next_unit_type)),
            ("CONTROL_PREV_UNIT_TYPE", edge(|i| i.prev_unit_type)),
            ("CONTROL_TOGGLE_MAP", edge(|i| i.toggle_map)),
            ("CONTROL_PAUSE", edge(|i| i.pause)),
            ("CONTROL_SKIP_CUTSCENE", edge(|i| i.confirm)),
            ("CONTROL_SKIP_CUTSCENE_ALT", edge(|i| i.skip_cutscene_alt)),
            ("GCONTROL_PAUSE_SELECT", edge(|i| i.confirm)),
        ] {
            buttons.insert(item.into(), b);
        }
        if host.world.skip_cutscene_prepared
            && (edge(|i| i.confirm).just_pressed || edge(|i| i.skip_cutscene_alt).just_pressed)
        {
            host.world.skip_cutscene_requested = true;
        }
        let mut scheduler = self.scheduler.clone();
        scheduler.paused = paused;
        scheduler.tick_with_input(&mut self.program.vm, &mut host, dt, buttons, 100_000)?;
        self.program.host = host;
        self.scheduler = scheduler;
        self.combat = combat;
        self.last_input = input;
        if let Some(recording) = &mut self.recording {
            recording.push(input);
        }
        Ok(())
    }
}
