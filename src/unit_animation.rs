//! Authored troop-to-animation bindings plus a replacement playback controller.
//! This does not upgrade the experimental BAN rotation/translation decoder.
use crate::{Object, ObjectGraph, ObjectId, Result, animation::Animation, original_scene::field};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Motion {
    Idle,
    AlertIdle,
    ContextIdle,
    Walk,
    Run,
    RunStop,
    WalkBack,
    RunBack,
    StrafeLeft,
    StrafeRight,
    StrafeLeftRun,
    StrafeRightRun,
    TurnLeft,
    TurnRight,
    RollForward,
    RollBack,
    RollLeft,
    RollRight,
    JumpRise,
    JumpPose,
    JumpLand,
    Fire,
    ForwardFire,
    Reload,
    FallFlail,
    HitGround,
    Getup,
    Swim,
    SwimBack,
    TreadWater,
    WaterDeath,
    SteerFront,
    SteerLeft,
    SteerRight,
    DriveIdle,
    GunnerFire,
    Death,
    Death2,
    Death3,
    Death4,
    Death5,
    Victory,
}
impl Motion {
    pub fn is_one_shot(self) -> bool {
        matches!(
            self,
            Self::Fire
                | Self::ForwardFire
                | Self::Reload
                | Self::RunStop
                | Self::RollForward
                | Self::RollBack
                | Self::RollLeft
                | Self::RollRight
                | Self::JumpRise
                | Self::JumpLand
                | Self::HitGround
                | Self::Getup
                | Self::WaterDeath
                | Self::Death
                | Self::Death2
                | Self::Death3
                | Self::Death4
                | Self::Death5
        )
    }
    pub fn is_death(self) -> bool {
        matches!(
            self,
            Self::Death
                | Self::Death2
                | Self::Death3
                | Self::Death4
                | Self::Death5
                | Self::WaterDeath
        )
    }
    pub fn is_locomotion(self) -> bool {
        matches!(
            self,
            Self::Walk
                | Self::Run
                | Self::WalkBack
                | Self::RunBack
                | Self::StrafeLeft
                | Self::StrafeRight
                | Self::StrafeLeftRun
                | Self::StrafeRightRun
                | Self::Swim
                | Self::SwimBack
        )
    }
    pub fn fallbacks(self) -> &'static [Self] {
        match self {
            Self::Idle => &[Self::Idle],
            Self::AlertIdle => &[Self::AlertIdle, Self::Idle],
            Self::ContextIdle => &[Self::ContextIdle, Self::AlertIdle, Self::Idle],
            Self::Walk => &[Self::Walk, Self::Run, Self::Idle],
            Self::Run => &[Self::Run, Self::Walk, Self::Idle],
            Self::RunStop => &[Self::RunStop, Self::AlertIdle, Self::Idle],
            Self::WalkBack => &[Self::WalkBack, Self::RunBack, Self::Walk, Self::Idle],
            Self::RunBack => &[Self::RunBack, Self::WalkBack, Self::Run, Self::Walk],
            Self::StrafeLeft => &[
                Self::StrafeLeft,
                Self::StrafeLeftRun,
                Self::Walk,
                Self::Run,
                Self::Idle,
            ],
            Self::StrafeRight => &[
                Self::StrafeRight,
                Self::StrafeRightRun,
                Self::Walk,
                Self::Run,
                Self::Idle,
            ],
            Self::StrafeLeftRun => &[
                Self::StrafeLeftRun,
                Self::StrafeLeft,
                Self::Run,
                Self::Walk,
                Self::Idle,
            ],
            Self::StrafeRightRun => &[
                Self::StrafeRightRun,
                Self::StrafeRight,
                Self::Run,
                Self::Walk,
                Self::Idle,
            ],
            Self::TurnLeft => &[Self::TurnLeft, Self::Walk, Self::AlertIdle, Self::Idle],
            Self::TurnRight => &[Self::TurnRight, Self::Walk, Self::AlertIdle, Self::Idle],
            Self::RollForward => &[Self::RollForward, Self::Run, Self::Walk],
            Self::RollBack => &[Self::RollBack, Self::RollForward, Self::RunBack, Self::Walk],
            Self::RollLeft => &[
                Self::RollLeft,
                Self::StrafeLeftRun,
                Self::RollForward,
                Self::Run,
            ],
            Self::RollRight => &[
                Self::RollRight,
                Self::StrafeRightRun,
                Self::RollForward,
                Self::Run,
            ],
            Self::JumpRise => &[Self::JumpRise, Self::JumpPose, Self::Run, Self::Idle],
            Self::JumpPose => &[Self::JumpPose, Self::JumpRise, Self::Run, Self::Idle],
            Self::JumpLand => &[Self::JumpLand, Self::RunStop, Self::AlertIdle, Self::Idle],
            Self::Fire => &[Self::Fire, Self::ForwardFire, Self::AlertIdle, Self::Idle],
            Self::ForwardFire => &[Self::ForwardFire, Self::Fire, Self::Run, Self::Walk],
            Self::Reload => &[Self::Reload, Self::AlertIdle, Self::Idle],
            Self::FallFlail => &[Self::FallFlail, Self::JumpPose, Self::HitGround, Self::Idle],
            Self::HitGround => &[Self::HitGround, Self::Death, Self::Idle],
            Self::Getup => &[Self::Getup, Self::AlertIdle, Self::Idle],
            Self::Swim => &[Self::Swim, Self::TreadWater, Self::Walk, Self::Idle],
            Self::SwimBack => &[Self::SwimBack, Self::Swim, Self::TreadWater, Self::WalkBack],
            Self::TreadWater => &[Self::TreadWater, Self::Swim, Self::Idle],
            Self::WaterDeath => &[Self::WaterDeath, Self::Death],
            Self::SteerFront => &[Self::SteerFront, Self::DriveIdle, Self::Idle],
            Self::SteerLeft => &[
                Self::SteerLeft,
                Self::SteerFront,
                Self::DriveIdle,
                Self::Idle,
            ],
            Self::SteerRight => &[
                Self::SteerRight,
                Self::SteerFront,
                Self::DriveIdle,
                Self::Idle,
            ],
            Self::DriveIdle => &[Self::DriveIdle, Self::SteerFront, Self::Idle],
            Self::GunnerFire => &[Self::GunnerFire, Self::Fire, Self::DriveIdle, Self::Idle],
            Self::Death => &[Self::Death],
            Self::Death2 => &[Self::Death2, Self::Death],
            Self::Death3 => &[Self::Death3, Self::Death2, Self::Death],
            Self::Death4 => &[Self::Death4, Self::Death2, Self::Death],
            Self::Death5 => &[Self::Death5, Self::Death2, Self::Death],
            Self::Victory => &[Self::Victory, Self::AlertIdle, Self::Idle],
        }
    }
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
        let candidates: &[(Motion, &[&str])] = &[
            (Motion::Idle, &["mBreathAnimation"]),
            (Motion::AlertIdle, &["mAlertBreathAnimation"]),
            (
                Motion::ContextIdle,
                &[
                    "mScratchAnimation",
                    "mAdjustHelmetAnimation",
                    "mAlertLook1Animation",
                    "mRelaxedLook1Animation",
                ],
            ),
            (Motion::Walk, &["mWalkAnimation"]),
            (Motion::Run, &["mRunAnimation", "mChargeAnimation"]),
            (Motion::RunStop, &["mRunStopAnimation"]),
            (
                Motion::WalkBack,
                &["mWalkBackAnimation", "mWalkHopBackAnimation"],
            ),
            (
                Motion::RunBack,
                &["mRunBackAnimation", "mRunHopBackAnimation"],
            ),
            (Motion::StrafeLeft, &["mStrafeLeftAnimation"]),
            (Motion::StrafeRight, &["mStrafeRightAnimation"]),
            (Motion::StrafeLeftRun, &["mStrafeLeftRunAnimation"]),
            (Motion::StrafeRightRun, &["mStrafeRightRunAnimation"]),
            (
                Motion::TurnLeft,
                &["mTurnLeftAnimation", "mLeftCrouchTurnAnimation"],
            ),
            (
                Motion::TurnRight,
                &["mTurnRightAnimation", "mRightCrouchTurnAnimation"],
            ),
            (Motion::RollForward, &["mForwardRollAnimation"]),
            (Motion::RollBack, &["mBackwardRollAnimation"]),
            (
                Motion::RollLeft,
                &["mCombatRollLeftAnimation", "mCombatDiveLeftAnimation"],
            ),
            (
                Motion::RollRight,
                &["mCombatRollRightAnimation", "mCombatDiveRightAnimation"],
            ),
            (
                Motion::JumpRise,
                &["mJumpRiseAnimation", "mClamberAnimation"],
            ),
            (
                Motion::JumpPose,
                &["mJumpStaticAnimation", "mParachuteAnimation"],
            ),
            (
                Motion::JumpLand,
                &["mJumpDropAnimation", "mJumpRunLandAnimation"],
            ),
            (
                Motion::Fire,
                &["mFullForwardFireAnimation", "mWeaponFireAnimation"],
            ),
            (
                Motion::ForwardFire,
                &["mForwardFireAnimation", "mFullForwardFireAnimation"],
            ),
            (Motion::Reload, &["mReloadAnimation"]),
            (
                Motion::FallFlail,
                &["mFallFlailAnimation", "mBlownUpAnimation"],
            ),
            (Motion::HitGround, &["mHitGroundAnimation"]),
            (
                Motion::Getup,
                &["mGetupAnimation", "mCrouch2StandAnimation"],
            ),
            (Motion::Swim, &["mSwimAnimation"]),
            (Motion::SwimBack, &["mSwimBackAnimation"]),
            (Motion::TreadWater, &["mTreadWaterAnimation"]),
            (Motion::WaterDeath, &["mWaterDeathAnimation"]),
            (Motion::SteerFront, &["mSteerFrontAnimation"]),
            (Motion::SteerLeft, &["mSteerLeftAnimation"]),
            (Motion::SteerRight, &["mSteerRightAnimation"]),
            (
                Motion::DriveIdle,
                &[
                    "mDriveBreathAnimation",
                    "mDriveStaticIdleAnimation",
                    "mDriveStaticBreathAnimation",
                ],
            ),
            (Motion::GunnerFire, &["mFireStraightAnimation"]),
            (Motion::Death, &["mDeath1Animation"]),
            (Motion::Death2, &["mDeath2Animation"]),
            (Motion::Death3, &["mDeath3Animation"]),
            (Motion::Death4, &["mDeath4Animation"]),
            (Motion::Death5, &["mDeath5Animation"]),
            (
                Motion::Victory,
                &[
                    "mVictory1Animation",
                    "mSaluteAnimation",
                    "mVictory2Animation",
                    "mVictory3Animation",
                ],
            ),
        ];
        for &(motion, keys) in candidates {
            for &key in keys {
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
                let name = field(resource, "mName")
                    .ok_or("Unnamed animation resource")?
                    .to_string();
                if !name.is_empty() {
                    bindings.insert(motion, name);
                    break;
                }
            }
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
    pub fn select_with_fallback(&mut self, preferred: Motion) -> Motion {
        for &candidate in preferred.fallbacks() {
            if self.bindings.contains_key(&candidate) {
                if self.motion != candidate {
                    let preserve_phase = self.motion.is_locomotion() && candidate.is_locomotion();
                    self.motion = candidate;
                    if !preserve_phase {
                        self.elapsed = 0.0;
                    }
                }
                return candidate;
            }
        }
        self.motion
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
            || !self.elapsed.is_sign_positive() && self.elapsed != 0.0
        {
            return Err("Invalid playback clock/duration".into());
        }
        let duration = clip.duration as f64;
        Ok(if duration == 0.0 {
            0.0
        } else if self.motion.is_one_shot() {
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
                (Motion::Walk, "walk".into()),
                (Motion::Run, "run".into()),
                (Motion::RollForward, "roll".into()),
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
        assert!(p.select(Motion::WaterDeath).is_err());
        assert_eq!(p.motion, Motion::Death);
        assert_eq!(p.select_with_fallback(Motion::StrafeLeft), Motion::Walk);
        p.advance(0.75).unwrap();
        assert_eq!(p.select_with_fallback(Motion::Run), Motion::Run);
        assert!(
            (p.elapsed - 0.75).abs() < 1e-9,
            "locomotion-to-locomotion transition must preserve animation phase"
        );
        assert_eq!(
            p.select_with_fallback(Motion::RollLeft),
            Motion::RollForward
        );
        assert_eq!(p.elapsed, 0.0);
        assert!(p.advance(f64::NAN).is_err());
    }
}
