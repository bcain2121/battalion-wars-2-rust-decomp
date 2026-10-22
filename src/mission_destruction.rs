//! Destroyable Kill control flow recovered from 0x8009fb70.
//! Concrete model/effect/attachment backends are required; this does not install a Kill stub.
use crate::{Object, ObjectId, Result};
use std::{
    collections::{BTreeSet, VecDeque},
    num::NonZeroU64,
};

pub const DEAD: u32 = 0x0010_0000;
pub const DEFERRED_DELETE: u32 = 0x0008_0000;

/// Runtime handle identity, not an authored resource ID or a native memory address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Handle(pub NonZeroU64);

#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    pub flags: u32,
    pub explosion: Option<ObjectId>,
}
impl Profile {
    /// Only the two fields controlling this Kill branch. Does not infer live occupancy.
    pub fn from_object(object: &Object) -> Result<Self> {
        if object.kind != "sDestroyBase" {
            return Err("Kill profile must be sDestroyBase".into());
        }
        let scalar = |name: &str| -> Result<&str> {
            let mut fields = object
                .fields
                .iter()
                .filter(|f| f.attributes.get("name").is_some_and(|n| n == name));
            let field = fields
                .next()
                .ok_or_else(|| format!("Missing destruction field {name}"))?;
            if fields.next().is_some() || field.items.len() != 1 {
                return Err(format!("Invalid destruction field {name}"));
            }
            let s = field.items[0].as_str();
            if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
                return Err(format!("Invalid destruction number {name}"));
            }
            Ok(s)
        };
        let flags = scalar("ObjFlags")?
            .parse()
            .map_err(|_| "Destruction flags overflow")?;
        let id = scalar("ExplodeType")?
            .parse::<u64>()
            .map_err(|_| "Explosion reference overflow")?;
        Ok(Self {
            flags,
            explosion: (id != 0).then_some(ObjectId(id)),
        })
    }
}

/// Supported concrete vtable: cDestroyableObject. Derived classes must be verified separately.
#[derive(Debug, Clone, PartialEq)]
pub struct Instance {
    pub object: ObjectId,
    /// Live allocation/generation identity; authored object IDs are not pointer identity.
    pub identity: Handle,
    pub flags: u32,
    /// Native byte +0x130, NOT a bool: constructor writes 4; Kill writes 1.
    pub lifecycle_state: u8,
    /// None means a known null native base, NOT unknown/unloaded profile data.
    pub profile: Option<Profile>,
    pub network_id: Option<u16>,
    /// Whether native +0x7c is non-null; not an inferred presence flag.
    pub spatial_data_present: bool,
    pub auxiliary_handle: Option<Handle>,
    /// Native +0xa0 byte; not the object's main flags or render visibility.
    pub component_flags: u8,
    /// Native list head at +0xac; each record has next at +0xc.
    pub pending_records: VecDeque<Handle>,
    /// Runtime prefix counted by +0x208. Null slots and source order are retained.
    pub attachments: Vec<Option<Handle>>,
}
#[derive(Debug, Clone, Copy)]
pub struct Context {
    pub notify: bool,
    pub networking_active: bool,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub object: ObjectId,
    pub identity: Handle,
    pub flags: u32,
    pub lifecycle_state: u8,
    pub component_flags: u8,
}
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    Notify {
        network_id: u16,
    },
    SpatialRemoval,
    /// Record virtual +8 with native r4=1; concrete record destructor is required.
    ReleaseRecord {
        handle: Handle,
    },
    /// Source transform/owner and native zero-valued spawn argument must be resolved by backend.
    Explosion {
        profile: ObjectId,
    },
    /// Native 0x801f9754 performs its stop/release calls and nulls this handle.
    ReleaseAuxiliary {
        handle: Handle,
    },
    /// Attachment virtual +8, only when its owner equals this destroyable.
    ReleaseAttachment {
        handle: Handle,
    },
}

/// Verified owner-local writes performed by a concrete effect backend.
/// This is reported by the backend, not inferred from an explosion profile's presence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectOutcome {
    Unchanged,
    /// Explosion setup may call the owner's component virtual +0x4c (0x8000f1d8).
    /// That callback ORs bit 0 into the owner's component byte before DEAD is set.
    ActivateOwnerComponent,
}

