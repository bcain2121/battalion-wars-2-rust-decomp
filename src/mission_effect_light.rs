//! Recovered explosion effect-light channel, not complete explosion/Kill support.
//! Native updates are counted, not converted to seconds or renderer frames.
use crate::{Object, ObjectId, Result};

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Spec {
    pub source: ObjectId,
    radius: f32,
    colour: [u8; 4],
    duration_updates: u32,
}

impl Spec {
    pub fn new(
        source: ObjectId,
        radius: f32,
        colour: [u8; 4],
        duration_updates: u32,
    ) -> Result<Self> {
        if !radius.is_finite() || radius < 0.0 || !(radius * duration_updates as f32).is_finite() {
            return Err("Unsupported effect-light radius/product".into());
        }
        Ok(Self {
            source,
            radius,
            colour,
            duration_updates,
        })
    }

    /// Reads only this channel. Other explosion fields are deliberately not executed.
    pub fn from_explosion(object: &Object) -> Result<Self> {
        if object.kind != "sExplodeBase" {
            return Err("Effect-light source must be sExplodeBase".into());
        }
        let scalar = |name: &str| -> Result<&str> {
            let mut fields = object
                .fields
                .iter()
                .filter(|f| f.attributes.get("name").is_some_and(|s| s == name));
            let field = fields
                .next()
                .ok_or_else(|| format!("Missing effect-light field {name}"))?;
            if fields.next().is_some() || field.items.len() != 1 {
                return Err(format!("Invalid effect-light field {name}"));
            }
            Ok(field.items[0].as_str())
        };
        let radius = scalar("mEffectLightRadius")?
            .parse::<f32>()
            .map_err(|_| "Invalid effect-light radius")?;
        let duration = scalar("mEffectLightDuration")?;
        if duration.is_empty() || !duration.bytes().all(|b| b.is_ascii_digit()) {
            return Err("Invalid effect-light duration".into());
        }
        let duration = duration
            .parse::<u32>()
            .map_err(|_| "Effect-light duration overflow")?;
        let mut colour = [0; 4];
        let parts = scalar("mEffectLightColour")?.split(',').collect::<Vec<_>>();
        if parts.len() != 4 {
            return Err("Invalid effect-light colour length".into());
        }
        for (dst, text) in colour.iter_mut().zip(parts) {
            if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
                return Err("Invalid effect-light colour".into());
            }
            *dst = text
                .parse::<u8>()
                .map_err(|_| "Effect-light colour overflow")?;
        }
        Self::new(object.id, radius, colour, duration)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Light {
    spec: Spec,
    radius: f32,
    remaining: u32,
    enabled: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Snapshot {
    pub source: ObjectId,
    pub radius: f32,
    /// Source colour bytes; no inferred alpha normalization or shader conversion.
    pub colour: [u8; 4],
    pub remaining_updates: u32,
    pub enabled: bool,
    pub active: bool,
}

/// Result of the explosion's update-entry branch, BEFORE state/repeat/chain processing.
/// These flags belong to effect +0x1c, not destroyable object main flags.
#[must_use]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateEntry {
    RunState,
    RetainWithoutState,
    /// Caller must execute the concrete destructor; this does not perform destruction.
    DestroyRequired,
}

/// The assignment at 0x800bce08..10, not the preceding state-4 condition checks.
/// The rest of that current update still runs; the gate is checked on the next entry.
pub fn mark_native_deletion(effect_flags: &mut u32) {
    *effect_flags |= 0x0028_0000;
}

impl Light {
    /// 0x800bc420..0x800bc500. Call on transaction-staged state at the start of a
    /// native effect update, not on every rendered frame. Non-RunState results
    /// skip light decay, state handlers, repeat timers, chain timers and RNG draws.
    /// Destructor failure must roll back the enclosing effect transaction.
    pub fn begin_effect_update(&mut self, effect_flags: &mut u32) -> UpdateEntry {
        if *effect_flags & 0x0008_0000 != 0 {
            self.enabled = false;
            if *effect_flags & 0x0020_0000 != 0 {
                *effect_flags &= !0x0020_0000;
                return UpdateEntry::RetainWithoutState;
            }
            return UpdateEntry::DestroyRequired;
        }
        self.native_update();
        UpdateEntry::RunState
    }

    /// 0x800ba038..b0 -> 0x8030f864: source radius/colour, enabled byte = 1.
    pub fn new(spec: Spec) -> Self {
        Self {
            radius: spec.radius,
            remaining: spec.duration_updates,
            enabled: true,
            spec,
        }
    }

