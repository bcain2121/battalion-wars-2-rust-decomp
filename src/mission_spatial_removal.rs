//! Transactional ordering of 0x8023d13c and its 0x8023d654 query callback.
//! Not the preceding 0x802322ec operation, a broadphase implementation, or a Kill adapter.
use crate::{Object, ObjectGraph, ObjectId, Result, mission_destruction::Handle};

pub const QUERY_MASK: u32 = 0xa040_0008;
pub const QUERY_MODE: u32 = 3;
pub const NEIGHBOR_CAPACITY: usize = 30;
const EXCLUDED_FLAGS: u32 = 0x0090_0000;

/// Authored fields identified by native reflection, not inferred from neighboring offsets.
#[derive(Debug, Clone, PartialEq)]
pub struct SpatialProfile {
    pub collide_type: u32,
    pub dead_model: Option<ObjectId>,
    pub dead_model_decay_time: f32,
    pub explosion: Option<ObjectId>,
}
fn scalar<'a>(object: &'a Object, name: &str) -> Result<&'a str> {
    let mut fields = object
        .fields
        .iter()
        .filter(|f| f.attributes.get("name").is_some_and(|n| n == name));
    let field = fields
        .next()
        .ok_or_else(|| format!("Missing spatial profile field {name}"))?;
    if fields.next().is_some() || field.items.len() != 1 {
        return Err(format!("Invalid spatial profile field {name}"));
    }
    Ok(&field.items[0])
}
fn unsigned(object: &Object, name: &str) -> Result<u32> {
    let text = scalar(object, name)?;
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("Invalid spatial profile integer {name}"));
    }
    text.parse()
        .map_err(|_| format!("Spatial profile integer overflow {name}"))
}
/// Load-time value at concrete cDestroyableObject +0x12c, as registered at 0x800a3900.
/// This is an authored snapshot, not a claim that later runtime updates never change it.
pub fn authored_instance_flags(object: &Object) -> Result<u32> {
    if object.kind != "cDestroyableObject" {
        return Err("Spatial instance flags require cDestroyableObject".into());
    }
    unsigned(object, "mInstanceFlags")
}
impl SpatialProfile {
    pub fn from_object(object: &Object) -> Result<Self> {
        let destruction = crate::mission_destruction::Profile::from_object(object)?;
        let decay: f32 = scalar(object, "mDeadModelDecayTime")?
            .parse()
            .map_err(|_| "Invalid dead-model decay time")?;
        if !decay.is_finite() {
            return Err("Nonfinite dead-model decay time".into());
        }
        let dead = scalar(object, "DeadModel")?;
        if dead.is_empty() || !dead.bytes().all(|b| b.is_ascii_digit()) {
            return Err("Invalid dead-model reference".into());
        }
        let dead: u64 = dead.parse().map_err(|_| "Dead-model reference overflow")?;
        Ok(Self {
            collide_type: unsigned(object, "CollideType")?,
            dead_model: (dead != 0).then_some(ObjectId(dead)),
            dead_model_decay_time: decay,
            explosion: destruction.explosion,
        })
    }
    /// 0x802322ec, AFTER its cDestroyableObject RTTI test. Not the selected operation itself.
    /// Constructor zero is not proof of the live +0x12c value: require it when read.
    pub fn pre_query_update_required(&self, live_flags_12c: Option<u32>) -> Result<bool> {
        if self.collide_type & 8 != 0 {
            return Ok(true);
        }
        Ok(live_flags_12c.ok_or("Live destroyable flags +0x12c not configured")? & 0x800 != 0)
    }
    /// Bind the proven null-dead-model subset; non-null references require a live resource bank.
    pub fn predicate_for_null_dead_model(
        &self,
        graph: &ObjectGraph,
    ) -> Result<DestroyablePredicateProfile> {
        if self.dead_model.is_some() {
            return Err("Dead model runtime binding not configured".into());
        }
        let flags = match self.explosion {
            None => None,
            Some(id) => {
                let object = graph.objects.get(&id).ok_or("Missing explosion profile")?;
                if object.kind != "sExplodeBase" {
                    return Err("Spatial predicate requires sExplodeBase".into());
                }
                Some(unsigned(object, "mObjFlags")?)
            }
        };
        Ok(DestroyablePredicateProfile {
            field_78: self.dead_model_decay_time,
            field_a8: 0,
            explosion_flags: flags,
        })
    }
}