/// Implementations must be transaction-local, deeply clone their mutable state, and perform
/// no external I/O. Unsupported effects must return Err, not silently record success.
/// The View exposes the native ordering: explosion precedes DEAD; tail cleanup follows it.
pub trait Backend: Clone {
    fn apply(&mut self, owner: View, effect: Effect) -> Result<EffectOutcome>;
    fn attachment_owner(&self, handle: Handle) -> Result<Option<Handle>>;
}
impl Instance {
    fn view(&self) -> View {
        View {
            object: self.object,
            identity: self.identity,
            flags: self.flags,
            lifecycle_state: self.lifecycle_state,
            component_flags: self.component_flags,
        }
    }
    pub fn kill(&mut self, backend: &mut impl Backend, context: Context) -> Result<()> {
        if self.attachments.len() > 4 {
            return Err("Destroyable attachment capacity exceeded".into());
        }
        // Bounded supported state, not a claimed native list-size limit.
        if self.pending_records.len() > 4096
            || self.pending_records.iter().collect::<BTreeSet<_>>().len()
                != self.pending_records.len()
        {
            return Err("Invalid/oversized destroyable record list".into());
        }
        let mut staged = self.clone();
        let mut effects = backend.clone();
        staged.kill_inner(&mut effects, context)?;
        *self = staged;
        *backend = effects;
        Ok(())
    }
    fn apply_effect(&mut self, backend: &mut impl Backend, effect: Effect) -> Result<()> {
        let is_explosion = matches!(&effect, Effect::Explosion { .. });
        match backend.apply(self.view(), effect)? {
            EffectOutcome::Unchanged => Ok(()),
            EffectOutcome::ActivateOwnerComponent if is_explosion => {
                self.component_flags |= 1;
                Ok(())
            }
            EffectOutcome::ActivateOwnerComponent => {
                Err("Owner component callback unsupported for this destruction effect".into())
            }
        }
    }
    fn kill_inner(&mut self, backend: &mut impl Backend, context: Context) -> Result<()> {
        if self.flags & DEAD != 0 {
            return Ok(());
        }
        self.lifecycle_state = 1;
        if context.networking_active && context.notify {
            let network_id = self
                .network_id
                .ok_or("Destroyable network identity not configured")?;
            self.apply_effect(backend, Effect::Notify { network_id })?;
        }
        // The native null-base path returns without marking DEAD or releasing attachments.
        let Some(profile) = self.profile.clone() else {
            return Ok(());
        };
        if self.spatial_data_present {
            self.apply_effect(backend, Effect::SpatialRemoval)?;
        }
        // +0x90 resolves through 0x8000e154 to 0x8004b708 for this concrete class.
        while let Some(handle) = self.pending_records.front().copied() {
            self.apply_effect(backend, Effect::ReleaseRecord { handle })?;
            self.pending_records.pop_front();
        }
        if profile.flags & 2 != 0 {
            if let Some(profile) = profile.explosion {
                self.apply_effect(backend, Effect::Explosion { profile })?;
            }
        } else if profile.flags & 16 != 0 {
            self.flags |= DEFERRED_DELETE;
        } else {
            // +0x4c resolves to 0x8000f1d8: OR bit 0 into component byte +0x18.
            self.component_flags |= 1;
        }
        self.flags |= DEAD;
        if let Some(handle) = self.auxiliary_handle {
            self.apply_effect(backend, Effect::ReleaseAuxiliary { handle })?;
            self.auxiliary_handle = None;
        }
        for index in 0..self.attachments.len() {
            if let Some(handle) = self.attachments[index] {
                if backend.attachment_owner(handle)? == Some(self.identity) {
                    self.apply_effect(backend, Effect::ReleaseAttachment { handle })?;
                }
                // Foreign/null owners do not get a release callback, but the local link clears.
                self.attachments[index] = None;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    #[derive(Clone, Debug, Default, PartialEq)]
    struct Recorder {
        calls: Vec<(View, Effect)>,
        owners: BTreeMap<Handle, Option<Handle>>,
        fail_at: Option<usize>,
        activate_on_explosion: bool,
        unsupported_activation: bool,
    }
    impl Backend for Recorder {
        fn apply(&mut self, view: View, effect: Effect) -> Result<EffectOutcome> {
            if self.fail_at == Some(self.calls.len()) {
                return Err("Test backend failure".into());
            }
            let activate = self.unsupported_activation
                || (self.activate_on_explosion && matches!(&effect, Effect::Explosion { .. }));
            self.calls.push((view, effect));
            Ok(if activate {
                EffectOutcome::ActivateOwnerComponent
            } else {
                EffectOutcome::Unchanged
            })
        }
        fn attachment_owner(&self, h: Handle) -> Result<Option<Handle>> {
            self.owners
                .get(&h)
                .copied()
                .ok_or("Unknown attachment".into())
        }
    }
    fn handle(n: u64) -> Handle {
        Handle(NonZeroU64::new(n).unwrap())
    }
    fn object(flags: u32) -> Instance {
        Instance {
            object: ObjectId(1),
            identity: handle(100),
            flags: 4,
            lifecycle_state: 4,
            profile: Some(Profile {
                flags,
                explosion: Some(ObjectId(9)),
            }),
            network_id: Some(7),
            spatial_data_present: true,
            auxiliary_handle: Some(handle(5)),
            component_flags: 0x80,
            pending_records: [handle(10), handle(11)].into(),
            attachments: vec![Some(handle(1)), None, Some(handle(2)), Some(handle(3))],
        }
    }
    fn backend() -> Recorder {
        Recorder {
            owners: [
                (handle(1), Some(handle(100))),
                (handle(2), Some(handle(200))),
                (handle(3), None),
            ]
            .into(),
            ..Default::default()
        }
    }
    const OFFLINE: Context = Context {
        notify: true,
        networking_active: false,
    };
    #[test]
    fn explosion_precedes_dead_and_only_owned_attachment_released() {
        let mut state = object(8194);
        let mut b = backend();
        state.kill(&mut b, OFFLINE).unwrap();
        assert_eq!(
            b.calls.iter().map(|(_, e)| e.clone()).collect::<Vec<_>>(),
            vec![
                Effect::SpatialRemoval,
                Effect::ReleaseRecord { handle: handle(10) },
                Effect::ReleaseRecord { handle: handle(11) },
                Effect::Explosion {
                    profile: ObjectId(9)
                },
                Effect::ReleaseAuxiliary { handle: handle(5) },
                Effect::ReleaseAttachment { handle: handle(1) }
            ]
        );
        assert!(
            b.calls[..4]
                .iter()
                .all(|(v, _)| v.lifecycle_state == 1 && v.flags & DEAD == 0)
        );
        assert!(b.calls[4..].iter().all(|(v, _)| v.flags & DEAD != 0));
        assert_eq!(state.flags, 4 | DEAD);
        assert_eq!(state.attachments, vec![None; 4]);
        assert_eq!(state.auxiliary_handle, None);
        let before = b.clone();
        state.kill(&mut b, OFFLINE).unwrap();
        assert_eq!(b, before);
    }
    #[test]
    fn explosion_callback_updates_owner_before_tail_cleanup() {
        let mut state = object(8194);
        state.lifecycle_state = 5;
        let mut b = backend();
        b.activate_on_explosion = true;
        state.kill(&mut b, OFFLINE).unwrap();
        assert_eq!(state.component_flags, 0x81);
        assert_eq!(b.calls[3].0.component_flags, 0x80);
        assert!(
            b.calls[4..]
                .iter()
                .all(|(v, _)| { v.component_flags == 0x81 && v.flags & DEAD != 0 })
        );
    }
    #[test]
    fn observed_art_owner_bytes_and_callback_rollback() {
        // Scalar observations from the first Art Kill. Mock backend only: this is
        // not a replay of native spatial queries, effect creation, or a mission tick.
        let mut state = object(8194);
        state.flags = 0x0200_0000;
        state.lifecycle_state = 5;
        state.component_flags = 0;
        state.pending_records.clear();
        state.attachments.clear();
        state.auxiliary_handle = None;
        let mut b = backend();
        b.activate_on_explosion = true;
        state.kill(&mut b, OFFLINE).unwrap();
        assert_eq!(state.flags, 0x0210_0000);
        assert_eq!(state.lifecycle_state, 1);
        assert_eq!(state.component_flags, 1);

        let mut state = object(8194);
        let mut b = backend();
        b.activate_on_explosion = true;
        b.fail_at = Some(4); // Auxiliary release, after the explosion callback.
        let before = (state.clone(), b.clone());
        assert!(state.kill(&mut b, OFFLINE).is_err());
        assert_eq!((state, b), before);
    }
    #[test]
    fn unexpected_owner_callback_rolls_back() {
        let mut state = object(8194);
        let mut b = backend();
        b.unsupported_activation = true;
        let before = (state.clone(), b.clone());
        assert!(state.kill(&mut b, OFFLINE).is_err());
        assert_eq!((state, b), before);
    }
    #[test]
    fn every_effect_failure_and_unknown_attachment_roll_back() {
        for flags in [0, 2, 16, 18] {
            for networking_active in [false, true] {
                let context = Context {
                    notify: true,
                    networking_active,
                };
                let mut reference = object(flags);
                let mut successful = backend();
                reference.kill(&mut successful, context).unwrap();
                for failure in 0..successful.calls.len() {
                    let mut s = object(flags);
                    let mut b = backend();
                    b.fail_at = Some(failure);
                    let before = (s.clone(), b.clone());
                    assert!(s.kill(&mut b, context).is_err());
                    assert_eq!((s, b), before);
                }
            }
        }
        let mut s = object(2);
        let mut b = Recorder::default();
        let before = (s.clone(), b.clone());
        assert!(s.kill(&mut b, OFFLINE).is_err());
        assert_eq!((s, b), before);
    }
    #[test]
    fn branch_precedence_null_explosion_and_missing_base() {
        for (flags, deferred, flagged, explosion) in [
            (0, false, true, false),
            (16, true, false, false),
            (18, false, false, true),
        ] {
            let mut s = object(flags);
            let mut b = backend();
            s.kill(&mut b, OFFLINE).unwrap();
            assert_eq!(s.flags & DEFERRED_DELETE != 0, deferred);
            assert_eq!(s.component_flags, 0x80 | u8::from(flagged));
            assert!(s.pending_records.is_empty());
            assert_eq!(
                b.calls
                    .iter()
                    .any(|(_, e)| matches!(e, Effect::Explosion { .. })),
                explosion
            );
        }
        let mut s = object(18);
        s.profile.as_mut().unwrap().explosion = None;
        let mut b = backend();
        s.kill(&mut b, OFFLINE).unwrap();
        assert_eq!(s.flags & DEFERRED_DELETE, 0);
        assert_eq!(s.component_flags, 0x80);
        assert!(
            !b.calls
                .iter()
                .any(|(_, e)| matches!(e, Effect::Explosion { .. }))
        );
        let mut s = object(2);
        s.profile = None;
        let before = s.clone();
        let mut b = backend();
        s.kill(&mut b, OFFLINE).unwrap();
        assert_eq!(s.lifecycle_state, 1);
        assert_eq!(s.flags, before.flags);
        assert_eq!(s.attachments, before.attachments);
        assert_eq!(s.auxiliary_handle, before.auxiliary_handle);
        assert_eq!(s.pending_records, before.pending_records);
        assert_eq!(s.component_flags, before.component_flags);
        assert!(b.calls.is_empty());
    }
    #[test]
    fn bounded_lists_and_empty_runtime_slots() {
        let mut s = object(2);
        s.pending_records.push_back(handle(10));
        let mut b = backend();
        let before = (s.clone(), b.clone());
        assert!(s.kill(&mut b, OFFLINE).is_err());
        assert_eq!((s, b), before);
        let mut s = object(2);
        s.attachments.push(None);
        let mut b = backend();
        let before = (s.clone(), b.clone());
        assert!(s.kill(&mut b, OFFLINE).is_err());
        assert_eq!((s, b), before);
        let mut s = object(0);
        s.spatial_data_present = false;
        s.pending_records.clear();
        s.attachments.clear();
        s.auxiliary_handle = None;
        s.network_id = None;
        let mut b = backend();
        s.kill(
            &mut b,
            Context {
                notify: false,
                networking_active: true,
            },
        )
        .unwrap();
        assert!(b.calls.is_empty());
        assert_eq!(s.flags, 4 | DEAD);
        assert_eq!(s.component_flags, 0x81);
        // Same authored ID, different live allocation: old attachments are not owned.
        let mut s = object(2);
        s.identity = handle(300);
        let mut b = backend();
        s.kill(&mut b, OFFLINE).unwrap();
        assert!(
            !b.calls
                .iter()
                .any(|(_, e)| matches!(e, Effect::ReleaseAttachment { .. }))
        );
        assert!(s.attachments.iter().all(Option::is_none));
    }
    #[test]
    fn notification_is_explicit_and_dead_objects_do_nothing() {
        let online = Context {
            notify: true,
            networking_active: true,
        };
        let mut s = object(2);
        let mut b = backend();
        s.kill(&mut b, online).unwrap();
        assert!(matches!(b.calls[0].1, Effect::Notify { network_id: 7 }));
        let mut s = object(2);
        s.network_id = None;
        let mut b = backend();
        let before = (s.clone(), b.clone());
        assert!(s.kill(&mut b, online).is_err());
        assert_eq!((s, b), before);
        let mut s = object(2);
        s.flags |= DEAD;
        let mut b = backend();
        let before = (s.clone(), b.clone());
        s.kill(&mut b, online).unwrap();
        assert_eq!((s, b), before);
    }
}