    /// 0x800bc49c..4fc. Only call when the containing native effect would update.
    /// No spatial registration, explosion chaining, clock selection or rendering is implied.
    pub fn native_update(&mut self) {
        if !self.enabled || self.radius == 0.0 {
            return;
        }
        if self.remaining == 0 {
            self.enabled = false;
        } else {
            // Preserve fmuls then fdivs, not radius * (remaining / initial).
            self.radius =
                (self.spec.radius * self.remaining as f32) / self.spec.duration_updates as f32;
            self.remaining -= 1;
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            source: self.spec.source,
            radius: self.radius,
            colour: self.spec.colour,
            remaining_updates: self.remaining,
            enabled: self.enabled,
            active: self.enabled && self.radius != 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queued_deletion_retains_once_then_requires_destructor_without_decay() {
        let mut light = Light::new(Spec::new(ObjectId(1), 80.0, [255, 77, 0, 129], 10).unwrap());
        let mut flags = 0x0100_0001;
        assert_eq!(light.begin_effect_update(&mut flags), UpdateEntry::RunState);
        let before = light.snapshot();
        mark_native_deletion(&mut flags);
        assert!(light.snapshot().enabled); // Marking does not immediately disable it.
        assert_eq!(
            light.begin_effect_update(&mut flags),
            UpdateEntry::RetainWithoutState
        );
        assert_eq!(flags, 0x0108_0001);
        let held = light.snapshot();
        assert!(!held.enabled);
        assert_eq!(held.radius, before.radius);
        assert_eq!(held.remaining_updates, before.remaining_updates);
        assert_eq!(
            light.begin_effect_update(&mut flags),
            UpdateEntry::DestroyRequired
        );
        assert_eq!(light.snapshot(), held);
        assert_eq!(flags, 0x0108_0001);
    }
    #[test]
    fn hold_bit_alone_does_not_skip_update_and_direct_delete_has_no_hold() {
        let mut light = Light::new(Spec::new(ObjectId(1), 80.0, [0; 4], 10).unwrap());
        let mut flags = 0x0020_0000;
        assert_eq!(light.begin_effect_update(&mut flags), UpdateEntry::RunState);
        assert_eq!(light.snapshot().remaining_updates, 9);
        assert_eq!(flags, 0x0020_0000);
        flags = 0x0008_0000;
        assert_eq!(
            light.begin_effect_update(&mut flags),
            UpdateEntry::DestroyRequired
        );
        assert_eq!(light.snapshot().remaining_updates, 9);
        assert!(!light.snapshot().active);
    }
    #[test]
    fn decay_preserves_first_update_and_deactivates_one_update_after_counter_zero() {
        let mut light = Light::new(Spec::new(ObjectId(7), 80.0, [255, 77, 0, 129], 10).unwrap());
        for n in (1..=10).rev() {
            light.native_update();
            let s = light.snapshot();
            assert_eq!(s.radius, 8.0 * n as f32);
            assert_eq!(s.remaining_updates, n - 1);
            assert!(s.active);
            assert_eq!(s.colour, [255, 77, 0, 129]);
        }
        light.native_update();
        assert!(!light.snapshot().active);
        assert_eq!(light.snapshot().radius, 8.0); // disable is not a radius write
        let terminal = light.clone();
        light.native_update();
        assert_eq!(light, terminal);
    }

    #[test]
    fn zero_duration_and_zero_radius_follow_distinct_native_branches() {
        let mut zero_time = Light::new(Spec::new(ObjectId(1), 80.0, [0; 4], 0).unwrap());
        assert!(zero_time.snapshot().active);
        zero_time.native_update();
        assert!(!zero_time.snapshot().enabled);
        let mut zero_radius = Light::new(Spec::new(ObjectId(1), 0.0, [0; 4], 10).unwrap());
        let initial = zero_radius.clone();
        zero_radius.native_update();
        assert_eq!(zero_radius, initial);
        assert!(!zero_radius.snapshot().active);
        assert!(zero_radius.snapshot().enabled);
    }

    #[test]
    fn unsupported_numeric_values_are_rejected() {
        for radius in [f32::NAN, f32::INFINITY, -1.0, f32::MAX] {
            assert!(Spec::new(ObjectId(1), radius, [0; 4], 10).is_err());
        }
        assert!(Spec::new(ObjectId(1), 1.0, [0; 4], u32::MAX).is_ok());
    }

    #[test]
    fn cloned_light_updates_do_not_mutate_the_original() {
        let original = Light::new(Spec::new(ObjectId(1), 80.0, [0; 4], 10).unwrap());
        let mut staged = original.clone();
        staged.native_update();
        assert_eq!(original.snapshot().remaining_updates, 10);
        assert_eq!(staged.snapshot().remaining_updates, 9);
    }
}
