//! Authored troop-to-animation bindings plus a replacement playback controller.
//! This does not upgrade the experimental BAN rotation/translation decoder.
use crate::{Object, ObjectGraph, ObjectId, Result, animation::Animation, original_scene::field};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Motion {
    Idle,
    Walk,
    Run,
    Fire,
    Death,
}
#[derive(Debug, Clone)]
pub struct Player {
    pub bindings: BTreeMap<Motion, String>,
    pub motion: Motion,
    pub elapsed: f64,
}
impl Player {
    pub fn from_base(base: &Object, graph: &ObjectGraph) -> Result<Self> {
        let id = field(base, "mAnimationSet")
            .ok_or("Missing troop animation set")?
            .parse::<u64>()
            .map_err(|_| "Invalid animation-set reference")?;
        let set = graph
            .objects
            .get(&ObjectId(id))
            .ok_or("Unresolved animation set")?;
        let mut bindings = BTreeMap::new();
        for (motion, key) in [
            (Motion::Idle, "mBreathAnimation"),
            (Motion::Walk, "mWalkAnimation"),
            (Motion::Run, "mRunAnimation"),
            (Motion::Fire, "mFullForwardFireAnimation"),
            (Motion::Death, "mDeath1Animation"),
        ] {
            let Some(raw) = field(set, key) else { continue };
            let id = raw
                .parse::<u64>()
                .map_err(|_| "Invalid animation reference")?;
            if id == 0 {
                continue;
            }
            let resource = graph
                .objects
                .get(&ObjectId(id))
                .ok_or("Unresolved animation resource")?;
            bindings.insert(
                motion,
                field(resource, "mName")
                    .ok_or("Unnamed animation resource")?
                    .to_string(),
            );
        }
        if !bindings.contains_key(&Motion::Idle) {
            return Err("No authored idle animation".into());
        }
        Ok(Self {
            bindings,
            motion: Motion::Idle,
            elapsed: 0.0,
        })
    }
    pub fn select(&mut self, motion: Motion) -> Result<()> {
        if !self.bindings.contains_key(&motion) {
            return Err(format!("Missing authored animation for {motion:?}"));
        }
        if self.motion != motion {
            self.motion = motion;
            self.elapsed = 0.0;
        }
        Ok(())
    }
    pub fn advance(&mut self, dt: f64) -> Result<()> {
        if !dt.is_finite() || dt < 0.0 || !(self.elapsed + dt).is_finite() {
            return Err("Invalid animation timestep".into());
        }
        self.elapsed += dt;
        Ok(())
    }
    pub fn sample_time(&self, clip: &Animation) -> Result<f32> {
        if self.bindings.get(&self.motion) != Some(&clip.name) {
            return Err("Animation resource does not match unit state".into());
        }
        if !clip.duration.is_finite()
            || clip.duration < 0.0
            || !self.elapsed.is_finite()
            || self.elapsed < 0.0
        {
            return Err("Invalid playback clock/duration".into());
        }
        let duration = clip.duration as f64;
        Ok(if duration == 0.0 {
            0.0
        } else if matches!(self.motion, Motion::Fire | Motion::Death) {
            self.elapsed.min(duration) as f32
        } else {
            self.elapsed.rem_euclid(duration) as f32
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn loop_once_and_transition() {
        let mut p = Player {
            bindings: [
                (Motion::Idle, "idle".into()),
                (Motion::Death, "death".into()),
            ]
            .into(),
            motion: Motion::Idle,
            elapsed: 0.0,
        };
        let mut clip = Animation {
            name: "idle".into(),
            duration: 2.0,
            flags: 0,
            tracks: vec![],
            trailing_data: vec![],
        };
        p.advance(5.0).unwrap();
        assert_eq!(p.sample_time(&clip).unwrap(), 1.0);
        p.select(Motion::Idle).unwrap();
        assert_eq!(p.elapsed, 5.0);
        p.select(Motion::Death).unwrap();
        assert_eq!(p.elapsed, 0.0);
        clip.name = "death".into();
        p.advance(5.0).unwrap();
        assert_eq!(p.sample_time(&clip).unwrap(), 2.0);
        assert!(p.select(Motion::Run).is_err());
        assert_eq!(p.motion, Motion::Death);
        assert!(p.advance(f64::NAN).is_err());
    }
}