/// Raw fields for the verified concrete cDestroyableObject implementation only.
/// `explosion_flags: None` means a known null explosion pointer, not missing data.
#[derive(Debug, Clone, Copy)]
pub struct DestroyablePredicateProfile {
    /// mDeadModelDecayTime, not current health or mass.
    pub field_78: f32,
    /// Native DeadModel pointer word. Never substitute an authored resource ID.
    pub field_a8: u32,
    pub explosion_flags: Option<u32>,
}
/// Concrete vtable +0x78 -> 0x800cf454. None means a known null model.
pub fn destroyable_virtual_78(model_flags: Option<u32>) -> bool {
    model_flags.is_some_and(|flags| flags & 0x0100_0000 != 0)
}
/// Concrete vtable +0x1a4 -> 0x800a1290. Do not reuse for unverified derived vtables.
/// Profile is required only on the branch which actually dereferences it.
pub fn destroyable_virtual_1a4(
    model_flags: Option<u32>,
    main_flags: u32,
    profile: Option<DestroyablePredicateProfile>,
) -> Result<bool> {
    if model_flags.is_none_or(|flags| flags & 0x0040_0000 == 0) {
        return Ok(false);
    }
    if main_flags & EXCLUDED_FLAGS == 0 {
        return Ok(true);
    }
    let profile = profile.ok_or("Destroyable spatial predicate profile not configured")?;
    let positive = profile.field_78 > 0.0;
    if profile.explosion_flags.is_some_and(|flags| flags & 4 != 0) && !positive {
        return Ok(true);
    }
    Ok(profile.field_a8 != 0)
}

/// Mutable state must be deeply cloned and transaction-local: no external I/O.
/// No method has a permissive default. Virtual slots are deliberately not given guessed semantics.
pub trait Backend: Clone {
    fn has_model(&mut self, owner: Handle) -> Result<bool>;
    /// Native query uses owner +0x60 coordinates and its model resource's sphere radius.
    /// Provider must preserve native candidate order, not sort by authored ID.
    fn begin_query(&mut self, owner: Handle, mask: u32, mode: u32) -> Result<()>;
    fn next_candidate(&mut self) -> Result<Option<Handle>>;
    fn end_query(&mut self) -> Result<()>;
    /// Virtual +0x38 with cWorldObject RTTI descriptor 0x805132e8.
    fn is_world_object(&mut self, candidate: Handle) -> Result<bool>;
    fn virtual_78(&mut self, candidate: Handle) -> Result<bool>;
    fn main_flags(&mut self, candidate: Handle) -> Result<u32>;
    /// 0x80239f8c(owner, 1): requires actual native node traversal/volume updates.
    fn remove_model_volumes(&mut self, owner: Handle) -> Result<()>;
    fn virtual_1a4(&mut self, candidate: Handle) -> Result<bool>;
    /// Write literal 1 to raw object byte +0x75; meaning not inferred here.
    fn set_byte_75(&mut self, object: Handle, value: u8) -> Result<()>;
}

