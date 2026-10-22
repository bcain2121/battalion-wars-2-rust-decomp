//! Authored unit routes. Offsets are waypoint-local X/Z, transformed by Mat.
//! Uses existing actor navigation, not camera mSpeed. Arrival uses the recovered
//! default 1-unit tolerance; collision/acceleration remain replacement policies.
use crate::{
    ObjectGraph, ObjectId, Result,
    mission_random::CrtRand,
    original_scene::{field, matrix},
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
#[derive(Clone, Debug)]
pub struct Node {
    pub matrix: [f32; 16],
    pub next: Option<ObjectId>,
    pub optional: [Option<ObjectId>; 2],
    pub flags: u32,
    pub pause: f64,
}
#[derive(Clone, Debug)]
pub struct Bank {
    nodes: BTreeMap<ObjectId, Node>,
}
impl Bank {
    pub fn load(graph: &ObjectGraph) -> Result<Self> {
        let mut nodes = BTreeMap::new();
        for o in graph.objects.values().filter(|o| o.kind == "cWaypoint") {
            let pointer = |key: &str| -> Result<Option<ObjectId>> {
                let id = field(o, key)
                    .ok_or_else(|| format!("Missing waypoint {key}"))?
                    .parse::<u64>()
                    .map_err(|_| "Invalid waypoint reference")?;
                Ok((id != 0).then_some(ObjectId(id)))
            };
            nodes.insert(
                o.id,
                Node {
                    matrix: matrix(field(o, "Mat").ok_or("Missing waypoint matrix")?)?,
                    next: pointer("NextWP")?,
                    optional: [pointer("mOptionalNextWP1")?, pointer("mOptionalNextWP2")?],
                    flags: field(o, "Flags")
                        .ok_or("Missing waypoint flags")?
                        .parse()
                        .map_err(|_| "Invalid waypoint flags")?,
                    pause: field(o, "mPauseTime")
                        .ok_or("Missing waypoint pause")?
                        .parse()
                        .map_err(|_| "Invalid waypoint pause")?,
                },
            );
        }
        Self::from_nodes(nodes)
    }
    pub fn from_nodes(nodes: BTreeMap<ObjectId, Node>) -> Result<Self> {
        for n in nodes.values() {
            if n.matrix.iter().any(|v| !v.is_finite())
                || !n.pause.is_finite()
                || n.pause < 0.0
                || n.matrix[3] != 0.0
                || n.matrix[7] != 0.0
                || n.matrix[11] != 0.0
                || n.matrix[15] != 1.0
            {
                return Err("Invalid waypoint transform/pause".into());
            }
            if n.next
                .into_iter()
                .chain(n.optional.into_iter().flatten())
                .any(|id| !nodes.contains_key(&id))
            {
                return Err("Missing waypoint successor".into());
            }
        }
        Ok(Self { nodes })
    }
    pub fn target(&self, id: ObjectId, offset: [f64; 2]) -> Result<[f64; 2]> {
        if offset.iter().any(|v| !v.is_finite()) {
            return Err("Invalid waypoint offset".into());
        }
        let m = &self.nodes.get(&id).ok_or("Unknown waypoint")?.matrix;
        let target = [
            m[12] as f64 + m[0] as f64 * offset[0] + m[8] as f64 * offset[1],
            m[14] as f64 + m[2] as f64 * offset[0] + m[10] as f64 * offset[1],
        ];
        if target.iter().any(|v| !(*v as f32).is_finite()) {
            return Err("Waypoint target overflow".into());
        }
        Ok(target)
    }
    /// `0x8003dfcc` (`cWaypoint::GetNextWaypoint`) with `exclude = current` (`0x8015f490`):
    /// returns `NextWP` directly without touching `rand()` when both optional slots are null;
    /// otherwise collects non-null, non-`current` slots in `[NextWP, mOptionalNextWP1, mOptionalNextWP2]`
    /// order (preserving duplicate slots) and indexes by `rand() % count`.
    pub fn select_successor(
        &self,
        current: ObjectId,
        rng: &mut CrtRand,
    ) -> Result<Option<ObjectId>> {
        let n = self.nodes.get(&current).ok_or("Unknown waypoint")?;
        if n.optional == [None, None] {
            return Ok(n.next);
        }
        let candidates = [n.next, n.optional[0], n.optional[1]]
            .into_iter()
            .flatten()
            .filter(|&id| id != current)
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            return Err("Invalid waypoint branch candidates".into());
        }
        let index = (rng.next_u15() as usize) % candidates.len();
        Ok(Some(candidates[index]))
    }
    fn validate_path(&self, start: ObjectId, offset: [f64; 2]) -> Result<()> {
        let mut seen = BTreeSet::new();
        let mut queue = VecDeque::from([start]);
        while let Some(id) = queue.pop_front() {
            if !seen.insert(id) {
                continue;
            }
            let n = self.nodes.get(&id).ok_or("Unknown waypoint")?;
            // Top speed, road/drain category tags and timed pause supported. Facing behavior
            // and camera-only paths remain rejected.
            if n.flags & !(1 | 8 | 16 | 128) != 0 {
                return Err("Unsupported unit waypoint flags".into());
            }
            if n.optional != [None, None]
                && ![n.next, n.optional[0], n.optional[1]]
                    .into_iter()
                    .flatten()
                    .any(|c| c != id)
            {
                return Err("Invalid waypoint branch candidates".into());
            }
            self.target(id, offset)?;
            for next in n.next.into_iter().chain(n.optional.into_iter().flatten()) {
                queue.push_back(next);
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct Route {
    pub node: ObjectId,
    offset: [f64; 2],
    pause_left: Option<f64>,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Move([f64; 2]),
    Wait,
    Complete,
}
impl Route {
    pub const RADIUS: f64 = 1.0;
    pub fn new(bank: &Bank, node: ObjectId, offset: [f64; 2]) -> Result<Self> {
        bank.validate_path(node, offset)?;
        Ok(Self {
            node,
            offset,
            pause_left: None,
        })
    }
    pub fn target(&self, bank: &Bank) -> Result<[f64; 2]> {
        bank.target(self.node, self.offset)
    }
    pub fn reached(&self, bank: &Bank, position: [f32; 3]) -> Result<bool> {
        if position.iter().any(|v| !v.is_finite()) {
            return Err("Invalid waypoint follower position".into());
        }
        let t = self.target(bank)?;
        Ok((t[0] - position[0] as f64).hypot(t[1] - position[2] as f64) <= Self::RADIUS + 0.001)
    }
    pub fn update(&mut self, bank: &Bank, position: [f32; 3], dt: f64) -> Result<Action> {
        self.update_with_rng(bank, position, dt, &mut CrtRand::default())
    }
    /// At most one link per tick. Pauses round conservatively to the next tick;
    /// no leftover tick time is incorrectly reused for movement.
    pub fn update_with_rng(
        &mut self,
        bank: &Bank,
        position: [f32; 3],
        dt: f64,
        rng: &mut CrtRand,
    ) -> Result<Action> {
        if !dt.is_finite() || !(0.0..=1.0).contains(&dt) {
            return Err("Invalid waypoint timestep".into());
        }
        if !self.reached(bank, position)? {
            return Ok(Action::Move(self.target(bank)?));
        }
        let n = &bank.nodes[&self.node];
        let pause = self
            .pause_left
            .get_or_insert(if n.flags & 16 != 0 { n.pause } else { 0.0 });
        if *pause > 0.0 {
            *pause = (*pause - dt).max(0.0);
            return Ok(Action::Wait);
        }
        if let Some(next) = bank.select_successor(self.node, rng)? {
            self.node = next;
            self.pause_left = None;
            Ok(Action::Move(self.target(bank)?))
        } else {
            Ok(Action::Complete)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn node(x: f32, next: Option<ObjectId>) -> Node {
        Node {
            matrix: [
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, x, 0.0, 0.0, 1.0,
            ],
            next,
            optional: [None, None],
            flags: 1,
            pause: 0.0,
        }
    }
    #[test]
    fn offsets_rotate_with_each_waypoint() {
        let mut n = node(10.0, None);
        n.matrix[0] = 0.0;
        n.matrix[2] = -1.0;
        n.matrix[8] = 1.0;
        n.matrix[10] = 0.0;
        let b = Bank::from_nodes([(ObjectId(1), n)].into()).unwrap();
        assert_eq!(b.target(ObjectId(1), [2.0, 3.0]).unwrap(), [13.0, -2.0]);
        assert!(b.target(ObjectId(1), [f64::MAX; 2]).is_err());
    }
    #[test]
    fn follows_links_and_honors_pause_only_when_flagged() {
        let mut a = node(0.0, Some(ObjectId(2)));
        a.flags = 16;
        a.pause = 0.5;
        let b =
            Bank::from_nodes([(ObjectId(1), a), (ObjectId(2), node(5.0, None))].into()).unwrap();
        let mut r = Route::new(&b, ObjectId(1), [0.0; 2]).unwrap();
        assert_eq!(r.update(&b, [0.0; 3], 0.25).unwrap(), Action::Wait);
        assert_eq!(r.update(&b, [0.0; 3], 0.25).unwrap(), Action::Wait);
        assert_eq!(
            r.update(&b, [0.0; 3], 0.25).unwrap(),
            Action::Move([5.0, 0.0])
        );
        assert_eq!(
            r.update(&b, [5.0, 0.0, 0.0], 0.25).unwrap(),
            Action::Complete
        );
    }
    #[test]
    fn rejects_unsupported_paths_and_bounds_cycles() {
        let mut n = node(0.0, Some(ObjectId(1)));
        n.flags = 2;
        let b = Bank::from_nodes([(ObjectId(1), n.clone())].into()).unwrap();
        assert!(Route::new(&b, ObjectId(1), [0.0; 2]).is_err());
        n.flags = 1;
        n.pause = 99.0;
        let b = Bank::from_nodes([(ObjectId(1), n)].into()).unwrap();
        let mut r = Route::new(&b, ObjectId(1), [0.0; 2]).unwrap();
        assert_eq!(r.update(&b, [0.0; 3], 1.0).unwrap(), Action::Move([0.0; 2]));
        assert!(r.update(&b, [0.0; 3], f64::NAN).is_err());
    }
    #[test]
    fn branch_selection_matches_native_slot_order_and_rand_consumption() {
        let mut n1 = node(0.0, Some(ObjectId(2)));
        let mut n2 = node(5.0, Some(ObjectId(3)));
        n2.optional = [Some(ObjectId(4)), Some(ObjectId(5))];
        let b = Bank::from_nodes(
            [
                (ObjectId(1), n1.clone()),
                (ObjectId(2), n2),
                (ObjectId(3), node(10.0, None)),
                (ObjectId(4), node(20.0, None)),
                (ObjectId(5), node(30.0, None)),
            ]
            .into(),
        )
        .unwrap();
        let mut rng = CrtRand::default();
        let mut r = Route::new(&b, ObjectId(1), [0.0; 2]).unwrap();
        // Unbranched arrival at node 1 must NOT advance CrtRand.
        assert_eq!(
            r.update_with_rng(&b, [0.0; 3], 0.25, &mut rng).unwrap(),
            Action::Move([5.0, 0.0])
        );
        assert_eq!(rng.state(), 1);
        // First CrtRand draw from seed 1 is 16_838; 16_838 % 3 == 2 -> mOptionalNextWP2 (ObjectId(5)).
        assert_eq!(
            r.update_with_rng(&b, [5.0, 0.0, 0.0], 0.25, &mut rng)
                .unwrap(),
            Action::Move([30.0, 0.0])
        );
        assert_eq!(r.node, ObjectId(5));
        assert_eq!(rng.state(), 1_103_527_590);
        // Self-loops in candidate slots are excluded while duplicate non-self candidates are preserved.
        n1.next = Some(ObjectId(1));
        n1.optional = [Some(ObjectId(2)), Some(ObjectId(2))];
        let b_self =
            Bank::from_nodes([(ObjectId(1), n1), (ObjectId(2), node(5.0, None))].into()).unwrap();
        assert_eq!(
            b_self.select_successor(ObjectId(1), &mut rng).unwrap(),
            Some(ObjectId(2))
        );
    }
    #[test]
    fn validates_reachable_optional_branches_and_rejects_degenerate_candidates() {
        let mut bad_branch = node(10.0, None);
        bad_branch.flags = 4;
        let mut root = node(0.0, Some(ObjectId(2)));
        root.optional = [None, Some(ObjectId(3))];
        let b = Bank::from_nodes(
            [
                (ObjectId(1), root.clone()),
                (ObjectId(2), node(5.0, None)),
                (ObjectId(3), bad_branch),
            ]
            .into(),
        )
        .unwrap();
        assert!(Route::new(&b, ObjectId(1), [0.0; 2]).is_err());

        // Missing optional successor rejected at Bank construction.
        assert!(Bank::from_nodes([(ObjectId(1), root)].into()).is_err());

        // Branching node whose only non-null candidates are self-loops is rejected.
        let mut self_only = node(0.0, None);
        self_only.optional = [Some(ObjectId(1)), None];
        let b_self = Bank::from_nodes([(ObjectId(1), self_only)].into()).unwrap();
        assert!(Route::new(&b_self, ObjectId(1), [0.0; 2]).is_err());
    }
}
