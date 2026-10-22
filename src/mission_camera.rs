//! Authored cinematic camera state. Linear waypoint travel is a replacement policy,
//! not recovered spline parity. Presentation must consume poses; this is not rendering.
use crate::{
    ObjectGraph, ObjectId, Result,
    lua50::Value,
    original_scene::{field, matrix},
};
use std::{collections::BTreeMap, sync::Arc};
#[derive(Clone, Debug)]
pub struct Waypoint {
    pub position: [f32; 3],
    pub next: Option<ObjectId>,
    pub speed: f64,
    pub pause: f64,
    pub flags: u32,
    pub branches: bool,
}
#[derive(Clone, Debug)]
pub struct Camera {
    pub position: [f32; 3],
    pub target: Option<ObjectId>,
    pub waypoint: Option<ObjectId>,
    pub fov: f64,
    pub kind: String,
}
#[derive(Clone, Default)]
pub struct Bank {
    pub cameras: BTreeMap<ObjectId, Camera>,
    pub waypoints: BTreeMap<ObjectId, Waypoint>,
    pub initial_active: Option<ObjectId>,
}
fn number(o: &crate::Object, key: &str) -> Result<f64> {
    let v = field(o, key)
        .ok_or_else(|| format!("Missing {key}"))?
        .parse::<f64>()
        .map_err(|_| format!("Invalid {key}"))?;
    if !v.is_finite() {
        return Err(format!("Nonfinite {key}"));
    }
    Ok(v)
}
fn reference(o: &crate::Object, key: &str) -> Result<Option<ObjectId>> {
    let id = field(o, key)
        .ok_or_else(|| format!("Missing {key}"))?
        .parse::<u64>()
        .map_err(|_| "Invalid camera reference")?;
    Ok((id != 0).then_some(ObjectId(id)))
}
impl Bank {
    pub fn load(graph: &ObjectGraph) -> Result<Self> {
        let mut bank = Self::default();
        for o in graph.objects.values() {
            if !matches!(o.kind.as_str(), "cCamera" | "cWaypoint") {
                continue;
            }
            let m = matrix(field(o, "Mat").ok_or("Missing camera/waypoint matrix")?)?;
            let position = [m[12], m[13], m[14]];
            if o.kind == "cWaypoint" {
                let speed = number(o, "mSpeed")?;
                let pause = number(o, "mPauseTime")?;
                if speed < 0.0 || pause < 0.0 {
                    return Err("Negative waypoint speed/pause".into());
                }
                bank.waypoints.insert(
                    o.id,
                    Waypoint {
                        position,
                        next: reference(o, "NextWP")?,
                        speed,
                        pause,
                        flags: field(o, "Flags")
                            .ok_or("Missing waypoint flags")?
                            .parse()
                            .map_err(|_| "Invalid waypoint flags")?,
                        branches: reference(o, "mOptionalNextWP1")?.is_some()
                            || reference(o, "mOptionalNextWP2")?.is_some(),
                    },
                );
            } else {
                let base = graph
                    .objects
                    .get(&reference(o, "mBase")?.ok_or("Missing camera base")?)
                    .ok_or("Unresolved camera base")?;
                let fov = number(base, "fov")?.to_degrees();
                if !(1.0..179.0).contains(&fov) {
                    return Err("Invalid authored FOV".into());
                }
                let kind: String = field(o, "mCamType").ok_or("Missing camera type")?.into();
                let flags: u32 = field(o, "mInstanceFlags")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                if (flags & 1) != 0 && kind == "eCAMTYPE_WAYPOINTCAM" {
                    bank.initial_active = Some(o.id);
                }
                bank.cameras.insert(
                    o.id,
                    Camera {
                        position,
                        target: reference(o, "mTarget")?,
                        waypoint: reference(o, "mCurrentWP")?,
                        fov,
                        kind,
                    },
                );
            }
        }
        for w in bank.waypoints.values() {
            if w.next.is_some_and(|id| !bank.waypoints.contains_key(&id)) {
                return Err("Unresolved waypoint successor".into());
            }
        }
        Ok(bank)
    }
    fn validate_path(&self, start: ObjectId) -> Result<()> {
        let mut seen = std::collections::BTreeSet::new();
        let mut current = Some(start);
        while let Some(id) = current {
            if !seen.insert(id) {
                break;
            }
            let w = self.waypoints.get(&id).ok_or("Unknown camera waypoint")?;
            // 0x20 is the authored WP_CAMERA_WAYPOINT category marker; 0x01/0x10/0x40 are authored camera waypoint flags.
            if w.flags & !0xff != 0 || w.branches {
                return Err("Camera waypoint flags/branches unsupported".into());
            }
            current = w.next;
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
struct Cursor {
    node: ObjectId,
    position: [f32; 3],
    pause: f64,
}
impl Cursor {
    fn new(bank: &Bank, id: ObjectId) -> Result<Self> {
        bank.validate_path(id)?;
        let w = &bank.waypoints[&id];
        Ok(Self {
            node: id,
            position: w.position,
            pause: w.pause,
        })
    }
    fn advance(&mut self, bank: &Bank, mut dt: f64) -> Result<()> {
        for _ in 0..1024 {
            if dt <= 0.0 {
                return Ok(());
            }
            let wait = dt.min(self.pause);
            dt -= wait;
            self.pause -= wait;
            if dt <= 0.0 {
                return Ok(());
            }
            let w = &bank.waypoints[&self.node];
            let Some(next) = w.next else { return Ok(()) };
            if w.speed == 0.0 {
                return Ok(());
            }
            let destination = bank.waypoints[&next].position;
            let delta =
                std::array::from_fn::<_, 3, _>(|i| destination[i] as f64 - self.position[i] as f64);
            let distance = delta.iter().map(|v| v * v).sum::<f64>().sqrt();
            let travel = distance / w.speed;
            if travel > dt {
                let ratio = dt / travel;
                self.position =
                    std::array::from_fn(|i| (self.position[i] as f64 + delta[i] * ratio) as f32);
                return Ok(());
            }
            dt -= travel;
            self.position = destination;
            self.node = next;
            self.pause = bank.waypoints[&next].pause;
        }
        Err("Camera waypoint transition budget exceeded".into())
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Pose {
    pub position: [f32; 3],
    pub look_at: [f32; 3],
    pub fov_degrees: f64,
}
#[derive(Clone)]
struct Fade {
    from: f64,
    to: f64,
    elapsed: f64,
    duration: f64,
}
#[derive(Clone)]
pub struct Rig {
    pub bank: Arc<Bank>,
    pub active: Option<ObjectId>,
    chase: Option<(ObjectId, ObjectId)>,
    chase_aim: [f64; 3],
    /// Black-overlay opacity for the renderer. Timing is simulation-clock driven.
    pub black_alpha: f64,
    fade: Option<Fade>,
    fov_motion: BTreeMap<ObjectId, (f64, f64)>,
    cameras: BTreeMap<ObjectId, Camera>,
    paths: BTreeMap<ObjectId, Cursor>,
    targets: BTreeMap<ObjectId, Cursor>,
}
impl Rig {
    pub fn new(bank: Arc<Bank>) -> Self {
        let mut paths = BTreeMap::new();
        let mut targets = BTreeMap::new();
        let active = bank.initial_active.and_then(|id| {
            let cam = bank.cameras.get(&id)?;
            if let Some(wp) = cam.waypoint
                && let Ok(cursor) = Cursor::new(&bank, wp)
            {
                paths.insert(id, cursor);
            }
            if let Some(t) = cam.target.filter(|t| bank.waypoints.contains_key(t))
                && let Ok(cursor) = Cursor::new(&bank, t)
            {
                targets.insert(id, cursor);
            }
            Some(id)
        });
        Self {
            cameras: bank.cameras.clone(),
            bank,
            active,
            chase: None,
            chase_aim: [0.0, 0.0, -1.0],
            black_alpha: 0.0,
            fade: None,
            fov_motion: BTreeMap::new(),
            paths,
            targets,
        }
    }
    /// Explicit Rust chase-camera adapter; caller supplies grounded target coordinates to pose().
    pub fn activate_chase(&mut self, camera: ObjectId, target: ObjectId) -> Result<()> {
        let c = self.cameras.get(&camera).ok_or("Unknown chase camera")?;
        if c.kind != "eCAMTYPE_CHASETARGET" {
            return Err("Not a chase camera".into());
        }
        self.chase = Some((camera, target));
        self.active = Some(camera);
        Ok(())
    }
    pub fn set_chase_aim(&mut self, direction: [f64; 3]) -> Result<()> {
        let length = direction[0].hypot(direction[1]).hypot(direction[2]);
        if direction.iter().any(|v| !v.is_finite()) || !length.is_finite() || length < 1e-8 {
            return Err("Invalid chase aim".into());
        }
        self.chase_aim = direction.map(|v| v / length);
        Ok(())
    }
    pub fn retarget_chase(&mut self, target: ObjectId) {
        if let Some((camera, _)) = self.chase {
            self.chase = Some((camera, target));
        }
    }
    pub fn chase_target(&self) -> Option<ObjectId> {
        self.chase
            .filter(|(camera, _)| self.active == Some(*camera))
            .map(|(_, target)| target)
    }
    /// Native fourth argument is angular speed, not transition duration.
    pub fn set_fov(&mut self, id: ObjectId, target: f64, mode: &str, rate: f64) -> Result<f64> {
        let cam = self.cameras.get(&id).ok_or("Unknown camera")?.clone();
        let target = if target == 0.0 {
            self.bank.cameras.get(&id).map_or(cam.fov, |b| b.fov)
        } else {
            target
        };
        if !target.is_finite() || !(1.0..179.0).contains(&target) || !rate.is_finite() {
            return Err("Invalid FOV arguments".into());
        }
        let current = cam.fov;
        if self.active.is_none() && cam.kind == "eCAMTYPE_WAYPOINTCAM" && cam.waypoint.is_some() {
            if !self.paths.contains_key(&id)
                && let Some(wp) = cam.waypoint
                && let Ok(cursor) = Cursor::new(&self.bank, wp)
            {
                self.paths.insert(id, cursor);
            }
            if !self.targets.contains_key(&id)
                && let Some(t) = cam.target.filter(|t| self.bank.waypoints.contains_key(t))
                && let Ok(cursor) = Cursor::new(&self.bank, t)
            {
                self.targets.insert(id, cursor);
            }
            self.active = Some(id);
        }
        match mode {
            "IMMEDIATE" => {
                self.cameras.get_mut(&id).unwrap().fov = target;
                self.fov_motion.remove(&id);
                Ok(0.0)
            }
            "SMOOTH" if rate > 0.0 => {
                let duration = (target - current).abs() / rate;
                if !duration.is_finite() || duration > 3600.0 {
                    return Err("Unsupported FOV transition duration".into());
                }
                self.fov_motion.insert(id, (target, rate));
                Ok(duration)
            }
            _ => Err("Unsupported FOV mode/rate/inactive camera".into()),
        }
    }
    pub fn start_fade(&mut self, direction: &str, seconds: f64) -> Result<()> {
        if !matches!(direction, "FADE_IN" | "FADE_OUT")
            || !seconds.is_finite()
            || !(0.0..=3600.0).contains(&seconds)
        {
            return Err("Invalid camera fade".into());
        }
        if self.fade.is_some() && self.cameras.len() <= 1 {
            return Err("Overlapping camera fades unsupported".into());
        }
        let (from, to) = if direction == "FADE_IN" {
            (1.0, 0.0)
        } else {
            (0.0, 1.0)
        };
        self.black_alpha = if seconds == 0.0 { to } else { from };
        self.fade = (seconds > 0.0).then_some(Fade {
            from,
            to,
            elapsed: 0.0,
            duration: seconds,
        });
        Ok(())
    }
    pub fn call(
        &mut self,
        name: &str,
        args: &[Value],
        positions: &BTreeMap<ObjectId, [f32; 3]>,
    ) -> Result<Vec<Value>> {
        match (name, args) {
            ("SetCamera", [Value::Object(id), rest @ ..])
                if rest.is_empty()
                    || matches!(rest, [Value::Enum(player)] if player == "PLAYER_ONE" || player == "PLAYER_TWO")
                    || matches!(rest, [Value::Number(n)] if *n == 0.0 || *n == 1.0) =>
            {
                if matches!(rest, [Value::Enum(player)] if player == "PLAYER_TWO")
                    || matches!(rest, [Value::Number(n)] if *n == 1.0)
                {
                    return Ok(Vec::new());
                }
                let c = self.cameras.get(id).ok_or("Unknown camera")?;
                if !matches!(c.kind.as_str(), "eCAMTYPE_WAYPOINTCAM" | "eCAMTYPE_LOOKAT") {
                    return Err("Player/chase camera activation adapter not configured".into());
                }
                let path = c
                    .waypoint
                    .map(|wp| Cursor::new(&self.bank, wp))
                    .transpose()?;
                let target = c
                    .target
                    .filter(|t| self.bank.waypoints.contains_key(t))
                    .map(|t| Cursor::new(&self.bank, t))
                    .transpose()?;
                if let Some(p) = path {
                    self.paths.insert(*id, p);
                }
                if let Some(t) = target {
                    self.targets.insert(*id, t);
                }
                self.active = Some(*id);
            }
            ("CameraSetWaypoint", [Value::Object(id), Value::Object(wp)]) => {
                if !self.cameras.contains_key(id) {
                    return Err("Unknown camera".into());
                }
                let cursor = Cursor::new(&self.bank, *wp)?;
                let cam = self.cameras.get_mut(id).unwrap();
                cam.waypoint = Some(*wp);
                if self.active.is_none() && cam.kind == "eCAMTYPE_WAYPOINTCAM" {
                    self.active = Some(*id);
                }
                self.paths.insert(*id, cursor);
            }
            ("CameraSetTarget", [Value::Object(id), Value::Object(target)]) => {
                if !self.cameras.contains_key(id) {
                    return Err("Unknown camera".into());
                }
                let cursor = if self.bank.waypoints.contains_key(target) {
                    Some(Cursor::new(&self.bank, *target)?)
                } else {
                    let p = positions.get(target).ok_or("Unknown camera target")?;
                    if p.iter().any(|v| !v.is_finite()) {
                        return Err("Nonfinite camera target".into());
                    }
                    None
                };
                let cam = self.cameras.get_mut(id).unwrap();
                cam.target = Some(*target);
                if self.active.is_none() && cam.kind == "eCAMTYPE_WAYPOINTCAM" {
                    self.active = Some(*id);
                }
                self.targets.remove(id);
                if let Some(c) = cursor {
                    self.targets.insert(*id, c);
                }
            }
            _ => return Err(format!("Unsupported camera call/signature: {name}")),
        }
        Ok(Vec::new())
    }
    pub fn advance(&mut self, dt: f64) -> Result<()> {
        if !dt.is_finite() || !(0.0..=1.0).contains(&dt) {
            return Err("Invalid camera timestep".into());
        }
        let mut staged = self.clone();
        if let Some(f) = &mut staged.fade {
            f.elapsed = (f.elapsed + dt).min(f.duration);
            staged.black_alpha = f.from + (f.to - f.from) * f.elapsed / f.duration;
            if f.elapsed + 1e-9 >= f.duration {
                staged.black_alpha = f.to;
                staged.fade = None;
            }
        }
        if let Some(id) = staged.active {
            if let Some(&(target, rate)) = staged.fov_motion.get(&id) {
                let c = staged.cameras.get_mut(&id).unwrap();
                let delta = target - c.fov;
                if delta.abs() <= rate * dt {
                    c.fov = target;
                    staged.fov_motion.remove(&id);
                } else {
                    c.fov += delta.signum() * rate * dt;
                }
            }
            if let Some(c) = staged.paths.get_mut(&id) {
                c.advance(&staged.bank, dt)?
            }
            if let Some(c) = staged.targets.get_mut(&id) {
                c.advance(&staged.bank, dt)?
            }
        }
        *self = staged;
        Ok(())
    }
    pub fn pose(&self, positions: &BTreeMap<ObjectId, [f32; 3]>) -> Result<Option<Pose>> {
        if let Some(target) = self.chase_target() {
            let p = *positions
                .get(&target)
                .ok_or("Missing chase target position")?;
            if p.iter().any(|v| !v.is_finite()) {
                return Err("Invalid chase target position".into());
            }
            return Ok(Some(Pose {
                position: [
                    p[0] - self.chase_aim[0] as f32 * 14.0,
                    p[1] + 1.6 - self.chase_aim[1] as f32 * 14.0,
                    p[2] - self.chase_aim[2] as f32 * 14.0,
                ],
                look_at: [p[0], p[1] + 1.6, p[2]],
                fov_degrees: self.cameras[&self.active.unwrap()].fov,
            }));
        }
        let Some(id) = self.active else {
            return Ok(None);
        };
        let c = &self.cameras[&id];
        let position = self.paths.get(&id).map_or(c.position, |p| p.position);
        let look_at = if let Some(t) = self.targets.get(&id) {
            t.position
        } else {
            *positions
                .get(&c.target.ok_or("Camera has no target")?)
                .ok_or("Missing live camera target")?
        };
        if position.iter().chain(&look_at).any(|v| !v.is_finite())
            || position
                .iter()
                .zip(look_at)
                .map(|(a, b)| (*a - b).powi(2))
                .sum::<f32>()
                < 1e-8
        {
            return Err("Invalid/degenerate camera pose".into());
        }
        Ok(Some(Pose {
            position,
            look_at,
            fov_degrees: c.fov,
        }))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn rig() -> Rig {
        let mut b = Bank::default();
        for (id, position, next, pause) in [
            (1, [0.0, 0.0, 0.0], Some(ObjectId(2)), 0.5),
            (2, [10.0, 0.0, 0.0], None, 0.0),
            (3, [0.0, 1.0, 5.0], None, 0.0),
        ] {
            b.waypoints.insert(
                ObjectId(id),
                Waypoint {
                    position,
                    next,
                    speed: 10.0,
                    pause,
                    flags: 0,
                    branches: false,
                },
            );
        }
        b.cameras.insert(
            ObjectId(4),
            Camera {
                position: [0.0; 3],
                target: Some(ObjectId(3)),
                waypoint: Some(ObjectId(1)),
                fov: 45.0,
                kind: "eCAMTYPE_WAYPOINTCAM".into(),
            },
        );
        Rig::new(Arc::new(b))
    }
    #[test]
    fn authored_pause_travel_and_endpoint() {
        let mut r = rig();
        let p = BTreeMap::new();
        r.call(
            "SetCamera",
            &[Value::Object(ObjectId(4)), Value::Enum("PLAYER_ONE".into())],
            &p,
        )
        .unwrap();
        r.advance(0.5).unwrap();
        assert_eq!(r.pose(&p).unwrap().unwrap().position, [0.0; 3]);
        r.advance(0.5).unwrap();
        assert_eq!(r.pose(&p).unwrap().unwrap().position, [5.0, 0.0, 0.0]);
        r.advance(1.0).unwrap();
        assert_eq!(r.pose(&p).unwrap().unwrap().position, [10.0, 0.0, 0.0]);
    }
    #[test]
    fn fov_uses_angular_speed_not_seconds() {
        let mut r = rig();
        r.call(
            "SetCamera",
            &[Value::Object(ObjectId(4)), Value::Enum("PLAYER_ONE".into())],
            &BTreeMap::new(),
        )
        .unwrap();
        assert_eq!(r.set_fov(ObjectId(4), 70.0, "SMOOTH", 2.0).unwrap(), 12.5);
        r.advance(1.0).unwrap();
        assert_eq!(r.pose(&BTreeMap::new()).unwrap().unwrap().fov_degrees, 47.0);
        assert!(r.set_fov(ObjectId(4), 75.0, "SMOOTH", 0.0).is_err());
        r.set_fov(ObjectId(4), 60.0, "IMMEDIATE", 10.0).unwrap();
        r.advance(1.0).unwrap();
        assert_eq!(r.pose(&BTreeMap::new()).unwrap().unwrap().fov_degrees, 60.0);
    }
    #[test]
    fn fade_timing_and_invalid_overlap() {
        let mut r = rig();
        r.start_fade("FADE_IN", 1.0).unwrap();
        assert_eq!(r.black_alpha, 1.0);
        r.advance(0.25).unwrap();
        assert_eq!(r.black_alpha, 0.75);
        assert!(r.start_fade("FADE_OUT", 1.0).is_err());
        assert_eq!(r.black_alpha, 0.75);
        r.advance(0.75).unwrap();
        assert_eq!(r.black_alpha, 0.0);
        r.start_fade("FADE_OUT", 0.0).unwrap();
        assert_eq!(r.black_alpha, 1.0);
        assert!(r.start_fade("FADE_IN", f64::NAN).is_err());
        assert_eq!(r.black_alpha, 1.0);
        r.start_fade("FADE_OUT", 2.0).unwrap();
        for _ in 0..120 {
            r.advance(1.0 / 60.0).unwrap();
        }
        assert_eq!(r.black_alpha, 1.0);
        r.start_fade("FADE_IN", 1.0).unwrap();
    }
    #[test]
    fn invalid_request_preserves_active_state() {
        let mut r = rig();
        let p = BTreeMap::new();
        assert!(
            r.call(
                "SetCamera",
                &[
                    Value::Object(ObjectId(99)),
                    Value::Enum("PLAYER_ONE".into())
                ],
                &p
            )
            .is_err()
        );
        assert!(r.active.is_none());
        assert!(r.advance(f64::NAN).is_err());
        assert!(
            r.call(
                "CameraSetWaypoint",
                &[Value::Object(ObjectId(4)), Value::Object(ObjectId(99))],
                &p
            )
            .is_err()
        );
    }
    #[test]
    fn cyclic_zero_length_path_is_bounded_and_atomic() {
        let mut r = rig();
        let b = Arc::make_mut(&mut r.bank);
        b.waypoints.get_mut(&ObjectId(1)).unwrap().pause = 0.0;
        b.waypoints.get_mut(&ObjectId(2)).unwrap().position = [0.0; 3];
        b.waypoints.get_mut(&ObjectId(2)).unwrap().next = Some(ObjectId(1));
        let p = BTreeMap::new();
        r.call(
            "SetCamera",
            &[Value::Object(ObjectId(4)), Value::Enum("PLAYER_ONE".into())],
            &p,
        )
        .unwrap();
        let before = r.pose(&p).unwrap();
        assert!(r.advance(1.0).is_err());
        assert_eq!(r.pose(&p).unwrap(), before);
    }
}