/// Immutable query registrations belonging to the transaction's live world.
/// An absent grid is unknown/unconfigured, never silently an empty scene.
#[derive(Clone)]
pub struct GridSnapshot {
    pub first: Option<std::sync::Arc<[crate::mission_spatial_grid::Layer; 3]>>,
    pub second: Option<std::sync::Arc<[crate::mission_spatial_grid::Layer; 3]>>,
}
/// Concrete backends must provide live owner coordinates/resource radius and resolve
/// record identity to owner identity. Neither is inferred from authored IDs.
pub trait GridBackend: Backend {
    fn owner_query(&self, owner: Handle) -> Result<crate::mission_spatial_cell::Sphere>;
    fn grid_snapshot(&self) -> Result<GridSnapshot>;
    fn record_owner(&self, record: u64) -> Result<Handle>;
}
/// Integrates cell/grid traversal with native candidate filtering and cleanup.
/// This bypasses the legacy begin/next/end provider; all query state is local.
/// Mutable world/volume/dirty-byte changes commit together only on success.
pub fn remove_using_grids<B: GridBackend>(backend: &mut B, owner: Handle) -> Result<()> {
    let mut staged = backend.clone();
    if staged.has_model(owner)? {
        let query = staged.owner_query(owner)?;
        let grids = staged.grid_snapshot()?;
        let mut neighbors = Vec::with_capacity(NEIGHBOR_CAPACITY);
        crate::mission_spatial_grid::dispatch(
            grids.first.as_deref(),
            grids.second.as_deref(),
            query,
            QUERY_MASK,
            QUERY_MODE,
            |record| {
                let candidate = staged.record_owner(record.identity)?;
                collect_neighbor(&mut staged, owner, candidate, &mut neighbors)
            },
        )?;
        finish_removal(&mut staged, owner, neighbors)?;
    }
    *backend = staged;
    Ok(())
}
fn collect_neighbor<B: Backend>(
    backend: &mut B,
    owner: Handle,
    candidate: Handle,
    neighbors: &mut Vec<Handle>,
) -> Result<bool> {
    if candidate == owner || !backend.is_world_object(candidate)? {
        return Ok(true);
    }
    if backend.virtual_78(candidate)? && backend.main_flags(candidate)? & EXCLUDED_FLAGS != 0 {
        return Ok(true);
    }
    if neighbors.len() == NEIGHBOR_CAPACITY {
        return Ok(false);
    }
    neighbors.push(candidate);
    Ok(true)
}
fn finish_removal<B: Backend>(
    backend: &mut B,
    owner: Handle,
    neighbors: Vec<Handle>,
) -> Result<()> {
    backend.remove_model_volumes(owner)?;
    backend.set_byte_75(owner, 1)?;
    for neighbor in neighbors {
        if backend.virtual_1a4(neighbor)? {
            backend.set_byte_75(neighbor, 1)?;
        }
    }
    Ok(())
}

