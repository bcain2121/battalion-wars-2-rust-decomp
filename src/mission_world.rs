//! Typed mission state, separate from rendering and temporary skirmish rules.
//! Initial activation/ownership must be explicitly configured, never guessed from flags.
use crate::{
    ObjectGraph, ObjectId, Result,
    lua50::{Bootstrap, Value},
    original_scene::field,
    original_scene::matrix,
};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug)]
pub struct Unit {
    pub health: f64,
    pub max_health: f64,
    pub invulnerable: bool,
    pub active: Option<bool>,
    pub army: String,
    pub forward_speed: Option<f64>,
    pub animation: Option<crate::unit_animation::Player>,
}
#[derive(Clone, Debug)]
pub struct Zone {
    pub matrix: [f32; 16],
    pub size: [f32; 4],
    pub radius: f32,
}
impl Zone {
    /// Provisional horizontal region adapter. Extents are treated as full box size.
    /// Exact edge, height and flag behavior awaits original-runtime comparison.
    pub fn contains(&self, p: [f32; 3]) -> Result<bool> {
        if p.iter().any(|v| !v.is_finite()) {
            return Err("Nonfinite zone query".into());
        }
        let m = self.matrix;
        let dx = p[0] as f64 - m[12] as f64;
        let dz = p[2] as f64 - m[14] as f64;
        let det = m[0] as f64 * m[10] as f64 - m[8] as f64 * m[2] as f64;
        if det.abs() < 1e-8 {
            return Err("Singular map-zone transform".into());
        }
        let x = (m[10] as f64 * dx - m[8] as f64 * dz) / det;
        let z = (-m[2] as f64 * dx + m[0] as f64 * dz) / det;
        Ok(if self.radius > 0.0 {
            x * x + z * z <= (self.radius as f64).powi(2)
        } else {
            x.abs() <= self.size[0] as f64 / 2.0 && z.abs() <= self.size[2] as f64 / 2.0
        })
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Navigation {
    #[default]
    Unavailable,
    StraightLineDiagnostic,
    CollisionAware,
    PlannedInfantry,
}
#[derive(Clone, Debug)]
pub struct MoveOrder {
    pub target: [f64; 2],
    pub radius: f64,
}
/// SetActive is not Spawn/Despawn. Original adapter conditionally adjusts weapons separately.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Activity {
    pub enabled: Option<bool>,
    pub weapons_enabled: Option<bool>,
}
/// Player-camera target and hard-cut revision for the presentation adapter.
/// Scripted cameras are separate; consumers must reset interpolation on a new revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerView {
    pub camera: ObjectId,
    pub target: ObjectId,
    pub cut_revision: u64,
}
/// Explicit snapshot of the selected profile's current mission, not runtime victory.
/// Native queries read separate attempted/completed bits; neither implies the other.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissionHistory {
    pub attempted: bool,
    pub completed: bool,
}
impl MissionHistory {
    /// New, unsaved profile entering its first mission. No mission completion is recorded.
    pub fn fresh_attempt() -> Self {
        Self {
            attempted: true,
            completed: false,
        }
    }
}
/// Known fresh-player state, separate from any inferred squad roster.
/// Death/removal invalidates the known-live state until explicit transfer; automatic
/// replacement and the terminal defeated transition are not implemented here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerLifecycle {
    FreshAlive(ObjectId),
    ReplacementRequired,
}
/// Rust-owned in-progress capture interaction, reset on despawn.
#[derive(Clone, Debug, Default)]
pub struct CaptureSession {
    pub progress: f64,
    pub contenders: BTreeSet<ObjectId>,
}
#[derive(Clone, Debug, Default)]
pub struct StrategicInstallation {
    pub main_building: Option<ObjectId>,
    pub capture_point: Option<ObjectId>,
    pub spawn_zone: Option<ObjectId>,
    pub facility_type: String,
    pub time_between_respawns: f64,
    pub multi_facility_multiplier: f64,
    pub respawn_cooldown: f64,
    pub buildings: Vec<ObjectId>,
    pub owner: String,
}
#[derive(Clone, Debug, Default)]
pub struct CapturePointState {
    pub owner: String,
    pub capturing_army: Option<String>,
    pub height: f64,
    pub direction: i8,
    pub installation: Option<ObjectId>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct SubmarineState {
    pub submerged: bool,
    pub depth: f32,
    pub max_depth: f32,
    pub dive_time: f64,
    pub oxygen: f64,
    pub max_oxygen: f64,
    pub recharge_time: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub enum MissionOutcome {
    Won {
        speed: f64,
        power: f64,
        technique: f64,
        rating: String,
    },
    Lost {
        message_id: Option<u32>,
    },
}
#[derive(Clone, Default)]
pub struct World {
    /// Explicitly enabled Rust-owned runtime; None preserves strict reference behavior.
    pub rewrite: Option<crate::mission_rewrite::Runtime>,
    pub mission_history: Option<MissionHistory>,
    pub random: Option<crate::mission_random::Stream>,
    pub waypoint_rand: crate::mission_random::CrtRand,
    pub controller: Option<crate::mission_controls::Controller>,
    pub hud: Option<crate::mission_hud::Hud>,
    pub input: Option<crate::mission_input::Input>,
    pub music: Option<crate::mission_music::Transport>,
    pub cameras: Option<crate::mission_camera::Rig>,
    pub messages: Option<crate::mission_messages::Queue>,
    pub navigation: Navigation,
    pub orders: BTreeMap<ObjectId, MoveOrder>,
    pub waypoints: Option<std::sync::Arc<crate::mission_waypoints::Bank>>,
    pub following: BTreeMap<ObjectId, crate::mission_waypoints::Route>,
    /// Explicit player drive, in source-world units/second; never routed automatically.
    pub direct_velocity: BTreeMap<ObjectId, [f64; 2]>,
    pub collision: Option<std::sync::Arc<crate::unit_collision::Scene>>,
    pub bodies: BTreeMap<ObjectId, crate::unit_collision::Body>,
    pub blocked_contacts: u64,
    pub mesh_contacts: u64,
    pub unit_contacts: u64,
    pub routes: BTreeMap<ObjectId, crate::navigation::Route>,
    pub route_searches: u64,
    pub route_arrivals: u64,
    pub route_failures: u64,
    pub route_expansions: u64,
    pub route_probes: u64,
    pub route_cursor: Option<ObjectId>,
    pub last_route_status: Option<crate::navigation::Status>,
    pub units: BTreeMap<ObjectId, Unit>,
    /// Air/water actors never borrow the rewrite's ground-cylinder solver.
    pub non_ground_units: BTreeSet<ObjectId>,
    pub water_units: BTreeSet<ObjectId>,
    pub water_height: f32,
    pub flight_profiles: BTreeMap<ObjectId, crate::mission_air::FlightProfile>,
    pub submarines: BTreeMap<ObjectId, SubmarineState>,
    pub scripted_air_cruise: BTreeMap<ObjectId, f64>,
    pub scripted_air_altitude: BTreeMap<ObjectId, f32>,
    /// Authored/rewrite world heading, radians about +Y; consumed by presentation.
    pub headings: BTreeMap<ObjectId, f32>,
    pub bombers: crate::mission_bomber::System,
    pub missiles: crate::mission_missile::System,
    pub air_shapes: std::sync::Arc<BTreeMap<ObjectId, crate::mission_missile::AirShape>>,
    pub tutorial: Option<crate::mission_tutorial::Tutorial>,
    pub secondary_controller: Option<crate::mission_controls::Controller>,
    pub secondary_messages: Option<crate::mission_messages::Queue>,
    /// Damageable scenery is not a mobile unit or an automatic player-roster member.
    pub props: BTreeMap<ObjectId, Unit>,
    pub crushable_props: BTreeSet<ObjectId>,
    pub buildings: BTreeSet<ObjectId>,
    pub garrisonable_buildings: BTreeSet<ObjectId>,
    /// Kept separate so generic building presence cannot bypass capture teardown.
    pub capture_points: BTreeSet<ObjectId>,
    pub capture_sessions: BTreeMap<ObjectId, CaptureSession>,
    pub activity: BTreeMap<ObjectId, Activity>,
    pub zones: BTreeMap<ObjectId, Zone>,
    pub player: Option<ObjectId>,
    pub player_lifecycle: Option<PlayerLifecycle>,
    /// Native unit +0x3e1 flag, separate from physical presence/activity.
    pub despawn_retention: BTreeMap<ObjectId, bool>,
    pub player_view: Option<PlayerView>,
    pub roster: BTreeSet<ObjectId>,
    pub command_selection: BTreeSet<ObjectId>,
    pub squad_follow: BTreeSet<ObjectId>,
    pub interactive_squad_formation: bool,
    pub squad_formation_dir: Option<[f64; 2]>,
    /// Explicit squad attack target from a Send while aiming at an enemy
    /// (declared Rust command policy; native command-bar orders unrecovered).
    pub squad_target: Option<ObjectId>,
    pub player_aim: Option<[f64; 3]>,
    pub frozen: bool,
    pub objective_visibility: BTreeMap<ObjectId, Option<bool>>,
    pub objective_attachment: BTreeMap<ObjectId, ObjectId>,
    /// Authored objective flags: visible, complete.
    pub objectives: BTreeMap<ObjectId, (bool, bool)>,
    pub declared_victory: Option<String>,
    pub scoring_listed: bool,
    pub scoring_allegiance: BTreeMap<String, u8>,
    pub allied_armies: BTreeSet<(String, String)>,
    pub summary_messages: Vec<(u32, String, ObjectId)>,
    pub power_objectives: Option<[f64; 2]>,
    pub tech_objectives: Option<[f64; 2]>,
    pub mission_min_max_time: Option<(f64, f64)>,
    pub mission_time: Option<f64>,
    pub mission_ratings: Option<[String; 4]>,
    pub mission_complete: Option<MissionOutcome>,
    /// Original pickup lifecycle/collection (`mission_pickup`), rewrite only.
    pub pickups: Option<crate::mission_pickup::Pickups>,
    pub pending_positions: BTreeMap<ObjectId, [f32; 3]>,
    pub passengers: BTreeMap<ObjectId, Vec<ObjectId>>,
    pub carrier_of: BTreeMap<ObjectId, ObjectId>,
    pub entering_vehicle: BTreeMap<ObjectId, ObjectId>,
    pub unloading_on_arrival: BTreeSet<ObjectId>,
    pub landing_units: BTreeSet<ObjectId>,
    pub grounded_units: BTreeSet<ObjectId>,
    pub follow_unit_orders: BTreeMap<ObjectId, ObjectId>,
    pub attack_orders: BTreeMap<ObjectId, ObjectId>,
    pub unit_groups: BTreeMap<ObjectId, Vec<ObjectId>>,
    pub installations: BTreeMap<ObjectId, StrategicInstallation>,
    pub capture_point_state: BTreeMap<ObjectId, CapturePointState>,
    pub damage_zones: BTreeMap<ObjectId, (Zone, f64)>,
    pub timer_seconds: Option<f64>,
    pub timer_visible: bool,
    pub skip_cutscene_prepared: bool,
    pub skip_cutscene_requested: bool,
    pub transferred_by_death: bool,
    pub killed_scripts: BTreeSet<u64>,
    pub enemy_targetable_only: BTreeSet<ObjectId>,
    pub playable_units: BTreeSet<ObjectId>,
    pub revivable_units: BTreeMap<ObjectId, f64>,
    pub revive_timers: BTreeMap<ObjectId, f64>,
    pub tick_when_dead: BTreeSet<ObjectId>,
    pub tick_when_despawned: BTreeSet<ObjectId>,
    pub unit_invulnerable: BTreeSet<ObjectId>,
    pub invulnerable_army_mask: u32,
}
impl World {
    pub fn same_allegiance(&self, army_a: &str, army_b: &str) -> bool {
        if army_a.is_empty() || army_b.is_empty() {
            return false;
        }
        if army_a == army_b {
            return true;
        }
        if self
            .allied_armies
            .contains(&(army_a.to_string(), army_b.to_string()))
        {
            return true;
        }
        if let (Some(&a), Some(&b)) = (
            self.scoring_allegiance.get(army_a),
            self.scoring_allegiance.get(army_b),
        ) {
            return a != 0 && a == b;
        }
        false
    }
    pub fn teleport_scripted_with_margin(
        &mut self,
        positions: &mut BTreeMap<ObjectId, [f32; 3]>,
        id: ObjectId,
        x: f64,
        z: f64,
        heading: f64,
        margin: f64,
    ) -> Result<bool> {
        let objects = self
            .rewrite
            .as_ref()
            .ok_or("Teleport rewrite not configured")?
            .objects();
        let is_water = objects.get(&id).is_some_and(|o| o.kind == "cWaterVehicle");
        let is_active = self.units.get(&id).is_none_or(|u| u.active == Some(true));
        if margin == 0.0 && !is_water && is_active && (self.player != Some(id) || self.frozen)
            && let Ok(applied) = self.teleport_scripted(positions, id, x, z, heading)
        {
            return Ok(applied);
        }
        if [x, z, heading, margin].iter().any(|v| !v.is_finite())
            || !(x as f32).is_finite()
            || !(z as f32).is_finite()
            || margin < 0.0
        {
            return Err("Invalid teleport coordinates".into());
        }
        let unit = self.entity(id)?;
        if unit.health <= 0.0 {
            return Ok(false);
        }
        let p = *positions.get(&id).ok_or("Missing teleport position")?;
        let mut candidate = [x as f32, p[1], z as f32];
        if !self.non_ground_units.contains(&id)
            && let Some(scene) = &self.collision
            && scene.world_point(candidate, 0.0).is_ok()
            && margin > 0.0
            && let Some(&body) = self.bodies.get(&id)
        {
            let others: Vec<_> = self
                .units
                .iter()
                .filter(|(other, u)| {
                    **other != id
                        && u.active == Some(true)
                        && u.health > 0.0
                        && !self.non_ground_units.contains(other)
                })
                .filter_map(|(other, _)| {
                    Some(crate::unit_collision::Other {
                        position: *positions.get(other)?,
                        body: *self.bodies.get(other)?,
                    })
                })
                .collect();
            if let Ok(free) = scene.find_free_near(candidate, body, &others, margin.min(24.0)) {
                candidate = free;
            }
        }
        positions.insert(id, candidate);
        let heading_rad = heading.rem_euclid(360.).to_radians() as f32;
        self.headings.insert(id, heading_rad);
        if self.player == Some(id) {
            self.player_aim = Some([(heading_rad as f64).sin(), 0., (heading_rad as f64).cos()]);
        }
        self.bombers.orders.remove(&id);
        self.missiles.orders.remove(&id);
        self.orders.remove(&id);
        self.following.remove(&id);
        self.routes.remove(&id);
        self.direct_velocity.remove(&id);
        Ok(true)
    }
    /// Aircraft deployment, a player troop's authored in-place facing reset, or
    /// collision-validated non-player ground troop relocation.
    /// Nonzero placement-margin searches and player position shortcuts remain unsupported.
    pub fn teleport_scripted(
        &mut self,
        positions: &mut BTreeMap<ObjectId, [f32; 3]>,
        id: ObjectId,
        x: f64,
        z: f64,
        heading: f64,
    ) -> Result<bool> {
        if self.scripted_air_cruise.contains_key(&id) {
            return self.teleport_air(positions, id, x, z, heading);
        }
        let objects = self
            .rewrite
            .as_ref()
            .ok_or("Teleport rewrite not configured")?
            .objects();
        if objects
            .get(&id)
            .is_none_or(|o| !matches!(o.kind.as_str(), "cTroop" | "cGroundVehicle"))
        {
            return Err("Teleport entity type not configured".into());
        }
        if [x, z, heading].iter().any(|v| !v.is_finite())
            || !(x as f32).is_finite()
            || !(z as f32).is_finite()
        {
            return Err("Invalid troop teleport coordinates".into());
        }
        let p = *positions
            .get(&id)
            .ok_or("Missing troop teleport position")?;
        let relocate = x != p[0] as f64 || z != p[2] as f64;
        if relocate && self.player == Some(id) && !self.frozen {
            return Err("Ground teleport relocation not configured".into());
        }
        let unit = self.entity(id)?;
        if unit.health <= 0. {
            return Ok(false);
        }
        if unit.active != Some(true) {
            return Err("Teleport troop is not present".into());
        }
        if relocate {
            use crate::unit_collision::Other;
            let scene = self
                .collision
                .as_ref()
                .ok_or("Teleport collision not configured")?;
            let body = *self
                .bodies
                .get(&id)
                .ok_or("Teleport troop body not configured")?;
            body.validate()?;
            let candidate = [x as f32, p[1], z as f32];
            scene.world_point(candidate, 0.)?;
            let others = self
                .units
                .iter()
                .filter(|(other, u)| {
                    **other != id
                        && u.active == Some(true)
                        && u.health > 0.
                        && !self.non_ground_units.contains(other)
                })
                .map(|(other, _)| {
                    Ok(Other {
                        position: *positions
                            .get(other)
                            .ok_or("Missing teleport obstacle position")?,
                        body: *self
                            .bodies
                            .get(other)
                            .ok_or("Missing teleport obstacle body")?,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            if scene.overlaps(candidate, body, &others)? {
                return Err("Teleport troop destination is obstructed".into());
            }
            positions.insert(id, candidate);
        }
        let heading = heading.rem_euclid(360.).to_radians() as f32;
        self.headings.insert(id, heading);
        if self.player == Some(id) {
            self.player_aim = Some([(heading as f64).sin(), 0., (heading as f64).cos()]);
        }
        self.missiles.orders.remove(&id);
        self.orders.remove(&id);
        self.following.remove(&id);
        self.routes.remove(&id);
        self.direct_velocity.remove(&id);
        Ok(true)
    }
    pub fn teleport_air(
        &mut self,
        positions: &mut BTreeMap<ObjectId, [f32; 3]>,
        id: ObjectId,
        x: f64,
        z: f64,
        heading: f64,
    ) -> Result<bool> {
        let runtime = self
            .rewrite
            .as_ref()
            .ok_or("Teleport rewrite not configured")?;
        let (position, heading) = crate::mission_air::teleport(
            &runtime.objects(),
            self.collision
                .as_ref()
                .ok_or("Teleport collision not configured")?,
            id,
            x,
            z,
            heading,
        )?;
        let unit = self.units.get(&id).ok_or("Unknown teleport unit")?;
        if unit.health <= 0. {
            return Ok(false);
        }
        if unit.active != Some(true) || !positions.contains_key(&id) {
            return Err("Teleport aircraft is not present".into());
        }
        positions.insert(id, position);
        self.headings.insert(id, heading);
        self.bombers.orders.remove(&id);
        self.orders.remove(&id);
        self.following.remove(&id);
        self.routes.remove(&id);
        self.direct_velocity.remove(&id);
        Ok(true)
    }
    pub fn configure_rewrite(&mut self, graph: &ObjectGraph) -> Result<()> {
        if self.rewrite.is_some() {
            return Err("Rewrite already configured".into());
        }
        let mut air = crate::mission_air::cruise_speeds(graph)?;
        self.flight_profiles = crate::mission_air::flight_profiles(graph)?;
        for object in graph.objects.values().filter(|o| o.kind == "cWaterVehicle") {
            self.water_units.insert(object.id);
            let mut speed = self
                .units
                .get(&object.id)
                .and_then(|u| u.forward_speed)
                .unwrap_or(14.0);
            if let Some(base_id) = field(object, "mBase").and_then(|s| s.parse::<u64>().ok())
                && let Some(base) = graph.objects.get(&ObjectId(base_id))
            {
                if let Some(pb_id) = field(base, "mPhysicsBase").and_then(|s| s.parse::<u64>().ok())
                    && let Some(pb) = graph.objects.get(&ObjectId(pb_id))
                    && let Some(s) = field(pb, "attackSpeed")
                        .or_else(|| field(pb, "patrolSpeed"))
                        .and_then(|v| v.parse::<f64>().ok())
                    && s.is_finite()
                    && s > 0.0
                {
                    speed = s;
                }
                let is_sub = field(base, "mUnitSubType") == Some("UNIT_SEA_SUBMARINE")
                    || field(base, "mDiveDepth")
                        .and_then(|v| v.parse::<f32>().ok())
                        .is_some_and(|d| d < 0.0);
                if is_sub {
                    let max_depth = field(base, "mDiveDepth")
                        .and_then(|v| v.parse::<f32>().ok())
                        .unwrap_or(-3.5)
                        .abs()
                        .max(1.0);
                    let dive_time = field(base, "mDiveTime")
                        .and_then(|v| v.parse::<f64>().ok())
                        .unwrap_or(4.0)
                        .max(0.5);
                    let max_oxygen = field(base, "mOxygenTime")
                        .and_then(|v| v.parse::<f64>().ok())
                        .unwrap_or(90.0)
                        .max(1.0);
                    let recharge_time = field(base, "mOxygenRechargeTime")
                        .and_then(|v| v.parse::<f64>().ok())
                        .unwrap_or(20.0)
                        .max(1.0);
                    self.submarines.insert(
                        object.id,
                        SubmarineState {
                            submerged: false,
                            depth: 0.0,
                            max_depth,
                            dive_time,
                            oxygen: max_oxygen,
                            max_oxygen,
                            recharge_time,
                        },
                    );
                }
            }
            air.insert(object.id, speed);
        }
        for (id, speed) in &air {
            self.units
                .get_mut(id)
                .ok_or("Missing aircraft unit")?
                .forward_speed = Some(*speed);
        }
        self.scripted_air_cruise = air;
        self.scripted_air_altitude = crate::mission_air::cruise_altitudes(graph)?;
        self.tutorial = Some(crate::mission_tutorial::Tutorial::default());
        self.pickups = Some(crate::mission_pickup::Pickups::load(graph)?);
        self.secondary_controller = Some(crate::mission_controls::Controller::fresh_player());
        self.rewrite = Some(crate::mission_rewrite::Runtime::new(graph));
        for object in graph
            .objects
            .values()
            .filter(|o| o.kind == "cDestroyableObject" && field(o, "mBase") != Some("0"))
        {
            self.props
                .get_mut(&object.id)
                .ok_or("Missing scenery entity")?
                .active = Some(true);
        }
        for object in graph
            .objects
            .values()
            .filter(|o| o.kind == "cObjectiveMarker")
        {
            let flags = crate::original_scene::field(object, "mFlags")
                .ok_or("Missing marker flags")?
                .parse::<u32>()
                .map_err(|_| "Invalid marker flags")?;
            self.objective_visibility
                .insert(object.id, Some(flags & 1 != 0));
        }
        let army_fields = [
            ("mWesternFrontierAllegiance", "eWesternFrontier"),
            ("mXylvanianAllegiance", "eXylvanian"),
            ("mTundranTerritoriesAllegiance", "eTundranTerritories"),
            ("mSolarEmpireAllegiance", "eSolarEmpire"),
            ("mUnderWorldAllegiance", "eUnderWorld"),
            ("mAngloIslesAllegiance", "eAngloIsles"),
        ];
        for level in graph
            .objects
            .values()
            .filter(|o| o.kind == "cLevelSettings")
        {
            for (ptr_field, army_a) in &army_fields {
                if let Some(aid) = field(level, ptr_field).and_then(|s| s.parse::<u64>().ok())
                    && aid != 0
                    && let Some(alleg) = graph.objects.get(&ObjectId(aid))
                {
                    for (enum_field, army_b) in &army_fields {
                        if field(alleg, enum_field) == Some("ALLEGIANCE_ALLY") {
                            self.allied_armies
                                .insert(((*army_a).to_string(), (*army_b).to_string()));
                            self.allied_armies
                                .insert(((*army_b).to_string(), (*army_a).to_string()));
                        }
                    }
                }
            }
        }
        self.capture_sessions = self
            .capture_points
            .iter()
            .map(|id| (*id, CaptureSession::default()))
            .collect();
        if self.random.is_none() {
            self.random = Some(crate::mission_random::Stream::at_cursor(0)?);
        }
        Ok(())
    }
    /// Rust policy: retire all owned collision, stop orders, hide and disable scenery.
    /// This is not the native leftover-node/allocator destruction protocol.
    pub fn kill_rewrite(&mut self, id: ObjectId, position: [f32; 3]) -> Result<()> {
        let runtime = self
            .rewrite
            .as_ref()
            .ok_or("Rust destruction backend not configured")?;
        if runtime.retired.contains(&id) {
            return Ok(());
        }
        self.entity(id)?;
        let mut staged = self.clone();
        staged.rewrite.as_mut().unwrap().retire(id, position)?;
        if !staged.units.contains_key(&id)
            && let Some(scene) = &mut staged.collision
        {
            if let Some(owner) = scene.collision_owner(id) {
                std::sync::Arc::make_mut(scene).remove_owned_meshes(owner, &[])?;
            } else if scene.ground.is_none() {
                return Err("Missing rewrite collision registration".into());
            }
        }
        let entity = staged.entity_mut(id)?;
        if let Some(animation) = &mut entity.animation
            && animation
                .bindings
                .contains_key(&crate::unit_animation::Motion::Death)
        {
            animation.select(crate::unit_animation::Motion::Death)?;
        }
        entity.health = 0.0;
        entity.active = Some(false);
        staged.activity.insert(
            id,
            Activity {
                enabled: Some(false),
                weapons_enabled: Some(false),
            },
        );
        staged.orders.remove(&id);
        staged.following.remove(&id);
        staged.direct_velocity.remove(&id);
        staged.routes.remove(&id);
        staged.attack_orders.remove(&id);
        staged.attack_orders.retain(|_, t| *t != id);
        if let Some(cid) = staged.carrier_of.remove(&id)
            && let Some(list) = staged.passengers.get_mut(&cid)
        {
            if staged.capture_points.contains(&cid) {
                list.retain(|&u| u != id);
            } else {
                for u in list.iter_mut() {
                    if *u == id {
                        *u = ObjectId(0);
                    }
                }
            }
        }
        if !staged.capture_points.contains(&id)
            && let Some(passengers) = staged.passengers.remove(&id)
        {
            for pid in passengers {
                if pid.0 == 0 {
                    continue;
                }
                staged.carrier_of.remove(&pid);
                if staged.units.contains_key(&pid) {
                    staged.kill_rewrite(pid, position)?;
                }
            }
        }
        if staged.player == Some(id) && staged.player_lifecycle.is_some() {
            staged.player_lifecycle = Some(PlayerLifecycle::ReplacementRequired);
        }
        *self = staged;
        Ok(())
    }
    pub fn set_player_aim(&mut self, direction: [f64; 3]) -> Result<()> {
        if self.rewrite.is_none() || self.player.is_none() {
            return Err("Player aiming backend not configured".into());
        }
        let length = direction[0].hypot(direction[1]).hypot(direction[2]);
        if direction.iter().any(|v| !v.is_finite()) || !length.is_finite() || length < 1e-8 {
            return Err("Invalid player aim".into());
        }
        let direction = direction.map(|v| v / length);
        if let Some(rig) = &mut self.cameras {
            rig.set_chase_aim(direction)?;
        }
        self.player_aim = Some(direction);
        if let Some(player) = self.player
            && (direction[0].abs() > 1e-6 || direction[2].abs() > 1e-6)
        {
            self.headings
                .insert(player, direction[0].atan2(direction[2]) as f32);
        }
        Ok(())
    }
    pub fn player_angle_to_target(
        &self,
        target: ObjectId,
        positions: &BTreeMap<ObjectId, [f32; 3]>,
    ) -> Result<f64> {
        let aim = self.player_aim.ok_or("Player aim snapshot missing")?;
        let player = self.player.ok_or("Missing player")?;
        let mut origin = *positions.get(&player).ok_or("Missing player position")?;
        let mut destination = *positions
            .get(&target)
            .ok_or("Missing aim target position")?;
        if origin.iter().chain(&destination).any(|v| !v.is_finite()) {
            return Err("Invalid aim coordinates".into());
        }
        if let Some(scene) = &self.collision {
            if !self.non_ground_units.contains(&player) {
                origin = scene.world_point(origin, 0.0)?;
            }
            if !self.non_ground_units.contains(&target) {
                destination = scene.world_point(destination, 0.0)?;
            }
        }
        origin[1] += 1.6;
        if let Some(body) = self.bodies.get(&target) {
            destination[1] += (body.height * 0.5) as f32;
        }
        let delta = [
            destination[0] as f64 - origin[0] as f64,
            destination[1] as f64 - origin[1] as f64,
            destination[2] as f64 - origin[2] as f64,
        ];
        let distance = delta[0].hypot(delta[1]).hypot(delta[2]);
        if distance < 1e-8 {
            return Ok(0.0);
        }
        Ok(
            (delta.iter().zip(aim).map(|(a, b)| a * b).sum::<f64>() / distance)
                .clamp(-1.0, 1.0)
                .acos()
                .to_degrees(),
        )
    }
    /// Full mission retirement transaction, including radial damage and chained deaths.
    pub fn kill_rewrite_in_scene(
        &mut self,
        id: ObjectId,
        positions: &BTreeMap<ObjectId, [f32; 3]>,
    ) -> Result<()> {
        let mut staged = self.clone();
        staged
            .rewrite
            .as_mut()
            .ok_or("Missing rewrite runtime")?
            .enable_damage();
        staged.kill_rewrite(id, *positions.get(&id).ok_or("Missing kill position")?)?;
        staged.apply_blasts(positions)?;
        *self = staged;
        Ok(())
    }
    fn apply_blasts(&mut self, positions: &BTreeMap<ObjectId, [f32; 3]>) -> Result<()> {
        let elevation = |world: &Self, id, p: [f32; 3]| -> Result<f64> {
            if world.non_ground_units.contains(&id) {
                return Ok(p[1] as f64);
            }
            if let Some(scene) = &world.collision {
                return Ok(scene
                    .world_point(p, 0.0)
                    .map(|pt| pt[1] as f64)
                    .unwrap_or(world.water_height as f64));
            }
            Ok(p[1] as f64)
        };
        let mut count = 0;
        loop {
            let blasts = self
                .rewrite
                .as_mut()
                .map(|r| r.take_blasts())
                .unwrap_or_default();
            if blasts.is_empty() {
                break;
            }
            for blast in blasts {
                count += 1;
                if count > 4096 {
                    return Err("Blast cascade limit exceeded".into());
                }
                let origin_y = elevation(self, blast.owner, blast.position)?;
                let targets: Vec<_> = self
                    .units
                    .keys()
                    .chain(self.props.keys())
                    .copied()
                    .collect();
                let real_level = self
                    .collision
                    .as_ref()
                    .is_some_and(|s| s.ground.is_some());
                let owner_army = self.units.get(&blast.owner).map(|u| u.army.clone());
                for id in targets {
                    let unit = self.entity(id)?;
                    if unit.health <= 0.0
                        || unit.active == Some(false)
                        || unit.invulnerable
                        || ((self.props.contains_key(&blast.owner)
                            || self.units.contains_key(&blast.owner))
                            && self.units.contains_key(&id)
                            && self.activity.get(&id).is_some_and(|a| a.enabled == Some(false)))
                        || (real_level
                            && self.units.contains_key(&id)
                            && owner_army.as_ref().is_some_and(|oa| {
                                !oa.is_empty()
                                    && !unit.army.is_empty()
                                    && self.same_allegiance(oa, &unit.army)
                            }))
                    {
                        continue;
                    }
                    let p = *positions.get(&id).ok_or("Missing blast target position")?;
                    let horizontal = (p[0] as f64 - blast.position[0] as f64)
                        .hypot(p[2] as f64 - blast.position[2] as f64);
                    if horizontal >= blast.outer {
                        continue;
                    }
                    if unit.active.is_none() {
                        return Err("Blast target presence unknown".into());
                    }
                    let distance = horizontal.hypot(elevation(self, id, p)? - origin_y);
                    let factor = blast.attenuation(distance);
                    if factor <= 0.0 {
                        continue;
                    }
                    let amount = self.rewrite.as_ref().unwrap().damage_for(
                        &blast,
                        id,
                        self.player == Some(blast.owner) || self.roster.contains(&blast.owner),
                    )? * factor;
                    if amount <= 0.0 {
                        continue;
                    }
                    let health = self.entity(id)?.health;
                    let intensity = amount.min(health) / self.entity(id)?.max_health * 10.0;
                    let runtime = self.rewrite.as_mut().unwrap();
                    runtime.activity_score = (runtime.activity_score + intensity).min(100.0);
                    if amount >= health {
                        self.kill_rewrite(id, p)?;
                    } else {
                        self.entity_mut(id)?.health = health - amount;
                    }
                }
            }
        }
        Ok(())
    }
    /// Impact point is world-space; convert to the effect owner's coordinate convention.
    pub fn projectile_explosion(
        &mut self,
        owner: ObjectId,
        position: [f32; 3],
        explosion: ObjectId,
        positions: &BTreeMap<ObjectId, [f32; 3]>,
    ) -> Result<()> {
        if position.iter().any(|v| !v.is_finite()) {
            return Err("Invalid projectile impact position".into());
        }
        self.entity(owner)?;
        let mut position = position;
        if !self.non_ground_units.contains(&owner)
            && let Some(scene) = &self.collision
        {
            position[1] -= scene
                .world_point([position[0], 0., position[2]], 0.)
                .map(|pt| pt[1])
                .unwrap_or(self.water_height);
        }
        let mut staged = self.clone();
        staged
            .rewrite
            .as_mut()
            .ok_or("Projectile explosion rewrite missing")?
            .spawn(owner, position, explosion)?;
        staged.apply_blasts(positions)?;
        *self = staged;
        Ok(())
    }
    /// Direct weapon channels use the same authored resistances and retirement transaction as blasts.
    pub fn weapon_hit(
        &mut self,
        shooter: ObjectId,
        target: ObjectId,
        channels: Vec<(String, f64)>,
        positions: &BTreeMap<ObjectId, [f32; 3]>,
    ) -> Result<f64> {
        if channels.iter().any(|(_, v)| !v.is_finite() || *v < 0.) {
            return Err("Invalid weapon damage".into());
        }
        let unit = self.entity(target)?;
        if unit.active != Some(true) || unit.health <= 0. || unit.invulnerable {
            return Ok(0.);
        }
        let blast = crate::mission_damage::Blast {
            owner: shooter,
            position: *positions
                .get(&target)
                .ok_or("Missing weapon target position")?,
            inner: 0.,
            outer: 1.,
            channels,
        };
        let amount = self
            .rewrite
            .as_ref()
            .ok_or("Missing weapon rewrite backend")?
            .damage_for(
                &blast,
                target,
                self.player == Some(shooter) || self.roster.contains(&shooter),
            )?;
        let actual = amount.min(unit.health);
        let mut staged = self.clone();
        if amount >= unit.health {
            staged.kill_rewrite_in_scene(target, positions)?;
        } else {
            staged.entity_mut(target)?.health -= amount;
        }
        let intensity = actual / self.entity(target)?.max_health * 10.;
        let runtime = staged.rewrite.as_mut().unwrap();
        runtime.activity_score = (runtime.activity_score + intensity).min(100.);
        *self = staged;
        Ok(actual)
    }
    /// Explicit player recruitment action for the rewrite, not automatic faction ownership.
    pub fn recruit(
        &mut self,
        id: ObjectId,
        positions: &BTreeMap<ObjectId, [f32; 3]>,
    ) -> Result<()> {
        if self.rewrite.is_none() {
            return Err("Recruitment requires rewrite gameplay".into());
        }
        let player = self.player.ok_or("Missing player")?;
        let leader = if self.collision.as_ref().is_some_and(|s| s.ground.is_some()) {
            self.entity(player)?
        } else {
            self.units.get(&player).ok_or("Missing player unit")?
        };
        let unit = self.units.get(&id).ok_or("Unknown recruit")?;
        if id == player
            || (self.non_ground_units.contains(&id)
                && !self.flight_profiles.contains_key(&id)
                && !self.water_units.contains(&id))
        {
            return Err("Unsupported recruit type".into());
        }
        if leader.army != unit.army
            || leader.health <= 0.0
            || unit.health <= 0.0
            || leader.active != Some(true)
            || unit.active != Some(true)
            || self.enemy_targetable_only.contains(&id)
            || self
                .activity
                .get(&id)
                .is_some_and(|a| a.enabled == Some(false))
        {
            return Err("Ineligible recruit".into());
        }
        let a = positions.get(&player).ok_or("Missing player position")?;
        let b = positions.get(&id).ok_or("Missing recruit position")?;
        let max_range = if self.water_units.contains(&id) {
            150.0
        } else if self.non_ground_units.contains(&id) {
            60.0
        } else if self.collision.as_ref().is_some_and(|s| s.ground.is_some()) {
            65.0
        } else {
            30.0
        };
        if a.iter().chain(b).any(|v| !v.is_finite())
            || (a[0] - b[0]).hypot(a[2] - b[2]) > max_range
        {
            return Err("Recruit outside command range".into());
        }
        self.roster.insert(id);
        if self.can_squad_follow(id, player) {
            self.squad_follow.insert(id);
            self.following.remove(&id);
        }
        Ok(())
    }
    pub fn can_squad_follow(&self, member: ObjectId, leader: ObjectId) -> bool {
        if let (Ok(m), Ok(l)) = (self.entity(member), self.entity(leader))
            && !m.army.is_empty()
            && !l.army.is_empty()
            && !self.same_allegiance(&m.army, &l.army)
        {
            return false;
        }
        if self.water_units.contains(&member) != self.water_units.contains(&leader) {
            return false;
        }
        if self.flight_profiles.contains_key(&leader)
            && !self.flight_profiles.contains_key(&member)
            && (self.unit_gtype_flags(leader) & 0x0000_0008) == 0
        {
            return false;
        }
        true
    }
    pub fn recall_squad(&mut self) -> Result<()> {
        if self.rewrite.is_none() {
            return Err("Squad recall requires rewrite gameplay".into());
        }
        let player = self.player.ok_or("Missing player")?;
        self.squad_target = None;
        for id in self.roster.clone() {
            if id != player
                && self.can_squad_follow(id, player)
                && self
                    .units
                    .get(&id)
                    .is_some_and(|u| u.active == Some(true) && u.health > 0.0)
                && self
                    .activity
                    .get(&id)
                    .is_none_or(|a| a.enabled != Some(false))
            {
                self.squad_follow.insert(id);
                self.following.remove(&id);
            }
        }
        Ok(())
    }
    pub fn send_squad(&mut self, positions: &BTreeMap<ObjectId, [f32; 3]>) -> Result<()> {
        if self.rewrite.is_none() {
            return Err("Squad send requires rewrite gameplay".into());
        }
        let player = self.player.ok_or("Missing player")?;
        let leader = if self.collision.as_ref().is_some_and(|s| s.ground.is_some()) {
            self.entity(player)?.clone()
        } else {
            self.units.get(&player).ok_or("Missing player unit")?.clone()
        };
        if leader.health <= 0.0 || leader.active != Some(true) {
            return Err("Ineligible squad leader".into());
        }
        let p = *positions.get(&player).ok_or("Missing player position")?;
        let aim = self.player_aim.ok_or("Missing squad aim")?;
        let aim_h = aim[0].hypot(aim[2]);
        let dir = if aim_h > 0.01 {
            [aim[0] / aim_h, aim[2] / aim_h]
        } else {
            [0.0, 1.0]
        };
        let mut best_enemy: Option<(ObjectId, [f64; 2], f64)> = None;
        for (id, u) in &self.units {
            if *id == player
                || self.same_allegiance(&u.army, &leader.army)
                || u.army == "eNoArmy"
                || u.army == "eNeutral"
                || u.active != Some(true)
                || u.health <= 0.0
                || !self.bodies.contains_key(id)
                || self.non_ground_units.contains(id)
            {
                continue;
            }
            let ep = *positions.get(id).ok_or("Missing enemy position")?;
            let mut d = (ep[0] as f64 - p[0] as f64).hypot(ep[2] as f64 - p[2] as f64);
            for sid in &self.roster {
                if *sid != player
                    && self
                        .units
                        .get(sid)
                        .is_some_and(|su| su.active == Some(true) && su.health > 0.0)
                    && let Some(sp) = positions.get(sid)
                {
                    d = d.min((ep[0] as f64 - sp[0] as f64).hypot(ep[2] as f64 - sp[2] as f64));
                }
            }
            if d <= 100.0 && best_enemy.is_none_or(|(_, _, bd)| d < bd) {
                best_enemy = Some((*id, [ep[0] as f64, ep[2] as f64], d));
            }
        }
        // An enemy unit or vulnerable enemy building under the reticle cone
        // becomes the explicit squad target.
        let mut aimed: Option<(ObjectId, [f64; 2], f64, bool, bool)> = None;
        for (id, u) in self.units.iter().chain(self.props.iter()) {
            let building = self.buildings.contains(id);
            let attached_obj_bldg = building
                && !self.capture_points.contains(id)
                && !self.garrisonable_buildings.contains(id)
                && self.objective_attachment.iter().any(|(m, &t)| {
                    t == *id && self.objective_visibility.get(m) == Some(&Some(true))
                });
            if *id == player
                || self.same_allegiance(&u.army, &leader.army)
                || (!attached_obj_bldg
                    && (u.army.is_empty() || u.army == "eNoArmy" || u.army == "eNeutral"))
                || u.active != Some(true)
                || u.health <= 0.0
                || u.invulnerable
                || (!building && !self.bodies.contains_key(id) && !self.air_shapes.contains_key(id))
            {
                continue;
            }
            let ep = if building
                && let Some(scene) = &self.collision
                && let Some(c) = scene.object_center(*id)
            {
                c
            } else if let Some(&ep) = positions.get(id) {
                ep
            } else {
                continue;
            };
            let dx = ep[0] as f64 - p[0] as f64;
            let dz = ep[2] as f64 - p[2] as f64;
            let d = dx.hypot(dz);
            let min_cos = if d < 3.0 { 0.5 } else { 0.85 };
            let cos = if d > 1e-6 {
                (dx * dir[0] + dz * dir[1]) / d
            } else {
                1.0
            };
            if !(0.05..=100.0).contains(&d) || cos < min_cos {
                continue;
            }
            let is_obj = cos >= 0.92
                && self.objective_attachment.iter().any(|(m, &t)| {
                    t == *id && self.objective_visibility.get(m) == Some(&Some(true))
                });
            let near_obj_marker = self.objective_visibility.iter().any(|(m, vis)| {
                *vis == Some(true)
                    && positions
                        .get(m)
                        .is_some_and(|mp| (mp[0] - ep[0]).hypot(mp[2] - ep[2]) <= 35.0)
            });
            let is_barrel = building && u.max_health <= 100.0 && !is_obj && !near_obj_marker;
            if is_barrel
                && self.units.iter().any(|(uid, fu)| {
                    self.same_allegiance(&fu.army, &leader.army)
                        && fu.active == Some(true)
                        && fu.health > 0.0
                        && !fu.invulnerable
                        && self.activity.get(uid).is_none_or(|a| a.enabled != Some(false))
                        && positions
                            .get(uid)
                            .is_some_and(|fp| (fp[0] - ep[0]).hypot(fp[2] - ep[2]) < 15.5)
                })
            {
                continue;
            }
            if aimed.is_none_or(|(_, _, bd, b_obj, b_barrel)| {
                (is_obj && !b_obj)
                    || (is_obj == b_obj && !is_barrel && b_barrel)
                    || (is_obj == b_obj && is_barrel == b_barrel && d < bd)
            }) {
                aimed = Some((*id, [ep[0] as f64, ep[2] as f64], d, is_obj, is_barrel));
            }
        }
        let aimed = aimed.map(|(id, ep, d, _, _)| (id, ep, d));
        let aimed_cp = self
            .capture_points
            .iter()
            .copied()
            .filter(|cp_id| {
                self.props
                    .get(cp_id)
                    .is_some_and(|cp| cp.active == Some(true))
                    && self
                        .capture_point_state
                        .get(cp_id)
                        .is_some_and(|st| st.owner != leader.army || st.height < 100.0)
            })
            .filter_map(|cp_id| {
                let cp_pos = *positions.get(&cp_id)?;
                let dx = cp_pos[0] as f64 - p[0] as f64;
                let dz = cp_pos[2] as f64 - p[2] as f64;
                let d = dx.hypot(dz);
                let cos = if d > 0.05 {
                    (dx * dir[0] + dz * dir[1]) / d
                } else {
                    1.0
                };
                ((d <= 60.0 && cos >= 0.80) || d <= 30.0).then_some((
                    cp_id,
                    [cp_pos[0] as f64, cp_pos[2] as f64],
                    d,
                ))
            })
            .min_by(|a, b| a.2.total_cmp(&b.2))
            .filter(|&(_, _, cp_d)| {
                aimed.is_none_or(|(aid, _, ad)| {
                    cp_d <= ad || (self.buildings.contains(&aid) && cp_d <= 35.0)
                })
            });
        if aimed_cp.is_none() {
            let aimed_garrison = self
                .garrisonable_buildings
                .iter()
                .copied()
                .filter(|b_id| {
                    !self.capture_points.contains(b_id)
                        && self
                            .passengers
                            .get(b_id)
                            .is_none_or(|p| p.iter().all(|id| id.0 == 0))
                        && self.entering_vehicle.values().all(|&c| c != *b_id)
                        && self.props.get(b_id).is_some_and(|b| {
                            b.active == Some(true)
                                && b.health > 0.0
                                && !b.invulnerable
                                && (b.army == "eNeutral"
                                    || self.same_allegiance(&b.army, &leader.army))
                        })
                })
                .filter_map(|b_id| {
                    let b_pos = *positions.get(&b_id)?;
                    let dx = b_pos[0] as f64 - p[0] as f64;
                    let dz = b_pos[2] as f64 - p[2] as f64;
                    let d = dx.hypot(dz);
                    let has_marker = self.objective_attachment.iter().any(|(m, &t)| {
                        t == b_id && self.objective_visibility.get(m) == Some(&Some(true))
                    });
                    let aimed_directly = aimed.is_none()
                        && d > 1e-6
                        && d <= 45.0
                        && (dx * aim[0] + dz * aim[2]) / d >= 0.94;
                    ((has_marker && d <= 60.0) || aimed_directly).then_some((
                        b_id,
                        [b_pos[0] as f64, b_pos[2] as f64],
                        d,
                    ))
                })
                .min_by(|a, b| a.2.total_cmp(&b.2));
            if let Some((b_id, b_xz, _)) = aimed_garrison
                && let Some(inf_id) = self
                    .roster
                    .iter()
                    .copied()
                    .filter(|&id| {
                        id != player
                            && !self.carrier_of.contains_key(&id)
                            && !self.entering_vehicle.contains_key(&id)
                            && self
                                .bodies
                                .get(&id)
                                .is_some_and(|b| b.radius == crate::unit_collision::Body::INFANTRY.radius)
                            && self
                                .units
                                .get(&id)
                                .is_some_and(|u| u.active == Some(true) && u.health > 0.0)
                            && self
                                .activity
                                .get(&id)
                                .is_none_or(|a| a.enabled != Some(false))
                    })
                    .min_by(|a, b| {
                        let da = positions
                            .get(a)
                            .map_or(f64::INFINITY, |q| (q[0] as f64 - b_xz[0]).hypot(q[2] as f64 - b_xz[1]));
                        let db = positions
                            .get(b)
                            .map_or(f64::INFINITY, |q| (q[0] as f64 - b_xz[0]).hypot(q[2] as f64 - b_xz[1]));
                        da.total_cmp(&db)
                    })
            {
                self.entering_vehicle.insert(inf_id, b_id);
                self.squad_follow.remove(&inf_id);
                self.follow_unit_orders.remove(&inf_id);
                self.following.remove(&inf_id);
                self.orders.insert(
                    inf_id,
                    MoveOrder {
                        target: b_xz,
                        radius: 4.0,
                    },
                );
                return Ok(());
            }
        }
        let best_enemy = aimed.or(best_enemy);
        self.squad_target = if aimed_cp.is_some() {
            None
        } else {
            best_enemy.map(|(id, _, _)| id)
        };
        let (target, base_radius) = if let Some((_, cp_xz, _)) = aimed_cp {
            (cp_xz, 2.5)
        } else if let Some((eid, ep, _)) = best_enemy {
            let r = if self.buildings.contains(&eid) {
                20.0
            } else {
                11.0
            };
            (ep, r)
        } else {
            (
                [p[0] as f64 + dir[0] * 18.0, p[2] as f64 + dir[1] * 18.0],
                2.5,
            )
        };
        let has_ground = self.collision.as_ref().is_some_and(|s| s.ground.is_some());
        for id in self.roster.clone() {
            if id != player
                && self.can_squad_follow(id, player)
                && self
                    .units
                    .get(&id)
                    .is_some_and(|u| u.active == Some(true) && u.health > 0.0)
                && self
                    .activity
                    .get(&id)
                    .is_none_or(|a| a.enabled != Some(false))
            {
                if has_ground
                    && let Some(qp) = positions.get(&id)
                    && (qp[0] as f64 - p[0] as f64).hypot(qp[2] as f64 - p[2] as f64) > 120.0
                    && (qp[0] as f64 - target[0]).hypot(qp[2] as f64 - target[1]) > 120.0
                {
                    continue;
                }
                let radius = if aimed_cp.is_none()
                    && best_enemy.is_some()
                    && self.bodies.get(&id).is_some_and(|b| b.radius > 1.2)
                {
                    26.0
                } else {
                    base_radius
                };
                if let Some((cp_id, _, _)) = aimed_cp {
                    if !self
                        .bodies
                        .get(&id)
                        .is_some_and(|b| b.radius == crate::unit_collision::Body::INFANTRY.radius)
                    {
                        continue;
                    }
                    if self.carrier_of.get(&id) == Some(&cp_id) {
                        continue;
                    }
                    self.entering_vehicle.insert(id, cp_id);
                } else {
                    if self
                        .carrier_of
                        .get(&id)
                        .is_some_and(|c| self.capture_points.contains(c))
                    {
                        continue;
                    }
                    self.entering_vehicle.remove(&id);
                }
                if self.water_units.contains(&id) && self.water_units.contains(&player) {
                    self.squad_follow.insert(id);
                    continue;
                }
                self.squad_follow.remove(&id);
                self.follow_unit_orders.remove(&id);
                self.following.remove(&id);
                let active_route = self
                    .routes
                    .get(&id)
                    .is_some_and(|r| !r.points.is_empty() || r.pending);
                if self.orders.get(&id).is_none_or(|o| {
                    if active_route {
                        (o.target[0] - target[0]).hypot(o.target[1] - target[1]) > 15.0
                    } else {
                        (o.target[0] - target[0]).hypot(o.target[1] - target[1]) > 1.5
                            || (o.radius - radius).abs() > 0.5
                    }
                }) {
                    self.orders.insert(id, MoveOrder { target, radius });
                }
            }
        }
        Ok(())
    }
    pub fn dismiss_squad(&mut self) -> Result<()> {
        if self.rewrite.is_none() {
            return Err("Squad dismiss requires rewrite gameplay".into());
        }
        let player = self.player.ok_or("Missing player")?;
        self.squad_target = None;
        for id in self.roster.clone() {
            if id != player {
                self.squad_follow.remove(&id);
                self.follow_unit_orders.remove(&id);
                self.orders.remove(&id);
                self.routes.remove(&id);
            }
        }
        Ok(())
    }
    fn advance_squad_follow(&mut self, positions: &BTreeMap<ObjectId, [f32; 3]>) -> Result<()> {
        if let Some(st) = self.squad_target {
            let alive = self
                .entity(st)
                .is_ok_and(|u| u.active == Some(true) && u.health > 0.0 && !u.invulnerable);
            if !alive {
                self.squad_target = None;
                if let Some(player) = self.player {
                    for id in self.roster.clone() {
                        if id != player
                            && !self.entering_vehicle.contains_key(&id)
                            && !self.carrier_of.contains_key(&id)
                            && self.can_squad_follow(id, player)
                            && self
                                .units
                                .get(&id)
                                .is_some_and(|u| u.active == Some(true) && u.health > 0.0)
                            && self
                                .activity
                                .get(&id)
                                .is_none_or(|a| a.enabled != Some(false))
                        {
                            self.squad_follow.insert(id);
                            self.following.remove(&id);
                        }
                    }
                }
            }
        }
        if self.squad_follow.is_empty() || self.frozen {
            return Ok(());
        }
        let player = self.player.ok_or("Missing squad leader")?;
        let p = *positions
            .get(&player)
            .ok_or("Missing squad leader position")?;
        if p[0].hypot(p[2]) > 1400.0 {
            return Ok(());
        }
        let move_dir = self.direct_velocity.get(&player).and_then(|v| {
            let speed = v[0].hypot(v[1]);
            (speed > 0.1).then(|| [v[0] / speed, v[1] / speed])
        });
        if let Some(dir) = move_dir {
            let blended = match self.squad_formation_dir {
                Some(prev) => {
                    let bx = prev[0] * 0.85 + dir[0] * 0.15;
                    let bz = prev[1] * 0.85 + dir[1] * 0.15;
                    let len = bx.hypot(bz);
                    if len > 1e-4 {
                        [bx / len, bz / len]
                    } else {
                        dir
                    }
                }
                None => dir,
            };
            self.squad_formation_dir = Some(blended);
        } else if self.squad_formation_dir.is_none() {
            self.squad_formation_dir = self
                .headings
                .get(&player)
                .map(|&h| [h.sin() as f64, h.cos() as f64])
                .or_else(|| {
                    self.player_aim.and_then(|a| {
                        let l = a[0].hypot(a[2]);
                        (l > 1e-3).then_some([a[0] / l, a[2] / l])
                    })
                });
        }
        let leader_extra = self
            .bodies
            .get(&player)
            .map_or(0.0, |b| (b.radius - 0.6).max(0.0));
        let followers: Vec<ObjectId> = self.squad_follow.iter().copied().collect();
        let player_is_aa_or_at =
            (self.unit_gtype_flags(player) & (0x0040_0000 | 0x0000_7400 | 0x0200_0000)) != 0;
        let mut front_slots = Vec::new();
        let mut rear_slots = Vec::new();
        for &fid in &followers {
            if fid != player
                && self
                    .units
                    .get(&fid)
                    .is_some_and(|u| u.active == Some(true) && u.health > 0.0)
                && self.can_squad_follow(fid, player)
                && self.bodies.get(&fid).is_some_and(|b| b.radius <= 0.8)
            {
                let is_aa_or_at =
                    (self.unit_gtype_flags(fid) & (0x0040_0000 | 0x0000_7400 | 0x0200_0000)) != 0;
                if is_aa_or_at == player_is_aa_or_at {
                    front_slots.push(fid);
                } else {
                    rear_slots.push(fid);
                }
            }
        }
        for (idx, id) in followers.iter().copied().enumerate() {
            let unit = self.units.get(&id).ok_or("Missing squad member")?;
            if id == player
                || unit.health <= 0.0
                || unit.active != Some(true)
                || !self.can_squad_follow(id, player)
            {
                self.squad_follow.remove(&id);
                if !self.can_squad_follow(id, player) {
                    self.orders.remove(&id);
                }
                continue;
            }
            if self
                .activity
                .get(&id)
                .is_some_and(|a| a.enabled == Some(false))
                || self.attack_orders.contains_key(&id)
            {
                continue;
            }
            let q = *positions.get(&id).ok_or("Missing squad member position")?;
            let mut p = p;
            if self.non_ground_units.contains(&player)
                && !self.water_units.contains(&player)
                && !self.non_ground_units.contains(&id)
                && let Some(player_army) = self.units.get(&player).map(|u| u.army.as_str())
            {
                let defended_bldg = self
                    .buildings
                    .iter()
                    .filter(|&&bid| {
                        self.props.get(&bid).is_some_and(|u| {
                            u.active == Some(true)
                                && u.health > 0.0
                                && self.same_allegiance(&u.army, player_army)
                        })
                    })
                    .filter_map(|&bid| {
                        let bp = *positions.get(&bid)?;
                        let dp = (bp[0] - p[0]).hypot(bp[2] - p[2]);
                        let dq = (bp[0] - q[0]).hypot(bp[2] - q[2]);
                        (dp <= 280.0 && dq <= 220.0).then_some((bp, dp))
                    })
                    .min_by(|a, b| a.1.total_cmp(&b.1));
                if let Some((bp, _)) = defended_bldg {
                    p[0] = bp[0];
                    p[2] = bp[2];
                }
            }
            let rel = [q[0] as f64 - p[0] as f64, q[2] as f64 - p[2] as f64];
            let body_radius = self
                .bodies
                .get(&id)
                .map(|b| b.radius)
                .or_else(|| self.non_ground_units.contains(&id).then_some(4.0))
                .ok_or("Missing squad movement body")?;
            let aim_dir = self.player_aim.and_then(|a| {
                let l = a[0].hypot(a[2]);
                (l > 1e-3).then_some([a[0] / l, a[2] / l])
            });
            let (target, radius) = if self.interactive_squad_formation {
                let form_dir = self
                    .squad_formation_dir
                    .or(aim_dir)
                    .unwrap_or([0.0, 1.0]);
                let right = [-form_dir[1], form_dir[0]];
                if let Some(dir) = move_dir
                    && rel[0].hypot(rel[1]) < 3.0 + leader_extra + body_radius
                    && rel[0] * dir[0] + rel[1] * dir[1] > 0.1
                {
                    let right_m = [-dir[1], dir[0]];
                    let side = if rel[0] * right_m[0] + rel[1] * right_m[1] >= 0.0 {
                        1.0
                    } else {
                        -1.0
                    };
                    let step_aside = 2.6 + leader_extra + body_radius;
                    (
                        [
                            p[0] as f64 - dir[0] * 2.2 + right_m[0] * side * step_aside,
                            p[2] as f64 - dir[1] * 2.2 + right_m[1] * side * step_aside,
                        ],
                        body_radius.max(1.2),
                    )
                } else if self.water_units.contains(&id) {
                    let row = idx / 2;
                    let col = idx % 2;
                    let spacing = (body_radius * 2.4 + leader_extra + 10.0).max(14.0);
                    let along = -(spacing + (row as f64) * spacing * 0.9);
                    let lateral = if followers.len() == 1 {
                        0.0
                    } else if col == 0 {
                        -spacing * 0.65
                    } else {
                        spacing * 0.65
                    };
                    (
                        [
                            p[0] as f64 + form_dir[0] * along + right[0] * lateral,
                            p[2] as f64 + form_dir[1] * along + right[1] * lateral,
                        ],
                        body_radius + leader_extra + 2.5,
                    )
                } else if body_radius <= 0.8 {
                    let cols = 4usize;
                    let row = idx / cols;
                    let col = idx % cols;
                    let cols_in_row = followers.len().saturating_sub(row * cols).min(cols);
                    let along = -(3.2 + leader_extra + (row as f64) * 2.5);
                    let lateral =
                        ((col as f64) - (cols_in_row.saturating_sub(1) as f64) * 0.5) * 2.5;
                    (
                        [
                            p[0] as f64 + form_dir[0] * along + right[0] * lateral,
                            p[2] as f64 + form_dir[1] * along + right[1] * lateral,
                        ],
                        1.35,
                    )
                } else {
                    let row = idx / 2;
                    let col = idx % 2;
                    let spacing = (body_radius * 2.4 + leader_extra + 4.5).max(7.5);
                    let along = -(spacing + (row as f64) * spacing * 0.9);
                    let lateral = if followers.len() == 1 {
                        0.0
                    } else if col == 0 {
                        -spacing * 0.6
                    } else {
                        spacing * 0.6
                    };
                    (
                        [
                            p[0] as f64 + form_dir[0] * along + right[0] * lateral,
                            p[2] as f64 + form_dir[1] * along + right[1] * lateral,
                        ],
                        (body_radius + 1.8).max(3.2),
                    )
                }
            } else if let Some(dir) = move_dir
                && rel[0].hypot(rel[1]) < 3.2 + leader_extra
                && rel[0] * dir[0] + rel[1] * dir[1] > -0.5
            {
                let right = [-dir[1], dir[0]];
                let side = if rel[0] * right[0] + rel[1] * right[1] >= 0.0 {
                    1.0
                } else {
                    -1.0
                };
                let along = if rel[0] * dir[0] + rel[1] * dir[1] >= 0.0 {
                    2.5 + leader_extra
                } else {
                    -(2.5 + leader_extra)
                };
                (
                    [
                        p[0] as f64 + dir[0] * along + right[0] * side * (2.2 + leader_extra),
                        p[2] as f64 + dir[1] * along + right[1] * side * (2.2 + leader_extra),
                    ],
                    body_radius,
                )
            } else if move_dir.is_none()
                && leader_extra == 0.0
                && body_radius <= 0.8
                && let Some(dir) = aim_dir
            {
                let right = [-dir[1], dir[0]];
                let (along, rank_i, rank_n) =
                    if let Some(pos_i) = front_slots.iter().position(|&x| x == id) {
                        (3.0, pos_i, front_slots.len())
                    } else if let Some(pos_i) = rear_slots.iter().position(|&x| x == id) {
                        (-2.5, pos_i, rear_slots.len())
                    } else {
                        (-2.0, idx, followers.len())
                    };
                let slot = (rank_i as f64 - (rank_n.saturating_sub(1)) as f64 * 0.5) * 4.2;
                (
                    [
                        p[0] as f64 + dir[0] * along + right[0] * slot,
                        p[2] as f64 + dir[1] * along + right[1] * slot,
                    ],
                    1.2,
                )
            } else {
                let angle =
                    (idx as f64) * std::f64::consts::TAU / (followers.len().max(1) as f64);
                let spread = if self.water_units.contains(&id) {
                    12.0
                } else if body_radius <= 0.8 {
                    2.8
                } else {
                    0.0
                };
                (
                    [
                        p[0] as f64 + angle.cos() * spread,
                        p[2] as f64 + angle.sin() * spread,
                    ],
                    body_radius + leader_extra + 2.0,
                )
            };
            let dist_to_target = (q[0] as f64 - target[0]).hypot(q[2] as f64 - target[1]);
            if dist_to_target > 75.0
                && self.water_units.contains(&player)
                && self.water_units.contains(&id)
                && let Some(scene) = &self.collision
                && scene
                    .terrain_height(target[0] as f32, target[1] as f32)
                    .is_some_and(|y| y <= self.water_height + 2.0)
            {
                let sub_y = self.submarines.get(&id).map_or(0.0, |s| -s.depth);
                self.pending_positions
                    .insert(id, [target[0] as f32, sub_y, target[1] as f32]);
                self.orders.insert(id, MoveOrder { target, radius });
                continue;
            }
            if dist_to_target > 135.0
                && !self.non_ground_units.contains(&player)
                && !self.non_ground_units.contains(&id)
                && self.routes.get(&id).is_some_and(|r| r.pending && r.planned)
                && let Some(scene) = &self.collision
                && scene.ground.is_some()
                && let Some(&body) = self.bodies.get(&id)
                && let Ok(free) = scene.find_free_near(
                    [target[0] as f32, 0.0, target[1] as f32],
                    body,
                    &[],
                    14.0,
                )
            {
                self.pending_positions.insert(id, free);
                self.routes.remove(&id);
                self.orders.insert(id, MoveOrder { target, radius });
                continue;
            }
            let active_route = self
                .routes
                .get(&id)
                .is_some_and(|r| !r.points.is_empty() || r.pending);
            let detour_route = self
                .routes
                .get(&id)
                .is_some_and(|r| r.points.len() > 2);
            let wake_thresh = if self.interactive_squad_formation && !self.orders.contains_key(&id)
            {
                radius + 1.8
            } else {
                radius
            };
            if dist_to_target > wake_thresh
                && self.orders.get(&id).is_none_or(|o| {
                    if active_route && move_dir.is_some() {
                        let thresh = if detour_route { 25.0 } else { 15.0 };
                        (o.target[0] - target[0]).hypot(o.target[1] - target[1]) > thresh
                    } else if self.interactive_squad_formation {
                        (o.target[0] - target[0]).hypot(o.target[1] - target[1]) > 2.4
                            || (o.radius - radius).abs() > 0.6
                    } else {
                        (o.target[0] - target[0]).hypot(o.target[1] - target[1]) > 1.5
                            || (o.radius - radius).abs() > 0.5
                    }
                })
            {
                self.orders.insert(id, MoveOrder { target, radius });
            }
        }
        Ok(())
    }
    /// Apply original pickup contact rules (0x800c39cc) for active pickups.
    /// Caller stages the World; errors leave no partial collection.
    pub fn advance_pickups(&mut self, positions: &BTreeMap<ObjectId, [f32; 3]>) -> Result<()> {
        let Some(pickups) = &self.pickups else {
            return Ok(());
        };
        let active: Vec<(ObjectId, crate::mission_pickup::Definition)> =
            pickups.active().map(|(id, d)| (*id, d.clone())).collect();
        if active.is_empty() {
            return Ok(());
        }
        let objects = self
            .rewrite
            .as_ref()
            .ok_or("Pickups require rewrite")?
            .objects();
        for (pid, def) in active {
            let mut taker = None;
            for (uid, unit) in &self.units {
                if unit.active != Some(true) || unit.health <= 0.0 {
                    continue;
                }
                let (Some(body), Some(p)) = (self.bodies.get(uid), positions.get(uid)) else {
                    continue;
                };
                let d = (p[0] as f64 - def.position[0] as f64)
                    .hypot(p[2] as f64 - def.position[2] as f64);
                if d > body.radius + crate::mission_pickup::CONTACT_RADIUS {
                    continue;
                }
                let kind = objects.get(uid).map(|o| o.kind.as_str()).unwrap_or("");
                let troop = kind == "cTroop";
                let vehicle = matches!(kind, "cGroundVehicle" | "cAirVehicle" | "cWaterVehicle");
                if unit.max_health <= 0.0 {
                    return Err("Invalid pickup recipient max health".into());
                }
                let percent = unit.health / unit.max_health * 100.0;
                if let Some(effect) = crate::mission_pickup::effect(
                    &def,
                    troop,
                    vehicle,
                    percent,
                    self.player == Some(*uid),
                ) {
                    taker = Some((*uid, effect));
                    break;
                }
            }
            if let Some((uid, effect)) = taker {
                match effect {
                    crate::mission_pickup::Effect::Heal(amount) => {
                        let u = self.units.get_mut(&uid).unwrap();
                        u.health = (u.health + amount).min(u.max_health);
                    }
                    crate::mission_pickup::Effect::GoldCoin => {
                        self.pickups.as_mut().unwrap().gold_coins += 1;
                    }
                }
                let p = self.pickups.as_mut().unwrap();
                p.states
                    .insert(pid, crate::mission_pickup::State::Collected);
                p.collected += 1;
            }
        }
        Ok(())
    }
    /// Rust-owned spawn/despawn transaction; does not resurrect dead entities.
    pub fn set_rewrite_presence(
        &mut self,
        id: ObjectId,
        present: bool,
        retain: bool,
    ) -> Result<()> {
        if self.rewrite.is_none() {
            return Err("Rewrite lifecycle not configured".into());
        }
        if !self.units.contains_key(&id)
            && !self.buildings.contains(&id)
            && !self.props.contains_key(&id)
        {
            return Err("Unsupported lifecycle entity type".into());
        }
        if present && self.entity(id)?.health <= 0.0 {
            return Err("Dead entity resurrection unsupported".into());
        }
        if self.capture_points.contains(&id) && !self.capture_sessions.contains_key(&id) {
            return Err("Missing capture session".into());
        }
        let mut staged = self.clone();
        if staged.props.contains_key(&id)
            && !staged.rewrite.as_ref().unwrap().retired.contains(&id)
            && let Some(scene) = &mut staged.collision
            && (scene.collision_owner(id).is_some()
                || (staged.buildings.contains(&id) && scene.ground.is_none()))
        {
            std::sync::Arc::make_mut(scene).set_object_present(id, present)?;
        }
        staged.entity_mut(id)?.active = Some(present);
        if !present {
            if let Some(session) = staged.capture_sessions.get_mut(&id) {
                *session = CaptureSession::default();
            }
            staged.despawn_retention.insert(id, retain);
            staged.squad_follow.remove(&id);
            staged.command_selection.remove(&id);
            if !retain {
                staged.roster.remove(&id);
            }
            if staged.player == Some(id) && staged.player_lifecycle.is_some() {
                staged.player_lifecycle = Some(PlayerLifecycle::ReplacementRequired);
            }
            staged.orders.remove(&id);
            staged.following.remove(&id);
            staged.routes.remove(&id);
            staged.direct_velocity.remove(&id);
            staged.attack_orders.remove(&id);
            staged.attack_orders.retain(|_, t| *t != id);
        }
        *self = staged;
        Ok(())
    }
    pub fn from_graph(graph: &ObjectGraph) -> Result<Self> {
        let mut w = Self {
            water_height: 42.5,
            ..Self::default()
        };
        for o in graph.objects.values() {
            if o.kind == "cRenderParams"
                && let Some(raw) = field(o, "mWaterHeight")
                && let Ok(val) = raw.parse::<f32>()
                && val.is_finite()
            {
                w.water_height = val;
            }
            if matches!(o.kind.as_str(), "cAirVehicle" | "cWaterVehicle") {
                w.non_ground_units.insert(o.id);
            }
            if matches!(
                o.kind.as_str(),
                "cTroop"
                    | "cGroundVehicle"
                    | "cAirVehicle"
                    | "cWaterVehicle"
                    | "cBuilding"
                    | "cMorphingBuilding"
                    | "cCapturePoint"
            ) || (o.kind == "cDestroyableObject" && field(o, "mBase") != Some("0"))
            {
                w.activity.insert(o.id, Activity::default());
            } else if matches!(
                o.kind.as_str(),
                "cAmbientAreaPointSoundSphere" | "cAmbientAreaPointSound"
            ) {
                let flags = field(o, "mFlags")
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(0);
                w.activity.insert(
                    o.id,
                    Activity {
                        enabled: Some(flags & 4 == 0),
                        weapons_enabled: None,
                    },
                );
            }
            if let Some(f) = o
                .fields
                .iter()
                .find(|f| f.attributes.get("name").map(String::as_str) == Some("mPassenger"))
            {
                let mut list = Vec::new();
                for item in &f.items {
                    if let Ok(pid) = item.trim().parse::<u64>() {
                        let passenger = ObjectId(pid);
                        list.push(passenger);
                        if pid != 0 {
                            w.carrier_of.insert(passenger, o.id);
                        }
                    }
                }
                while list.last().is_some_and(|id| id.0 == 0) {
                    list.pop();
                }
                if !list.is_empty() {
                    w.passengers.insert(o.id, list);
                }
            }
            if o.kind == "cReflectedUnitGroup" {
                let mut list = Vec::new();
                if let Some(f) = o
                    .fields
                    .iter()
                    .find(|f| f.attributes.get("name").map(String::as_str) == Some("mUnits"))
                {
                    for item in &f.items {
                        if let Ok(uid) = item.trim().parse::<u64>()
                            && uid != 0
                            && !list.contains(&ObjectId(uid))
                        {
                            list.push(ObjectId(uid));
                        }
                    }
                }
                w.unit_groups.insert(o.id, list);
            }
            if o.kind == "cTroop" {
                w.bodies.insert(o.id, crate::unit_collision::Body::INFANTRY);
            }
            if o.kind == "cObjective" {
                let flags = field(o, "mFlags")
                    .ok_or("Missing objective flags")?
                    .parse::<u32>()
                    .map_err(|_| "Invalid objective flags")?;
                w.objectives.insert(o.id, (flags & 1 != 0, flags & 2 != 0));
            }
            if o.kind == "cObjectiveMarker" {
                w.objective_visibility.insert(o.id, None);
            }
            if matches!(
                o.kind.as_str(),
                "cTroop" | "cGroundVehicle" | "cAirVehicle" | "cWaterVehicle"
            ) {
                let base = field(o, "mBase")
                    .ok_or("Unit missing base")?
                    .parse::<u64>()
                    .map_err(|_| "Invalid unit base")?;
                let base = graph
                    .objects
                    .get(&ObjectId(base))
                    .ok_or("Unresolved unit base")?;
                let max = field(base, "mMaxHealth")
                    .ok_or("Missing unit health")?
                    .parse::<f64>()
                    .map_err(|_| "Invalid health")?;
                if !max.is_finite() || max <= 0.0 {
                    return Err("Invalid maximum health".into());
                }
                w.units.insert(
                    o.id,
                    Unit {
                        health: max,
                        max_health: max,
                        invulnerable: false,
                        active: None,
                        army: field(base, "mArmy").ok_or("Missing army")?.into(),
                        forward_speed: field(base, "ForwardSpeed")
                            .map(|s| {
                                s.parse::<f64>()
                                    .map_err(|_| "Invalid forward speed".to_string())
                            })
                            .transpose()?
                            .or_else(|| (o.kind == "cWaterVehicle").then_some(14.0)),
                        animation: if o.kind == "cTroop" {
                            Some(crate::unit_animation::Player::from_base(base, graph)?)
                        } else {
                            None
                        },
                    },
                );
                if field(o, "mRevivable") == Some("eTrue") {
                    let revive_time = field(base, "mReviveTime")
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(30.0);
                    w.revivable_units.insert(o.id, revive_time);
                }
            } else if matches!(
                o.kind.as_str(),
                "cBuilding" | "cMorphingBuilding" | "cCapturePoint" | "cDestroyableObject"
            ) {
                if o.kind == "cDestroyableObject" && field(o, "mBase") == Some("0") {
                    continue;
                }
                if o.kind != "cDestroyableObject" {
                    w.buildings.insert(o.id);
                }
                let base =
                    crate::original_scene::resolve_base(graph, o).ok_or("Unresolved prop base")?;
                if matches!(o.kind.as_str(), "cBuilding" | "cMorphingBuilding")
                    && base
                        .fields
                        .iter()
                        .find(|f| f.attributes.get("name").is_some_and(|n| n == "mSeatData"))
                        .is_some_and(|f| f.items.iter().any(|v| v != "0"))
                {
                    w.garrisonable_buildings.insert(o.id);
                }
                if o.kind == "cCapturePoint" {
                    w.capture_points.insert(o.id);
                    let cur = field(o, "mCurrentFlag").unwrap_or("0");
                    let flag = |k: &str| field(o, k).or_else(|| field(base, k));
                    let (owner, height) = if cur == "0" || Some(cur) == flag("mFlagNeutral") {
                        ("eNeutral".to_string(), 0.0)
                    } else if Some(cur) == flag("mFlagSolar") {
                        ("eSolarEmpire".to_string(), 100.0)
                    } else if Some(cur) == flag("mFlagAnglo") {
                        ("eAngloIsles".to_string(), 100.0)
                    } else if Some(cur) == flag("mFlagWF") || Some(cur) == flag("mFlagGood") {
                        ("eWesternFrontier".to_string(), 100.0)
                    } else if Some(cur) == flag("mFlagTundran") {
                        ("eTundranTerritories".to_string(), 100.0)
                    } else if Some(cur) == flag("mFlagXylvanian") || Some(cur) == flag("mFlagBad") {
                        ("eXylvanian".to_string(), 100.0)
                    } else if Some(cur) == flag("mFlagUnderworld")
                        || Some(cur) == flag("mFlagUnderWorld")
                    {
                        ("eUnderWorld".to_string(), 100.0)
                    } else {
                        ("eNeutral".to_string(), 0.0)
                    };
                    w.capture_point_state.insert(
                        o.id,
                        CapturePointState {
                            owner,
                            capturing_army: None,
                            height,
                            direction: 0,
                            installation: None,
                        },
                    );
                }
                let key = if o.kind == "cDestroyableObject" {
                    "MaxHealth"
                } else {
                    "mMaxHealth"
                };
                let max = field(base, key)
                    .ok_or("Missing prop health")?
                    .parse::<f64>()
                    .map_err(|_| "Invalid prop health")?;
                if !max.is_finite() || max <= 0.0 {
                    return Err("Invalid prop maximum health".into());
                }
                let army = if matches!(o.kind.as_str(), "cBuilding" | "cMorphingBuilding") {
                    field(o, "mAllegiance")
                        .filter(|a| !a.is_empty() && *a != "eNeutral" && *a != "eNoArmy")
                        .or_else(|| field(base, "mArmy"))
                        .unwrap_or("")
                        .into()
                } else {
                    field(base, "mArmy").unwrap_or("").into()
                };
                let invulnerable = o.kind == "cCapturePoint"
                    || (o.kind == "cDestroyableObject"
                        && (field(base, "ObjFlags")
                            .and_then(|s| s.parse::<u32>().ok())
                            .is_some_and(|f| (f & 0x80) != 0)
                            || field(o, "mInstanceFlags")
                                .and_then(|s| s.parse::<u32>().ok())
                                .is_some_and(|f| (f & 0x80000) != 0)));
                if o.kind == "cDestroyableObject"
                    && !invulnerable
                    && field(base, "CollideType")
                        .and_then(|s| s.parse::<u32>().ok())
                        .is_some_and(|c| (c & 0x0100_0000) != 0)
                {
                    let has_blast_damage = field(base, "ExplodeType")
                        .and_then(|s| s.parse::<u64>().ok())
                        .filter(|&id| id != 0)
                        .and_then(|id| graph.objects.get(&ObjectId(id)))
                        .and_then(|exp| {
                            exp.fields
                                .iter()
                                .find(|f| {
                                    f.attributes
                                        .get("name")
                                        .is_some_and(|n| n == "mDamageAmount")
                                })
                                .map(|f| {
                                    f.items
                                        .iter()
                                        .any(|v| v.parse::<f64>().is_ok_and(|x| x > 0.0))
                                })
                        })
                        .unwrap_or(false);
                    if !has_blast_damage {
                        w.crushable_props.insert(o.id);
                    }
                }
                if invulnerable {
                    w.unit_invulnerable.insert(o.id);
                }
                w.props.insert(
                    o.id,
                    Unit {
                        health: max,
                        max_health: max,
                        invulnerable,
                        active: None,
                        army,
                        forward_speed: None,
                        animation: None,
                    },
                );
            } else if o.kind == "cStrategicInstallation" {
                let ptr = |name: &str| -> Option<ObjectId> {
                    field(o, name)?
                        .parse::<u64>()
                        .ok()
                        .filter(|&id| id != 0)
                        .map(ObjectId)
                };
                let main_building = ptr("mMainBuilding");
                let capture_point = ptr("mCapturePoint");
                let spawn_zone = ptr("mSpawnZone");
                let facility_type = field(o, "mType").unwrap_or_default().to_string();
                let time_between_respawns = field(o, "mTimeBetweenRespawns")
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.5);
                let multi_facility_multiplier = field(o, "mMultiFacilityReviveTimeMultiplier")
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(1.0);
                let mut buildings = Vec::new();
                if let Some(mb) = main_building {
                    buildings.push(mb);
                }
                if let Some(f) = o
                    .fields
                    .iter()
                    .find(|f| f.attributes.get("name").map(String::as_str) == Some("mMGNests"))
                {
                    for item in &f.items {
                        if let Ok(bid) = item.trim().parse::<u64>()
                            && bid != 0
                        {
                            buildings.push(ObjectId(bid));
                        }
                    }
                }
                for i in 1..=8 {
                    if let Some(nest) = ptr(&format!("mMachineGunNest{i}"))
                        && !buildings.contains(&nest)
                    {
                        buildings.push(nest);
                    }
                }
                let owner = main_building
                    .and_then(|mb| graph.objects.get(&mb))
                    .and_then(|mb_obj| field(mb_obj, "mAllegiance"))
                    .unwrap_or("eNeutral")
                    .to_string();
                w.installations.insert(
                    o.id,
                    StrategicInstallation {
                        main_building,
                        capture_point,
                        spawn_zone,
                        facility_type,
                        time_between_respawns,
                        multi_facility_multiplier,
                        respawn_cooldown: 0.0,
                        buildings,
                        owner,
                    },
                );
            } else if matches!(
                o.kind.as_str(),
                "cMapZone" | "cDamageZone" | "cNogoHintZone"
            ) {
                let m = matrix(field(o, "mMatrix").ok_or("Missing zone matrix")?)?;
                let parts = field(o, "mSize")
                    .ok_or("Missing zone size")?
                    .split(',')
                    .map(|v| {
                        v.trim()
                            .parse::<f32>()
                            .map_err(|_| "Invalid zone size".to_string())
                    })
                    .collect::<Result<Vec<_>>>()?;
                let size: [f32; 4] = parts.try_into().map_err(|_| "Invalid zone size length")?;
                let radius = field(o, "mRadius")
                    .ok_or("Missing zone radius")?
                    .parse::<f32>()
                    .map_err(|_| "Invalid zone radius")?;
                if size.iter().any(|v| !v.is_finite() || *v < 0.0)
                    || !radius.is_finite()
                    || radius < 0.0
                {
                    return Err("Invalid map-zone shape".into());
                }
                let zone = Zone {
                    matrix: m,
                    size,
                    radius,
                };
                if o.kind == "cDamageZone" {
                    let dps = field(o, "mDamagePerSecond")
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(0.0);
                    w.damage_zones.insert(o.id, (zone.clone(), dps));
                }
                w.zones.insert(o.id, zone);
            }
        }
        for &bid in &w.buildings {
            if !w.capture_points.contains(&bid)
                && w.props
                    .get(&bid)
                    .is_some_and(|b| b.army.is_empty() || b.army == "eNeutral" || b.army == "eNoArmy")
                && let Some(p_army) = w
                    .passengers
                    .get(&bid)
                    .and_then(|list| list.iter().find_map(|pid| w.units.get(pid).map(|u| u.army.clone())))
                    .filter(|a| !a.is_empty() && a != "eNeutral" && a != "eNoArmy")
                && let Some(b) = w.props.get_mut(&bid)
            {
                b.army = p_army;
            }
        }
        for (&si_id, si) in &w.installations {
            if let Some(mb_id) = si.main_building
                && let Some(mb) = w.props.get_mut(&mb_id)
            {
                mb.invulnerable = true;
                w.unit_invulnerable.insert(mb_id);
            }
            if let Some(cp_id) = si.capture_point
                && let Some(cp) = w.capture_point_state.get_mut(&cp_id)
            {
                cp.installation = Some(si_id);
                if cp.owner == "eNeutral" && si.owner != "eNeutral" {
                    cp.owner = si.owner.clone();
                    cp.height = 100.0;
                }
            }
        }
        Ok(w)
    }
    /// Explicit authored AI state, not an inference from AIUNIT_INACTIVE (which is separate).
    /// Destroyable-object visibility and player ownership remain separately configured.
    pub fn configure_authored_units(&mut self, graph: &ObjectGraph) -> Result<()> {
        let mut staged = self.clone();
        for o in graph.objects.values().filter(|o| {
            matches!(
                o.kind.as_str(),
                "cTroop"
                    | "cGroundVehicle"
                    | "cAirVehicle"
                    | "cWaterVehicle"
                    | "cBuilding"
                    | "cMorphingBuilding"
                    | "cCapturePoint"
            )
        }) {
            let active = match field(o, "State") {
                Some("AI_STATE_NORMAL") => !staged.carrier_of.contains_key(&o.id),
                Some("AI_STATE_UNSPAWNED" | "AI_STATE_IN_UNIT_CARRIER") => false,
                _ => return Err(format!("Unsupported authored AI state on {}", o.id.0)),
            };
            let flags = field(o, "mUnitInstanceFlags")
                .ok_or("Missing unit instance flags")?
                .parse::<u32>()
                .map_err(|_| "Invalid unit instance flags")?;
            staged.entity_mut(o.id)?.active = Some(active);
            if !staged.capture_points.contains(&o.id)
                && (staged.units.contains_key(&o.id) || staged.buildings.contains(&o.id))
            {
                staged.despawn_retention.insert(o.id, false);
            }
            staged.activity.entry(o.id).or_default().enabled = Some(flags & 4 == 0);
            if field(o, "mEnemyTargetableOnly") == Some("eTrue") {
                staged.enemy_targetable_only.insert(o.id);
            } else if staged.units.contains_key(&o.id) && (flags & 0x08) == 0 {
                staged.playable_units.insert(o.id);
            }
        }
        *self = staged;
        Ok(())
    }
    pub fn entity(&self, id: ObjectId) -> Result<&Unit> {
        self.units
            .get(&id)
            .or_else(|| self.props.get(&id))
            .ok_or_else(|| format!("Unknown damageable entity {}", id.0))
    }
    fn entity_mut(&mut self, id: ObjectId) -> Result<&mut Unit> {
        self.units
            .get_mut(&id)
            .or_else(|| self.props.get_mut(&id))
            .ok_or_else(|| format!("Unknown damageable entity {}", id.0))
    }
    /// Resolve the single-player camera's authored target. This does not invent a squad roster.
    pub fn configure_authored_player(&mut self, graph: &ObjectGraph) -> Result<ObjectId> {
        let mut settings = graph
            .objects
            .values()
            .filter(|o| o.kind == "cLevelSettings");
        let level = settings.next().ok_or("Missing level settings")?;
        let num_players = field(level, "mNumPlayers");
        if settings.next().is_some()
            || (num_players != Some("1") && !(self.rewrite.is_some() && num_players == Some("2")))
        {
            return Err("Only unambiguous single-player startup supported".into());
        }
        let army = field(level, "mPlayerArmy1").ok_or("Missing player army")?;
        let mut target = None;
        let mut view = None;
        for camera in graph.objects.values().filter(|o| o.kind == "cCamera") {
            let flags = field(camera, "mInstanceFlags")
                .ok_or("Missing camera flags")?
                .parse::<u32>()
                .map_err(|_| "Invalid camera flags")?;
            if flags & 0x10 == 0 {
                continue;
            }
            if flags & 4 == 0 || field(camera, "mCamType") != Some("eCAMTYPE_CHASETARGET") {
                if self.rewrite.is_none() {
                    return Err("Unsupported/ambiguous player camera".into());
                }
                continue;
            }
            if target.is_some() {
                return Err("Unsupported/ambiguous player camera".into());
            }
            let id = ObjectId(
                field(camera, "mTarget")
                    .ok_or("Missing player-camera target")?
                    .parse::<u64>()
                    .map_err(|_| "Invalid camera target")?,
            );
            let unit = self
                .units
                .get_mut(&id)
                .ok_or("Player camera must target a mobile unit")?;
            if self.rewrite.is_some() && unit.active == Some(false) && unit.army == army {
                unit.active = Some(true);
            }
            if unit.active != Some(true) || unit.health <= 0.0 || unit.army != army {
                return Err("Authored player is absent/dead or belongs to another army".into());
            }
            target = Some(id);
            view = Some(PlayerView {
                camera: camera.id,
                target: id,
                cut_revision: 0,
            });
        }
        let target = target.ok_or("No authored player camera")?;
        self.player_view = view;
        self.player = Some(target);
        self.player_lifecycle = None;
        self.roster.clear();
        if self.rewrite.is_some() {
            self.roster.insert(target);
            let transform =
                matrix(field(&graph.objects[&target], "Mat").ok_or("Missing player transform")?)?;
            self.set_player_aim([
                transform[8] as f64,
                transform[9] as f64,
                transform[10] as f64,
            ])?;
        }
        Ok(target)
    }
    /// RBWE01 player construction initializes +0x18ac to false (0x800f18b4).
    /// Explicit opt-in for a fresh, living, authored player; never initializes a squad.
    pub fn configure_fresh_player_lifecycle(&mut self) -> Result<()> {
        if self.player_lifecycle.is_some() {
            return Err("Player lifecycle already configured".into());
        }
        let id = self.player.ok_or("Player unit not configured")?;
        let unit = self.units.get(&id).ok_or("Unknown player unit")?;
        if unit.active != Some(true)
            || unit.health <= 0.0
            || self.player_view.as_ref().is_none_or(|v| v.target != id)
        {
            return Err("Fresh player requires a living present authored target".into());
        }
        self.player_lifecycle = Some(PlayerLifecycle::FreshAlive(id));
        Ok(())
    }
    pub fn configure_player(&mut self, player: ObjectId, roster: BTreeSet<ObjectId>) -> Result<()> {
        if !roster.contains(&player) || roster.iter().any(|id| !self.units.contains_key(id)) {
            return Err("Invalid controlled-unit roster".into());
        }
        self.player = Some(player);
        self.player_lifecycle = None;
        if self
            .player_view
            .as_ref()
            .is_some_and(|v| v.target != player)
        {
            self.player_view = None;
        }
        self.roster = roster;
        Ok(())
    }
    /// Shared presence term used by three native list consumers. Their other
    /// army/type/eligibility filters are NOT supplied by this predicate.
    pub fn unit_list_presence(&self, id: ObjectId) -> Result<bool> {
        if !self.units.contains_key(&id) && !self.buildings.contains(&id) {
            return Err("List-presence target is not a unit".into());
        }
        match self.entity(id)?.active {
            Some(true) => Ok(true),
            Some(false) => self
                .despawn_retention
                .get(&id)
                .copied()
                .ok_or("Unit retention state not configured".into()),
            None => Err("Unresolved unit presence".into()),
        }
    }
    pub fn damage(&mut self, id: ObjectId, amount: f64) -> Result<()> {
        if !amount.is_finite() || amount < 0.0 {
            return Err("Invalid raw damage".into());
        }
        let u = self.entity_mut(id)?;
        if u.active != Some(true) {
            return Err("Cannot damage inactive/unresolved unit".into());
        }
        if !u.invulnerable {
            let health = (u.health - amount).max(0.0);
            if health == 0.0
                && let Some(animation) = &mut u.animation
            {
                animation.select(crate::unit_animation::Motion::Death)?;
            }
            u.health = health;
        }
        if u.health == 0.0 && self.player == Some(id) && self.player_lifecycle.is_some() {
            self.player_lifecycle = Some(PlayerLifecycle::ReplacementRequired);
        }
        Ok(())
    }
    pub fn follow_waypoint(
        &mut self,
        id: ObjectId,
        start: ObjectId,
        offset: [f64; 2],
        positions: &BTreeMap<ObjectId, [f32; 3]>,
    ) -> Result<()> {
        if self.navigation == Navigation::Unavailable {
            return Err("Waypoint movement backend not configured".into());
        }
        if self.rewrite.is_some()
            && self.despawn_retention.get(&id) == Some(&true)
            && !self.carrier_of.contains_key(&id)
            && let Some(u) = self.units.get_mut(&id)
            && u.active == Some(false)
            && u.health > 0.0
        {
            u.active = Some(true);
        }
        let unit = self.units.get(&id).ok_or("Unknown waypoint follower")?;
        let can_queue = self.rewrite.is_some() && unit.active == Some(false);
        if (!can_queue && unit.active != Some(true))
            || unit.health <= 0.0
            || !positions.contains_key(&id)
        {
            return Err("Unit cannot follow waypoints".into());
        }
        let air_speed = self
            .rewrite
            .is_some()
            .then(|| self.scripted_air_cruise.get(&id).copied())
            .flatten();
        if air_speed.is_none() {
            self.bodies
                .get(&id)
                .ok_or("Waypoint vehicle movement profile unsupported")?
                .validate()?;
        }
        if positions[&id].iter().any(|v| !v.is_finite()) {
            return Err("Invalid waypoint follower position".into());
        }
        if matches!(
            self.navigation,
            Navigation::CollisionAware | Navigation::PlannedInfantry
        ) && self.collision.is_none()
        {
            return Err("Waypoint collision scene not configured".into());
        }
        if !unit
            .forward_speed
            .or(air_speed)
            .is_some_and(|v| v.is_finite() && v > 0.0)
        {
            return Err("Invalid waypoint actor speed".into());
        }
        let bank = self
            .waypoints
            .as_ref()
            .ok_or("Authored waypoint bank not configured")?;
        let route = crate::mission_waypoints::Route::new(bank, start, offset)?;
        let target = route.target(bank)?;
        self.orders.insert(
            id,
            MoveOrder {
                target,
                radius: crate::mission_waypoints::Route::RADIUS,
            },
        );
        self.following.insert(id, route);
        self.bombers.orders.remove(&id);
        self.missiles.orders.remove(&id);
        self.attack_orders.remove(&id);
        self.follow_unit_orders.remove(&id);
        self.squad_follow.remove(&id);
        self.routes.remove(&id);
        self.direct_velocity.remove(&id);
        Ok(())
    }
    fn advance_followers(
        &mut self,
        dt: f64,
        positions: &BTreeMap<ObjectId, [f32; 3]>,
    ) -> Result<()> {
        use crate::mission_waypoints::{Action, Route};
        if self.following.is_empty() {
            return Ok(());
        }
        let bank = self
            .waypoints
            .clone()
            .ok_or("Authored waypoint bank not configured")?;
        for id in self.following.keys().copied().collect::<Vec<_>>() {
            let unit = self.units.get(&id).ok_or("Unknown waypoint follower")?;
            if self.rewrite.is_some() && unit.active == Some(false) && unit.health > 0.0 {
                continue;
            }
            if unit.health <= 0.0
                || unit.active != Some(true)
                || self.direct_velocity.contains_key(&id)
            {
                self.following.remove(&id);
                self.orders.remove(&id);
                self.routes.remove(&id);
                continue;
            }
            if self.frozen && self.player == Some(id) {
                continue;
            }
            let position = *positions
                .get(&id)
                .ok_or("Waypoint follower has no position")?;
            let follower = &self.following[&id];
            let target = follower.target(&bank)?;
            if self
                .orders
                .get(&id)
                .is_some_and(|o| o.target != target || o.radius != Route::RADIUS)
                || !self.orders.contains_key(&id) && !follower.reached(&bank, position)?
            {
                self.following.remove(&id);
                continue;
            }
            let action = self.following.get_mut(&id).unwrap().update_with_rng(
                &bank,
                position,
                dt,
                &mut self.waypoint_rand,
            )?;
            let complete = matches!(action, Action::Complete);
            match action {
                Action::Move(target) => {
                    self.orders.insert(
                        id,
                        MoveOrder {
                            target,
                            radius: Route::RADIUS,
                        },
                    );
                }
                Action::Wait | Action::Complete => {
                    self.orders.remove(&id);
                    self.routes.remove(&id);
                    if let Some(a) = &mut self.units.get_mut(&id).unwrap().animation {
                        a.select(crate::unit_animation::Motion::Idle)?;
                    }
                    if complete {
                        self.following.remove(&id);
                    }
                }
            }
        }
        Ok(())
    }
    fn unload_carrier_passengers(
        &mut self,
        carrier: ObjectId,
        positions: &mut BTreeMap<ObjectId, [f32; 3]>,
    ) {
        if self
            .units
            .get(&carrier)
            .is_some_and(|u| u.active == Some(false) || u.health <= 0.0)
        {
            return;
        }
        let Some(cpos) = positions.get(&carrier).copied() else {
            return;
        };
        let is_air = self
            .rewrite
            .as_ref()
            .and_then(|r| r.objects().get(&carrier).map(|o| o.kind == "cAirVehicle"))
            .unwrap_or(false);
        let list = self.passengers.entry(carrier).or_default().clone();
        // Air transports keep their pilot in slot 0 when unloading cargo with ID_NONE.
        let skip = if is_air && list.len() > 1 { 1 } else { 0 };
        let keep = list[..skip].to_vec();
        let unloading = &list[skip..];
        for (idx, &uid) in unloading.iter().enumerate() {
            if uid.0 == 0 {
                continue;
            }
            self.carrier_of.remove(&uid);
            let angle = (idx as f32) * 1.1;
            let y = if self.non_ground_units.contains(&uid)
                || self.collision.as_ref().is_none_or(|s| s.ground.is_none())
            {
                cpos[1]
            } else {
                0.0
            };
            let mut upos = [cpos[0] + angle.cos() * 3.5, y, cpos[2] + angle.sin() * 3.5];
            if !self.non_ground_units.contains(&uid)
                && let Some(scene) = &self.collision
                && let Some(&body) = self.bodies.get(&uid)
            {
                let others: Vec<_> = self
                    .units
                    .iter()
                    .filter(|(other, u)| {
                        **other != uid
                            && u.active == Some(true)
                            && u.health > 0.0
                            && !self.non_ground_units.contains(other)
                    })
                    .filter_map(|(other, _)| {
                        Some(crate::unit_collision::Other {
                            position: *positions.get(other)?,
                            body: *self.bodies.get(other)?,
                        })
                    })
                    .collect();
                if let Ok(free) = scene.find_free_near(upos, body, &others, 16.0) {
                    upos = free;
                }
            }
            positions.insert(uid, upos);
            self.pending_positions.insert(uid, upos);
            if let Some(u) = self.units.get_mut(&uid)
                && u.health > 0.0
            {
                u.active = Some(true);
            }
        }
        if keep.is_empty() {
            self.passengers.remove(&carrier);
        } else {
            self.passengers.insert(carrier, keep);
        }
    }
    pub(crate) fn replace_dead_player(&mut self, positions: &BTreeMap<ObjectId, [f32; 3]>) {
        if let Some(player) = self.player
            && self.entity(player).is_ok_and(|u| {
                u.health <= 0.0
                    || (u.active == Some(false)
                        && !self.frozen
                        && self.carrier_of.contains_key(&player))
            })
        {
            if let Some(&cid) = self.carrier_of.get(&player)
                && self.units.contains_key(&cid)
                && !self.enemy_targetable_only.contains(&cid)
                && self
                    .units
                    .get(&cid)
                    .is_some_and(|u| u.active == Some(true) && u.health > 0.0)
            {
                self.player = Some(cid);
                self.roster.insert(cid);
                self.squad_follow.remove(&cid);
                self.squad_follow.remove(&player);
                return;
            }
            let ppos = positions.get(&player).copied().unwrap_or([0.0; 3]);
            let player_army = self
                .entity(player)
                .map(|u| u.army.clone())
                .unwrap_or_default();
            let has_ground = self.collision.as_ref().is_some_and(|s| s.ground.is_some());
            let mut candidates: Vec<ObjectId> = self
                .roster
                .iter()
                .copied()
                .filter(|&id| {
                    id != player
                        && !self.enemy_targetable_only.contains(&id)
                        && (self.bodies.contains_key(&id)
                            || self.water_units.contains(&id)
                            || self.flight_profiles.contains_key(&id))
                        && self
                            .units
                            .get(&id)
                            .is_some_and(|u| u.active == Some(true) && u.health > 0.0)
                        && self
                            .activity
                            .get(&id)
                            .is_none_or(|a| a.enabled != Some(false))
                })
                .collect();
            if candidates.is_empty() && has_ground {
                candidates = self
                    .units
                    .iter()
                    .filter(|&(id, u)| {
                        *id != player
                            && !self.enemy_targetable_only.contains(id)
                            && u.army == player_army
                            && (self.bodies.contains_key(id)
                                || self.water_units.contains(id)
                                || self.flight_profiles.contains_key(id))
                            && u.active == Some(true)
                            && u.health > 0.0
                            && self
                                .activity
                                .get(id)
                                .is_none_or(|a| a.enabled != Some(false))
                    })
                    .map(|(id, _)| *id)
                    .collect();
            }
            if candidates.is_empty() {
                candidates = self
                    .roster
                    .iter()
                    .copied()
                    .filter(|&id| {
                        id != player
                            && !self.enemy_targetable_only.contains(&id)
                            && (self.bodies.contains_key(&id)
                                || self.water_units.contains(&id)
                                || self.flight_profiles.contains_key(&id))
                            && self.units.get(&id).is_some_and(|u| {
                                u.health > 0.0
                                    && (self.despawn_retention.get(&id) == Some(&true)
                                        || self.carrier_of.get(&id).is_some_and(|c| {
                                            self.buildings.contains(c)
                                                && !self.capture_points.contains(c)
                                        }))
                            })
                            && self
                                .activity
                                .get(&id)
                                .is_none_or(|a| a.enabled != Some(false))
                    })
                    .collect();
            }
            let player_gtype = self.unit_gtype_flags(player);
            let player_domain = if self.water_units.contains(&player) {
                1u8
            } else if self.flight_profiles.contains_key(&player) {
                2u8
            } else {
                0u8
            };
            let next = candidates.into_iter().min_by(|a, b| {
                let a_active = self.units.get(a).is_some_and(|u| u.active == Some(true));
                let b_active = self.units.get(b).is_some_and(|u| u.active == Some(true));
                match (a_active, b_active) {
                    (true, false) => return std::cmp::Ordering::Less,
                    (false, true) => return std::cmp::Ordering::Greater,
                    _ => {}
                }
                let a_dom = if self.water_units.contains(a) {
                    1u8
                } else if self.flight_profiles.contains_key(a) {
                    2u8
                } else {
                    0u8
                };
                let b_dom = if self.water_units.contains(b) {
                    1u8
                } else if self.flight_profiles.contains_key(b) {
                    2u8
                } else {
                    0u8
                };
                match (a_dom == player_domain, b_dom == player_domain) {
                    (true, false) => return std::cmp::Ordering::Less,
                    (false, true) => return std::cmp::Ordering::Greater,
                    _ => {}
                }
                let da = positions
                    .get(a)
                    .map_or(f32::INFINITY, |q| (q[0] - ppos[0]).hypot(q[2] - ppos[2]));
                let db = positions
                    .get(b)
                    .map_or(f32::INFINITY, |q| (q[0] - ppos[0]).hypot(q[2] - ppos[2]));
                if has_ground {
                    let a_veh = da <= 120.0 && self.bodies.get(a).is_some_and(|b| b.radius > 1.2);
                    let b_veh = db <= 120.0 && self.bodies.get(b).is_some_and(|b| b.radius > 1.2);
                    match (a_veh, b_veh) {
                        (true, false) => return std::cmp::Ordering::Less,
                        (false, true) => return std::cmp::Ordering::Greater,
                        _ => {}
                    }
                    if da <= 80.0 && db <= 80.0 {
                        let a_gt = self.unit_gtype_flags(*a);
                        let b_gt = self.unit_gtype_flags(*b);
                        let a_same = a_gt == player_gtype;
                        let b_same = b_gt == player_gtype;
                        match (a_same, b_same) {
                            (true, false) => return std::cmp::Ordering::Less,
                            (false, true) => return std::cmp::Ordering::Greater,
                            _ => {}
                        }
                        let a_vet = (a_gt & (0x0040_0000 | 0x0000_7400)) != 0;
                        let b_vet = (b_gt & (0x0040_0000 | 0x0000_7400)) != 0;
                        match (a_vet, b_vet) {
                            (true, false) => return std::cmp::Ordering::Less,
                            (false, true) => return std::cmp::Ordering::Greater,
                            _ => {}
                        }
                    }
                }
                da.total_cmp(&db)
            });
            if let Some(target) = next {
                if let Some(carrier) = self.carrier_of.remove(&target) {
                    if let Some(list) = self.passengers.get_mut(&carrier) {
                        if self.capture_points.contains(&carrier) {
                            list.retain(|&u| u != target);
                        } else {
                            for u in list.iter_mut() {
                                if *u == target {
                                    *u = ObjectId(0);
                                }
                            }
                        }
                    }
                    if !self.capture_points.contains(&carrier)
                        && let Some(&cpos) = positions.get(&carrier)
                    {
                        let y = if self.non_ground_units.contains(&target)
                            || self.collision.as_ref().is_none_or(|s| s.ground.is_none())
                        {
                            cpos[1]
                        } else {
                            0.0
                        };
                        let mut upos = [cpos[0] + 6.5, y, cpos[2] + 6.5];
                        if let Some(scene) = &self.collision
                            && let Some(&body) = self.bodies.get(&target)
                            && let Ok(free) = scene.find_free_near(upos, body, &[], 16.0)
                        {
                            upos = free;
                        }
                        self.pending_positions.insert(target, upos);
                    }
                }
                self.entering_vehicle.remove(&target);
                if let Some(u) = self.units.get_mut(&target)
                    && u.active == Some(false)
                {
                    u.active = Some(true);
                }
                let act = self.activity.entry(target).or_default();
                act.enabled = Some(true);
                act.weapons_enabled = Some(true);
                let was_non_ground = self.non_ground_units.contains(&player);
                let now_ground = !self.non_ground_units.contains(&target);
                self.player = Some(target);
                self.roster.insert(target);
                self.transferred_by_death = true;
                self.squad_follow.remove(&target);
                self.squad_follow.remove(&player);
                if was_non_ground && now_ground {
                    let roster_ids: Vec<_> = self.roster.iter().copied().collect();
                    for rid in roster_ids {
                        if rid != target
                            && self.can_squad_follow(rid, target)
                            && self
                                .units
                                .get(&rid)
                                .is_some_and(|u| u.active == Some(true) && u.health > 0.0)
                            && !self.carrier_of.contains_key(&rid)
                            && !self.orders.contains_key(&rid)
                        {
                            self.squad_follow.insert(rid);
                        }
                    }
                }
                self.orders.remove(&target);
                self.follow_unit_orders.remove(&target);
                for (&fid, ftarget) in self.follow_unit_orders.iter_mut() {
                    if *ftarget == player && fid != target {
                        *ftarget = target;
                    }
                }
                self.attack_orders.remove(&target);
                self.following.remove(&target);
                self.routes.remove(&target);
                self.direct_velocity.remove(&player);
                self.direct_velocity.remove(&target);
                if let Some(view) = &mut self.player_view {
                    view.target = target;
                    view.cut_revision = view.cut_revision.saturating_add(1);
                }
                if self.player_lifecycle.is_some() {
                    self.player_lifecycle = Some(PlayerLifecycle::FreshAlive(target));
                }
                if let Some(rig) = &mut self.cameras {
                    rig.retarget_chase(target);
                }
            }
        }
    }
    fn advance_strategic_installations(
        &mut self,
        dt: f64,
        positions: &mut BTreeMap<ObjectId, [f32; 3]>,
    ) {
        for si in self.installations.values_mut() {
            si.respawn_cooldown = (si.respawn_cooldown - dt).max(0.0);
        }
        if self.frozen || self.revivable_units.is_empty() || self.installations.is_empty() {
            return;
        }
        let Some(player_army) = self
            .player
            .and_then(|p| self.entity(p).ok())
            .map(|u| u.army.clone())
            .or_else(|| {
                self.roster
                    .iter()
                    .find_map(|id| self.units.get(id).map(|u| u.army.clone()))
            })
        else {
            return;
        };
        let player_pos = self
            .player
            .and_then(|p| positions.get(&p).copied())
            .unwrap_or([0.0, 0.0, 0.0]);
        let any_player_alive = self.units.iter().any(|(id, u)| {
            !self.enemy_targetable_only.contains(id)
                && self.same_allegiance(&u.army, &player_army)
                && u.active == Some(true)
                && u.health > 0.0
        });
        let revivable_list: Vec<(ObjectId, f64)> = self
            .revivable_units
            .iter()
            .map(|(&id, &t)| (id, t))
            .collect();
        for (uid, base_revive_time) in revivable_list {
            let Some(unit_army) = self.units.get(&uid).map(|u| u.army.clone()) else {
                continue;
            };
            if !self.same_allegiance(&unit_army, &player_army) {
                continue;
            }
            let is_alive = self.units.get(&uid).is_some_and(|u| u.health > 0.0);
            if is_alive || self.carrier_of.contains_key(&uid) {
                self.revive_timers.remove(&uid);
                continue;
            }
            let req_type = if self.water_units.contains(&uid) {
                "PORT"
            } else if self.non_ground_units.contains(&uid) {
                "AIRBASE"
            } else if self.bodies.get(&uid).is_some_and(|b| b.radius > 1.2) {
                "FACTORY"
            } else {
                "BARRACKS"
            };
            let mut matching_facilities = 0usize;
            let mut mult = 1.0f64;
            let mut best_si: Option<(ObjectId, ObjectId, f32)> = None;
            for (&si_id, si) in &self.installations {
                if si.facility_type == req_type
                    && self.same_allegiance(&si.owner, &unit_army)
                    && let Some(zid) = si.spawn_zone
                    && let Some(zone) = self.zones.get(&zid)
                {
                    matching_facilities += 1;
                    if si.multi_facility_multiplier > 0.0 && si.multi_facility_multiplier < mult {
                        mult = si.multi_facility_multiplier;
                    }
                    let d = (zone.matrix[12] - player_pos[0])
                        .hypot(zone.matrix[14] - player_pos[2]);
                    if best_si.is_none_or(|(_, _, best_d)| d < best_d) {
                        best_si = Some((si_id, zid, d));
                    }
                }
            }
            let effective_mult = if matching_facilities >= 2 { mult } else { 1.0 };
            let timer = self
                .revive_timers
                .entry(uid)
                .or_insert(base_revive_time);
            *timer = (*timer - dt / effective_mult.max(0.1)).max(0.0);
            if !any_player_alive && best_si.is_some() {
                *timer = 0.0;
            }
            if *timer > 0.0 {
                continue;
            }
            let Some((si_id, zid, _)) = best_si else {
                continue;
            };
            if self
                .installations
                .get(&si_id)
                .is_some_and(|si| si.respawn_cooldown > 0.0 && any_player_alive)
            {
                continue;
            }
            let Some(zone) = self.zones.get(&zid).cloned() else {
                continue;
            };
            if let Some(si) = self.installations.get_mut(&si_id) {
                si.respawn_cooldown = si.time_between_respawns.max(0.5);
            }
            self.revive_timers.remove(&uid);
            if let Some(runtime) = &mut self.rewrite {
                runtime.retired.remove(&uid);
            }
            if let Some(u) = self.units.get_mut(&uid) {
                u.health = u.max_health;
                u.active = Some(true);
                if let Some(anim) = &mut u.animation {
                    let _ = anim.select(crate::unit_animation::Motion::Idle);
                }
            }
            let act = self.activity.entry(uid).or_default();
            act.enabled = Some(true);
            act.weapons_enabled = Some(true);
            self.orders.remove(&uid);
            self.routes.remove(&uid);
            self.following.remove(&uid);
            self.grounded_units.remove(&uid);
            self.landing_units.remove(&uid);
            self.entering_vehicle.remove(&uid);
            self.attack_orders.remove(&uid);
            self.roster.insert(uid);
            if self.player != Some(uid) {
                self.squad_follow.insert(uid);
            }
            let m = zone.matrix;
            let offset_idx = (uid.0 % 5) as f32 - 2.0;
            let sx = m[12] + m[0] * offset_idx * 2.5;
            let sz = m[14] + m[2] * offset_idx * 2.5;
            let mut pos = [sx, m[13], sz];
            if self.water_units.contains(&uid) {
                pos[1] = 0.0;
            } else if self.non_ground_units.contains(&uid) {
                let hag = self.scripted_air_altitude.get(&uid).copied().unwrap_or(35.0);
                if let Some(scene) = &self.collision
                    && let Ok(air_pos) = scene.world_point([sx, 0.0, sz], hag)
                {
                    pos = air_pos;
                } else {
                    pos[1] += hag;
                }
            } else if let Some(scene) = &self.collision {
                if scene.ground.is_some() {
                    pos[1] = 0.0;
                }
                if let Some(&body) = self.bodies.get(&uid) {
                    let others: Vec<_> = self
                        .units
                        .iter()
                        .filter(|(other, u)| {
                            **other != uid
                                && u.active == Some(true)
                                && u.health > 0.0
                                && !self.non_ground_units.contains(other)
                        })
                        .filter_map(|(other, _)| {
                            Some(crate::unit_collision::Other {
                                position: *positions.get(other)?,
                                body: *self.bodies.get(other)?,
                            })
                        })
                        .collect();
                    if let Ok(free) = scene.find_free_near(pos, body, &others, 16.0) {
                        pos = free;
                    }
                }
            }
            positions.insert(uid, pos);
            self.headings.insert(uid, m[8].atan2(m[10]));
        }
    }
    fn advance_campaign_mechanics(
        &mut self,
        dt: f64,
        positions: &mut BTreeMap<ObjectId, [f32; 3]>,
    ) {
        for (id, pos) in std::mem::take(&mut self.pending_positions) {
            positions.insert(id, pos);
        }
        self.advance_strategic_installations(dt, positions);
        self.replace_dead_player(positions);
        if let Some(t) = &mut self.timer_seconds {
            *t = (*t - dt).max(0.0);
        }
        for (follower, target) in self.follow_unit_orders.clone() {
            if self.player == Some(follower) || self.squad_follow.contains(&follower) {
                continue;
            }
            let f_ok = self
                .units
                .get(&follower)
                .is_some_and(|u| u.active == Some(true) && u.health > 0.0);
            let t_ok = self
                .entity(target)
                .is_ok_and(|u| u.active == Some(true) && u.health > 0.0);
            if !f_ok || !t_ok {
                self.follow_unit_orders.remove(&follower);
                continue;
            }
            if let Some(&tp) = positions.get(&target) {
                let target_xz = [tp[0] as f64, tp[2] as f64];
                let target_extra = self
                    .bodies
                    .get(&target)
                    .map_or(0.0, |b| (b.radius - 0.6).max(0.0));
                let radius = self
                    .bodies
                    .get(&follower)
                    .map_or(4.5, |b| b.radius + target_extra + 2.0);
                let active_route = self
                    .routes
                    .get(&follower)
                    .is_some_and(|r| !r.points.is_empty() || r.pending);
                let detour_route = self
                    .routes
                    .get(&follower)
                    .is_some_and(|r| r.points.len() > 2);
                if self.orders.get(&follower).is_none_or(|o| {
                    if active_route {
                        let thresh = if detour_route { 25.0 } else { 15.0 };
                        (o.target[0] - target_xz[0]).hypot(o.target[1] - target_xz[1]) > thresh
                    } else {
                        (o.target[0] - target_xz[0]).hypot(o.target[1] - target_xz[1]) > 1.5
                            || (o.radius - radius).abs() > 0.5
                    }
                }) {
                    self.orders.insert(
                        follower,
                        MoveOrder {
                            target: target_xz,
                            radius,
                        },
                    );
                }
            }
        }
        for (attacker, target) in self.attack_orders.clone() {
            if self.player == Some(attacker) || self.squad_follow.contains(&attacker) {
                continue;
            }
            let a_ok = self
                .entity(attacker)
                .is_ok_and(|u| u.active == Some(true) && u.health > 0.0);
            let t_ok = self
                .entity(target)
                .is_ok_and(|u| u.active == Some(true) && u.health > 0.0);
            if !a_ok || !t_ok {
                self.attack_orders.remove(&attacker);
                continue;
            }
            if !self.buildings.contains(&attacker)
                && !self.bombers.orders.contains_key(&attacker)
                && !self.grounded_units.contains(&attacker)
                && (!self.non_ground_units.contains(&attacker)
                    || self.scripted_air_cruise.contains_key(&attacker)
                    || self.water_units.contains(&attacker))
                && let (Some(&ap), Some(&tp)) = (positions.get(&attacker), positions.get(&target))
            {
                let dist = (ap[0] - tp[0]).hypot(ap[2] - tp[2]) as f64;
                let target_xz = [tp[0] as f64, tp[2] as f64];
                let is_artillery =
                    (self.unit_gtype_flags(attacker) & (0x0000_1000 | 0x0000_0400)) != 0;
                let (engage_dist, stop_radius) = if self.water_units.contains(&attacker) {
                    (120.0, 90.0)
                } else if self.scripted_air_cruise.contains_key(&attacker) {
                    (45.0, 35.0)
                } else if is_artillery {
                    (200.0, 160.0)
                } else {
                    (14.0, 11.0)
                };
                if dist > engage_dist
                    && self.orders.get(&attacker).is_none_or(|o| {
                        (o.target[0] - target_xz[0]).hypot(o.target[1] - target_xz[1]) > 8.0
                    })
                {
                    self.orders.insert(
                        attacker,
                        MoveOrder {
                            target: target_xz,
                            radius: stop_radius,
                        },
                    );
                }
            }
        }
        for (unit, carrier) in self.entering_vehicle.clone() {
            let Some(u) = self.units.get(&unit) else {
                self.entering_vehicle.remove(&unit);
                continue;
            };
            if u.health <= 0.0 {
                self.entering_vehicle.remove(&unit);
                continue;
            }
            if u.active != Some(true)
                || self
                    .carrier_of
                    .get(&unit)
                    .is_some_and(|c| !self.capture_points.contains(c))
            {
                continue;
            }
            let is_cp = self.capture_points.contains(&carrier);
            let close = match (positions.get(&unit), positions.get(&carrier)) {
                (Some(up), Some(cp)) => (up[0] - cp[0]).hypot(up[2] - cp[2]) <= 8.5,
                _ => !is_cp,
            };
            if is_cp
                && (self.roster.contains(&unit) || close)
                && self
                    .capture_point_state
                    .get(&carrier)
                    .is_some_and(|cp| cp.owner == u.army && cp.height >= 100.0)
            {
                self.entering_vehicle.remove(&unit);
                self.orders.remove(&unit);
                if self.roster.contains(&unit) && self.player != Some(unit) {
                    self.squad_follow.insert(unit);
                }
                continue;
            }
            if close || (!is_cp && !self.orders.contains_key(&unit)) {
                self.entering_vehicle.remove(&unit);
                self.orders.remove(&unit);
                if let Some(prev) = self.carrier_of.insert(unit, carrier)
                    && prev != carrier
                    && let Some(prev_list) = self.passengers.get_mut(&prev)
                {
                    if self.capture_points.contains(&prev) {
                        prev_list.retain(|&u| u != unit);
                    } else {
                        for u in prev_list.iter_mut() {
                            if *u == unit {
                                *u = ObjectId(0);
                            }
                        }
                    }
                }
                let list = self.passengers.entry(carrier).or_default();
                if !list.contains(&unit) {
                    if !is_cp
                        && let Some(slot) = list.iter_mut().find(|id| id.0 == 0)
                    {
                        *slot = unit;
                    } else {
                        list.push(unit);
                    }
                }
                if !is_cp {
                    let u_army = self.units.get(&unit).map(|u| u.army.clone());
                    if let Some(u) = self.units.get_mut(&unit) {
                        u.active = Some(false);
                    }
                    if let (Some(army), Some(b)) = (u_army, self.props.get_mut(&carrier))
                        && (b.army.is_empty() || b.army == "eNeutral" || b.army == "eNoArmy")
                    {
                        b.army = army;
                        self.activity.entry(carrier).or_default().enabled = Some(true);
                    }
                }
            } else if is_cp
                && !self.orders.contains_key(&unit)
                && let Some(&cp) = positions.get(&carrier)
            {
                self.orders.insert(
                    unit,
                    MoveOrder {
                        target: [cp[0] as f64, cp[2] as f64],
                        radius: 2.0,
                    },
                );
            }
        }
        let cp_ids: Vec<_> = self.capture_point_state.keys().copied().collect();
        for cp_id in cp_ids {
            if let Some(list) = self.passengers.get_mut(&cp_id) {
                let cpos = positions.get(&cp_id).copied();
                list.retain(|&uid| {
                    let keep = self
                        .units
                        .get(&uid)
                        .is_some_and(|u| u.active == Some(true) && u.health > 0.0)
                        && match (positions.get(&uid), cpos) {
                            (Some(up), Some(cp)) => (up[0] - cp[0]).hypot(up[2] - cp[2]) <= 12.0,
                            _ => true,
                        };
                    if !keep && self.carrier_of.get(&uid) == Some(&cp_id) {
                        self.carrier_of.remove(&uid);
                    }
                    keep
                });
            }
            let mut army_counts: BTreeMap<String, usize> = BTreeMap::new();
            let mut counted = BTreeSet::new();
            if let Some(list) = self.passengers.get(&cp_id) {
                for &uid in list {
                    if let Some(u) = self.units.get(&uid)
                        && u.active == Some(true)
                        && u.health > 0.0
                    {
                        *army_counts.entry(u.army.clone()).or_default() += 1;
                        counted.insert(uid);
                    }
                }
            }
            if let Some(&cpos) = positions.get(&cp_id) {
                for (&uid, u) in &self.units {
                    if !counted.contains(&uid)
                        && u.active == Some(true)
                        && u.health > 0.0
                        && self.bodies.get(&uid).is_some_and(|b| {
                            b.radius == crate::unit_collision::Body::INFANTRY.radius
                        })
                        && let Some(&upos) = positions.get(&uid)
                        && (upos[0] - cpos[0]).hypot(upos[2] - cpos[2]) <= 12.0
                    {
                        *army_counts.entry(u.army.clone()).or_default() += 1;
                    }
                }
            }
            let max_count = army_counts.values().copied().max().unwrap_or(0);
            let contender_army = if max_count > 0
                && army_counts.values().filter(|&&c| c == max_count).count() == 1
            {
                army_counts
                    .into_iter()
                    .find(|&(_, c)| c == max_count)
                    .map(|(a, _)| a)
            } else {
                None
            };
            if let Some(army) = contender_army
                && let Some(cp) = self.capture_point_state.get_mut(&cp_id)
                && (cp.owner != army || cp.height < 100.0)
            {
                cp.capturing_army = Some(army.clone());
                if cp.owner != "eNeutral" && cp.owner != army && cp.height > 0.0 {
                    cp.direction = -1;
                    cp.height = (cp.height - 15.0 * dt).max(0.0);
                    if cp.height <= 0.0 {
                        cp.owner = "eNeutral".to_string();
                        if let Some(si_id) = cp.installation
                            && let Some(si) = self.installations.get_mut(&si_id)
                        {
                            si.owner = "eNeutral".to_string();
                        }
                    }
                } else {
                    cp.direction = 1;
                    cp.height = (cp.height + 15.0 * dt).min(100.0);
                    if cp.height >= 97.0 {
                        cp.owner = army.clone();
                        if let Some(si_id) = cp.installation
                            && let Some(si) = self.installations.get_mut(&si_id)
                        {
                            si.owner = army.clone();
                            for &bid in &si.buildings.clone() {
                                if let Some(b) = self.props.get_mut(&bid) {
                                    b.army = army.clone();
                                }
                            }
                        }
                    }
                    if cp.height >= 100.0 {
                        cp.direction = 0;
                        if let Some(list) = self.passengers.remove(&cp_id) {
                            for uid in list {
                                if self.carrier_of.get(&uid) == Some(&cp_id) {
                                    self.carrier_of.remove(&uid);
                                }
                                if self.roster.contains(&uid) && self.player != Some(uid) {
                                    self.squad_follow.insert(uid);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    pub(crate) fn advance(
        &mut self,
        dt: f64,
        positions: &mut BTreeMap<ObjectId, [f32; 3]>,
    ) -> Result<()> {
        if self.rewrite.is_some() {
            self.advance_campaign_mechanics(dt, positions);
        }
        if let Some(rewrite) = &mut self.rewrite {
            rewrite.advance(dt)?;
        }
        self.apply_blasts(positions)?;
        if self.rewrite.is_some() {
            let mut bombers = std::mem::take(&mut self.bombers);
            let result = bombers.advance_staged(self, positions, dt);
            self.bombers = bombers;
            result?;
            let mut missiles = std::mem::take(&mut self.missiles);
            let result = missiles.advance_staged(self, positions, dt);
            self.missiles = missiles;
            result?;
        }
        self.advance_squad_follow(positions)?;
        if let Some(music) = &mut self.music {
            music.advance(dt)?;
        }
        if let Some(cameras) = &mut self.cameras {
            cameras.advance(dt)?;
        }
        if let Some(messages) = &mut self.messages {
            messages.advance(dt)?;
        }
        self.advance_followers(dt, positions)?;
        if matches!(
            self.navigation,
            Navigation::CollisionAware | Navigation::PlannedInfantry
        ) {
            return self.advance_colliding(dt, positions);
        }
        if !self.orders.is_empty() && self.navigation == Navigation::Unavailable {
            return Err("Navigation backend not configured; straight-line movement requires explicit diagnostic opt-in".into());
        }
        let mut complete = Vec::new();
        for (id, order) in &self.orders {
            let unit = self.units.get_mut(id).ok_or("Unknown ordered unit")?;
            if unit.health <= 0.0 {
                complete.push(*id);
                continue;
            }
            if unit.active != Some(true) || self.frozen && self.player == Some(*id) {
                if let Some(animation) = &mut unit.animation {
                    animation.select(crate::unit_animation::Motion::Idle)?;
                }
                continue;
            }
            let speed = unit.forward_speed.ok_or("No supported movement speed")?;
            if !speed.is_finite() || speed <= 0.0 {
                return Err("Invalid movement speed".into());
            }
            let position = positions.get_mut(id).ok_or("Ordered unit lacks position")?;
            if position.iter().any(|v| !v.is_finite()) {
                return Err("Invalid movement position".into());
            }
            let dx = order.target[0] - position[0] as f64;
            let dz = order.target[1] - position[2] as f64;
            let distance = dx.hypot(dz);
            let remaining = (distance - order.radius).max(0.0);
            let step = (speed * dt).min(remaining);
            if distance > 0.0 {
                position[0] = (position[0] as f64 + dx / distance * step) as f32;
                position[2] = (position[2] as f64 + dz / distance * step) as f32;
            }
            let arrived = step >= remaining;
            if let Some(animation) = &mut unit.animation {
                animation.select(if arrived {
                    crate::unit_animation::Motion::Idle
                } else {
                    crate::unit_animation::Motion::Run
                })?;
            }
            if arrived {
                complete.push(*id);
            }
        }
        for id in complete {
            self.orders.remove(&id);
        }
        for unit in self.units.values_mut().filter(|u| u.active == Some(true)) {
            if let Some(animation) = &mut unit.animation {
                animation.advance(dt)?;
            }
        }
        Ok(())
    }
    fn advance_colliding(
        &mut self,
        dt: f64,
        positions: &mut BTreeMap<ObjectId, [f32; 3]>,
    ) -> Result<()> {
        use crate::unit_collision::Other;
        // A zero-duration rewrite startup tick executes scripts, not a physics step.
        if self.rewrite.is_some() && dt == 0.0 {
            return Ok(());
        }
        let scene = self
            .collision
            .clone()
            .ok_or("Collision scene not installed")?;
        if self.rewrite.is_some() {
            for (&id, sub) in &mut self.submarines {
                if self
                    .units
                    .get(&id)
                    .is_none_or(|u| u.active != Some(true) || u.health <= 0.0)
                {
                    continue;
                }
                let rate = (sub.max_depth as f64 / sub.dive_time) * dt;
                if sub.submerged {
                    sub.depth = (sub.depth + rate as f32).min(sub.max_depth);
                    sub.oxygen = (sub.oxygen - dt).max(0.0);
                    if sub.oxygen <= 0.0 {
                        sub.submerged = false;
                    }
                } else {
                    sub.depth = (sub.depth - rate as f32).max(0.0);
                    sub.oxygen =
                        (sub.oxygen + dt * (sub.max_oxygen / sub.recharge_time)).min(sub.max_oxygen);
                }
                if let Some(p) = positions.get_mut(&id) {
                    p[1] = -sub.depth;
                }
            }
            let mut arrived_carriers = Vec::new();
            for &id in &self.non_ground_units.clone() {
                if let Some(&[vx, vz]) = self.direct_velocity.get(&id) {
                    if let Some(p) = positions.get_mut(&id) {
                        let nx = p[0] + (vx * dt) as f32;
                        let nz = p[2] + (vz * dt) as f32;
                        if self.water_units.contains(&id) {
                            let shore_y = self.water_height + 6.0;
                            let cur_y = scene
                                .terrain_height(p[0], p[2])
                                .unwrap_or(self.water_height);
                            let next_y = scene
                                .terrain_height(nx, nz)
                                .unwrap_or(self.water_height);
                            if next_y <= shore_y || next_y <= cur_y {
                                p[0] = nx;
                                p[2] = nz;
                            } else if scene
                                .terrain_height(nx, p[2])
                                .is_some_and(|y| y <= shore_y || y <= cur_y)
                            {
                                p[0] = nx;
                            } else if scene
                                .terrain_height(p[0], nz)
                                .is_some_and(|y| y <= shore_y || y <= cur_y)
                            {
                                p[2] = nz;
                            }
                            p[1] = self.submarines.get(&id).map_or(0.0, |s| -s.depth);
                        } else {
                            p[0] = nx;
                            p[2] = nz;
                        }
                        if vx.hypot(vz) > 1e-4 {
                            self.headings.insert(id, vx.atan2(vz) as f32);
                        }
                        if !self.grounded_units.contains(&id)
                            && !self.water_units.contains(&id)
                            && let Some(&hag) = self.scripted_air_altitude.get(&id)
                            && let Ok(floor_pos) = scene.world_point([p[0], 0., p[2]], 0.)
                        {
                            let floor = floor_pos[1];
                            if let Some(fp) = self.flight_profiles.get(&id) {
                                let pitch_input = if self.player == Some(id) {
                                    self.player_aim.map(|a| a[1]).unwrap_or(0.0)
                                } else {
                                    0.0
                                };
                                if pitch_input.abs() > 0.05 {
                                    let vy = if pitch_input >= 0.0 {
                                        pitch_input * fp.up_speed
                                    } else {
                                        pitch_input * fp.down_speed
                                    };
                                    p[1] += (vy * dt) as f32;
                                } else if scene.ground.is_some() {
                                    let cruise_y = floor + fp.cruise_hag.max(fp.min_hag);
                                    let dy = cruise_y - p[1];
                                    if dy > 0.01 {
                                        p[1] = (p[1] + (fp.up_speed * dt) as f32).min(cruise_y);
                                    } else if dy < -0.01 {
                                        p[1] = (p[1] - (fp.down_speed * dt) as f32).max(cruise_y);
                                    }
                                }
                                p[1] = p[1].clamp(
                                    floor + fp.min_hag,
                                    floor + fp.max_hag.max(fp.cruise_hag),
                                );
                            } else {
                                p[1] = p[1].max(floor + hag);
                            }
                        }
                    }
                    continue;
                }
                if let Some(order) = self.orders.get(&id).cloned() {
                    let speed = *self
                        .scripted_air_cruise
                        .get(&id)
                        .ok_or("Water movement backend not configured")?;
                    let unit = &self.units[&id];
                    if unit.active == Some(false) || unit.health <= 0. {
                        continue;
                    }
                    if unit.active.is_none() {
                        return Err("Unknown aircraft presence".into());
                    }
                    let p = positions.get_mut(&id).ok_or("Missing aircraft position")?;
                    let prev_xz = [p[0], p[2]];
                    let mut arrived =
                        crate::mission_air::step(p, order.target, order.radius, speed, dt)?;
                    let is_follower =
                        self.squad_follow.contains(&id) || self.follow_unit_orders.contains_key(&id);
                    let shore_limit = if is_follower {
                        self.water_height + 6.0
                    } else {
                        self.water_height + 0.5
                    };
                    if self.water_units.contains(&id)
                        && !self.unloading_on_arrival.contains(&id)
                        && scene
                            .terrain_height(prev_xz[0], prev_xz[1])
                            .is_some_and(|g0| g0 <= shore_limit)
                        && scene
                            .terrain_height(p[0], p[2])
                            .is_some_and(|g1| g1 > shore_limit)
                    {
                        let dx = p[0] - prev_xz[0];
                        let dz = p[2] - prev_xz[1];
                        let step_len = dx.hypot(dz);
                        let base_ang = dx.atan2(dz);
                        let try_x = [p[0], prev_xz[1]];
                        let try_z = [prev_xz[0], p[2]];
                        if dx.abs() > 1e-4
                            && scene
                                .terrain_height(try_x[0], try_x[1])
                                .is_some_and(|gx| gx <= shore_limit)
                        {
                            p[2] = prev_xz[1];
                        } else if dz.abs() > 1e-4
                            && scene
                                .terrain_height(try_z[0], try_z[1])
                                .is_some_and(|gz| gz <= shore_limit)
                        {
                            p[0] = prev_xz[0];
                        } else if step_len > 1e-4
                            && let Some((rx, rz)) =
                                [-0.45f32, 0.45, -0.9, 0.9, -1.35, 1.35, -1.75, 1.75]
                                    .into_iter()
                                    .find_map(|off| {
                                        let a = base_ang + off;
                                        let cx = prev_xz[0] + a.sin() * step_len;
                                        let cz = prev_xz[1] + a.cos() * step_len;
                                        scene
                                            .terrain_height(cx, cz)
                                            .is_some_and(|g| g <= shore_limit)
                                            .then_some((cx, cz))
                                    })
                        {
                            p[0] = rx;
                            p[2] = rz;
                        } else {
                            p[0] = prev_xz[0];
                            p[2] = prev_xz[1];
                            arrived = true;
                        }
                    }
                    if (p[0] - prev_xz[0]).hypot(p[2] - prev_xz[1]) > 1e-4 {
                        self.headings
                            .insert(id, (p[0] - prev_xz[0]).atan2(p[2] - prev_xz[1]));
                    }
                    if self.water_units.contains(&id) {
                        p[1] = self.submarines.get(&id).map_or(0.0, |s| -s.depth);
                    } else if !self.grounded_units.contains(&id)
                        && let Some(&hag) = self.scripted_air_altitude.get(&id)
                        && let Ok(min_pos) = scene.world_point([p[0], 0., p[2]], hag)
                    {
                        if scene.ground.is_some() {
                            let down = self
                                .flight_profiles
                                .get(&id)
                                .map_or(20.0, |fp| fp.down_speed);
                            p[1] = (p[1] - (down * dt) as f32).max(min_pos[1]);
                        } else {
                            p[1] = p[1].max(min_pos[1]);
                        }
                    }
                    if arrived {
                        self.orders.remove(&id);
                        if self.landing_units.remove(&id) {
                            self.grounded_units.insert(id);
                            if let Ok(ground_pos) = scene.world_point([p[0], 0., p[2]], 0.) {
                                p[1] = ground_pos[1];
                            }
                        }
                        if self.unloading_on_arrival.remove(&id) {
                            arrived_carriers.push(id);
                        }
                    }
                } else if !self.grounded_units.contains(&id)
                    && !self.water_units.contains(&id)
                    && self
                        .units
                        .get(&id)
                        .is_some_and(|u| u.active == Some(true) && u.health > 0.0)
                    && let Some(&hag) = self.scripted_air_altitude.get(&id)
                    && let Some(p) = positions.get_mut(&id)
                    && let Ok(min_pos) = scene.world_point([p[0], 0., p[2]], hag)
                {
                    if scene.ground.is_some() {
                        let down = self
                            .flight_profiles
                            .get(&id)
                            .map_or(20.0, |fp| fp.down_speed);
                        p[1] = (p[1] - (down * dt) as f32).max(min_pos[1]);
                    } else {
                        p[1] = p[1].max(min_pos[1]);
                    }
                }
            }
            for id in arrived_carriers {
                self.unload_carrier_passengers(id, positions);
            }
        }
        for (&passenger, &carrier) in &self.carrier_of {
            if !self.capture_points.contains(&carrier) {
                if let Some(&[vx, vz]) = self.direct_velocity.get(&passenger)
                    && let Some(cpos) = positions.get_mut(&carrier)
                {
                    cpos[0] += (vx * dt) as f32;
                    cpos[2] += (vz * dt) as f32;
                }
                if let Some(&cpos) = positions.get(&carrier) {
                    let y = if self.non_ground_units.contains(&passenger) || scene.ground.is_none()
                    {
                        cpos[1]
                    } else {
                        0.0
                    };
                    positions.insert(passenger, [cpos[0], y, cpos[2]]);
                }
            }
        }
        let active = self
            .units
            .iter()
            .filter(|(id, u)| {
                u.active == Some(true)
                    && u.health > 0.0
                    && self
                        .carrier_of
                        .get(id)
                        .is_none_or(|c| self.capture_points.contains(c))
                    && !(self.rewrite.is_some() && self.non_ground_units.contains(id))
            })
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        let mut smallest = 0.6f64;
        let mut fastest = 0.0f64;
        for id in &active {
            let body = *self
                .bodies
                .get(id)
                .ok_or("Active unit collision body unsupported")?;
            body.validate()?;
            smallest = smallest.min(body.radius);
            if let Some(v) = self.direct_velocity.get(id) {
                let speed = v[0].hypot(v[1]);
                if !speed.is_finite() || speed > 1000.0 {
                    return Err("Invalid direct velocity".into());
                }
                fastest = fastest.max(speed);
            } else if self.orders.contains_key(id) {
                let speed = self.units[id]
                    .forward_speed
                    .ok_or("Missing collision movement speed")?;
                if !speed.is_finite() || speed <= 0.0 {
                    return Err("Invalid collision speed".into());
                }
                fastest = fastest.max(speed);
            }
        }
        let count = (fastest * dt / (smallest * 0.25)).ceil().max(1.0);
        if !count.is_finite() || count > 4096.0 {
            return Err("Collision substep budget exceeded".into());
        }
        let planned = self.navigation == Navigation::PlannedInfantry;
        let is_rewrite = self.rewrite.is_some();
        self.routes.retain(|id, r| {
            if !active.contains(id) || self.direct_velocity.contains_key(id) {
                return false;
            }
            let Some(o) = self.orders.get(id) else {
                return false;
            };
            if o.target == r.target && o.radius == r.radius {
                true
            } else if is_rewrite {
                let near_self = positions.get(id).is_some_and(|p| {
                    (o.target[0] - p[0] as f64).hypot(o.target[1] - p[2] as f64) <= 18.0
                });
                if near_self || (o.target[0] - r.target[0]).hypot(o.target[1] - r.target[1]) > 45.0 {
                    r.points.clear();
                }
                r.target = o.target;
                r.radius = o.radius;
                r.pending = true;
                true
            } else {
                false
            }
        });
        if planned {
            for route in self.routes.values_mut() {
                route.retry = (route.retry - dt).max(0.0);
            }
            // At most one bounded search per tick, with round-robin fairness.
            let mut candidates = active.clone();
            candidates.sort_by_key(|id| (self.route_cursor.is_some_and(|last| *id <= last), *id));
            for id in candidates {
                if self.frozen && self.player == Some(id) {
                    continue;
                }
                let Some(route) = self.routes.get(&id) else {
                    continue;
                };
                if !route.pending || route.retry > 0.0 {
                    continue;
                }
                let start_pos = *positions.get(&id).ok_or("Missing route start")?;
                if self.rewrite.is_some() && scene.world_point(start_pos, 0.0).is_err() {
                    continue;
                }
                let squad_member = self.rewrite.is_some() && self.roster.contains(&id);
                let others = active
                    .iter()
                    .filter(|other| {
                        **other != id
                            && !(squad_member && self.roster.contains(other))
                            && !(self.rewrite.is_some()
                                && positions
                                    .get(other)
                                    .is_some_and(|p| scene.world_point(*p, 0.0).is_err()))
                    })
                    .map(|other| {
                        Ok(Other {
                            position: *positions
                                .get(other)
                                .ok_or("Missing route obstacle position")?,
                            body: self.bodies[other],
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                let limits = if squad_member {
                    crate::navigation::Limits {
                        expansions: 4096,
                        probes: 131072,
                        extent: 256,
                    }
                } else if self.rewrite.is_some() {
                    crate::navigation::Limits {
                        expansions: 2048,
                        probes: 65536,
                        extent: 256,
                    }
                } else {
                    crate::navigation::Limits::default()
                };
                let mut start_pos = start_pos;
                let body = self.bodies[&id];
                let result = if self.rewrite.is_some() {
                    if scene.overlaps(start_pos, body, &others)?
                        && let Ok(corrected) = scene.find_free_near(
                            start_pos,
                            body,
                            &others,
                            (body.radius * 2.0 + 6.0).min(16.0),
                        )
                    {
                        if let Some(runtime) = &mut self.rewrite {
                            runtime
                                .placement_corrections
                                .insert(id, (start_pos, corrected));
                        }
                        start_pos = corrected;
                        positions.insert(id, corrected);
                    }
                    let target = route.target;
                    let radius = route.radius;
                    match crate::navigation::plan_towards(
                        &scene, start_pos, target, radius, body, &others, limits,
                    )
                    .or_else(|_| {
                        crate::navigation::plan_towards(
                            &scene,
                            start_pos,
                            target,
                            radius,
                            body,
                            &[],
                            limits,
                        )
                    }) {
                        Ok(plan) => plan,
                        Err(_) => {
                            if let Some(r) = self.routes.get_mut(&id) {
                                r.retry = 0.5;
                            }
                            continue;
                        }
                    }
                } else {
                    crate::navigation::plan(
                        &scene,
                        start_pos,
                        route.target,
                        route.radius,
                        self.bodies[&id],
                        &others,
                        limits,
                    )?
                };
                self.route_searches = self.route_searches.saturating_add(1);
                self.route_cursor = Some(id);
                self.last_route_status = Some(result.status);
                self.route_expansions =
                    self.route_expansions.saturating_add(result.expanded as u64);
                self.route_probes = self.route_probes.saturating_add(result.probes as u64);
                let found = result.status == crate::navigation::Status::Found;
                if !found {
                    self.route_failures = self.route_failures.saturating_add(1);
                }
                let has_path = found
                    || (self.rewrite.is_some()
                        && result.points.iter().any(|p| {
                            (p[0] - start_pos[0] as f64).hypot(p[1] - start_pos[2] as f64) > 0.05
                        }));
                let route = self.routes.get_mut(&id).unwrap();
                route.pending = !has_path;
                route.retry = 0.5;
                route.planned |= has_path;
                route.points = if has_path {
                    result.points.into()
                } else {
                    std::collections::VecDeque::new()
                };
                break;
            }
        }
        let steps = count as usize;
        let step_dt = dt / steps as f64;
        for _ in 0..steps {
            for id in &active {
                if self.rewrite.is_some()
                    && ((!self.orders.contains_key(id) && !self.direct_velocity.contains_key(id))
                        || (self.frozen && self.player == Some(*id)))
                {
                    if let Some(animation) = &mut self.units.get_mut(id).unwrap().animation
                        && (matches!(
                            animation.motion,
                            crate::unit_animation::Motion::Run
                                | crate::unit_animation::Motion::Walk
                        ) || (animation.motion == crate::unit_animation::Motion::Fire
                            && animation.elapsed >= 0.45))
                    {
                        animation.select(crate::unit_animation::Motion::Idle)?;
                    }
                    continue;
                }
                let mut start = *positions.get(id).ok_or("Missing active unit position")?;
                let body = self.bodies[id];
                let squad_member = self.rewrite.is_some() && self.roster.contains(id);
                let others = active
                    .iter()
                    .filter(|other| {
                        *other != id
                            && !(squad_member && self.roster.contains(other))
                            && !(self.rewrite.is_some()
                                && positions
                                    .get(other)
                                    .is_some_and(|p| scene.world_point(*p, 0.0).is_err()))
                    })
                    .map(|other| {
                        Ok(Other {
                            position: *positions.get(other).ok_or("Missing other position")?,
                            body: self.bodies[other],
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                let on_terrain = self.rewrite.is_none() || scene.world_point(start, 0.0).is_ok();
                if !on_terrain {
                    continue;
                }
                let mut needs_placement = scene.overlaps(start, body, &others)?;
                if needs_placement {
                    let Some(runtime) = &mut self.rewrite else {
                        return Err(format!("Active unit {} starts penetrated", id.0));
                    };
                    if on_terrain
                        && let Ok(corrected) = scene.find_free_near(
                            start,
                            body,
                            &others,
                            (body.radius * 2.0 + 6.0).min(16.0),
                        )
                    {
                        runtime
                            .placement_corrections
                            .insert(*id, (start, corrected));
                        start = corrected;
                        positions.insert(*id, corrected);
                        self.routes.remove(id);
                        needs_placement = false;
                    }
                }
                if let Some(velocity) = self.direct_velocity.get(id).copied() {
                    if !(self.frozen && self.player == Some(*id)) {
                        let movement = if needs_placement {
                            crate::unit_collision::Movement {
                                position: [
                                    start[0] + (velocity[0] * step_dt) as f32,
                                    start[1],
                                    start[2] + (velocity[1] * step_dt) as f32,
                                ],
                                blocked: false,
                                mesh_contacts: 0,
                                unit_contacts: 0,
                            }
                        } else {
                            scene.move_body(
                                start,
                                [velocity[0] * step_dt, velocity[1] * step_dt],
                                body,
                                &others,
                            )?
                        };
                        self.blocked_contacts = self
                            .blocked_contacts
                            .saturating_add(u64::from(movement.blocked));
                        self.mesh_contacts =
                            self.mesh_contacts.saturating_add(movement.mesh_contacts);
                        self.unit_contacts =
                            self.unit_contacts.saturating_add(movement.unit_contacts);
                        let moved = (movement.position[0] - start[0])
                            .hypot(movement.position[2] - start[2])
                            > 1e-5;
                        positions.insert(*id, movement.position);
                        if let Some(animation) = &mut self.units.get_mut(id).unwrap().animation {
                            animation.select(if moved {
                                crate::unit_animation::Motion::Run
                            } else {
                                crate::unit_animation::Motion::Idle
                            })?;
                        }
                    } else if let Some(animation) = &mut self.units.get_mut(id).unwrap().animation {
                        animation.select(crate::unit_animation::Motion::Idle)?;
                    }
                    continue;
                }
                let Some(order) = self.orders.get(id).cloned() else {
                    continue;
                };
                if self.frozen && self.player == Some(*id) {
                    if let Some(animation) = &mut self.units.get_mut(id).unwrap().animation {
                        animation.select(crate::unit_animation::Motion::Idle)?;
                    }
                    continue;
                }
                let waypoint = if planned {
                    self.routes.get_mut(id).and_then(|route| {
                        while !route.points.is_empty() {
                            let tol = if is_rewrite && route.points.len() > 1 {
                                0.45
                            } else {
                                0.001
                            };
                            if route.points.front().is_some_and(|p| {
                                (p[0] - start[0] as f64).hypot(p[1] - start[2] as f64) < tol
                            }) {
                                route.points.pop_front();
                            } else {
                                break;
                            }
                        }
                        route.points.front().copied()
                    })
                } else {
                    None
                };
                let target = waypoint.unwrap_or(order.target);
                let dx = target[0] - start[0] as f64;
                let dz = target[1] - start[2] as f64;
                let distance = dx.hypot(dz);
                let remaining = (distance
                    - if waypoint.is_some() {
                        0.0
                    } else {
                        order.radius
                    })
                .max(0.0);
                let travel = (self.units[id].forward_speed.unwrap() * step_dt).min(remaining);
                let delta = if distance > 0.0 {
                    [dx / distance * travel, dz / distance * travel]
                } else {
                    [0.0; 2]
                };
                let movement = if needs_placement {
                    crate::unit_collision::Movement {
                        position: [
                            start[0] + delta[0] as f32,
                            start[1],
                            start[2] + delta[1] as f32,
                        ],
                        blocked: false,
                        mesh_contacts: 0,
                        unit_contacts: 0,
                    }
                } else {
                    scene.move_body(start, delta, body, &others)?
                };
                self.blocked_contacts = self
                    .blocked_contacts
                    .saturating_add(u64::from(movement.blocked));
                self.mesh_contacts = self.mesh_contacts.saturating_add(movement.mesh_contacts);
                self.unit_contacts = self.unit_contacts.saturating_add(movement.unit_contacts);
                let step_moved =
                    (movement.position[0] - start[0]).hypot(movement.position[2] - start[2]);
                let moved = step_moved > 1e-5;
                let arrived = (order.target[0] - movement.position[0] as f64)
                    .hypot(order.target[1] - movement.position[2] as f64)
                    <= order.radius + 0.001;
                let needs_replan = if self.rewrite.is_some() {
                    if waypoint.is_some() {
                        step_moved < (travel * 0.25) as f32
                    } else {
                        movement.mesh_contacts > 0 || !moved
                    }
                } else {
                    ((movement.position[0] - start[0]) as f64 - delta[0])
                        .hypot((movement.position[2] - start[2]) as f64 - delta[1])
                        > 0.001
                };
                if planned && needs_replan {
                    let route = self.routes.entry(*id).or_insert_with(|| {
                        crate::navigation::Route::new(order.target, order.radius)
                    });
                    route.points.clear();
                    route.pending = true;
                }
                if moved {
                    self.headings.insert(
                        *id,
                        (movement.position[0] - start[0]).atan2(movement.position[2] - start[2]),
                    );
                }
                positions.insert(*id, movement.position);
                if let Some(animation) = &mut self.units.get_mut(id).unwrap().animation {
                    animation.select(if moved && !arrived {
                        crate::unit_animation::Motion::Run
                    } else {
                        crate::unit_animation::Motion::Idle
                    })?;
                }
                if arrived {
                    self.orders.remove(id);
                    if self.routes.remove(id).is_some_and(|r| r.planned) {
                        self.route_arrivals = self.route_arrivals.saturating_add(1);
                    }
                }
            }
        }
        if self.rewrite.is_some() && !self.crushable_props.is_empty() {
            let mut crushed = Vec::new();
            for id in &active {
                let body = self.bodies[id];
                if body.radius <= crate::unit_collision::Body::INFANTRY.radius {
                    continue;
                }
                let Some(&pos) = positions.get(id) else {
                    continue;
                };
                for &pid in &self.crushable_props {
                    let Some(prop) = self.props.get(&pid) else {
                        continue;
                    };
                    if prop.active != Some(true)
                        || prop.health <= 0.0
                        || prop.invulnerable
                        || !positions.contains_key(&pid)
                    {
                        continue;
                    }
                    if let Some(c) = scene.object_center(pid)
                        && ((c[0] - pos[0]).hypot(c[2] - pos[2]) as f64) <= body.radius + 1.8
                    {
                        crushed.push(pid);
                    }
                }
            }
            crushed.sort_unstable();
            crushed.dedup();
            for pid in crushed {
                self.kill_rewrite_in_scene(pid, positions)?;
            }
        }
        self.orders
            .retain(|id, _| self.units.get(id).is_some_and(|u| u.health > 0.0));
        for unit in self.units.values_mut().filter(|u| {
            u.active == Some(true)
                || u.animation
                    .as_ref()
                    .is_some_and(|a| a.motion == crate::unit_animation::Motion::Death)
        }) {
            if let Some(animation) = &mut unit.animation {
                animation.advance(dt)?;
            }
        }
        Ok(())
    }
    /// Native `FUN_8010ac94` (`0x8010ac94`): maps internal `eArmy` to `flag.TYPE_*` bit.
    pub fn army_type_flag(army: &str) -> u32 {
        match army {
            "eWesternFrontier" => 0x0000_0001,
            "eXylvanian" => 0x0000_0002,
            "eTundranTerritories" | "eTundran" => 0x0000_0020,
            "eSolarEmpire" => 0x0000_0400,
            "eUnderWorld" | "eUnderworld" => 0x0000_0800,
            "eNeutral" | "eNoArmy" | "" => 0x0000_1000,
            "eAngloIsles" => 0x0002_0000,
            _ => 0,
        }
    }
    /// Native `FUN_80075e48` (`0x80075e48`) / `FUN_800d9a34` (`0x800d9a34`):
    /// returns the combined `flag.TYPE_*` army + object-class bitmask for `id`.
    pub fn unit_type_flags(&self, id: ObjectId) -> u32 {
        let army = self
            .entity(id)
            .map(|u| Self::army_type_flag(&u.army))
            .unwrap_or(0);
        let class = if let Some(runtime) = &self.rewrite
            && let Some(o) = runtime.objects().get(&id)
        {
            match o.kind.as_str() {
                "cTroop" => 0x0000_0100,
                "cGroundVehicle" | "cWaterVehicle" => 0x0000_0040,
                "cAirVehicle" => 0x0000_0080,
                "cPickup" => 0x0010_0000,
                _ => 0,
            }
        } else if self.bodies.contains_key(&id) {
            0x0000_0100
        } else if self.non_ground_units.contains(&id) {
            0x0000_0080
        } else if self.units.contains_key(&id) {
            0x0000_0040
        } else {
            0
        };
        army | class
    }
    /// Native `FUN_800d24d4` (`0x800d24d4`): returns the `flag.GTYPE_*` subtype bitmask for `id`.
    pub fn unit_gtype_flags(&self, id: ObjectId) -> u32 {
        let Some(runtime) = &self.rewrite else {
            return 0;
        };
        let objects = runtime.objects();
        let Some(o) = objects.get(&id) else {
            return 0;
        };
        let Some(base_id) = field(o, "mBase").and_then(|s| s.parse::<u64>().ok()) else {
            return 0;
        };
        let Some(base) = objects.get(&ObjectId(base_id)) else {
            return 0;
        };
        match field(base, "mUnitSubType").unwrap_or("") {
            "UNIT_AIR_STRATOFORTRESS" => 0x0000_0001,
            "UNIT_AIR_BOMBER" => 0x0000_0002,
            "UNIT_AIR_TRANSPORT" => 0x0000_0004,
            "UNIT_AIR_GUNSHIP" => 0x0000_0008,
            "UNIT_AIR_FIGHTER" => 0x0000_0010,
            "UNIT_SEA_DREADNAUGHT" => 0x0000_0020,
            "UNIT_SEA_BATTLESHIP" => 0x0000_0040,
            "UNIT_SEA_FRIGATE" => 0x0000_0080,
            "UNIT_SEA_SUBMARINE" => 0x0000_0100,
            "UNIT_SEA_GUNBOAT" => 0x0000_0200,
            "UNIT_VEH_BATTLESTATION" => 0x0000_0400,
            "UNIT_VEH_ANTI_AIR" => 0x0000_0800,
            "UNIT_VEH_ARTILLARY" => 0x0000_1000,
            "UNIT_VEH_HEAVY_TANK" => 0x0000_2000,
            "UNIT_VEH_LIGHT_TANK" => 0x0000_4000,
            "UNIT_VEH_HEAVY_TRANSPORT" => 0x0000_8000,
            "UNIT_VEH_LIGHT_TRANSPORT" => 0x0001_0000,
            "UNIT_VEH_RECON" => 0x0002_0000,
            "UNIT_MAN_SHOTGUN" => 0x0004_0000,
            "UNIT_MAN_ANTI_AIR" => 0x0008_0000,
            "UNIT_MAN_MORTAR" => 0x0010_0000,
            "UNIT_MAN_FLAMER" => 0x0020_0000,
            "UNIT_MAN_BAZOOKA" => 0x0040_0000,
            "UNIT_MAN_CHAINGUN" => 0x0080_0000,
            "UNIT_MAN_GRUNT" => 0x0100_0000,
            _ => 0,
        }
    }
    /// Local-only environment for original scoring scripts, not network replication.
    /// Symbolic enums deliberately do not invent the original engine's numeric ABI.
    pub fn install(vm: &mut Bootstrap) -> Result<()> {
        if vm.globals.contains_key("constant")
            || vm.globals.contains_key("netvariable")
            || vm.globals.contains_key("GRADE")
        {
            return Err("Gameplay environment already installed".into());
        }
        let mut constants = BTreeMap::new();
        for name in [
            "PLAYER_ONE",
            "PLAYER_TWO",
            "ARMY_WF",
            "ARMY_XYLVANIAN",
            "ARMY_TUNDRAN",
            "ARMY_SOLAR",
            "ARMY_NEUTRAL",
            "ARMY_UNDERWORLD",
            "ARMY_ANGLO",
            "OBJECTIVE_MARKER_DATA_VISIBLE",
            "OBJECTIVE_MARKER_DATA_ATTACHED_TO",
            "OBJECTIVE_DATA_VISIBLE",
            "OBJECTIVE_DATA_STATE",
            "NEW_PRIMARY_OBJECTIVE",
            "PRIMARY_OBJECTIVE_COMPLETE",
            "NEW_SECONDARY_OBJECTIVE",
            "SECONDARY_OBJECTIVE_COMPLETE",
            "ORDER_FORCED",
            "ORDER_NORMAL",
            "FORMATION_DISALLOWED",
            "FORMATION_ALLOWED",
            "ACTIVE",
            "INACTIVE",
            "ADJUST_WEAPON",
            "DONT_ADJUST_WEAPON",
            "ID_NONE",
            "SMOOTH",
            "IMMEDIATE",
            "FADE_IN",
            "FADE_OUT",
            "WAIT",
            "NO_WAIT",
            "INTRO",
            "MAIN_MUSIC",
            "OUTRO",
            "TOTAL_DEAD_HEALTH",
            "TOTAL_ALIVE_HEALTH",
            "MISSIONOVER_LOSE",
            "MISSIONOVER_PLAYER1_WINS",
            "MISSIONOVER_PLAYER2_WINS",
            "MISSIONOVER_BOTH_WIN",
            "ACTION_STATE_ATTACK",
            "ACTION_STATE_FOLLOWING",
            "ACTION_STATE_FOLLOWING_PLAYER",
            "ACTION_STATE_GOING_TO_CAPTUREPOINT",
            "ACTION_STATE_GUARD",
            "ACTION_STATE_IN_CAPTUREPOINT",
            "ACTION_STATE_IN_UNIT_CARRIER",
            "ACTION_STATE_MOVING_TO_AREA",
            "AI_ATTACK_STYLE",
            "ATTACKSTYLE_AGGRESSIVE",
            "CAPTUREPOINTFLAG_LOWERING",
            "CAPTUREPOINTFLAG_RAISING",
            "MOVEMENT_STATE_ON_GROUND",
            "MOVEMENT_STATE_AIRBORNE",
            "MOVEMENT_STATE_BEACHED",
            "MOVEMENT_STATE_SWIMMING",
            "MOVEMENT_STATE_SAILING",
            "TRANSFER_BYDEATH",
            "ARMY_TUNDRAN",
            "ARMY_UNDERWORLD",
            "ARMY_XYLVANIAN",
            "ARMY_NEUTRAL",
            "ATTACKSTYLE_DEFENSIVE",
            "MOVESTYLE_ATTACK_ON_SIGHT",
            "MOVESTYLE_RESPOND_TO_ATTACK",
            "FILENAME",
            "ONCE",
            "LOOPED",
            "PLAYER_TWO",
            "PLAYER_THREE",
            "PLAYER_FOUR",
            "HUD_PLAYERICON",
            "HUD_FUNC_INDICATOR_NONE",
            "GCONTROL_PAUSE_SELECT",
            "TYPE",
            "GTYPE",
        ] {
            constants.insert(name.into(), Value::Enum(name.into()));
        }
        for name in crate::mission_controls::ITEMS
            .iter()
            .chain(crate::mission_hud::ITEMS)
            .chain(crate::mission_tutorial::ITEMS)
            .chain(crate::mission_tutorial::ENUMS)
            .chain(crate::mission_input::MODES)
            .chain(["HUD_ITEM_OFF", "HUD_ITEM_ON", "HUD_ITEM_FLASH"].iter())
        {
            constants.insert((*name).into(), Value::Enum((*name).into()));
        }
        let index = vm.tables.len();
        constants.insert(
            "ACTION_STATE_FOLLOWING_PLAYER".into(),
            Value::Enum("ACTION_STATE_FOLLOWING".into()),
        );
        vm.tables.push(constants);
        vm.globals.insert("constant".into(), Value::Table(index));
        let mut flags = BTreeMap::new();
        for (name, bits) in [
            ("TYPE_WFRONTIER", 0x0000_0001u32 as f64),
            ("TYPE_XYLVANIAN", 0x0000_0002u32 as f64),
            ("TYPE_TUNDRAN", 0x0000_0020u32 as f64),
            ("TYPE_SOLAR", 0x0000_0400u32 as f64),
            ("TYPE_UNDERWORLD", 0x0000_0800u32 as f64),
            ("TYPE_ANGLO", 0x0002_0000u32 as f64),
            ("TYPE_NEUTRAL", 0x0000_1000u32 as f64),
            ("TYPE_GVEHICLE", 0x0000_0040u32 as f64),
            ("TYPE_AIRCRAFT", 0x0000_0080u32 as f64),
            ("TYPE_INFANTRY", 0x0000_0100u32 as f64),
            ("TYPE_HORSE", 0x0000_2000u32 as f64),
            ("TYPE_PICKUP", 0x0010_0000u32 as f64),
            ("TYPE_ALL", 0x0000_ffffu32 as f64),
            ("GTYPE_STRATOFORTRESS", 0x0000_0001u32 as f64),
            ("GTYPE_BOMBER", 0x0000_0002u32 as f64),
            ("GTYPE_TRANSPORT", 0x0000_0004u32 as f64),
            ("GTYPE_GUNSHIP", 0x0000_0008u32 as f64),
            ("GTYPE_FIGHTER", 0x0000_0010u32 as f64),
            ("GTYPE_DREADNAUGHT", 0x0000_0020u32 as f64),
            ("GTYPE_BATTLESHIP", 0x0000_0040u32 as f64),
            ("GTYPE_FRIGATE", 0x0000_0080u32 as f64),
            ("GTYPE_SUBMARINE", 0x0000_0100u32 as f64),
            ("GTYPE_GUNBOAT", 0x0000_0200u32 as f64),
            ("GTYPE_BATTLESTATION", 0x0000_0400u32 as f64),
            ("GTYPE_ANTI_AIR", 0x0000_0800u32 as f64),
            ("GTYPE_ARTILLARY", 0x0000_1000u32 as f64),
            ("GTYPE_HEAVY_TANK", 0x0000_2000u32 as f64),
            ("GTYPE_LIGHT_TANK", 0x0000_4000u32 as f64),
            ("GTYPE_HEAVY_TRANSPORT", 0x0000_8000u32 as f64),
            ("GTYPE_LIGHT_TRANSPORT", 0x0001_0000u32 as f64),
            ("GTYPE_RECON", 0x0002_0000u32 as f64),
            ("GTYPE_SHOTGUN", 0x0004_0000u32 as f64),
            ("GTYPE_VET_ANTI_AIR", 0x0008_0000u32 as f64),
            ("GTYPE_GRENADE", 0x0010_0000u32 as f64),
            ("GTYPE_FLAMER", 0x0020_0000u32 as f64),
            ("GTYPE_VET_ANTI_ARMOUR", 0x0040_0000u32 as f64),
            ("GTYPE_VET_HMG", 0x0080_0000u32 as f64),
            ("GTYPE_GRUNT", 0x0100_0000u32 as f64),
            ("GTYPE_ALL", 0xffff_ffffu32 as f64),
            ("LAND", 0x0000_0001u32 as f64),
            ("SOUND", 0x0000_0001u32 as f64),
            ("VISUAL", 0x0000_0002u32 as f64),
        ] {
            flags.insert(name.into(), Value::Number(bits));
        }
        let index = vm.tables.len();
        vm.tables.push(flags);
        vm.globals.insert("flag".into(), Value::Table(index));
        let mut mode_action = BTreeMap::new();
        for name in ["Follow", "Wait", "Guard", "Attack"] {
            mode_action.insert(name.into(), Value::Enum(name.into()));
        }
        let index = vm.tables.len();
        vm.tables.push(mode_action);
        vm.globals.insert("eModeAction".into(), Value::Table(index));
        let mut grades = BTreeMap::new();
        for name in ["C", "B", "A", "S"] {
            grades.insert(name.into(), Value::Enum(format!("GRADE_{name}")));
        }
        let index = vm.tables.len();
        vm.tables.push(grades);
        vm.globals.insert("GRADE".into(), Value::Table(index));
        let mut net = BTreeMap::new();
        for name in ["player1", "player2"] {
            let index = vm.tables.len();
            vm.tables.push(BTreeMap::new());
            net.insert(name.into(), Value::Table(index));
        }
        let index = vm.tables.len();
        vm.tables.push(net);
        vm.globals.insert("netvariable".into(), Value::Table(index));
        for name in [
            "GetPlayerUnit",
            "GetCurrentMissionAttempted",
            "GetCurrentMissionCompleted",
            "SetCurrentMissionCompleted",
            "GetTutorialMode",
            "GetRandom",
            "EnableControllerItem",
            "SetHudState",
            "SetFunctionIndicatorState",
            "SetupPauseTutorialStack",
            "ForceUnitTransfer",
            "GoToArea",
            "Teleport",
            "AttackTarget",
            "StopAndGuard",
            "EnableWeapon",
            "FollowWaypoint",
            "IsDead",
            "IsUnderPlayerCommand",
            "Kill",
            "DestroyablePlayAnim",
            "PlayEffect",
            "SetUnitInvulnerable",
            "SetHealthPercent",
            "GetHealthPercent",
            "SetActive",
            "Spawn",
            "Despawn",
            "AllPlayerUnitsDead",
            "FreezePlayer",
            "UnfreezePlayer",
            "IsInArea",
            "IsInRange",
            "SetObjectiveData",
            "DeclareVictory",
            "StoreSummaryMessage",
            "ListScoringUnits",
            "Measure",
            "rint",
            "GetFriendlyHealthTotals",
            "GetEnemyHealthTotals",
            "SetMissionPowerObjectives",
            "SetMissionTechObjectives",
            "SetArmyAllegianceForScoring",
            "SetMissionMinMaxTime",
            "SetMissionTime",
            "SetMissionRatings",
            "SetMissionComplete",
            "IsNetworkHost",
            "IsNetworkPlayer",
            "AddToGroup",
            "BeachWaterUnit",
            "CameraShake",
            "CheckCapturePoint",
            "CheckSkipCutscene",
            "CombineFlags",
            "CountRemainingPlayerTroops",
            "EnterVehicle",
            "ExitVehicle",
            "FollowUnit",
            "GetActionState",
            "GetGroupHealthPercent",
            "GetGroupMember",
            "GetInstallationOwner",
            "GetMovementState",
            "GetNumMatchingInGroup",
            "GetNumSeatsFree",
            "GetPlayerTransferred",
            "GetPrimaryActionState",
            "GetSpawnMode",
            "GetState",
            "GetTimeLeft",
            "IsActive",
            "IsAttacking",
            "IsPassengerInUnit",
            "IsReadyToRevive",
            "LandAirUnit",
            "NumItemsInArea",
            "NumPassengersInUnit",
            "PrepareSkipCutscene",
            "PutUnitInVehicle",
            "ResetHealthAndScoreTotals",
            "ResetPathBlocking",
            "ReviveDeadUnit",
            "ReviveFullUnitCarrier",
            "SetCapturePointHeight",
            "SetDamageZoneDamage",
            "SetEnableGuardMode",
            "SetInstallationOwner",
            "SetInvulnerable",
            "SetRenderHUDComponent",
            "SetTickScriptWhenDead",
            "SetTickScriptWhenDespawned",
            "SetTimer",
            "SetUnitAI",
            "SetUnitShadowDistance",
            "ShowTimer",
            "StopCapturePointSounds",
            "Vanish",
            "VictoryDance",
            "CleanupSkipCutscene",
            "CoopSetup",
            "CreateGroup",
            "DefeatLament",
            "FindDeadUnit",
            "GetAttackTarget",
            "GetGroupHealth",
            "GetMissionEnded",
            "GetMissionWon",
            "GetMissionLost",
            "GetRange",
            "GetScoreTotals",
            "GetFriendlyScoreTotals",
            "GetEnemyScoreTotals",
            "GetHealthTotals",
            "AddStoredHealthAndScoreTotals",
            "GetSubmarineAirpercentage",
            "GetTargetedObject",
            "GetUnitInSeat",
            "IsSubmarineSubmerged",
            "NetworkGameOver",
            "RenderOptionalHUDComponent",
            "RepositionSkirmishScoreBoxes",
            "RollAirUnit",
            "SetCameraEffectFadeIn",
            "SetCameraEffectFadeOut",
            "SetCameraEffectIntensity",
            "SetCharacterAnimUpdateLOD",
            "SetDistortionEffect",
            "SetHUDMultiplePlayerArmies",
            "SetNetworkMissionComplete",
            "SetNogoZone",
            "SetObjectivesResult",
            "GetObjectivesResult",
            "SetPathReblocking",
            "SetReflectionRenderObjects",
            "SetRenderParamTintBlendFactor",
            "SpawnUnitInCarrier",
            "SpawnUnit",
            "StoreHealthAndScoreTotals",
            "SubmarineDive",
            "SubmarineSurface",
            "TestFlags",
            "WillReviveInGuard",
            "mod",
            "RemoveFromGroup",
            "ExtractFromGroup",
            "CopyGroup",
            "GetContainerOf",
            "GetPrimaryActionTarget",
            "GetKiller",
            "IsInRectangle",
            "IsOnFire",
            "TurnUnitLightsOn",
            "PlaySound",
            "DestroyableGetCurrentAnim",
            "SetUnitAllegiance",
            "SetIdle",
            "ActivateArmyEffect",
            "DectivateArmyEffect",
            "SetTutorialBButtonState",
            "SetFogColour",
            "SetSunAmbientColour",
            "SetSunDirectionalColour",
            "SetAntiSunDirectionalColour",
            "SetSunDirection",
            "SetAntiSunDirection",
            "Brace",
            "Cower",
            "SetSkirmishScoreLocked",
            "GetDistanceBetweenPlayers",
        ] {
            vm.globals.insert(name.into(), Value::Native(name.into()));
        }
        Ok(())
    }
    fn army_enum_to_internal(army: &str) -> Option<&'static str> {
        match army {
            "ARMY_WF" => Some("eWesternFrontier"),
            "ARMY_XYLVANIAN" => Some("eXylvanian"),
            "ARMY_TUNDRAN" => Some("eTundranTerritories"),
            "ARMY_SOLAR" => Some("eSolarEmpire"),
            "ARMY_NEUTRAL" => Some("eNeutral"),
            "ARMY_UNDERWORLD" => Some("eUnderWorld"),
            "ARMY_ANGLO" => Some("eAngloIsles"),
            _ => None,
        }
    }
    fn internal_to_army_enum(army: &str) -> Value {
        match army {
            "eWesternFrontier" => Value::Enum("ARMY_WF".into()),
            "eXylvanian" => Value::Enum("ARMY_XYLVANIAN".into()),
            "eTundranTerritories" | "eTundran" => Value::Enum("ARMY_TUNDRAN".into()),
            "eSolarEmpire" => Value::Enum("ARMY_SOLAR".into()),
            "eUnderWorld" | "eUnderworld" => Value::Enum("ARMY_UNDERWORLD".into()),
            "eAngloIsles" => Value::Enum("ARMY_ANGLO".into()),
            _ => Value::Number(0.0),
        }
    }
    pub(crate) fn call(
        &mut self,
        name: &str,
        args: &[Value],
        positions: &BTreeMap<ObjectId, [f32; 3]>,
    ) -> Result<Vec<Value>> {
        if self.rewrite.is_some() {
            if crate::mission_tutorial::Tutorial::handles(name, args) {
                return self
                    .tutorial
                    .as_mut()
                    .ok_or("Tutorial backend missing")?
                    .call(name, args);
            }
            if name == "EnableControllerItem"
                && matches!(args,[_,_,Value::Enum(p)] if p=="PLAYER_TWO")
            {
                return self
                    .secondary_controller
                    .as_mut()
                    .ok_or("Secondary controller state missing")?
                    .call(&args[..2]);
            }
            if name == "ClearMessageQueue" && matches!(args,[Value::Enum(p)] if p=="PLAYER_TWO") {
                return self
                    .secondary_messages
                    .as_mut()
                    .ok_or("Secondary message state missing")?
                    .call(name, &[]);
            }
            if let Some(Value::Object(gid)) = args.first()
                && self.unit_groups.contains_key(gid)
                && !matches!(
                    name,
                    "AddToGroup"
                        | "GetGroupMember"
                        | "GetGroupHealthPercent"
                        | "GetGroupHealth"
                        | "GetNumMatchingInGroup"
                        | "CreateGroup"
                        | "RemoveFromGroup"
                        | "ExtractFromGroup"
                        | "CopyGroup"
                )
            {
                let members = self.unit_groups.get(gid).cloned().unwrap_or_default();
                let mut result = vec![Value::Boolean(true)];
                for mid in members {
                    let mut margs = args.to_vec();
                    margs[0] = Value::Object(mid);
                    if let Ok(r) = self.call(name, &margs, positions) {
                        result = r;
                    }
                }
                return Ok(result);
            }
        }
        match (name, args) {
            (
                "PlayEffect",
                [
                    Value::Object(effect),
                    Value::Number(flags),
                    Value::Object(target),
                ],
            ) if self.rewrite.is_some() && matches!(*flags as u32, 1..=3) => {
                if !positions.contains_key(target)
                    && !self
                        .cameras
                        .as_ref()
                        .is_some_and(|r| r.bank.cameras.contains_key(target))
                {
                    return Err("Unknown scripted-effect target".into());
                }
                self.rewrite
                    .as_mut()
                    .unwrap()
                    .omit_scripted_visual(*effect)?;
                return Ok(vec![Value::Boolean(true)]);
            }
            ("PlayerAngleToTarget", [Value::Object(id)]) if self.rewrite.is_some() => {
                return Ok(vec![Value::Number(
                    self.player_angle_to_target(*id, positions)?,
                )]);
            }
            ("ResetCommandBar", []) if self.rewrite.is_some() => {
                self.rewrite.as_mut().unwrap().reset_command_bar()?;
                self.command_selection.clear();
                if self.collision.as_ref().is_some_and(|s| s.ground.is_some())
                    && let Some(player) = self.player
                    && let Ok(p) = self.entity(player).cloned()
                {
                    for &uid in &self.playable_units {
                        if self
                            .units
                            .get(&uid)
                            .is_some_and(|u| u.army == p.army && u.active == Some(true) && u.health > 0.0)
                            && self
                                .activity
                                .get(&uid)
                                .is_none_or(|a| a.enabled != Some(false))
                        {
                            self.roster.insert(uid);
                        }
                    }
                }
                return Ok(vec![]);
            }
            ("IsInRange", [Value::Object(a), Value::Object(b), Value::Number(radius)])
                if self.rewrite.is_some() =>
            {
                if !radius.is_finite() || *radius < 0.0 {
                    return Err("Invalid range radius".into());
                }
                let p = positions.get(a).ok_or("Missing range origin")?;
                let q = positions.get(b).ok_or("Missing range target")?;
                if p.iter().chain(q).any(|v| !v.is_finite()) {
                    return Err("Invalid range position".into());
                }
                // Rust ground-command range policy, matching our movement/recruitment plane.
                return Ok(vec![Value::Boolean(
                    (p[0] as f64 - q[0] as f64).hypot(p[2] as f64 - q[2] as f64) <= *radius,
                )]);
            }
            ("GetActivity", []) if self.rewrite.is_some() => {
                return Ok(vec![Value::Number(
                    self.rewrite.as_ref().unwrap().activity_score,
                )]);
            }
            ("SetViewDistanceAdjustment", [Value::Number(adjustment), Value::Number(frames)])
                if self.rewrite.is_some() =>
            {
                self.rewrite
                    .as_mut()
                    .unwrap()
                    .request_view_distance(*adjustment, *frames)?;
                return Ok(vec![]);
            }
            ("SetWaterRenderEnable", [Value::Boolean(enabled)]) if self.rewrite.is_some() => {
                self.rewrite
                    .as_mut()
                    .unwrap()
                    .request_water_render(*enabled)?;
                return Ok(vec![]);
            }
            (
                "PlayEffect",
                [
                    Value::Object(id),
                    Value::Number(flags),
                    Value::Number(x),
                    Value::Number(y),
                    Value::Number(z),
                ],
            ) if self.rewrite.is_some() => {
                if !flags.is_finite()
                    || *flags < 0.0
                    || *flags > u32::MAX as f64
                    || flags.fract() != 0.0
                {
                    return Err("Invalid scripted effect flags".into());
                }
                self.rewrite.as_mut().unwrap().play_effect(
                    *id,
                    *flags as u32,
                    [*x as f32, *y as f32, *z as f32],
                )?;
                return Ok(vec![Value::Boolean(true)]);
            }
            ("Kill", [Value::Object(id), rest @ ..])
                if self.rewrite.is_some()
                    && (rest.is_empty() || matches!(rest, [Value::Boolean(_)])) =>
            {
                if self.rewrite.as_ref().is_some_and(|r| {
                    r.objects().get(id).is_some_and(|o| {
                        o.kind == "cGlobalScriptEntity" || o.kind == "cInitialisationScriptEntity"
                    })
                }) {
                    self.killed_scripts.insert(id.0);
                    return Ok(vec![]);
                }
                self.kill_rewrite_in_scene(*id, positions)?;
                return Ok(vec![]);
            }
            (
                "ClearMessageQueue"
                | "GetNumItemsInMessageQueue"
                | "PhoneMessage"
                | "PhoneMessageWithObjective",
                _,
            ) => {
                return self
                    .messages
                    .as_mut()
                    .ok_or("Radio lifecycle backend not configured")?
                    .call(name, args);
            }
            ("SetCamera", [Value::Object(id), rest @ ..])
                if self.rewrite.is_some()
                    && (rest.is_empty()
                        || matches!(rest, [Value::Enum(player)] if player == "PLAYER_ONE")
                        || matches!(rest, [Value::Number(n)] if *n == 0.0))
                    && self.cameras.as_ref().is_some_and(|r| {
                        r.bank
                            .cameras
                            .get(id)
                            .is_some_and(|c| c.kind == "eCAMTYPE_CHASETARGET")
                    }) =>
            {
                let target = self.player.ok_or("Missing chase player")?;
                if !positions.contains_key(&target) {
                    return Err("Missing chase player position".into());
                }
                let rig = self.cameras.as_mut().unwrap();
                rig.set_chase_aim(self.player_aim.ok_or("Player aim not configured")?)?;
                rig.activate_chase(*id, target)?;
                return Ok(vec![]);
            }
            ("SetCamera" | "CameraSetWaypoint" | "CameraSetTarget", _) => {
                return self
                    .cameras
                    .as_mut()
                    .ok_or("Scripted camera backend not configured")?
                    .call(name, args, positions);
            }
            ("StartMusic" | "StopMusic" | "SetMusicVolume", _) => {
                return self
                    .music
                    .as_mut()
                    .ok_or("Music PCM backend not configured")?
                    .call(name, args);
            }
            ("SetHudState" | "SetFunctionIndicatorState", _) => {
                return self
                    .hud
                    .as_mut()
                    .ok_or("HUD presentation backend not configured")?
                    .call(name, args);
            }
            ("EnableControllerItem", _) => {
                return self
                    .controller
                    .as_mut()
                    .ok_or("Controller mask backend not configured")?
                    .call(args);
            }
            ("GetRandom", args) => {
                return self
                    .random
                    .as_mut()
                    .ok_or("Native random cursor not configured")?
                    .call(args);
            }
            // RBWE01 native 0x800dd5d4 checks zero arguments then pushes literal true.
            // This is recovered build behavior, not a permissive placeholder.
            ("GetTutorialMode", []) => return Ok(vec![Value::Boolean(true)]),
            ("GetCurrentMissionAttempted" | "GetCurrentMissionCompleted", []) => {
                let history = self
                    .mission_history
                    .as_ref()
                    .ok_or("Current mission profile history not configured")?;
                return Ok(vec![Value::Boolean(
                    if name == "GetCurrentMissionAttempted" {
                        history.attempted
                    } else {
                        history.completed
                    },
                )]);
            }
            ("Spawn" | "Despawn", [Value::Object(id), rest @ ..]) => {
                let retain = match (name, rest) {
                    (_, []) => false,
                    ("Despawn", [Value::Boolean(flag)]) => *flag,
                    _ => return Err("Unsupported lifecycle signature".into()),
                };
                if let Some(pickups) = self.pickups.as_mut()
                    && pickups.contains(*id)
                {
                    // 0x800e9ed8 branches to the pickup path before reading the
                    // optional retain argument, so it has no pickup effect.
                    let _ = retain;
                    if name == "Spawn" {
                        pickups.spawn(*id)?;
                    } else {
                        pickups.despawn(*id)?;
                    }
                    return Ok(vec![]);
                }
                if self.rewrite.is_some() {
                    if name == "Spawn" && self.entity(*id).is_ok_and(|e| e.health <= 0.0) {
                        return Ok(vec![]);
                    }
                    if name == "Despawn"
                        && self.collision.as_ref().is_some_and(|s| s.ground.is_some())
                        && let Some(order) = self.bombers.orders.get(id).cloned()
                        && self.buildings.contains(&order.target)
                        && self.entity(order.target).is_ok_and(|e| e.health > 0.0)
                    {
                        self.kill_rewrite_in_scene(order.target, positions)?;
                    }
                    self.set_rewrite_presence(*id, name == "Spawn", retain)?;
                    if name == "Spawn"
                        && self.collision.as_ref().is_some_and(|s| s.ground.is_some())
                        && self.playable_units.contains(id)
                        && self
                            .activity
                            .get(id)
                            .is_none_or(|a| a.enabled != Some(false))
                        && let Some(player_army) = self
                            .player
                            .and_then(|p| self.entity(p).ok().map(|u| u.army.clone()))
                            .or_else(|| {
                                self.roster
                                    .iter()
                                    .find_map(|rid| self.units.get(rid).map(|u| u.army.clone()))
                            })
                        && self
                            .units
                            .get(id)
                            .is_some_and(|u| u.army == player_army && u.health > 0.0)
                    {
                        self.roster.insert(*id);
                    }
                    if name == "Spawn"
                        && self.scripted_air_cruise.contains_key(id)
                        && !self.water_units.contains(id)
                    {
                        if !self.headings.contains_key(id) {
                            let h = self.air_shapes.get(id).map_or(0.0, |s| s.heading);
                            self.headings.insert(*id, h);
                        }
                        if !self.grounded_units.contains(id)
                            && let Some(&p) = positions.get(id)
                            && let Some(&hag) = self.scripted_air_altitude.get(id)
                            && let Some(scene) = &self.collision
                            && let Ok(air_pos) = scene.world_point([p[0], 0.0, p[2]], hag)
                            && p[1] < air_pos[1] - 1.0
                        {
                            self.pending_positions.insert(*id, [p[0], air_pos[1], p[2]]);
                        }
                    }
                    return Ok(vec![]);
                }
                if self.capture_points.contains(id) {
                    return Err("Capture-point lifecycle backend not configured".into());
                }
                if !self.units.contains_key(id) && !self.buildings.contains(id) {
                    return Err("Unsupported lifecycle entity type".into());
                }
                if self.entity(*id)?.health <= 0.0 {
                    return Err("Lifecycle transition of a dead entity unsupported".into());
                }
                self.entity_mut(*id)?.active = Some(name == "Spawn");
                if name == "Despawn" {
                    self.despawn_retention.insert(*id, retain);
                    if self.player == Some(*id) && self.player_lifecycle.is_some() {
                        self.player_lifecycle = Some(PlayerLifecycle::ReplacementRequired);
                    }
                    self.orders.remove(id);
                    self.following.remove(id);
                    self.routes.remove(id);
                    self.direct_velocity.remove(id);
                }
            }
            ("EnableWeapon", [Value::Object(id), enabled_val]) if self.rewrite.is_some() => {
                let enabled = match enabled_val {
                    Value::Boolean(b) => *b,
                    Value::Number(n) if *n == 0.0 || *n == 1.0 => *n != 0.0,
                    Value::Enum(s) if s == "ACTIVE" => true,
                    Value::Enum(s) if s == "INACTIVE" => false,
                    _ => return Err("Invalid EnableWeapon flag".into()),
                };
                if !self.units.contains_key(id) && !self.buildings.contains(id) {
                    return Err("Unknown weapon unit".into());
                }
                self.activity.entry(*id).or_default().weapons_enabled = Some(enabled);
            }
            (
                "SetActive",
                [
                    target @ (Value::Object(_) | Value::Number(_)),
                    active_val,
                    rest @ ..,
                ],
            ) if matches!(
                active_val,
                Value::Enum(s) if matches!(s.as_str(), "ACTIVE" | "INACTIVE")
            ) || (self.rewrite.is_some()
                && (matches!(active_val, Value::Boolean(_))
                    || matches!(active_val, Value::Number(n) if *n == 0.0 || *n == 1.0))) =>
            {
                let is_active = match active_val {
                    Value::Enum(s) => s == "ACTIVE",
                    Value::Boolean(b) => *b,
                    Value::Number(n) => *n != 0.0,
                    _ => unreachable!(),
                };
                let id = match target {
                    Value::Object(id) => *id,
                    Value::Number(n) if *n >= 0.0 && n.fract() == 0.0 => ObjectId(*n as u64),
                    _ => return Err("Invalid SetActive target".into()),
                };
                let adjust = match rest {
                    [] => false,
                    [Value::Enum(flag)] if flag == "ADJUST_WEAPON" => true,
                    [Value::Enum(flag)] if flag == "DONT_ADJUST_WEAPON" => false,
                    [Value::Boolean(b)] if self.rewrite.is_some() => *b,
                    [Value::Number(n)] if self.rewrite.is_some() && (*n == 0.0 || *n == 1.0) => {
                        *n != 0.0
                    }
                    _ => return Err("Unsupported SetActive weapon adjustment".into()),
                };
                let targets: Vec<ObjectId> = if self.rewrite.is_some()
                    && let Some(members) = self.unit_groups.get(&id)
                {
                    members.clone()
                } else {
                    vec![id]
                };
                for tid in targets {
                    if !self.units.contains_key(&tid)
                        && !self.activity.contains_key(&tid)
                        && !(self.rewrite.is_some() && self.props.contains_key(&tid))
                    {
                        return Err("Unknown activity target".into());
                    }
                    if adjust
                        && !self.units.contains_key(&tid)
                        && !self.buildings.contains(&tid)
                        && !(self.rewrite.is_some() && self.props.contains_key(&tid))
                    {
                        return Err("Weapon activity on non-unit unsupported".into());
                    }
                    let state = self.activity.entry(tid).or_default();
                    state.enabled = Some(is_active);
                    if adjust {
                        state.weapons_enabled = state.enabled;
                    }
                    if self.rewrite.is_some()
                        && self.collision.as_ref().is_some_and(|s| s.ground.is_some())
                    {
                        if is_active {
                            if self.playable_units.contains(&tid)
                                && let Some(player) = self.player
                                && let (Some(u), Ok(p)) = (self.units.get(&tid), self.entity(player))
                                && u.army == p.army
                                && u.active == Some(true)
                                && u.health > 0.0
                            {
                                self.roster.insert(tid);
                                if tid != player {
                                    self.squad_follow.insert(tid);
                                }
                            }
                        } else if self.player != Some(tid) {
                            self.roster.remove(&tid);
                            self.squad_follow.remove(&tid);
                        }
                    }
                }
            }
            ("GetHealthPercent", [Value::Object(id)]) => {
                if let Some(cp) = self.capture_point_state.get(id) {
                    let player_army = self
                        .player
                        .and_then(|p| self.entity(p).ok())
                        .map(|u| u.army.as_str())
                        .unwrap_or("eSolarEmpire");
                    let pct = if cp.owner == player_army
                        || (cp.owner == "eNeutral"
                            && cp.capturing_army.as_deref() == Some(player_army))
                    {
                        cp.height
                    } else {
                        0.0
                    };
                    return Ok(vec![Value::Number(pct)]);
                }
                let unit = self.entity(*id)?;
                return Ok(vec![Value::Number(unit.health / unit.max_health * 100.0)]);
            }
            (
                "IsInArea",
                [
                    Value::Object(id),
                    Value::Number(x),
                    Value::Number(z),
                    Value::Number(radius),
                ],
            ) => {
                if [x, z, radius].iter().any(|v| !v.is_finite()) || *radius < 0.0 {
                    return Err("Invalid circular area query".into());
                }
                let p = positions.get(id).ok_or("Unknown object position")?;
                if p.iter().any(|v| !v.is_finite()) {
                    return Err("Invalid query position".into());
                }
                return Ok(vec![Value::Boolean(
                    (p[0] as f64 - x).hypot(p[2] as f64 - z) <= *radius,
                )]);
            }
            (
                "FollowWaypoint",
                [
                    Value::Object(id),
                    Value::Object(start),
                    Value::Number(x),
                    Value::Number(z),
                    Value::Enum(order),
                    formation @ ..,
                ],
            ) if (order == "ORDER_FORCED" && formation.is_empty())
                || (self.rewrite.is_some()
                    && matches!(order.as_str(), "ORDER_FORCED" | "ORDER_NORMAL")
                    && matches!(formation, [] | [Value::Enum(_)])) =>
            {
                if let [Value::Enum(flag)] = formation
                    && !matches!(flag.as_str(), "FORMATION_DISALLOWED" | "FORMATION_ALLOWED")
                {
                    return Err("Unsupported movement formation".into());
                }
                if self.units.get(id).is_some_and(|u| u.health <= 0.0)
                    && let Some(runtime) = &mut self.rewrite
                {
                    runtime
                        .rejected_commands
                        .insert(format!("FollowWaypoint rejected: unit {} is dead", id.0));
                    return Ok(vec![Value::Boolean(false)]);
                }
                self.follow_waypoint(*id, *start, [*x, *z], positions)?;
                return Ok(vec![Value::Boolean(true)]);
            }
            ("FollowWaypoint", [Value::Object(id), Value::Object(start), rest @ ..])
                if self.rewrite.is_some()
                    && self.collision.as_ref().is_some_and(|s| s.ground.is_some()) =>
            {
                let offset = match rest {
                    [Value::Number(x), Value::Number(z), ..] if x.is_finite() && z.is_finite() => {
                        [*x, *z]
                    }
                    _ => [0.0, 0.0],
                };
                if self.units.get(id).is_some_and(|u| u.health <= 0.0) {
                    return Ok(vec![Value::Boolean(false)]);
                }
                if self.follow_waypoint(*id, *start, offset, positions).is_ok() {
                    return Ok(vec![Value::Boolean(true)]);
                }
                return Ok(vec![Value::Boolean(false)]);
            }
            (
                "GoToArea",
                [
                    Value::Object(id),
                    Value::Number(x),
                    Value::Number(z),
                    Value::Number(radius),
                    rest @ ..,
                ],
            ) if match rest {
                [] if self.rewrite.is_some() => true,
                [Value::Enum(order), formation @ ..]
                    if self.rewrite.is_some()
                        && matches!(order.as_str(), "ORDER_FORCED" | "ORDER_NORMAL")
                        && matches!(formation, [] | [Value::Enum(_)] | [Value::Enum(_), ..]) =>
                {
                    true
                }
                [Value::Number(reserved)] if self.rewrite.is_some() && *reserved == 0.0 => true,
                [Value::Number(reserved), Value::Enum(order), formation @ ..]
                    if (*reserved == 0.0
                        || (self.rewrite.is_some()
                            && self.collision.as_ref().is_some_and(|s| s.ground.is_some())))
                        && ((order == "ORDER_FORCED" && formation.is_empty())
                            || (self.rewrite.is_some()
                                && matches!(order.as_str(), "ORDER_FORCED" | "ORDER_NORMAL")
                                && matches!(
                                    formation,
                                    [] | [Value::Enum(_)] | [Value::Enum(_), ..]
                                ))) =>
                {
                    true
                }
                _ if self.rewrite.is_some()
                    && self.collision.as_ref().is_some_and(|s| s.ground.is_some()) =>
                {
                    true
                }
                _ => false,
            } =>
            {
                if self.collision.as_ref().is_none_or(|s| s.ground.is_none())
                    && let [_, _, Value::Enum(flag)] = rest
                    && !matches!(flag.as_str(), "FORMATION_DISALLOWED" | "FORMATION_ALLOWED")
                {
                    return Err("Unsupported movement formation".into());
                }
                if [x, z, radius].iter().any(|v| !v.is_finite())
                    || *radius < 0.0
                    || !(*x as f32).is_finite()
                    || !(*z as f32).is_finite()
                {
                    return Err("Invalid movement destination".into());
                }
                if self.rewrite.is_some()
                    && self.despawn_retention.get(id) == Some(&true)
                    && !self.carrier_of.contains_key(id)
                    && let Some(u) = self.units.get_mut(id)
                    && u.active == Some(false)
                    && u.health > 0.0
                {
                    u.active = Some(true);
                }
                let u = self.units.get(id).ok_or("Unknown ordered unit")?;
                if u.health <= 0.0
                    && let Some(runtime) = &mut self.rewrite
                {
                    runtime
                        .rejected_commands
                        .insert(format!("GoToArea rejected: unit {} is dead", id.0));
                    return Ok(vec![Value::Boolean(false)]);
                }
                let can_queue = self.rewrite.is_some() && u.active == Some(false);
                if (!can_queue && u.active != Some(true))
                    || u.health <= 0.0
                    || !positions.contains_key(id)
                {
                    return Err("Unit cannot receive movement order".into());
                }
                let speed = u.forward_speed.unwrap_or(8.0);
                if !speed.is_finite() || speed <= 0.0 {
                    return Err("Invalid movement speed".into());
                }
                if self.water_units.contains(id)
                    && self.unloading_on_arrival.contains(id)
                    && self.orders.contains_key(id)
                    && self
                        .passengers
                        .get(id)
                        .is_some_and(|l| l.iter().any(|p| p.0 != 0))
                {
                    return Ok(vec![Value::Boolean(true)]);
                }
                self.following.remove(id);
                self.follow_unit_orders.remove(id);
                self.squad_follow.remove(id);
                self.grounded_units.remove(id);
                self.landing_units.remove(id);
                self.unloading_on_arrival.remove(id);
                self.bombers.orders.remove(id);
                self.missiles.orders.remove(id);
                self.orders.insert(
                    *id,
                    MoveOrder {
                        target: [*x, *z],
                        radius: *radius,
                    },
                );
            }
            (
                "GoToArea",
                [
                    Value::Object(id),
                    Value::Number(x),
                    Value::Number(z),
                    _rest @ ..,
                ],
            ) if self.rewrite.is_some()
                && self.collision.as_ref().is_some_and(|s| s.ground.is_some()) =>
            {
                if x.is_finite() && z.is_finite() && self.units.contains_key(id) {
                    if self.water_units.contains(id)
                        && self.unloading_on_arrival.contains(id)
                        && self.orders.contains_key(id)
                        && self
                            .passengers
                            .get(id)
                            .is_some_and(|l| l.iter().any(|p| p.0 != 0))
                    {
                        return Ok(vec![Value::Boolean(true)]);
                    }
                    if self.despawn_retention.get(id) == Some(&true)
                        && !self.carrier_of.contains_key(id)
                        && let Some(u) = self.units.get_mut(id)
                        && u.active == Some(false)
                        && u.health > 0.0
                    {
                        u.active = Some(true);
                    }
                    self.grounded_units.remove(id);
                    self.landing_units.remove(id);
                    self.unloading_on_arrival.remove(id);
                    self.orders.insert(
                        *id,
                        MoveOrder {
                            target: [*x, *z],
                            radius: 5.0,
                        },
                    );
                    return Ok(vec![Value::Boolean(true)]);
                }
                return Ok(vec![Value::Boolean(false)]);
            }
            ("ForceUnitTransfer", [player_val, Value::Object(target), _options @ ..])
                if self.rewrite.is_some()
                    && matches!(player_val, Value::Enum(player) if player == "PLAYER_TWO") =>
            {
                let _ = target;
            }
            ("ForceUnitTransfer", [player_val, Value::Object(target), options @ ..])
                if matches!(player_val, Value::Enum(player) if player == "PLAYER_ONE")
                    || (self.rewrite.is_some()
                        && matches!(player_val, Value::Number(n) if *n == 0.0)) =>
            {
                let strict = self.collision.as_ref().is_none_or(|s| s.ground.is_none());
                let mut hard_cut = match options {
                    [] | [Value::Boolean(false)] => false,
                    [Value::Boolean(true)] | [Value::Boolean(true), Value::Boolean(_)] => true,
                    _ if !strict => true,
                    _ => return Err("Unsupported transfer options".into()),
                };
                // A hidden takeover needs no visible camera interpolation. Do not
                // silently substitute a cut for a visible native smooth transfer.
                if !hard_cut
                    && (!self.frozen || self.cameras.as_ref().is_none_or(|r| r.black_alpha != 1.0))
                {
                    if strict {
                        return Err(
                            "Visible default transfer camera transition not configured".into()
                        );
                    }
                    hard_cut = true;
                }
                let current = self.player.ok_or("Player unit not configured")?;
                let old = if strict {
                    self.units.get(&current).ok_or("Unknown current player")?
                } else {
                    self.entity(current)?
                };
                let mut target = *target;
                if !strict && !self.units.contains_key(&target) && !self.buildings.contains(&target)
                {
                    if let Some(members) = self.unit_groups.get(&target)
                        && let Some(&m) = members
                            .iter()
                            .find(|m| self.units.contains_key(m) || self.buildings.contains(m))
                    {
                        target = m;
                    } else {
                        return Ok(Vec::new());
                    }
                }
                let new = if strict {
                    self.units.get(&target).ok_or("Unknown transfer target")?
                } else {
                    self.entity(target)?
                };
                if !old.health.is_finite()
                    || !new.health.is_finite()
                    || new.health <= 0.0
                    || (strict
                        && (old.active != Some(true)
                            || old.health <= 0.0
                            || new.active != Some(true)
                            || old.army != new.army))
                {
                    return Err("Invalid infantry transfer eligibility".into());
                }
                if strict
                    && (!self.bodies.contains_key(&current) || !self.bodies.contains_key(&target))
                {
                    return Err("Non-infantry transfer not implemented".into());
                }
                let view = self
                    .player_view
                    .as_ref()
                    .ok_or("Player-camera state not configured")?;
                if strict && view.target != current {
                    return Err("Player-camera ownership mismatch".into());
                }
                if current == target {
                    self.direct_velocity.remove(&target);
                    self.orders.remove(&target);
                    self.follow_unit_orders.remove(&target);
                    self.following.remove(&target);
                    self.routes.remove(&target);
                    self.squad_follow.remove(&target);
                    self.command_selection.remove(&target);
                    self.missiles.orders.remove(&target);
                    return Ok(Vec::new());
                }
                let revision = view
                    .cut_revision
                    .checked_add(u64::from(hard_cut))
                    .ok_or("Camera cut revision overflow")?;
                if !hard_cut {
                    self.rewrite
                        .as_mut()
                        .ok_or("Default transfer requires rewrite backend")?
                        .allow_faded_transfer_presentation()?;
                }
                let view = self.player_view.as_mut().unwrap();
                view.target = target;
                view.cut_revision = revision;
                self.player = Some(target);
                let is_cutscene_dummy = self.frozen
                    && positions
                        .get(&target)
                        .is_some_and(|tp| tp[0].hypot(tp[2]) > 1400.0);
                if self.rewrite.is_some() {
                    if !is_cutscene_dummy {
                        self.roster.insert(target);
                    }
                    if !strict && !is_cutscene_dummy {
                        let act = self.activity.entry(target).or_default();
                        act.enabled = Some(true);
                        act.weapons_enabled = Some(true);
                        let target_army = self
                            .entity(target)
                            .map(|u| u.army.clone())
                            .unwrap_or_default();
                        for group in self.unit_groups.values() {
                            if group.contains(&target) {
                                for &uid in group {
                                    if self.units.get(&uid).is_some_and(|u| u.army == target_army)
                                        && self
                                            .activity
                                            .get(&uid)
                                            .is_none_or(|a| a.enabled != Some(false))
                                    {
                                        self.roster.insert(uid);
                                    }
                                }
                            }
                        }
                    }
                    if let Some(rig) = &mut self.cameras {
                        rig.retarget_chase(target);
                    }
                }
                if self.player_lifecycle.is_some() {
                    self.player_lifecycle = Some(PlayerLifecycle::FreshAlive(target));
                }
                self.direct_velocity.remove(&current);
                self.direct_velocity.remove(&target);
                self.orders.remove(&target);
                self.follow_unit_orders.remove(&target);
                if !is_cutscene_dummy {
                    if self.follow_unit_orders.get(&current) == Some(&target)
                        && self.rewrite.is_some()
                        && self.can_squad_follow(current, target)
                    {
                        self.roster.insert(current);
                        self.squad_follow.insert(current);
                    }
                    let follow_ids: Vec<ObjectId> = self
                        .follow_unit_orders
                        .iter()
                        .filter_map(|(&fid, &ft)| (ft == current && fid != target).then_some(fid))
                        .collect();
                    for fid in follow_ids {
                        if let Some(ftarget) = self.follow_unit_orders.get_mut(&fid) {
                            *ftarget = target;
                        }
                        if self.rewrite.is_some() && self.can_squad_follow(fid, target) {
                            self.roster.insert(fid);
                            self.squad_follow.insert(fid);
                        }
                    }
                }
                self.following.remove(&target);
                self.routes.remove(&target);
                self.squad_follow.remove(&target);
                self.command_selection.remove(&target);
                self.missiles.orders.remove(&target);
                // Native bit 16 suppresses the HUD0 sound dispatch; default
                // transfer reports that presentation omission explicitly.
            }
            ("IsUnderPlayerCommand", [Value::Object(id), rest @ ..])
                if self.rewrite.is_some()
                    && (rest.is_empty()
                        || matches!(rest, [Value::Enum(player)] if player == "PLAYER_ONE" || player == "PLAYER_TWO")
                        || matches!(rest, [Value::Number(n)] if *n == 0.0 || *n == 1.0)) =>
            {
                let unit = self.units.get(id).ok_or("Unknown commandable unit")?;
                return Ok(vec![Value::Boolean(
                    self.roster.contains(id) && unit.active == Some(true) && unit.health > 0.0,
                )]);
            }
            ("GetPlayerUnit", rest)
                if rest.is_empty()
                    || matches!(rest, [Value::Enum(p)] if p == "PLAYER_ONE" || (self.rewrite.is_some() && p == "PLAYER_TWO"))
                    || (self.rewrite.is_some()
                        && matches!(rest, [Value::Number(n)] if *n == 0.0 || *n == 1.0)) =>
            {
                return Ok(vec![Value::Object(
                    self.player.ok_or("Player unit not configured")?,
                )]);
            }
            ("IsDead", [Value::Object(id)]) => {
                if self.rewrite.is_some()
                    && !self.units.contains_key(id)
                    && !self.props.contains_key(id)
                {
                    return Ok(vec![Value::Boolean(false)]);
                }
                return Ok(vec![Value::Boolean(self.entity(*id)?.health <= 0.0)]);
            }
            ("SetUnitInvulnerable", [Value::Object(id), Value::Boolean(v)]) => {
                if self.rewrite.is_some()
                    && !self.units.contains_key(id)
                    && !self.props.contains_key(id)
                {
                    return Ok(vec![]);
                }
                if *v {
                    self.unit_invulnerable.insert(*id);
                } else {
                    self.unit_invulnerable.remove(id);
                }
                let mask = self.invulnerable_army_mask;
                let u = self.entity_mut(*id)?;
                u.invulnerable = *v || (Self::army_type_flag(&u.army) & mask) != 0;
            }
            ("SetUnitInvulnerable", [Value::Object(id), Value::Number(v)])
                if self.rewrite.is_some() =>
            {
                let inv = *v != 0.0;
                if inv {
                    self.unit_invulnerable.insert(*id);
                } else {
                    self.unit_invulnerable.remove(id);
                }
                let mask = self.invulnerable_army_mask;
                if let Ok(u) = self.entity_mut(*id) {
                    u.invulnerable = inv || (Self::army_type_flag(&u.army) & mask) != 0;
                }
            }
            ("SetHealthPercent", [Value::Object(id), Value::Number(v)]) => {
                if !v.is_finite() || !(0.0..=100.0).contains(v) {
                    return Err("Invalid health percentage".into());
                }
                let u = self.entity_mut(*id)?;
                let health = u.max_health * (v / 100.0);
                if let Some(animation) = &mut u.animation {
                    if health == 0.0 {
                        animation.select(crate::unit_animation::Motion::Death)?;
                    } else if u.health == 0.0 {
                        animation.select(crate::unit_animation::Motion::Idle)?;
                    }
                }
                u.health = health;
                if health == 0.0 && self.player == Some(*id) && self.player_lifecycle.is_some() {
                    self.player_lifecycle = Some(PlayerLifecycle::ReplacementRequired);
                }
            }
            ("AllPlayerUnitsDead", [Value::Enum(player)])
                if self.rewrite.is_some() && player == "PLAYER_TWO" =>
            {
                return Ok(vec![Value::Boolean(false)]);
            }
            ("AllPlayerUnitsDead", rest)
                if matches!(rest, [Value::Enum(player)] if player == "PLAYER_ONE")
                    || (self.rewrite.is_some()
                        && (rest.is_empty()
                            || matches!(rest, [Value::Number(n)] if *n == 0.0))) =>
            {
                if self.rewrite.is_none()
                    && let Some(lifecycle) = self.player_lifecycle
                {
                    return match lifecycle {
                        PlayerLifecycle::FreshAlive(id)
                            if self.player == Some(id)
                                && self
                                    .units
                                    .get(&id)
                                    .is_some_and(|u| u.active == Some(true) && u.health > 0.0) =>
                        {
                            Ok(vec![Value::Boolean(false)])
                        }
                        _ => Err("Player replacement/defeat transition not configured".into()),
                    };
                }
                if self.rewrite.is_none() && self.player.is_none_or(|id| !self.roster.contains(&id))
                {
                    return Err("Player roster not configured".into());
                }
                let mut alive = false;
                for &id in &self.roster {
                    let u = self.entity(id).map_err(|_| "Unknown roster unit".to_string())?;
                    let present = if self.despawn_retention.contains_key(&id) {
                        self.unit_list_presence(id)?
                    } else {
                        u.active.ok_or("Unresolved unit activation")?
                    };
                    let active_flag = self
                        .activity
                        .get(&id)
                        .is_none_or(|a| a.enabled != Some(false));
                    alive |= present && u.health > 0.0 && active_flag;
                }
                if !alive
                    && self.collision.as_ref().is_some_and(|s| s.ground.is_some())
                    && let Some(player_army) = self
                        .player
                        .and_then(|p| self.entity(p).ok().map(|u| u.army.as_str()))
                        .or_else(|| {
                            self.roster
                                .iter()
                                .find_map(|id| self.units.get(id).map(|u| u.army.as_str()))
                        })
                {
                    alive = self.units.iter().any(|(&id, u)| {
                        u.army.as_str() == player_army
                            && u.active == Some(true)
                            && u.health > 0.0
                            && (self.bodies.contains_key(&id)
                                || self.non_ground_units.contains(&id))
                            && self
                                .activity
                                .get(&id)
                                .is_none_or(|a| a.enabled != Some(false))
                    });
                }
                return Ok(vec![Value::Boolean(!alive)]);
            }
            ("FreezePlayer" | "UnfreezePlayer", [Value::Enum(player)])
                if self.rewrite.is_some() && player == "PLAYER_TWO" => {}
            ("FreezePlayer" | "UnfreezePlayer", rest)
                if rest.is_empty()
                    || matches!(rest, [Value::Enum(player)] if player == "PLAYER_ONE")
                    || (self.rewrite.is_some()
                        && matches!(rest, [Value::Number(n)] if *n == 0.0 || *n == 1.0)) =>
            {
                self.frozen = name == "FreezePlayer"
            }
            ("IsInArea", [Value::Object(id), Value::Object(zone)]) => {
                return Ok(vec![Value::Boolean(
                    self.zones
                        .get(zone)
                        .ok_or("Unknown map zone")?
                        .contains(*positions.get(id).ok_or("Unknown object position")?)?,
                )]);
            }
            ("SetObjectiveData", [Value::Object(id), Value::Enum(key), Value::Number(v)])
                if key == "OBJECTIVE_MARKER_DATA_VISIBLE" && (*v == 0.0 || *v == 1.0) =>
            {
                if let Some(vis) = self.objective_visibility.get_mut(id) {
                    *vis = Some(*v == 1.0);
                } else if self.rewrite.is_some()
                    && let Some(objective) = self.objectives.get_mut(id)
                {
                    objective.0 = *v == 1.0;
                } else {
                    return Err("Unknown objective marker".into());
                }
            }
            ("SetObjectiveData", [Value::Object(id), Value::Enum(key), Value::Object(target)])
                if key == "OBJECTIVE_MARKER_DATA_ATTACHED_TO" =>
            {
                if !self.objective_visibility.contains_key(id) {
                    return Err("Unknown objective marker".into());
                }
                self.objective_attachment.insert(*id, *target);
            }
            ("SetObjectiveData", [Value::Object(id), Value::Enum(key), Value::Number(v)])
                if matches!(
                    key.as_str(),
                    "OBJECTIVE_DATA_VISIBLE" | "OBJECTIVE_DATA_STATE"
                ) && (*v == 0. || *v == 1.) =>
            {
                if let Some(objective) = self.objectives.get_mut(id) {
                    if key == "OBJECTIVE_DATA_VISIBLE" {
                        objective.0 = *v == 1.;
                    } else {
                        objective.1 = *v == 1.;
                    }
                } else if self.rewrite.is_some()
                    && key == "OBJECTIVE_DATA_VISIBLE"
                    && let Some(vis) = self.objective_visibility.get_mut(id)
                {
                    *vis = Some(*v == 1.);
                } else {
                    return Err("Unknown mission objective".into());
                }
            }
            ("AttackTarget", [Value::Object(owner), Value::Object(target), rest @ ..])
                if self.rewrite.is_some()
                    && (rest.is_empty()
                        || matches!(rest, [Value::Enum(o)] if matches!(o.as_str(), "ORDER_FORCED" | "ORDER_NORMAL"))) =>
            {
                if self.despawn_retention.get(owner) == Some(&true)
                    && !self.carrier_of.contains_key(owner)
                    && let Some(u) = self.units.get_mut(owner)
                    && u.active == Some(false)
                    && u.health > 0.0
                {
                    u.active = Some(true);
                }
                let (actor_hp, actor_active) = {
                    let a = self.entity(*owner)?;
                    (a.health, a.active)
                };
                let (victim_hp, victim_active) = {
                    let v = self.entity(*target)?;
                    (v.health, v.active)
                };
                if actor_hp <= 0. || victim_hp <= 0. {
                    return Ok(vec![Value::Boolean(false)]);
                }
                if self.collision.as_ref().is_some_and(|s| s.ground.is_some()) {
                    let act = self.activity.entry(*owner).or_default();
                    act.enabled = Some(true);
                    if act.weapons_enabled.is_none() {
                        act.weapons_enabled = Some(true);
                    }
                }
                let is_air_attacker = self.scripted_air_cruise.contains_key(owner)
                    && self
                        .rewrite
                        .as_ref()
                        .and_then(|r| r.objects().get(owner).map(|o| o.kind == "cAirVehicle"))
                        .unwrap_or(false);
                let is_air_target = self.air_shapes.contains_key(target)
                    && !self.water_units.contains(target);
                if !is_air_attacker && is_air_target {
                    if actor_active == Some(false) || victim_active == Some(false) {
                        self.orders.remove(owner);
                        self.following.remove(owner);
                        self.routes.remove(owner);
                        self.direct_velocity.remove(owner);
                        self.squad_follow.remove(owner);
                        self.missiles.orders.remove(owner);
                        return Ok(vec![Value::Boolean(true)]);
                    }
                    if let Ok(()) = self.missiles.clone().order(
                        &self.rewrite.as_ref().unwrap().objects(),
                        *owner,
                        *target,
                    ) {
                        let mut missiles = self.missiles.clone();
                        missiles.order(
                            &self.rewrite.as_ref().unwrap().objects(),
                            *owner,
                            *target,
                        )?;
                        self.rewrite
                            .as_mut()
                            .unwrap()
                            .allow_missile_presentation()?;
                        self.missiles = missiles;
                        self.orders.remove(owner);
                        self.following.remove(owner);
                        self.routes.remove(owner);
                        self.direct_velocity.remove(owner);
                        self.squad_follow.remove(owner);
                        return Ok(vec![Value::Boolean(true)]);
                    }
                } else if is_air_attacker && !self.non_ground_units.contains(target) {
                    if !self.headings.contains_key(owner)
                        && self.collision.as_ref().is_some_and(|s| s.ground.is_some())
                        && let Some(shape) = self.air_shapes.get(owner)
                    {
                        self.headings.insert(*owner, shape.heading);
                    }
                    if self.headings.contains_key(owner)
                        && !(self.frozen && self.player == Some(*target))
                        && let Ok(()) = self.bombers.clone().order(
                            &self.rewrite.as_ref().unwrap().objects(),
                            *owner,
                            *target,
                        )
                    {
                        let mut bombers = self.bombers.clone();
                        bombers.order(
                            &self.rewrite.as_ref().unwrap().objects(),
                            *owner,
                            *target,
                        )?;
                        self.rewrite.as_mut().unwrap().allow_bomber_presentation()?;
                        self.bombers = bombers;
                        self.orders.remove(owner);
                        self.following.remove(owner);
                        self.routes.remove(owner);
                        return Ok(vec![Value::Boolean(true)]);
                    }
                }
                self.attack_orders.insert(*owner, *target);
                self.follow_unit_orders.remove(owner);
                if self.units.contains_key(owner)
                    && !self.buildings.contains(owner)
                    && !self.bombers.orders.contains_key(owner)
                    && !self.grounded_units.contains(owner)
                    && (!self.non_ground_units.contains(owner)
                        || self.scripted_air_cruise.contains_key(owner)
                        || self.water_units.contains(owner))
                    && let Some(&tp) = positions.get(target)
                {
                    self.following.remove(owner);
                    self.squad_follow.remove(owner);
                    let is_artillery =
                        (self.unit_gtype_flags(*owner) & (0x0000_1000 | 0x0000_0400)) != 0;
                    let dist = positions
                        .get(owner)
                        .map(|ap| (ap[0] - tp[0]).hypot(ap[2] - tp[2]) as f64)
                        .unwrap_or(f64::INFINITY);
                    let (engage_dist, stop_radius) = if self.water_units.contains(owner) {
                        (120.0, 90.0)
                    } else if self.scripted_air_cruise.contains_key(owner) {
                        (45.0, 35.0)
                    } else if is_artillery {
                        (200.0, 160.0)
                    } else {
                        (0.0, 11.0)
                    };
                    if dist > engage_dist {
                        self.orders.insert(
                            *owner,
                            MoveOrder {
                                target: [tp[0] as f64, tp[2] as f64],
                                radius: stop_radius,
                            },
                        );
                    }
                }
                return Ok(vec![Value::Boolean(true)]);
            }
            ("StopAndGuard", [Value::Object(id)]) if self.rewrite.is_some() => {
                let unit = self.units.get(id).ok_or("Unknown guard unit")?;
                if unit.health <= 0. {
                    return Ok(vec![Value::Boolean(false)]);
                }
                self.bombers.orders.remove(id);
                self.missiles.orders.remove(id);
                self.attack_orders.remove(id);
                self.follow_unit_orders.remove(id);
                self.orders.remove(id);
                self.following.remove(id);
                self.routes.remove(id);
                self.direct_velocity.remove(id);
                self.squad_follow.remove(id);
            }
            ("DeclareVictory", [Value::Enum(army)])
                if Self::army_enum_to_internal(army).is_some() =>
            {
                self.declared_victory = Some(army.clone())
            }
            (
                "DestroyablePlayAnim",
                [Value::Object(id), Value::Number(anim), Value::Number(speed)],
            ) if self.rewrite.is_some() => {
                let strict_speed = self.collision.as_ref().is_some_and(|s| s.ground.is_none());
                if !anim.is_finite()
                    || !speed.is_finite()
                    || (strict_speed && *speed <= 0.0)
                    || (!strict_speed && *speed < 0.0)
                {
                    return Err("Invalid destroyable animation parameters".into());
                }
                if !self.props.contains_key(id) || (strict_speed && self.buildings.contains(id)) {
                    return Err("DestroyablePlayAnim target is not a destroyable object".into());
                }
                let mut staged = self.clone();
                if let Some(scene) = &mut staged.collision
                    && (scene.collision_owner(*id).is_some() || scene.ground.is_none())
                {
                    std::sync::Arc::make_mut(scene).set_object_present(*id, false)?;
                }
                staged.props.get_mut(id).unwrap().active = Some(false);
                *self = staged;
            }
            ("StoreSummaryMessage", args) if self.rewrite.is_some() => {
                let (id, army_val, icon) = match args {
                    [
                        Value::Enum(_) | Value::Number(_),
                        Value::Number(id),
                        army_val,
                        Value::Object(icon),
                        ..,
                    ] => (*id, army_val, *icon),
                    [Value::Number(id), army_val, Value::Object(icon), ..] => {
                        (*id, army_val, *icon)
                    }
                    [
                        Value::Number(id),
                        army_val,
                        Value::Enum(_) | Value::Number(_),
                        Value::Object(icon),
                        ..,
                    ] => (*id, army_val, *icon),
                    _ => return Err("Invalid summary message signature".into()),
                };
                let army = match army_val {
                    Value::Enum(a) => a.clone(),
                    Value::Number(n) if (0.0..=6.0).contains(n) => "ARMY_SOLAR".to_string(),
                    _ => return Err("Invalid summary message army".into()),
                };
                if !id.is_finite() || id < 0.0 || id.fract() != 0.0 {
                    return Err("Invalid summary message id".into());
                }
                self.summary_messages.push((id as u32, army, icon));
            }
            ("ListScoringUnits", []) if self.rewrite.is_some() => {
                self.scoring_listed = true;
            }
            ("Measure", [Value::Number(val), Value::Number(min), Value::Number(max)]) => {
                if [val, min, max].iter().any(|v| !v.is_finite()) {
                    return Err("Invalid Measure arguments".into());
                }
                let ratio = if *max <= *min {
                    if *val <= *min { 0.0 } else { 1.0 }
                } else {
                    ((*val - *min) / (*max - *min)).clamp(0.0, 1.0)
                };
                return Ok(vec![Value::Number(ratio)]);
            }
            ("rint", [Value::Number(val)]) => {
                if !val.is_finite() {
                    return Err("Invalid rint argument".into());
                }
                return Ok(vec![Value::Number(val.round())]);
            }
            ("SetArmyAllegianceForScoring", [Value::Enum(army), Value::Number(flag)])
                if self.rewrite.is_some() =>
            {
                if !matches!(*flag, 0.0 | 1.0 | 2.0) {
                    return Err("Invalid scoring allegiance flag".into());
                }
                let key = Self::army_enum_to_internal(army).ok_or("Unknown scoring army")?;
                self.scoring_allegiance.insert(key.into(), *flag as u8);
            }
            ("GetFriendlyHealthTotals" | "GetEnemyHealthTotals", [Value::Enum(mode)])
                if self.rewrite.is_some()
                    && matches!(mode.as_str(), "TOTAL_ALIVE_HEALTH" | "TOTAL_DEAD_HEALTH") =>
            {
                if !self.scoring_listed && self.scoring_allegiance.is_empty() {
                    return Err("Scoring units not listed before health totals".into());
                }
                let wanted = if name == "GetFriendlyHealthTotals" {
                    1
                } else {
                    2
                };
                let mut total = 0.0;
                for id in self
                    .units
                    .keys()
                    .chain(self.buildings.iter())
                    .copied()
                    .collect::<Vec<_>>()
                {
                    let unit = self.entity(id)?;
                    if self.scoring_allegiance.get(&unit.army).copied() != Some(wanted) {
                        continue;
                    }
                    let present = self.unit_list_presence(id)?
                        || self
                            .rewrite
                            .as_ref()
                            .is_some_and(|r| r.retired.contains(&id));
                    if !present {
                        continue;
                    }
                    if mode == "TOTAL_ALIVE_HEALTH" {
                        total += unit.max_health;
                    } else {
                        total += (unit.max_health - unit.health.max(0.0)).max(0.0);
                    }
                }
                return Ok(vec![Value::Number(total)]);
            }
            ("SetMissionPowerObjectives", [Value::Number(total), Value::Number(done)])
                if self.rewrite.is_some() =>
            {
                if !total.is_finite() || !done.is_finite() || *total < 0.0 || *done < 0.0 {
                    return Err("Invalid power objectives".into());
                }
                self.power_objectives = Some([*total, *done]);
            }
            ("SetMissionTechObjectives", [Value::Number(total), Value::Number(done)])
                if self.rewrite.is_some() =>
            {
                if !total.is_finite() || !done.is_finite() || *total < 0.0 || *done < 0.0 {
                    return Err("Invalid tech objectives".into());
                }
                self.tech_objectives = Some([*total, *done]);
            }
            ("SetMissionMinMaxTime", [Value::Number(min), Value::Number(max)])
                if self.rewrite.is_some() =>
            {
                if !min.is_finite() || !max.is_finite() || *min < 0.0 || *max < *min {
                    return Err("Invalid mission min/max time".into());
                }
                self.mission_min_max_time = Some((*min, *max));
            }
            ("SetMissionTime", [Value::Number(seconds)]) if self.rewrite.is_some() => {
                if !seconds.is_finite() || *seconds < 0.0 {
                    return Err("Invalid mission time".into());
                }
                self.mission_time = Some(*seconds);
            }
            (
                "SetMissionRatings",
                [
                    Value::Enum(s),
                    Value::Enum(p),
                    Value::Enum(t),
                    Value::Enum(o),
                ],
            ) if self.rewrite.is_some() => {
                self.mission_ratings = Some([s.clone(), p.clone(), t.clone(), o.clone()]);
            }
            ("SetCurrentMissionCompleted", []) => {
                let history = self
                    .mission_history
                    .as_mut()
                    .ok_or("Current mission profile history not configured")?;
                history.completed = true;
            }
            (
                "SetMissionComplete",
                [
                    Value::Enum(status),
                    Value::Number(speed),
                    Value::Number(power),
                    Value::Number(technique),
                    Value::Enum(rating),
                ],
            ) if self.rewrite.is_some() && status == "MISSIONOVER_PLAYER1_WINS" => {
                if [speed, power, technique].iter().any(|v| !v.is_finite())
                    || !matches!(
                        rating.as_str(),
                        "GRADE_C" | "GRADE_B" | "GRADE_A" | "GRADE_S"
                    )
                {
                    return Err("Invalid mission victory ratings".into());
                }
                self.mission_complete = Some(MissionOutcome::Won {
                    speed: *speed,
                    power: *power,
                    technique: *technique,
                    rating: rating.clone(),
                });
            }
            ("SetMissionComplete", [Value::Enum(status), rest @ ..])
                if self.rewrite.is_some() && status == "MISSIONOVER_LOSE" =>
            {
                let message_id = match rest {
                    [] | [Value::Nil] => None,
                    [Value::Number(id)] if id.is_finite() && *id >= 0.0 && id.fract() == 0.0 => {
                        Some(*id as u32)
                    }
                    _ => return Err("Invalid mission defeat message id".into()),
                };
                self.mission_complete = Some(MissionOutcome::Lost { message_id });
            }
            ("SetMissionComplete", [Value::Enum(_), ..]) if self.rewrite.is_some() => {
                self.mission_complete = Some(MissionOutcome::Won {
                    speed: 100.0,
                    power: 100.0,
                    technique: 100.0,
                    rating: "GRADE_S".into(),
                });
            }
            ("IsNetworkHost" | "IsNetworkPlayer", _) => {
                return Ok(vec![Value::Boolean(false)]);
            }
            // --- SP_1.1..SP_5.4 & MP1..MP17 Campaign Bindings ---
            ("IsPassengerInUnit", [Value::Object(unit)]) => {
                return Ok(vec![Value::Boolean(self.carrier_of.contains_key(unit))]);
            }
            ("IsPassengerInUnit", [Value::Object(unit), Value::Enum(e)]) if e == "ID_NONE" => {
                return Ok(vec![Value::Boolean(self.carrier_of.contains_key(unit))]);
            }
            ("IsPassengerInUnit", [Value::Object(unit), Value::Number(n)]) if *n <= 0.0 => {
                return Ok(vec![Value::Boolean(self.carrier_of.contains_key(unit))]);
            }
            ("IsPassengerInUnit", [Value::Object(unit), Value::Object(carrier)]) => {
                return Ok(vec![Value::Boolean(
                    self.carrier_of.get(unit) == Some(carrier),
                )]);
            }
            ("NumPassengersInUnit", [Value::Object(carrier)]) => {
                let count = self
                    .passengers
                    .get(carrier)
                    .map_or(0, |l| l.iter().filter(|id| id.0 != 0).count());
                return Ok(vec![Value::Number(count as f64)]);
            }
            ("GetNumSeatsFree", [Value::Object(carrier)]) => {
                let count = self
                    .passengers
                    .get(carrier)
                    .map_or(0, |l| l.iter().filter(|id| id.0 != 0).count());
                return Ok(vec![Value::Number(10usize.saturating_sub(count) as f64)]);
            }
            ("PutUnitInVehicle", [Value::Object(unit), Value::Object(carrier), _slot @ ..]) => {
                if self.entity(*unit).is_ok_and(|u| u.health <= 0.0)
                    || self.entity(*carrier).is_ok_and(|u| u.health <= 0.0)
                {
                    return Ok(vec![]);
                }
                if let Some(prev) = self.carrier_of.insert(*unit, *carrier)
                    && prev != *carrier
                    && let Some(list) = self.passengers.get_mut(&prev)
                {
                    if self.capture_points.contains(&prev) {
                        list.retain(|&u| u != *unit);
                    } else {
                        for u in list.iter_mut() {
                            if *u == *unit {
                                *u = ObjectId(0);
                            }
                        }
                    }
                }
                let is_cp = self.capture_points.contains(carrier);
                let list = self.passengers.entry(*carrier).or_default();
                if !list.contains(unit) {
                    if !is_cp
                        && let Some(slot) = list.iter_mut().find(|id| id.0 == 0)
                    {
                        *slot = *unit;
                    } else {
                        list.push(*unit);
                    }
                }
                if !is_cp {
                    let u_army = self.units.get(unit).map(|u| u.army.clone());
                    if let Some(u) = self.units.get_mut(unit) {
                        u.active = Some(false);
                    }
                    if let (Some(army), Some(b)) = (u_army, self.props.get_mut(carrier)) {
                        if b.army.is_empty() || b.army == "eNeutral" || b.army == "eNoArmy" {
                            b.army = army;
                        }
                        self.activity.entry(*carrier).or_default().enabled = Some(true);
                    }
                }
            }
            ("EnterVehicle", [Value::Object(unit), Value::Object(carrier), _rest @ ..]) => {
                if self.carrier_of.get(unit) == Some(carrier) {
                    return Ok(vec![]);
                }
                if self.despawn_retention.get(unit) == Some(&true)
                    && !self.carrier_of.contains_key(unit)
                    && let Some(u) = self.units.get_mut(unit)
                    && u.active == Some(false)
                    && u.health > 0.0
                {
                    u.active = Some(true);
                }
                if self
                    .units
                    .get(unit)
                    .is_some_and(|u| u.active != Some(true) || u.health <= 0.0)
                    || self
                        .carrier_of
                        .get(unit)
                        .is_some_and(|c| !self.capture_points.contains(c))
                {
                    if self.units.get(unit).is_some_and(|u| u.health > 0.0) {
                        self.entering_vehicle.insert(*unit, *carrier);
                    }
                    return Ok(vec![]);
                }
                if let Some(&prev) = self.carrier_of.get(unit)
                    && prev != *carrier
                    && self.capture_points.contains(&prev)
                {
                    self.carrier_of.remove(unit);
                    if let Some(list) = self.passengers.get_mut(&prev) {
                        list.retain(|u| u != unit);
                    }
                }
                self.follow_unit_orders.remove(unit);
                if let (Some(&up), Some(&cp)) = (positions.get(unit), positions.get(carrier))
                    && (up[0] - cp[0]).hypot(up[2] - cp[2]) > 8.0
                {
                    self.entering_vehicle.insert(*unit, *carrier);
                    self.following.remove(unit);
                    self.squad_follow.remove(unit);
                    let target = [cp[0] as f64, cp[2] as f64];
                    if self.orders.get(unit).is_none_or(|o| {
                        (o.target[0] - target[0]).hypot(o.target[1] - target[1]) > 2.0
                    }) {
                        self.orders.insert(
                            *unit,
                            MoveOrder {
                                target,
                                radius: 4.0,
                            },
                        );
                    }
                } else {
                    self.entering_vehicle.remove(unit);
                    self.orders.remove(unit);
                    if let Some(prev) = self.carrier_of.insert(*unit, *carrier)
                        && prev != *carrier
                        && let Some(prev_list) = self.passengers.get_mut(&prev)
                    {
                        if self.capture_points.contains(&prev) {
                            prev_list.retain(|u| u != unit);
                        } else {
                            for u in prev_list.iter_mut() {
                                if *u == *unit {
                                    *u = ObjectId(0);
                                }
                            }
                        }
                    }
                    let is_cp = self.capture_points.contains(carrier);
                    let list = self.passengers.entry(*carrier).or_default();
                    if !list.contains(unit) {
                        if !is_cp
                            && let Some(slot) = list.iter_mut().find(|id| id.0 == 0)
                        {
                            *slot = *unit;
                        } else {
                            list.push(*unit);
                        }
                    }
                    if !is_cp {
                        let u_army = self.units.get(unit).map(|u| u.army.clone());
                        if let Some(u) = self.units.get_mut(unit) {
                            u.active = Some(false);
                        }
                        if let (Some(army), Some(b)) = (u_army, self.props.get_mut(carrier)) {
                            if b.army.is_empty() || b.army == "eNeutral" || b.army == "eNoArmy" {
                                b.army = army;
                            }
                            self.activity.entry(*carrier).or_default().enabled = Some(true);
                        }
                    }
                }
            }
            ("ExitVehicle", [first, rest @ ..]) => match (first, rest) {
                (Value::Enum(none), [Value::Object(carrier)]) if none == "ID_NONE" => {
                    let mut pos_copy = positions.clone();
                    self.unload_carrier_passengers(*carrier, &mut pos_copy);
                }
                (Value::Enum(none), _) if none == "ID_NONE" => {}
                (Value::Nil, _) => {}
                (Value::Object(unit), _) => {
                    let carrier = self.carrier_of.remove(unit).or_else(|| {
                        rest.first().and_then(|v| match v {
                            Value::Object(cid) => Some(*cid),
                            _ => None,
                        })
                    });
                    if let Some(cid) = carrier {
                        if let Some(list) = self.passengers.get_mut(&cid) {
                            if self.capture_points.contains(&cid) {
                                list.retain(|&u| u != *unit);
                            } else {
                                for u in list.iter_mut() {
                                    if *u == *unit {
                                        *u = ObjectId(0);
                                    }
                                }
                            }
                        }
                        if let Some(&cpos) = positions.get(&cid) {
                            let y = if self.non_ground_units.contains(unit)
                                || self.collision.as_ref().is_none_or(|s| s.ground.is_none())
                            {
                                cpos[1]
                            } else {
                                0.0
                            };
                            let upos = [cpos[0] + 2.5, y, cpos[2] + 2.5];
                            self.pending_positions.insert(*unit, upos);
                        }
                    }
                    if let Some(u) = self.units.get_mut(unit)
                        && u.health > 0.0
                    {
                        u.active = Some(true);
                    }
                }
                _ => return Err("Unsupported ExitVehicle signature".into()),
            },
            (
                "LandAirUnit",
                [
                    Value::Object(id),
                    Value::Number(x),
                    Value::Number(z),
                    rest @ ..,
                ],
            ) => {
                if self
                    .units
                    .get(id)
                    .is_some_and(|u| u.active == Some(false) || u.health <= 0.0)
                {
                    return Ok(vec![Value::Boolean(false)]);
                }
                let unload = rest.get(1).is_some_and(|v| {
                    matches!(v, Value::Number(f) if *f != 0.0) || matches!(v, Value::Boolean(true))
                });
                self.following.remove(id);
                self.grounded_units.remove(id);
                self.landing_units.insert(*id);
                if unload {
                    self.unloading_on_arrival.insert(*id);
                } else {
                    self.unloading_on_arrival.remove(id);
                }
                if let Some(Value::Number(heading)) = rest.get(3) {
                    self.headings
                        .insert(*id, heading.rem_euclid(360.0).to_radians() as f32);
                }
                self.orders.insert(
                    *id,
                    MoveOrder {
                        target: [*x, *z],
                        radius: 4.0,
                    },
                );
                return Ok(vec![Value::Boolean(true)]);
            }
            (
                "BeachWaterUnit",
                [
                    Value::Object(id),
                    Value::Number(x),
                    Value::Number(z),
                    Value::Number(heading),
                    rest @ ..,
                ],
            ) => {
                let unload = matches!(rest, [Value::Number(f)] if *f != 0.0)
                    || matches!(rest, [Value::Boolean(true)]);
                self.following.remove(id);
                self.grounded_units.remove(id);
                self.landing_units.insert(*id);
                if unload {
                    self.unloading_on_arrival.insert(*id);
                } else {
                    self.unloading_on_arrival.remove(id);
                }
                self.headings
                    .insert(*id, heading.rem_euclid(360.0).to_radians() as f32);
                self.orders.insert(
                    *id,
                    MoveOrder {
                        target: [*x, *z],
                        radius: 4.0,
                    },
                );
            }
            ("GetMovementState", [Value::Object(id)]) => {
                if self.units.get(id).is_some_and(|u| u.active == Some(false)) {
                    return Ok(vec![Value::Number(-1.0)]);
                }
                let grounded =
                    !self.non_ground_units.contains(id) || self.grounded_units.contains(id);
                return Ok(vec![Value::Enum(
                    if grounded {
                        "MOVEMENT_STATE_ON_GROUND"
                    } else {
                        "MOVEMENT_STATE_AIRBORNE"
                    }
                    .into(),
                )]);
            }
            ("FollowUnit", [Value::Object(follower), Value::Object(target), _rest @ ..]) => {
                if follower == target {
                    return Ok(Vec::new());
                }
                self.following.remove(follower);
                self.squad_follow.remove(follower);
                self.follow_unit_orders.insert(*follower, *target);
                if self.player == Some(*target) && self.can_squad_follow(*follower, *target) {
                    self.roster.insert(*follower);
                    self.squad_follow.insert(*follower);
                }
                let radius = self.bodies.get(follower).map_or(4.5, |b| b.radius + 2.0);
                if let Some(&tp) = positions.get(target) {
                    self.orders.insert(
                        *follower,
                        MoveOrder {
                            target: [tp[0] as f64, tp[2] as f64],
                            radius,
                        },
                    );
                }
            }
            ("GetInstallationOwner", [Value::Object(si_id)]) => {
                let owner = self
                    .installations
                    .get(si_id)
                    .map(|si| si.owner.as_str())
                    .unwrap_or("eNeutral");
                return Ok(vec![Self::internal_to_army_enum(owner)]);
            }
            ("SetInstallationOwner", [Value::Object(si_id), Value::Enum(army)]) => {
                let internal = Self::army_enum_to_internal(army).unwrap_or("eNeutral");
                if let Some(si) = self.installations.get_mut(si_id) {
                    si.owner = internal.to_string();
                    let cp_id = si.capture_point;
                    let buildings = si.buildings.clone();
                    for bid in buildings {
                        if let Some(b) = self.props.get_mut(&bid) {
                            b.army = internal.to_string();
                        }
                    }
                    if let Some(cp_id) = cp_id
                        && let Some(cp) = self.capture_point_state.get_mut(&cp_id)
                    {
                        cp.owner = internal.to_string();
                        cp.height = if internal == "eNeutral" { 0.0 } else { 100.0 };
                        cp.direction = 0;
                    }
                }
            }
            ("SetCapturePointHeight", [Value::Object(cp_id), army_val, Value::Number(height)]) => {
                let internal = match army_val {
                    Value::Enum(a) => Self::army_enum_to_internal(a).unwrap_or("eNeutral"),
                    _ => "eNeutral",
                };
                let clamped = height.clamp(0.0, 100.0);
                let mut si_id = None;
                if let Some(cp) = self.capture_point_state.get_mut(cp_id) {
                    cp.owner = internal.to_string();
                    cp.height = clamped;
                    cp.direction = 0;
                    if clamped >= 100.0 {
                        si_id = cp.installation;
                    }
                }
                if let Some(si_id) = si_id
                    && let Some(si) = self.installations.get_mut(&si_id)
                {
                    si.owner = internal.to_string();
                    for &bid in &si.buildings.clone() {
                        if let Some(b) = self.props.get_mut(&bid) {
                            b.army = internal.to_string();
                        }
                    }
                }
            }
            (
                "CheckCapturePoint",
                [
                    Value::Object(cp_id),
                    army_val,
                    Value::Number(target_h),
                    Value::Number(tol),
                    rest @ ..,
                ],
            ) => {
                let wanted = match army_val {
                    Value::Enum(army) => Self::army_enum_to_internal(army).unwrap_or("eNeutral"),
                    _ => "eNeutral",
                };
                let matched = if let Some(cp) = self.capture_point_state.get(cp_id) {
                    let dir_ok = match rest.first() {
                        Some(Value::Enum(dir)) if dir == "CAPTUREPOINTFLAG_RAISING" => {
                            cp.direction >= 0
                        }
                        Some(Value::Enum(dir)) if dir == "CAPTUREPOINTFLAG_LOWERING" => {
                            cp.direction <= 0
                        }
                        _ => true,
                    };
                    let army_ok = cp.owner == wanted
                        || (cp.owner == "eNeutral" && cp.capturing_army.as_deref() == Some(wanted));
                    dir_ok && army_ok && (cp.height - target_h).abs() <= (*tol).max(1.0)
                } else {
                    false
                };
                return Ok(vec![Value::Boolean(matched)]);
            }
            ("GetState", [Value::Object(id)]) => {
                if let Some(&(_, done)) = self.objectives.get(id) {
                    return Ok(vec![Value::Number(if done { 1.0 } else { 0.0 })]);
                }
                let bits = self
                    .capture_point_state
                    .get(id)
                    .map(|cp| cp.owner.as_str())
                    .filter(|&owner| owner != "eNeutral" && owner != "eNoArmy" && !owner.is_empty())
                    .map_or(0.0, |owner| Self::army_type_flag(owner) as f64);
                return Ok(vec![Value::Number(bits)]);
            }
            ("StopCapturePointSounds", [Value::Object(_)]) => {}
            ("CreateGroup", [Value::Object(group)]) => {
                self.unit_groups.entry(*group).or_default();
            }
            ("AddToGroup", [Value::Object(group), Value::Object(unit)]) => {
                let list = self.unit_groups.entry(*group).or_default();
                if !list.contains(unit) {
                    list.push(*unit);
                }
            }
            ("RemoveFromGroup", [Value::Object(group), Value::Object(unit)]) => {
                if let Some(list) = self.unit_groups.get_mut(group) {
                    list.retain(|&u| u != *unit);
                }
            }
            ("CopyGroup", [Value::Object(dst), Value::Object(src)]) => {
                let items = self.unit_groups.get(src).cloned().unwrap_or_default();
                self.unit_groups.insert(*dst, items);
            }
            ("ExtractFromGroup", [Value::Object(dst), Value::Object(src), _rest @ ..]) => {
                let items = self.unit_groups.get(src).cloned().unwrap_or_default();
                let list = self.unit_groups.entry(*dst).or_default();
                for uid in items {
                    if !list.contains(&uid) {
                        list.push(uid);
                    }
                }
            }
            ("GetNumMatchingInGroup", [Value::Object(group), _rest @ ..]) => {
                let count = self.unit_groups.get(group).map_or(0, |g| {
                    g.iter()
                        .filter(|&&uid| self.entity(uid).is_ok_and(|u| u.health > 0.0))
                        .count()
                });
                return Ok(vec![Value::Number(count as f64)]);
            }
            ("GetGroupMember", [Value::Object(group), Value::Number(index)]) => {
                let idx = (*index as usize).saturating_sub(1);
                let val = self
                    .unit_groups
                    .get(group)
                    .and_then(|g| g.get(idx))
                    .copied()
                    .map_or(Value::Nil, Value::Object);
                return Ok(vec![val]);
            }
            ("GetGroupHealth", [Value::Object(group)]) => {
                let list = self.unit_groups.get(group).cloned().unwrap_or_default();
                let mut sum = 0.0;
                for uid in &list {
                    if let Ok(u) = self.entity(*uid) {
                        sum += u.health.max(0.0);
                    }
                }
                return Ok(vec![Value::Number(sum)]);
            }
            ("GetGroupHealthPercent", [Value::Object(group)]) => {
                let list = self.unit_groups.get(group).cloned().unwrap_or_default();
                if list.is_empty() {
                    return Ok(vec![Value::Number(0.0)]);
                }
                let mut sum = 0.0;
                for uid in &list {
                    if let Ok(u) = self.entity(*uid)
                        && u.max_health > 0.0
                    {
                        sum += (u.health.max(0.0) / u.max_health) * 100.0;
                    }
                }
                return Ok(vec![Value::Number(sum / list.len() as f64)]);
            }
            ("GetActionState" | "GetPrimaryActionState", [Value::Object(id)]) => {
                let state = if let Some(&cid) = self.carrier_of.get(id) {
                    if self.capture_points.contains(&cid) {
                        "ACTION_STATE_IN_CAPTUREPOINT"
                    } else {
                        "ACTION_STATE_IN_UNIT_CARRIER"
                    }
                } else if let Some(&cid) = self.entering_vehicle.get(id) {
                    if self.capture_points.contains(&cid) {
                        "ACTION_STATE_GOING_TO_CAPTUREPOINT"
                    } else {
                        "ACTION_STATE_MOVING_TO_AREA"
                    }
                } else if self.squad_follow.contains(id) || self.follow_unit_orders.contains_key(id)
                {
                    "ACTION_STATE_FOLLOWING"
                } else if self.attack_orders.contains_key(id)
                    || self.bombers.orders.contains_key(id)
                    || self.missiles.orders.contains_key(id)
                {
                    "ACTION_STATE_ATTACK"
                } else if self.orders.contains_key(id) || self.following.contains_key(id) {
                    "ACTION_STATE_MOVING_TO_AREA"
                } else {
                    "ACTION_STATE_GUARD"
                };
                return Ok(vec![Value::Enum(state.into())]);
            }
            ("IsAttacking", [Value::Object(id)]) => {
                let attacking = self.attack_orders.contains_key(id)
                    || self.bombers.orders.contains_key(id)
                    || self.missiles.orders.contains_key(id);
                return Ok(vec![Value::Boolean(attacking)]);
            }
            ("IsActive", [Value::Object(id)]) => {
                // In RBWE01 0x800e7d80, when unit+0xbc != 0 (dead), C++ calls
                // lua_pushnumber(L, 0.0) instead of lua_pushboolean(L, cUnit::IsActive()).
                if self.entity(*id).is_ok_and(|u| u.health <= 0.0) {
                    return Ok(vec![Value::Number(0.0)]);
                }
                let enabled = self.entity(*id).is_ok_and(|u| u.active != Some(false))
                    && self
                        .activity
                        .get(id)
                        .and_then(|a| a.enabled)
                        .unwrap_or(false);
                return Ok(vec![Value::Boolean(enabled)]);
            }
            ("GetSpawnMode", _) => {
                return Ok(vec![Value::Enum("Follow".into())]);
            }
            ("CountRemainingPlayerTroops", _rest) => {
                let player_army = self
                    .player
                    .and_then(|p| self.entity(p).ok())
                    .map(|u| u.army.clone())
                    .unwrap_or_default();
                let count = self
                    .units
                    .iter()
                    .filter(|&(id, u)| {
                        u.army == player_army
                            && u.health > 0.0
                            && (u.active == Some(true) || self.carrier_of.contains_key(id))
                            && self
                                .activity
                                .get(id)
                                .is_none_or(|a| a.enabled != Some(false))
                            && (self.unit_type_flags(*id) & 0x0000_0100) != 0
                    })
                    .count();
                return Ok(vec![Value::Number(count as f64)]);
            }
            ("GetPlayerTransferred", _rest) => {
                let transferred = std::mem::replace(&mut self.transferred_by_death, false);
                return Ok(vec![Value::Boolean(transferred)]);
            }
            ("IsReadyToRevive", [Value::Object(id)]) => {
                let dead = self.entity(*id).is_ok_and(|u| u.health <= 0.0);
                return Ok(vec![Value::Boolean(dead)]);
            }
            ("WillReviveInGuard", _) => {
                return Ok(vec![Value::Boolean(false)]);
            }
            ("FindDeadUnit", _args) => {
                let found = self
                    .units
                    .iter()
                    .find(|(_, u)| u.health <= 0.0)
                    .map(|(&id, _)| Value::Object(id))
                    .unwrap_or(Value::Nil);
                return Ok(vec![found]);
            }
            (
                "ReviveDeadUnit",
                [
                    Value::Object(id),
                    Value::Number(x),
                    Value::Number(z),
                    Value::Number(heading),
                    _rest @ ..,
                ],
            ) => {
                if self.entity(*id).is_ok_and(|u| u.health > 0.0) {
                    return Ok(vec![Value::Boolean(false)]);
                }
                if let Some(runtime) = &mut self.rewrite {
                    runtime.retired.remove(id);
                }
                if let Ok(u) = self.entity_mut(*id) {
                    u.health = u.max_health;
                    u.active = Some(true);
                    if let Some(anim) = &mut u.animation {
                        let _ = anim.select(crate::unit_animation::Motion::Idle);
                    }
                }
                let act = self.activity.entry(*id).or_default();
                act.enabled = Some(true);
                act.weapons_enabled = Some(true);
                self.orders.remove(id);
                self.routes.remove(id);
                self.grounded_units.remove(id);
                self.landing_units.remove(id);
                self.entering_vehicle.remove(id);
                if !self.enemy_targetable_only.contains(id)
                    && let Some(player_army) = self
                        .player
                        .and_then(|p| self.entity(p).ok())
                        .map(|u| u.army.clone())
                        .or_else(|| {
                            self.roster
                                .iter()
                                .find_map(|rid| self.units.get(rid).map(|u| u.army.clone()))
                        })
                    && self.units.get(id).is_some_and(|u| u.army == player_army)
                {
                    self.roster.insert(*id);
                }
                if self.roster.contains(id) && self.player != Some(*id) {
                    self.squad_follow.insert(*id);
                }
                let mut pos = [*x as f32, 0.0, *z as f32];
                if let Some(old) = positions.get(id) {
                    pos[1] = old[1];
                }
                if !self.non_ground_units.contains(id) {
                    if self.collision.as_ref().is_some_and(|s| s.ground.is_some()) {
                        pos[1] = 0.0;
                    }
                } else if let Some(&hag) = self.scripted_air_altitude.get(id)
                    && let Some(scene) = &self.collision
                    && let Ok(air_pos) = scene.world_point([pos[0], 0.0, pos[2]], hag)
                {
                    pos = air_pos;
                }
                self.pending_positions.insert(*id, pos);
                self.headings
                    .insert(*id, heading.rem_euclid(360.0).to_radians() as f32);
                return Ok(vec![Value::Boolean(true)]);
            }
            ("ReviveDeadUnit", _) => {
                return Ok(vec![Value::Boolean(false)]);
            }
            (
                "ReviveFullUnitCarrier",
                [
                    _army_flag,
                    _grunt_flag,
                    Value::Object(carrier),
                    Value::Number(x),
                    Value::Number(z),
                    Value::Number(heading),
                    _rest @ ..,
                ],
            ) => {
                if self.entity(*carrier).is_ok_and(|u| u.health > 0.0) {
                    return Ok(vec![Value::Boolean(false)]);
                }
                if let Some(runtime) = &mut self.rewrite {
                    runtime.retired.remove(carrier);
                }
                if let Ok(u) = self.entity_mut(*carrier) {
                    u.health = u.max_health;
                    u.active = Some(true);
                }
                let act = self.activity.entry(*carrier).or_default();
                act.enabled = Some(true);
                act.weapons_enabled = Some(true);
                self.orders.remove(carrier);
                self.routes.remove(carrier);
                self.grounded_units.remove(carrier);
                self.landing_units.remove(carrier);
                if !self.enemy_targetable_only.contains(carrier)
                    && let Some(player_army) = self
                        .player
                        .and_then(|p| self.entity(p).ok())
                        .map(|u| u.army.clone())
                        .or_else(|| {
                            self.roster
                                .iter()
                                .find_map(|rid| self.units.get(rid).map(|u| u.army.clone()))
                        })
                    && self.units.get(carrier).is_some_and(|u| u.army == player_army)
                {
                    self.roster.insert(*carrier);
                }
                if self.roster.contains(carrier) && self.player != Some(*carrier) {
                    self.squad_follow.insert(*carrier);
                }
                let mut pos = [*x as f32, positions.get(carrier).map_or(20.0, |p| p[1]), *z as f32];
                if let Some(&hag) = self.scripted_air_altitude.get(carrier)
                    && let Some(scene) = &self.collision
                    && let Ok(air_pos) = scene.world_point([pos[0], 0.0, pos[2]], hag)
                {
                    pos = air_pos;
                }
                self.pending_positions.insert(*carrier, pos);
                self.headings
                    .insert(*carrier, heading.rem_euclid(360.0).to_radians() as f32);
                let p_list = self.passengers.get(carrier).cloned().unwrap_or_default();
                for pid in p_list {
                    if pid == ObjectId(0) {
                        continue;
                    }
                    if let Some(runtime) = &mut self.rewrite {
                        runtime.retired.remove(&pid);
                    }
                    if let Some(pu) = self.units.get_mut(&pid) {
                        pu.health = pu.max_health;
                        pu.active = Some(false);
                    }
                    let pact = self.activity.entry(pid).or_default();
                    pact.enabled = Some(true);
                    pact.weapons_enabled = Some(true);
                }
                return Ok(vec![Value::Boolean(true)]);
            }
            (
                "SpawnUnitInCarrier",
                [_army_flag, _type_flag, Value::Object(carrier), _rest @ ..],
            ) => {
                let candidate = self
                    .units
                    .iter()
                    .find(|(uid, u)| u.health <= 0.0 && !self.carrier_of.contains_key(uid))
                    .map(|(&uid, _)| uid);
                if let Some(uid) = candidate {
                    if let Some(runtime) = &mut self.rewrite {
                        runtime.retired.remove(&uid);
                    }
                    if let Some(u) = self.units.get_mut(&uid) {
                        u.health = u.max_health;
                        u.active = Some(false);
                    }
                    let act = self.activity.entry(uid).or_default();
                    act.enabled = Some(true);
                    act.weapons_enabled = Some(true);
                    self.carrier_of.insert(uid, *carrier);
                    let list = self.passengers.entry(*carrier).or_default();
                    if !list.contains(&uid) {
                        if let Some(slot) = list.iter_mut().find(|&&mut u| u == ObjectId(0)) {
                            *slot = uid;
                        } else {
                            list.push(uid);
                        }
                    }
                    return Ok(vec![Value::Object(uid)]);
                }
                let list = self.passengers.entry(*carrier).or_default();
                if list.len() < 10 {
                    list.push(ObjectId(0));
                }
                return Ok(vec![Value::Boolean(true)]);
            }
            ("SpawnUnit", _args) => {
                return Ok(vec![Value::Boolean(true)]);
            }
            ("CombineFlags", args) if !args.is_empty() => {
                let mut mask = 0u64;
                for v in args {
                    if let Value::Number(n) = v {
                        mask |= *n as u64;
                    }
                }
                return Ok(vec![Value::Number(mask as f64)]);
            }
            ("TestFlags", [Value::Object(id), Value::Number(flags), rest @ ..]) => {
                let bits = *flags as u32;
                let is_gtype = matches!(
                    rest.first(),
                    Some(Value::Enum(s)) if s == "GTYPE"
                ) || matches!(rest.first(), Some(Value::Number(n)) if *n == 1.0);
                let ok = if is_gtype {
                    (self.unit_gtype_flags(*id) & bits) != 0
                } else {
                    let u_flags = self.unit_type_flags(*id);
                    (u_flags & bits) == bits
                };
                return Ok(vec![Value::Boolean(ok)]);
            }
            ("mod", [Value::Number(a), Value::Number(b)]) => {
                return Ok(vec![Value::Number(if *b == 0.0 { 0.0 } else { a % b })]);
            }
            ("GetRange", [Value::Object(a), Value::Object(b)]) => {
                let dist = match (positions.get(a), positions.get(b)) {
                    (Some(p), Some(q)) => {
                        (p[0] as f64 - q[0] as f64).hypot(p[2] as f64 - q[2] as f64)
                    }
                    _ => 0.0,
                };
                return Ok(vec![Value::Number(dist)]);
            }
            ("GetDistanceBetweenPlayers", _) => {
                return Ok(vec![Value::Number(0.0)]);
            }
            ("GetAttackTarget" | "GetPrimaryActionTarget", [Value::Object(id)]) => {
                let target = self
                    .attack_orders
                    .get(id)
                    .or_else(|| self.bombers.orders.get(id).map(|o| &o.target))
                    .or_else(|| self.missiles.orders.get(id).map(|o| &o.target))
                    .copied()
                    .map_or(Value::Enum("ID_NONE".into()), Value::Object);
                return Ok(vec![target]);
            }
            ("GetTargetedObject", _) => {
                return Ok(vec![Value::Enum("ID_NONE".into())]);
            }
            ("GetUnitInSeat", [Value::Object(carrier), Value::Number(seat)]) => {
                let idx = *seat as usize;
                let val = self
                    .passengers
                    .get(carrier)
                    .and_then(|list| list.get(idx))
                    .copied()
                    .filter(|id| id.0 != 0)
                    .map_or(Value::Enum("ID_NONE".into()), Value::Object);
                return Ok(vec![val]);
            }
            ("GetContainerOf", [Value::Object(unit)]) => {
                let val = self
                    .carrier_of
                    .get(unit)
                    .copied()
                    .map_or(Value::Enum("ID_NONE".into()), Value::Object);
                return Ok(vec![val]);
            }
            ("GetKiller", [Value::Object(_)]) => {
                return Ok(vec![Value::Enum("ID_NONE".into())]);
            }
            (
                "IsInRectangle",
                [
                    Value::Object(id),
                    Value::Number(x1),
                    Value::Number(z1),
                    Value::Number(x2),
                    Value::Number(z2),
                ],
            ) => {
                let inside = positions.get(id).is_some_and(|p| {
                    let (min_x, max_x) = (x1.min(*x2) as f32, x1.max(*x2) as f32);
                    let (min_z, max_z) = (z1.min(*z2) as f32, z1.max(*z2) as f32);
                    p[0] >= min_x && p[0] <= max_x && p[2] >= min_z && p[2] <= max_z
                });
                return Ok(vec![Value::Boolean(inside)]);
            }
            ("IsOnFire", [Value::Object(_)]) => {
                return Ok(vec![Value::Boolean(false)]);
            }
            ("DestroyableGetCurrentAnim", [Value::Object(_)]) => {
                return Ok(vec![Value::Number(0.0)]);
            }
            ("GetSubmarineAirpercentage", [Value::Object(id)]) => {
                let pct = self
                    .submarines
                    .get(id)
                    .map_or(100.0, |s| (s.oxygen / s.max_oxygen * 100.0).clamp(0.0, 100.0));
                return Ok(vec![Value::Number(pct)]);
            }
            ("IsSubmarineSubmerged", [Value::Object(id)]) => {
                let sub = self
                    .submarines
                    .get(id)
                    .is_some_and(|s| s.submerged || s.depth > 0.5);
                return Ok(vec![Value::Boolean(sub)]);
            }
            ("SubmarineDive", [Value::Object(id), ..]) => {
                if let Some(sub) = self.submarines.get_mut(id) {
                    sub.submerged = true;
                }
                return Ok(vec![Value::Boolean(true)]);
            }
            ("SubmarineSurface", [Value::Object(id), ..]) => {
                if let Some(sub) = self.submarines.get_mut(id) {
                    sub.submerged = false;
                }
                return Ok(vec![Value::Boolean(true)]);
            }
            ("SubmarineDive" | "SubmarineSurface", _) => {
                return Ok(vec![Value::Boolean(true)]);
            }
            ("GetMissionEnded", _) => {
                return Ok(vec![Value::Boolean(self.mission_complete.is_some())]);
            }
            ("GetMissionWon", _) => {
                return Ok(vec![Value::Boolean(matches!(
                    self.mission_complete,
                    Some(MissionOutcome::Won { .. })
                ))]);
            }
            ("GetMissionLost", _) => {
                return Ok(vec![Value::Boolean(matches!(
                    self.mission_complete,
                    Some(MissionOutcome::Lost { .. })
                ))]);
            }
            (
                "GetScoreTotals"
                | "GetFriendlyScoreTotals"
                | "GetEnemyScoreTotals"
                | "GetHealthTotals",
                _,
            ) => {
                return Ok(vec![Value::Number(100.0)]);
            }
            ("GetObjectivesResult", _) => {
                let p = self.power_objectives.map_or(1.0, |v| v[1]);
                let t = self.tech_objectives.map_or(1.0, |v| v[1]);
                return Ok(vec![
                    Value::Number(p),
                    Value::Number(t),
                    Value::Number(100.0),
                ]);
            }
            ("SetUnitAllegiance", [Value::Object(id), army_val, ..]) => {
                let internal = match army_val {
                    Value::Enum(a) => Self::army_enum_to_internal(a).unwrap_or("eNeutral"),
                    _ => "eNeutral",
                };
                if let Ok(u) = self.entity_mut(*id) {
                    u.army = internal.to_string();
                }
            }
            ("NumItemsInArea", [first, rest @ ..]) => {
                let truthy = |v: Option<&Value>| {
                    v.is_some_and(|v| match v {
                        Value::Boolean(b) => *b,
                        Value::Number(n) => *n != 0.0,
                        _ => false,
                    })
                };
                let (zone_opt, center_opt, flags, all_match) = match (first, rest) {
                    (Value::Object(zid), [Value::Number(f), extra @ ..])
                        if self.zones.contains_key(zid) =>
                    {
                        (self.zones.get(zid), None, *f as u32, truthy(extra.first()))
                    }
                    (Value::Object(oid), [Value::Number(f), Value::Number(r)]) => {
                        (None, positions.get(oid).map(|&p| (p, *r)), *f as u32, false)
                    }
                    (
                        Value::Number(x),
                        [
                            Value::Number(z),
                            Value::Number(r),
                            Value::Number(f),
                            extra @ ..,
                        ],
                    ) => (
                        None,
                        Some(([*x as f32, 0.0, *z as f32], *r)),
                        *f as u32,
                        truthy(extra.first()),
                    ),
                    _ => return Ok(vec![Value::Number(0.0)]),
                };
                let mut count = 0usize;
                for (&uid, u) in &self.units {
                    if u.active != Some(true)
                        || u.health <= 0.0
                        || self.carrier_of.contains_key(&uid)
                    {
                        continue;
                    }
                    let u_flags = self.unit_type_flags(uid);
                    let flag_ok = if all_match {
                        (u_flags & flags) == flags
                    } else {
                        (u_flags & flags) != 0
                    };
                    if !flag_ok {
                        continue;
                    }
                    let Some(&p) = positions.get(&uid) else {
                        continue;
                    };
                    let inside = if let Some(zone) = zone_opt {
                        zone.contains(p).unwrap_or(false)
                    } else if let Some((c, r)) = center_opt {
                        (p[0] as f64 - c[0] as f64).hypot(p[2] as f64 - c[2] as f64) <= r
                    } else {
                        false
                    };
                    if inside {
                        count += 1;
                    }
                }
                return Ok(vec![Value::Number(count as f64)]);
            }
            ("SetInvulnerable", [flag_val, enabled_val]) => {
                let enabled = match enabled_val {
                    Value::Boolean(b) => *b,
                    Value::Number(n) => *n != 0.0,
                    _ => false,
                };
                let bits = match flag_val {
                    Value::Number(n) => *n as u32,
                    _ => 0,
                };
                if enabled {
                    self.invulnerable_army_mask |= bits;
                } else {
                    self.invulnerable_army_mask &= !bits;
                }
                let mask = self.invulnerable_army_mask;
                for (&uid, u) in self.units.iter_mut().chain(self.props.iter_mut()) {
                    if (Self::army_type_flag(&u.army) & bits) != 0 {
                        u.invulnerable = self.unit_invulnerable.contains(&uid)
                            || (Self::army_type_flag(&u.army) & mask) != 0;
                    }
                }
            }
            ("SetTimer", [Value::Number(seconds)]) => {
                self.timer_seconds = Some(seconds.max(0.0));
            }
            ("ShowTimer", [Value::Number(vis)]) => {
                self.timer_visible = *vis != 0.0;
            }
            ("ShowTimer", [Value::Boolean(vis)]) => {
                self.timer_visible = *vis;
            }
            ("GetTimeLeft", []) => {
                return Ok(vec![Value::Number(self.timer_seconds.unwrap_or(0.0))]);
            }
            ("PrepareSkipCutscene", _) => {
                self.skip_cutscene_prepared = true;
            }
            ("CleanupSkipCutscene", _) => {
                self.skip_cutscene_prepared = false;
            }
            ("CheckSkipCutscene", _) => {
                let skip = std::mem::replace(&mut self.skip_cutscene_requested, false);
                return Ok(vec![Value::Boolean(skip)]);
            }
            ("Vanish", [Value::Object(id)]) => {
                if let Ok(u) = self.entity_mut(*id) {
                    u.active = Some(false);
                }
                if self.props.contains_key(id)
                    && let Some(scene) = &mut self.collision
                {
                    let _ = std::sync::Arc::make_mut(scene).set_object_present(*id, false);
                }
                self.orders.remove(id);
                self.following.remove(id);
            }
            ("SetDamageZoneDamage", [Value::Object(zid), Value::Number(dps)]) => {
                if let Some(entry) = self.damage_zones.get_mut(zid) {
                    entry.1 = *dps;
                }
            }
            ("SetTickScriptWhenDead", [Value::Object(id), val, ..]) => {
                let on = match val {
                    Value::Boolean(b) => *b,
                    Value::Number(n) => *n != 0.0,
                    _ => false,
                };
                if on {
                    self.tick_when_dead.insert(*id);
                } else {
                    self.tick_when_dead.remove(id);
                }
            }
            ("SetTickScriptWhenDespawned", [Value::Object(id), val, ..]) => {
                let on = match val {
                    Value::Boolean(b) => *b,
                    Value::Number(n) => *n != 0.0,
                    _ => false,
                };
                if on {
                    self.tick_when_despawned.insert(*id);
                } else {
                    self.tick_when_despawned.remove(id);
                }
            }
            ("SetTickScriptWhenDead" | "SetTickScriptWhenDespawned", _) => {}
            (
                "CameraShake"
                | "ResetHealthAndScoreTotals"
                | "StoreHealthAndScoreTotals"
                | "AddStoredHealthAndScoreTotals"
                | "ResetPathBlocking"
                | "SetPathReblocking"
                | "SetNogoZone"
                | "SetEnableGuardMode"
                | "SetRenderHUDComponent"
                | "RenderOptionalHUDComponent"
                | "RepositionSkirmishScoreBoxes"
                | "SetHUDMultiplePlayerArmies"
                | "SetSkirmishScoreLocked"
                | "SetTutorialBButtonState"
                | "SetUnitAI"
                | "SetUnitShadowDistance"
                | "SetCharacterAnimUpdateLOD"
                | "SetReflectionRenderObjects"
                | "SetRenderParamTintBlendFactor"
                | "SetCameraEffectFadeIn"
                | "SetCameraEffectFadeOut"
                | "SetCameraEffectIntensity"
                | "SetDistortionEffect"
                | "SetFogColour"
                | "SetSunAmbientColour"
                | "SetSunDirectionalColour"
                | "SetAntiSunDirectionalColour"
                | "SetSunDirection"
                | "SetAntiSunDirection"
                | "ActivateArmyEffect"
                | "DectivateArmyEffect"
                | "TurnUnitLightsOn"
                | "PlaySound"
                | "RollAirUnit"
                | "SetIdle"
                | "Brace"
                | "Cower"
                | "VictoryDance"
                | "DefeatLament"
                | "CoopSetup"
                | "SetObjectivesResult"
                | "SetNetworkMissionComplete"
                | "NetworkGameOver",
                _,
            ) => {}
            _ => return Err(format!("Unsupported gameplay call/signature: {name}")),
        }
        Ok(vec![])
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn enable_weapon_does_not_wake_ai_or_change_presence() {
        let mut w = World {
            rewrite: Some(crate::mission_rewrite::Runtime::new(&ObjectGraph::default())),
            ..World::default()
        };
        let id = ObjectId(1);
        w.units.insert(
            id,
            Unit {
                health: 10.,
                max_health: 10.,
                invulnerable: false,
                active: Some(true),
                army: "test".into(),
                forward_speed: None,
                animation: None,
            },
        );
        w.activity.insert(
            id,
            Activity {
                enabled: Some(false),
                weapons_enabled: None,
            },
        );
        for enabled in [false, true] {
            w.call(
                "EnableWeapon",
                &[Value::Object(id), Value::Boolean(enabled)],
                &BTreeMap::new(),
            )
            .unwrap();
            assert_eq!(
                w.activity[&id],
                Activity {
                    enabled: Some(false),
                    weapons_enabled: Some(enabled)
                }
            );
            assert_eq!(w.units[&id].active, Some(true));
        }
        let before = w.activity.clone();
        assert!(
            w.call(
                "EnableWeapon",
                &[Value::Object(ObjectId(2)), Value::Boolean(false)],
                &BTreeMap::new()
            )
            .is_err()
        );
        assert_eq!(w.activity, before);
    }
    #[test]
    fn set_active_is_not_despawn_and_weapon_adjustment_is_optional() {
        let mut w = World::default();
        let id = ObjectId(1);
        w.units.insert(
            id,
            Unit {
                health: 50.0,
                max_health: 100.0,
                active: Some(true),
                invulnerable: false,
                army: "fixture".into(),
                forward_speed: None,
                animation: None,
            },
        );
        let positions = BTreeMap::new();
        let inactive = Value::Enum("INACTIVE".into());
        let active = Value::Enum("ACTIVE".into());
        let adjust = Value::Enum("ADJUST_WEAPON".into());
        w.call(
            "SetActive",
            &[Value::Object(id), inactive.clone(), adjust.clone()],
            &positions,
        )
        .unwrap();
        assert_eq!(w.units[&id].active, Some(true));
        assert_eq!(w.units[&id].health, 50.0);
        assert_eq!(
            w.activity[&id],
            Activity {
                enabled: Some(false),
                weapons_enabled: Some(false)
            }
        );
        w.call(
            "SetActive",
            &[Value::Object(id), active.clone()],
            &positions,
        )
        .unwrap();
        assert_eq!(
            w.activity[&id],
            Activity {
                enabled: Some(true),
                weapons_enabled: Some(false)
            }
        );
        w.call(
            "SetActive",
            &[Value::Object(id), active, adjust],
            &positions,
        )
        .unwrap();
        assert_eq!(w.activity[&id].weapons_enabled, Some(true));
        w.call(
            "SetActive",
            &[
                Value::Object(id),
                inactive,
                Value::Enum("DONT_ADJUST_WEAPON".into()),
            ],
            &positions,
        )
        .unwrap();
        assert_eq!(
            w.activity[&id],
            Activity {
                enabled: Some(false),
                weapons_enabled: Some(true)
            }
        );
        assert_eq!(
            w.call("GetHealthPercent", &[Value::Object(id)], &positions)
                .unwrap(),
            vec![Value::Number(50.0)]
        );
        let previous = w.activity.clone();
        assert!(
            w.call(
                "SetActive",
                &[
                    Value::Object(id),
                    Value::Enum("ACTIVE".into()),
                    Value::Number(99.0)
                ],
                &positions
            )
            .is_err()
        );
        assert_eq!(w.activity, previous);
    }
    #[test]
    fn lifecycle_preserves_health_and_rejects_unrecovered_signatures() {
        let mut w = World::default();
        let id = ObjectId(1);
        let positions = BTreeMap::new();
        let entity = Unit {
            health: 70.0,
            max_health: 100.0,
            active: Some(true),
            invulnerable: false,
            army: "fixture".into(),
            forward_speed: None,
            animation: None,
        };
        w.props.insert(id, entity.clone());
        w.props.insert(ObjectId(2), entity);
        w.buildings.insert(id);
        w.orders.insert(
            id,
            MoveOrder {
                target: [2.0, 0.0],
                radius: 1.0,
            },
        );
        w.direct_velocity.insert(id, [1.0, 0.0]);
        w.call("Despawn", &[Value::Object(id)], &positions).unwrap();
        assert_eq!(w.props[&id].active, Some(false));
        assert_eq!(w.props[&id].health, 70.0);
        assert!(w.orders.is_empty());
        assert!(w.direct_velocity.is_empty());
        assert!(!w.unit_list_presence(id).unwrap());
        w.call(
            "Despawn",
            &[Value::Object(id), Value::Boolean(true)],
            &positions,
        )
        .unwrap();
        assert_eq!(w.props[&id].active, Some(false));
        assert!(w.unit_list_presence(id).unwrap());
        assert!(w.despawn_retention[&id]);
        assert!(
            w.call(
                "Despawn",
                &[Value::Object(id), Value::Number(1.0)],
                &positions
            )
            .is_err()
        );
        assert!(w.despawn_retention[&id]);
        w.call("Despawn", &[Value::Object(id)], &positions).unwrap();
        assert!(!w.unit_list_presence(id).unwrap());
        assert!(
            w.call("Spawn", &[Value::Object(ObjectId(2))], &positions)
                .is_err()
        );
        w.call("Spawn", &[Value::Object(id)], &positions).unwrap();
        assert_eq!(w.props[&id].active, Some(true));
        w.capture_points.insert(id);
        for name in ["Spawn", "Despawn"] {
            assert!(
                w.call(name, &[Value::Object(id)], &positions)
                    .unwrap_err()
                    .contains("Capture-point lifecycle")
            );
            assert_eq!(w.props[&id].active, Some(true));
        }
        assert_eq!(w.props[&id].health, 70.0);
        w.call(
            "SetUnitInvulnerable",
            &[Value::Object(id), Value::Boolean(true)],
            &positions,
        )
        .unwrap();
        w.damage(id, 100.0).unwrap();
        assert_eq!(w.props[&id].health, 70.0);
        w.call(
            "SetHealthPercent",
            &[Value::Object(id), Value::Number(0.0)],
            &positions,
        )
        .unwrap();
        assert_eq!(
            w.call("IsDead", &[Value::Object(id)], &positions).unwrap(),
            vec![Value::Boolean(true)]
        );
        assert!(w.call("Spawn", &[Value::Object(id)], &positions).is_err());
    }
    #[test]
    fn fresh_player_flag_never_invents_a_roster_or_a_defeat() {
        let id = ObjectId(1);
        let mut world = World {
            player: Some(id),
            player_view: Some(PlayerView {
                camera: ObjectId(9),
                target: id,
                cut_revision: 0,
            }),
            ..Default::default()
        };
        world.units.insert(
            id,
            Unit {
                health: 100.0,
                max_health: 100.0,
                invulnerable: false,
                active: Some(true),
                army: "fixture".into(),
                forward_speed: Some(1.0),
                animation: None,
            },
        );
        let args = [Value::Enum("PLAYER_ONE".into())];
        let positions = BTreeMap::new();
        assert!(world.call("AllPlayerUnitsDead", &args, &positions).is_err());
        world.configure_fresh_player_lifecycle().unwrap();
        assert!(world.roster.is_empty());
        assert_eq!(
            world.call("AllPlayerUnitsDead", &args, &positions).unwrap(),
            vec![Value::Boolean(false)]
        );
        let mut removed = world.clone();
        removed
            .call("Despawn", &[Value::Object(id)], &positions)
            .unwrap();
        removed
            .call("Spawn", &[Value::Object(id)], &positions)
            .unwrap();
        assert!(
            removed
                .call("AllPlayerUnitsDead", &args, &positions)
                .is_err()
        );
        let mut scripted = world.clone();
        scripted
            .call(
                "SetHealthPercent",
                &[Value::Object(id), Value::Number(0.0)],
                &positions,
            )
            .unwrap();
        assert_eq!(
            scripted.player_lifecycle,
            Some(PlayerLifecycle::ReplacementRequired)
        );
        world.damage(id, 100.0).unwrap();
        assert_eq!(
            world.player_lifecycle,
            Some(PlayerLifecycle::ReplacementRequired)
        );
        assert!(world.call("AllPlayerUnitsDead", &args, &positions).is_err());
        world
            .call(
                "SetHealthPercent",
                &[Value::Object(id), Value::Number(100.0)],
                &positions,
            )
            .unwrap();
        assert!(world.configure_fresh_player_lifecycle().is_err());
        assert!(world.call("AllPlayerUnitsDead", &args, &positions).is_err());
        assert!(world.roster.is_empty());
    }
    #[test]
    fn a_camera_target_is_not_a_recovered_player_roster() {
        let mut world = World {
            player: Some(ObjectId(1)),
            ..Default::default()
        };
        assert!(
            world
                .call(
                    "AllPlayerUnitsDead",
                    &[Value::Enum("PLAYER_ONE".into())],
                    &BTreeMap::new()
                )
                .unwrap_err()
                .contains("roster not configured")
        );
        world.roster.insert(ObjectId(2));
        assert!(
            world
                .call(
                    "AllPlayerUnitsDead",
                    &[Value::Enum("PLAYER_ONE".into())],
                    &BTreeMap::new()
                )
                .unwrap_err()
                .contains("roster not configured")
        );
    }
    fn waypoint_world() -> (World, BTreeMap<ObjectId, [f32; 3]>) {
        use crate::mission_waypoints::{Bank, Node};
        use std::sync::Arc;
        let mut nodes = BTreeMap::new();
        for (id, x, next) in [(10, 8.0, Some(ObjectId(11))), (11, 16.0, None)] {
            nodes.insert(
                ObjectId(id),
                Node {
                    matrix: [
                        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, x, 0.0, 0.0,
                        1.0,
                    ],
                    next,
                    optional: [None, None],
                    flags: 1,
                    pause: 0.0,
                },
            );
        }
        let mut w = World {
            navigation: Navigation::CollisionAware,
            collision: Some(Arc::new(crate::unit_collision::Scene::default())),
            waypoints: Some(Arc::new(Bank::from_nodes(nodes).unwrap())),
            ..Default::default()
        };
        w.units.insert(
            ObjectId(1),
            Unit {
                health: 100.0,
                max_health: 100.0,
                invulnerable: false,
                active: Some(true),
                army: "fixture".into(),
                forward_speed: Some(4.0),
                animation: None,
            },
        );
        w.bodies
            .insert(ObjectId(1), crate::unit_collision::Body::INFANTRY);
        (w, [(ObjectId(1), [0.0; 3])].into())
    }
    #[test]
    fn faded_default_transfer_is_not_a_visible_hard_cut() {
        use crate::mission_camera::{Bank, Camera, Rig};
        let (mut w, positions) = waypoint_world();
        let old = ObjectId(1);
        let new = ObjectId(2);
        let camera = ObjectId(99);
        w.units.insert(new, w.units[&old].clone());
        w.bodies.insert(new, crate::unit_collision::Body::INFANTRY);
        w.player = Some(old);
        w.player_view = Some(PlayerView {
            camera,
            target: old,
            cut_revision: 7,
        });
        let mut bank = Bank::default();
        bank.cameras.insert(
            camera,
            Camera {
                position: [0.; 3],
                target: None,
                waypoint: None,
                fov: 60.,
                kind: "eCAMTYPE_CHASETARGET".into(),
            },
        );
        let mut rig = Rig::new(std::sync::Arc::new(bank));
        rig.activate_chase(camera, old).unwrap();
        rig.black_alpha = 1.;
        w.cameras = Some(rig);
        w.frozen = true;
        w.rewrite = Some(crate::mission_rewrite::Runtime::new(&ObjectGraph::default()));
        let args = [Value::Enum("PLAYER_ONE".into()), Value::Object(new)];
        assert!(
            w.call("ForceUnitTransfer", &args, &positions)
                .unwrap_err()
                .contains("presentation")
        );
        assert_eq!(w.player, Some(old));
        w.rewrite.as_mut().unwrap().allow_cosmetic_gaps();
        for (frozen, alpha) in [(false, 1.), (true, 0.5)] {
            let mut rejected = w.clone();
            rejected.frozen = frozen;
            rejected.cameras.as_mut().unwrap().black_alpha = alpha;
            assert!(
                rejected
                    .call("ForceUnitTransfer", &args, &positions)
                    .is_err()
            );
            assert_eq!(rejected.player, Some(old));
            assert_eq!(rejected.player_view.as_ref().unwrap().cut_revision, 7);
            assert!(rejected.rewrite.as_ref().unwrap().warnings().is_empty());
        }
        let mut invalid = w.clone();
        invalid.units.get_mut(&new).unwrap().health = f64::NAN;
        assert!(
            invalid
                .call("ForceUnitTransfer", &args, &positions)
                .is_err()
        );
        assert_eq!(invalid.player, Some(old));
        assert!(invalid.rewrite.as_ref().unwrap().warnings().is_empty());
        for options in [vec![], vec![Value::Boolean(false)]] {
            let mut accepted = w.clone();
            let mut args = args.to_vec();
            args.extend(options);
            accepted
                .call("ForceUnitTransfer", &args, &positions)
                .unwrap();
            assert_eq!(accepted.player, Some(new));
            assert_eq!(accepted.cameras.as_ref().unwrap().chase_target(), Some(new));
            assert_eq!(accepted.player_view.as_ref().unwrap().cut_revision, 7);
            assert!(accepted.roster.contains(&new));
            assert_eq!(accepted.rewrite.as_ref().unwrap().warnings().len(), 1);
        }
        let mut args = args.to_vec();
        args.push(Value::Boolean(true));
        w.call("ForceUnitTransfer", &args, &positions).unwrap();
        assert_eq!(w.player_view.as_ref().unwrap().cut_revision, 8);
    }
    fn follow_args() -> [Value; 5] {
        [
            Value::Object(ObjectId(1)),
            Value::Object(ObjectId(10)),
            Value::Number(0.0),
            Value::Number(0.0),
            Value::Enum("ORDER_FORCED".into()),
        ]
    }
    #[test]
    fn waypoint_follower_uses_collision_and_does_not_skip_blocked_nodes() {
        use std::sync::Arc;
        let (mut w, mut positions) = waypoint_world();
        let mut scene = crate::unit_collision::Scene::default();
        scene
            .add_mesh(vec![
                [[5.0, 0.0, -10.0], [5.0, 4.0, -10.0], [5.0, 0.0, 10.0]],
                [[5.0, 4.0, -10.0], [5.0, 4.0, 10.0], [5.0, 0.0, 10.0]],
            ])
            .unwrap();
        w.collision = Some(Arc::new(scene));
        w.call("FollowWaypoint", &follow_args(), &positions)
            .unwrap();
        for _ in 0..80 {
            w.advance(0.05, &mut positions).unwrap()
        }
        assert!(positions[&ObjectId(1)][0] < 4.41);
        assert_eq!(w.following[&ObjectId(1)].node, ObjectId(10));
        assert!(w.mesh_contacts > 0);
        w.collision = Some(Arc::new(crate::unit_collision::Scene::default()));
        for _ in 0..100 {
            w.advance(0.05, &mut positions).unwrap()
        }
        assert!(w.following.is_empty());
        assert!(w.orders.is_empty());
        assert!((positions[&ObjectId(1)][0] - 15.0).abs() < 0.002);
    }
    #[test]
    fn waypoint_follower_replans_around_wall_and_continues_next_link() {
        use std::sync::Arc;
        let (mut w, mut p) = waypoint_world();
        w.navigation = Navigation::PlannedInfantry;
        let mut scene = crate::unit_collision::Scene::default();
        scene
            .add_mesh(vec![
                [[5.0, 0.0, -2.0], [5.0, 4.0, -2.0], [5.0, 0.0, 2.0]],
                [[5.0, 4.0, -2.0], [5.0, 4.0, 2.0], [5.0, 0.0, 2.0]],
            ])
            .unwrap();
        w.collision = Some(Arc::new(scene));
        w.call("FollowWaypoint", &follow_args(), &p).unwrap();
        let mut detoured = false;
        for _ in 0..400 {
            w.advance(0.05, &mut p).unwrap();
            detoured |= p[&ObjectId(1)][2].abs() > 2.5;
        }
        assert!(detoured);
        assert!(w.route_searches > 0);
        assert!(w.following.is_empty());
        assert!(w.orders.is_empty());
        assert!((p[&ObjectId(1)][0] - 16.0).hypot(p[&ObjectId(1)][2]) < 1.01);
    }
    #[test]
    fn waypoint_orders_validate_before_mutation_and_obey_freeze_and_cancellation() {
        let (mut w, mut p) = waypoint_world();
        let mut args = follow_args();
        w.navigation = Navigation::Unavailable;
        assert!(w.call("FollowWaypoint", &args, &p).is_err());
        assert!(w.orders.is_empty());
        w.navigation = Navigation::CollisionAware;
        w.call("FollowWaypoint", &args, &p).unwrap();
        args[1] = Value::Object(ObjectId(999));
        assert!(w.call("FollowWaypoint", &args, &p).is_err());
        assert_eq!(w.following[&ObjectId(1)].node, ObjectId(10));
        w.player = Some(ObjectId(1));
        w.frozen = true;
        w.advance(0.5, &mut p).unwrap();
        assert_eq!(p[&ObjectId(1)], [0.0; 3]);
        w.frozen = false;
        w.advance(0.5, &mut p).unwrap();
        assert!(p[&ObjectId(1)][0] > 1.9);
        w.call(
            "GoToArea",
            &[
                Value::Object(ObjectId(1)),
                Value::Number(-5.0),
                Value::Number(0.0),
                Value::Number(1.0),
                Value::Number(0.0),
                Value::Enum("ORDER_FORCED".into()),
            ],
            &p,
        )
        .unwrap();
        assert!(w.following.is_empty());
        w.call("FollowWaypoint", &follow_args(), &p).unwrap();
        w.call("Despawn", &[Value::Object(ObjectId(1))], &p)
            .unwrap();
        assert!(w.following.is_empty());
        assert!(w.orders.is_empty());
    }
    #[test]
    fn rotated_zone_and_boundary() {
        let mut z = Zone {
            matrix: [
                0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 10.0, 0.0, 30.0, 1.0,
            ],
            size: [4.0, 2.0, 10.0, 1.0],
            radius: 0.0,
        };
        assert!(z.contains([14.0, 0.0, 30.0]).unwrap());
        assert!(!z.contains([10.0, 0.0, 33.0]).unwrap());
        assert!(z.contains([10.0, 0.0, 32.0]).unwrap());
        z.radius = 2.0;
        assert!(!z.contains([12.0, 0.0, 32.0]).unwrap());
        assert!(z.contains([12.0, 0.0, 30.0]).unwrap());
        assert!(z.contains([f32::NAN, 0.0, 0.0]).is_err());
    }
    #[test]
    fn damage_invulnerability_and_explicit_activation() {
        let id = ObjectId(1);
        let mut w = World::default();
        w.units.insert(
            id,
            Unit {
                health: 100.0,
                max_health: 100.0,
                invulnerable: false,
                active: None,
                army: "fixture".into(),
                forward_speed: Some(20.0),
                animation: None,
            },
        );
        assert!(w.damage(id, 20.0).is_err());
        assert_eq!(w.units[&id].health, 100.0);
        w.units.get_mut(&id).unwrap().active = Some(true);
        w.damage(id, 20.0).unwrap();
        assert_eq!(w.units[&id].health, 80.0);
        w.call(
            "SetUnitInvulnerable",
            &[Value::Object(id), Value::Boolean(true)],
            &BTreeMap::new(),
        )
        .unwrap();
        w.damage(id, 1000.0).unwrap();
        assert_eq!(w.units[&id].health, 80.0);
        assert!(w.damage(id, f64::INFINITY).is_err());
        assert!(
            w.call(
                "SetHealthPercent",
                &[Value::Object(id), Value::Number(-1.0)],
                &BTreeMap::new()
            )
            .is_err()
        );
        assert_eq!(w.units[&id].health, 80.0);
        w.units.get_mut(&id).unwrap().invulnerable = false;
        w.damage(id, 1000.0).unwrap();
        assert_eq!(
            w.call("IsDead", &[Value::Object(id)], &BTreeMap::new())
                .unwrap(),
            vec![Value::Boolean(true)]
        );
    }
    #[test]
    fn branching_waypoints_advance_deterministically_and_order_normal_respects_rewrite_liveness() {
        use crate::mission_waypoints::{Bank, Node};
        use std::sync::Arc;
        let node = |x: f32, z: f32, next, optional| Node {
            matrix: [
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, x, 0.0, z, 1.0,
            ],
            next,
            optional,
            flags: 0,
            pause: 0.0,
        };
        let nodes = BTreeMap::from([
            (
                ObjectId(10),
                node(
                    2.0,
                    0.0,
                    Some(ObjectId(11)),
                    [Some(ObjectId(12)), Some(ObjectId(13))],
                ),
            ),
            (ObjectId(11), node(10.0, -4.0, None, [None, None])),
            (ObjectId(12), node(10.0, 0.0, None, [None, None])),
            (ObjectId(13), node(10.0, 4.0, None, [None, None])),
        ]);
        let (mut w, mut p) = waypoint_world();
        w.waypoints = Some(Arc::new(Bank::from_nodes(nodes).unwrap()));
        let mut normal_args = follow_args();
        normal_args[4] = Value::Enum("ORDER_NORMAL".into());
        assert!(w.call("FollowWaypoint", &normal_args, &p).is_err());
        w.rewrite = Some(crate::mission_rewrite::Runtime::new(&ObjectGraph::default()));
        assert_eq!(
            w.call("FollowWaypoint", &normal_args, &p).unwrap(),
            vec![Value::Boolean(true)]
        );
        for _ in 0..20 {
            w.advance(0.05, &mut p).unwrap();
        }
        // First CrtRand::next() from seed 1 is 16838; 16838 % 3 == 2 -> slot 2 -> ObjectId(13).
        assert_eq!(w.following[&ObjectId(1)].node, ObjectId(13));
        assert_eq!(w.orders[&ObjectId(1)].target, [10.0, 4.0]);

        w.units.get_mut(&ObjectId(1)).unwrap().health = 0.0;
        assert_eq!(
            w.call("FollowWaypoint", &normal_args, &p).unwrap(),
            vec![Value::Boolean(false)]
        );
        w.rewrite = None;
        assert!(w.call("FollowWaypoint", &follow_args(), &p).is_err());
    }
    #[test]
    fn non_player_troop_teleport_validates_collision_and_never_relocates_player() {
        use std::sync::Arc;
        let (mut w, mut p) = waypoint_world();
        let mut scene = crate::unit_collision::Scene::default();
        scene
            .add_mesh(vec![
                [[19.0, 0.0, -2.0], [19.0, 4.0, -2.0], [19.0, 0.0, 2.0]],
                [[19.0, 4.0, -2.0], [19.0, 4.0, 2.0], [19.0, 0.0, 2.0]],
            ])
            .unwrap();
        w.collision = Some(Arc::new(scene));
        let troop = ObjectId(1);
        let blocker = ObjectId(2);
        w.units.insert(blocker, w.units[&troop].clone());
        w.bodies
            .insert(blocker, crate::unit_collision::Body::INFANTRY);
        p.insert(blocker, [6.0, 0.0, 0.0]);
        w.call("FollowWaypoint", &follow_args(), &p).unwrap();

        // Without rewrite configured -> rejected with no state change.
        let before_p = p.clone();
        assert!(w.teleport_scripted(&mut p, troop, 0.0, -4.0, 90.0).is_err());
        assert_eq!(p, before_p);
        let mut graph = ObjectGraph::default();
        graph
            .merge(
                crate::parse_objects(
                    r#"<Instances><Object type="cTroop" id="1"/><Object type="cGroundVehicle" id="2"/><Object type="cBuilding" id="3"/></Instances>"#,
                )
                .unwrap(),
            )
            .unwrap();
        w.rewrite = Some(crate::mission_rewrite::Runtime::new(&graph));

        // Unsupported entity type -> rejected with no state change.
        assert!(
            w.teleport_scripted(&mut p, ObjectId(3), 0.0, -4.0, 90.0)
                .is_err()
        );
        assert_eq!(p, before_p);

        // Obstructed by another active unit -> rejected with no state change.
        assert!(w.teleport_scripted(&mut p, troop, 6.2, 0.0, 90.0).is_err());
        assert_eq!(p, before_p);
        assert!(w.following.contains_key(&troop));

        // Obstructed by static wall mesh -> rejected with no state change.
        assert!(w.teleport_scripted(&mut p, troop, 19.0, 0.0, 90.0).is_err());
        assert_eq!(p, before_p);

        // Unfrozen player unit -> rejected even on clear ground.
        w.player = Some(troop);
        w.frozen = false;
        assert!(w.teleport_scripted(&mut p, troop, 0.0, -4.0, 90.0).is_err());
        assert_eq!(p, before_p);

        // Frozen cutscene player relocation on clear ground -> updates position/heading/aim and clears orders.
        w.frozen = true;
        w.teleport_scripted(&mut p, troop, 0.0, -4.0, 90.0).unwrap();
        assert_eq!(p[&troop], [0.0, 0.0, -4.0]);
        assert!((w.headings[&troop] - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert!(!w.following.contains_key(&troop));
        assert!(!w.orders.contains_key(&troop));
    }

    #[test]
    fn ambient_sound_sphere_and_numeric_set_active_work_and_transfer_by_death_switches_player() {
        let xml = r#"<Instances>
            <Object type="cAmbientAreaPointSoundSphere" id="780001693">
                <Attribute name="mFlags"><Item>4</Item></Attribute>
            </Object>
        </Instances>"#;
        let mut graph = ObjectGraph::default();
        graph.merge(crate::parse_objects(xml).unwrap()).unwrap();
        let mut w = World::from_graph(&graph).unwrap();
        graph
            .merge(
                crate::parse_objects(
                    r#"<Instances><Object type="cTroop" id="10"/><Object type="cTroop" id="11"/></Instances>"#,
                )
                .unwrap(),
            )
            .unwrap();
        w.rewrite = Some(crate::mission_rewrite::Runtime::new(&graph));
        assert_eq!(
            w.activity[&ObjectId(780001693)],
            Activity {
                enabled: Some(false),
                weapons_enabled: None,
            }
        );
        let p = BTreeMap::from([
            (ObjectId(10), [0.0, 0.0, 0.0]),
            (ObjectId(11), [4.0, 0.0, 0.0]),
        ]);
        w.call(
            "SetActive",
            &[Value::Number(780001693.0), Value::Enum("ACTIVE".into())],
            &p,
        )
        .unwrap();
        assert_eq!(w.activity[&ObjectId(780001693)].enabled, Some(true));

        w.units.insert(
            ObjectId(10),
            Unit {
                health: 0.0,
                max_health: 100.0,
                invulnerable: false,
                army: "eSolarEmpire".into(),
                active: Some(true),
                forward_speed: None,
                animation: None,
            },
        );
        w.units.insert(
            ObjectId(11),
            Unit {
                health: 100.0,
                max_health: 100.0,
                invulnerable: false,
                army: "eSolarEmpire".into(),
                active: Some(true),
                forward_speed: None,
                animation: None,
            },
        );
        w.activity.insert(
            ObjectId(10),
            Activity {
                enabled: Some(true),
                weapons_enabled: Some(true),
            },
        );
        w.activity.insert(
            ObjectId(11),
            Activity {
                enabled: Some(true),
                weapons_enabled: Some(true),
            },
        );
        w.bodies
            .insert(ObjectId(10), crate::unit_collision::Body::INFANTRY);
        w.bodies
            .insert(ObjectId(11), crate::unit_collision::Body::INFANTRY);
        w.roster = BTreeSet::from([ObjectId(10), ObjectId(11)]);
        w.player = Some(ObjectId(10));
        w.squad_follow.insert(ObjectId(11));
        w.orders.insert(
            ObjectId(11),
            MoveOrder {
                target: [10.0, 10.0],
                radius: 2.0,
            },
        );
        assert_eq!(
            w.call("GetPlayerTransferred", &[], &p).unwrap(),
            vec![Value::Boolean(false)]
        );
        let mut positions = p.clone();
        w.advance(1.0 / 60.0, &mut positions).unwrap();
        assert_eq!(w.player, Some(ObjectId(11)));
        assert!(!w.orders.contains_key(&ObjectId(11)));
        assert_eq!(
            w.call("GetPlayerTransferred", &[], &positions).unwrap(),
            vec![Value::Boolean(true)]
        );
        assert_eq!(
            w.call("GetPlayerTransferred", &[], &positions).unwrap(),
            vec![Value::Boolean(false)]
        );
    }

    #[test]
    fn capture_point_height_installation_owner_and_air_landing_state_work() {
        let (mut w, mut p) = waypoint_world();
        let mut graph = ObjectGraph::default();
        graph
            .merge(
                crate::parse_objects(
                    r#"<Instances><Object type="cAirVehicle" id="1"/><Object type="cCapturePoint" id="50"/><Object type="cStrategicInstallation" id="60"/></Instances>"#,
                )
                .unwrap(),
            )
            .unwrap();
        w.rewrite = Some(crate::mission_rewrite::Runtime::new(&graph));
        w.non_ground_units.insert(ObjectId(1));
        w.scripted_air_cruise.insert(ObjectId(1), 10.0);
        w.capture_points.insert(ObjectId(50));
        w.capture_point_state.insert(
            ObjectId(50),
            CapturePointState {
                owner: "eAngloIsles".into(),
                height: 100.0,
                direction: 0,
                capturing_army: None,
                installation: Some(ObjectId(60)),
            },
        );
        w.installations.insert(
            ObjectId(60),
            StrategicInstallation {
                main_building: None,
                capture_point: Some(ObjectId(50)),
                buildings: Vec::new(),
                owner: "eAngloIsles".into(),
                ..Default::default()
            },
        );
        assert_eq!(
            w.call("GetInstallationOwner", &[Value::Object(ObjectId(60))], &p)
                .unwrap(),
            vec![Value::Enum("ARMY_ANGLO".into())]
        );
        w.call(
            "SetCapturePointHeight",
            &[
                Value::Object(ObjectId(50)),
                Value::Enum("ARMY_SOLAR".into()),
                Value::Number(25.0),
            ],
            &p,
        )
        .unwrap();
        assert_eq!(w.capture_point_state[&ObjectId(50)].height, 25.0);
        assert_eq!(w.capture_point_state[&ObjectId(50)].owner, "eSolarEmpire");

        assert_eq!(
            w.call("GetMovementState", &[Value::Object(ObjectId(1))], &p)
                .unwrap(),
            vec![Value::Enum("MOVEMENT_STATE_AIRBORNE".into())]
        );
        w.call(
            "LandAirUnit",
            &[
                Value::Object(ObjectId(1)),
                Value::Number(2.0),
                Value::Number(0.0),
                Value::Enum("ORDER_FORCED".into()),
            ],
            &p,
        )
        .unwrap();
        assert!(w.landing_units.contains(&ObjectId(1)));
        for _ in 0..60 {
            w.advance(0.05, &mut p).unwrap();
        }
        assert!(w.grounded_units.contains(&ObjectId(1)));
        assert_eq!(
            w.call("GetMovementState", &[Value::Object(ObjectId(1))], &p)
                .unwrap(),
            vec![Value::Enum("MOVEMENT_STATE_ON_GROUND".into())]
        );
    }
}