pub fn remove<B: Backend>(backend: &mut B, owner: Handle) -> Result<()> {
    let mut staged = backend.clone();
    execute(&mut staged, owner)?;
    *backend = staged;
    Ok(())
}
fn execute<B: Backend>(backend: &mut B, owner: Handle) -> Result<()> {
    if !backend.has_model(owner)? {
        return Ok(());
    }
    backend.begin_query(owner, QUERY_MASK, QUERY_MODE)?;
    let mut neighbors = Vec::with_capacity(NEIGHBOR_CAPACITY);
    while let Some(candidate) = backend.next_candidate()? {
        if !collect_neighbor(backend, owner, candidate, &mut neighbors)? {
            break;
        }
    }
    backend.end_query()?;
    finish_removal(backend, owner, neighbors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, VecDeque};
    fn h(n: u64) -> Handle {
        Handle(std::num::NonZeroU64::new(n).unwrap())
    }
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Fake {
        model: bool,
        candidates: VecDeque<Handle>,
        predicates: BTreeMap<Handle, (bool, bool, u32, bool)>,
        bytes: BTreeMap<Handle, u8>,
        events: Vec<String>,
        fail: Option<usize>,
    }
    impl Fake {
        fn new(ids: &[u64]) -> Self {
            Self {
                model: true,
                candidates: ids.iter().copied().map(h).collect(),
                predicates: ids.iter().map(|&i| (h(i), (true, true, 0, true))).collect(),
                bytes: BTreeMap::new(),
                events: vec![],
                fail: None,
            }
        }
        fn event(&mut self, s: String) -> Result<()> {
            if self.fail == Some(self.events.len()) {
                return Err("Injected spatial failure".into());
            }
            self.events.push(s);
            Ok(())
        }
    }
    impl GridBackend for Fake {
        fn owner_query(&self, _: Handle) -> Result<crate::mission_spatial_cell::Sphere> {
            Ok(crate::mission_spatial_cell::Sphere {
                center: [0.0; 3],
                radius: 1.0,
            })
        }
        fn grid_snapshot(&self) -> Result<GridSnapshot> {
            use crate::mission_spatial_cell::{Cell, Contents, Record, Sphere};
            use crate::mission_spatial_grid::Layer;
            let empty = || Layer {
                maximum_record_radius: 1.0,
                cells: BTreeMap::new(),
            };
            let mut first = [empty(), empty(), empty()];
            let records = self
                .candidates
                .iter()
                .map(|h| Record {
                    identity: h.0.get(),
                    mask: QUERY_MASK,
                    sphere: Sphere {
                        center: [0.0; 3],
                        radius: 1.0,
                    },
                })
                .collect();
            first[0].cells.insert(
                (32, 32),
                Cell {
                    mask: QUERY_MASK,
                    contents: Contents::Leaf(records),
                },
            );
            Ok(GridSnapshot {
                first: Some(std::sync::Arc::new(first)),
                second: Some(std::sync::Arc::new([empty(), empty(), empty()])),
            })
        }
        fn record_owner(&self, record: u64) -> Result<Handle> {
            // Synthetic fixture mapping only; production must resolve native record ownership.
            let owner = h(record);
            self.predicates
                .contains_key(&owner)
                .then_some(owner)
                .ok_or("Unknown record owner".into())
        }
    }
    #[test]
    fn real_grid_walk_feeds_filter_and_stops_before_unknown_record() {
        let ids: Vec<_> = (1..=32).collect();
        let mut backend = Fake::new(&ids);
        backend.predicates.remove(&h(32));
        remove_using_grids(&mut backend, h(100)).unwrap();
        assert_eq!(backend.bytes.len(), 31); // owner plus the first 30 eligible neighbors
        assert_eq!(backend.bytes[&h(100)], 1);
        assert!(!backend.bytes.contains_key(&h(31)));
        assert!(
            !backend
                .events
                .iter()
                .any(|e| e == "begin" || e == "next" || e == "end")
        );
        let volume = backend.events.iter().position(|e| e == "volumes").unwrap();
        assert!(
            backend.events[..volume]
                .iter()
                .all(|e| !e.starts_with("byte:"))
        );
    }
    #[test]
    fn integrated_grid_cleanup_rolls_back_every_backend_failure() {
        let mut reference = Fake::new(&[1, 1, 2]);
        remove_using_grids(&mut reference, h(100)).unwrap();
        assert_eq!(
            reference
                .events
                .iter()
                .filter(|e| e.as_str() == "byte:1")
                .count(),
            2
        );
        for failure in 0..reference.events.len() {
            let mut backend = Fake::new(&[1, 1, 2]);
            backend.fail = Some(failure);
            let before = backend.clone();
            assert!(remove_using_grids(&mut backend, h(100)).is_err());
            assert_eq!(backend, before);
        }
        let mut backend = Fake::new(&[1, 2]);
        backend.predicates.remove(&h(2));
        let before = backend.clone();
        assert!(remove_using_grids(&mut backend, h(100)).is_err());
        assert_eq!(backend, before);
    }
    impl Backend for Fake {
        fn has_model(&mut self, _: Handle) -> Result<bool> {
            self.event("model".into())?;
            Ok(self.model)
        }
        fn begin_query(&mut self, _: Handle, mask: u32, mode: u32) -> Result<()> {
            assert_eq!((mask, mode), (QUERY_MASK, QUERY_MODE));
            self.event("begin".into())
        }
        fn next_candidate(&mut self) -> Result<Option<Handle>> {
            self.event("next".into())?;
            Ok(self.candidates.pop_front())
        }
        fn end_query(&mut self) -> Result<()> {
            self.event("end".into())
        }
        fn is_world_object(&mut self, c: Handle) -> Result<bool> {
            self.event(format!("type:{}", c.0))?;
            Ok(self.predicates[&c].0)
        }
        fn virtual_78(&mut self, c: Handle) -> Result<bool> {
            self.event(format!("78:{}", c.0))?;
            Ok(self.predicates[&c].1)
        }
        fn main_flags(&mut self, c: Handle) -> Result<u32> {
            self.event(format!("flags:{}", c.0))?;
            Ok(self.predicates[&c].2)
        }
        fn remove_model_volumes(&mut self, _: Handle) -> Result<()> {
            self.event("volumes".into())
        }
        fn virtual_1a4(&mut self, c: Handle) -> Result<bool> {
            self.event(format!("1a4:{}", c.0))?;
            Ok(self.predicates[&c].3)
        }
        fn set_byte_75(&mut self, c: Handle, v: u8) -> Result<()> {
            self.event(format!("byte:{}", c.0))?;
            self.bytes.insert(c, v);
            Ok(())
        }
    }
    #[test]
    fn concrete_destroyable_predicates_use_model_bits_and_lazy_profile_access() {
        assert!(!destroyable_virtual_78(None));
        assert!(!destroyable_virtual_78(Some(0x0040_0000)));
        assert!(destroyable_virtual_78(Some(0x0100_0000)));
        assert!(!destroyable_virtual_1a4(None, u32::MAX, None).unwrap());
        assert!(!destroyable_virtual_1a4(Some(0), u32::MAX, None).unwrap());
        assert!(destroyable_virtual_1a4(Some(0x400000), 0, None).unwrap());
        assert!(destroyable_virtual_1a4(Some(0x400000), 0x100000, None).is_err());
        for (flags, value, a8, expected) in [
            (None, 0.0, 0, false),
            (None, 0.0, 1, true),
            (Some(4), 0.0, 0, true),
            (Some(4), 1.0, 0, false),
            (Some(16), 0.0, 0, false),
            (Some(4), -1.0, 0, true),
        ] {
            let p = DestroyablePredicateProfile {
                field_78: value,
                field_a8: a8,
                explosion_flags: flags,
            };
            assert_eq!(
                destroyable_virtual_1a4(Some(0x400000), 0x100000, Some(p)).unwrap(),
                expected
            );
        }
    }
    #[test]
    fn callback_filters_short_circuit_preserve_duplicates_and_order() {
        let mut f = Fake::new(&[1, 2, 3, 4, 5, 6, 6]);
        f.predicates.insert(h(2), (false, true, 0, true));
        f.predicates.insert(h(3), (true, true, 0x100000, true));
        f.predicates.insert(h(4), (true, true, 0x800000, true));
        f.predicates.insert(h(5), (true, false, 0x900000, false));
        remove(&mut f, h(1)).unwrap();
        assert!(!f.events.contains(&"type:1".into()));
        assert!(!f.events.contains(&"78:2".into()));
        assert!(!f.events.contains(&"flags:5".into()));
        let start = f.events.iter().position(|e| e == "volumes").unwrap();
        assert_eq!(
            &f.events[start..],
            [
                "volumes", "byte:1", "1a4:5", "1a4:6", "byte:6", "1a4:6", "byte:6"
            ]
        );
        assert_eq!(f.bytes, BTreeMap::from([(h(1), 1), (h(6), 1)]));
        // 0x00080000 is NOT 0x00800000: deferred-delete alone does not exclude.
        for (flags, kept) in [
            (0, true),
            (0x80000, true),
            (0x100000, false),
            (0x800000, false),
            (0x900000, false),
        ] {
            let mut f = Fake::new(&[2]);
            f.predicates.insert(h(2), (true, true, flags, true));
            remove(&mut f, h(1)).unwrap();
            assert_eq!(f.bytes.contains_key(&h(2)), kept);
        }
    }
    #[test]
    fn capacity_stops_on_next_eligible_and_leaves_later_candidates_unread() {
        let ids = (2..=33).collect::<Vec<_>>();
        let mut f = Fake::new(&ids);
        remove(&mut f, h(1)).unwrap();
        assert_eq!(f.bytes.len(), 31); // owner + 30 neighbors
        assert_eq!(f.candidates, VecDeque::from([h(33)]));
        assert!(f.events.contains(&"flags:32".into()));
        assert!(!f.events.contains(&"1a4:32".into()));
        let mut f = Fake::new(&ids);
        f.predicates.insert(h(32), (false, false, 0, false));
        remove(&mut f, h(1)).unwrap();
        assert!(f.candidates.is_empty());
        let mut f = Fake::new(&ids);
        for id in 2..=31 {
            f.predicates.insert(h(id), (true, true, 0, false));
        }
        remove(&mut f, h(1)).unwrap();
        // The final predicate does not reclaim collector capacity.
        assert_eq!(f.bytes, BTreeMap::from([(h(1), 1)]));
        assert!(!f.events.contains(&"1a4:32".into()));
    }
    #[test]
    fn all_operation_failures_roll_back_and_known_null_model_skips_query() {
        for ids in [vec![], vec![2, 3, 2], (2..=35).collect()] {
            let initial = Fake::new(&ids);
            let mut success = initial.clone();
            remove(&mut success, h(1)).unwrap();
            for step in 0..success.events.len() {
                let mut failing = initial.clone();
                failing.fail = Some(step);
                let before = failing.clone();
                assert!(remove(&mut failing, h(1)).is_err());
                assert_eq!(failing, before);
            }
        }
        let mut f = Fake::new(&[2]);
        f.model = false;
        remove(&mut f, h(1)).unwrap();
        assert_eq!(f.events, ["model"]);
        assert!(f.bytes.is_empty());
    }
}
