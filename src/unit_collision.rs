//! Replacement infantry cylinder collision. Original collision ABI is not asserted.
//! Continuous horizontal sweeps against height-clipped mesh triangles and active bodies.
use crate::mission_destruction::Handle;
use crate::{Result, original_scene, skirmish::Ground};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

/// Live instance identity plus source node index; not an authored object ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ColliderKey {
    pub owner: Handle,
    pub node: usize,
}
type V2 = [f64; 2];
fn add(a: V2, b: V2) -> V2 {
    [a[0] + b[0], a[1] + b[1]]
}
fn sub(a: V2, b: V2) -> V2 {
    [a[0] - b[0], a[1] - b[1]]
}
fn mul(a: V2, s: f64) -> V2 {
    [a[0] * s, a[1] * s]
}
fn dot(a: V2, b: V2) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
fn len(a: V2) -> f64 {
    dot(a, a).sqrt()
}
const SKIN: f64 = 0.002;
#[derive(Debug, Clone, Copy)]
pub struct Body {
    pub radius: f64,
    pub height: f64,
    pub step: f64,
}
impl Body {
    pub const INFANTRY: Self = Self {
        radius: 0.6,
        height: 2.0,
        step: 0.25,
    };
    pub fn validate(self) -> Result<()> {
        if !self.radius.is_finite()
            || !self.height.is_finite()
            || !self.step.is_finite()
            || self.radius <= 0.0
            || self.height <= 0.0
            || self.step < 0.0
            || self.step >= self.height
        {
            Err("Invalid collision body".into())
        } else {
            Ok(())
        }
    }
}
thread_local! {
    static CANDIDATES: std::cell::RefCell<Vec<u32>> = const { std::cell::RefCell::new(Vec::new()) };
}
#[derive(Debug, Clone)]
struct Mesh {
    owner: Option<ColliderKey>,
    triangles: Arc<Vec<[[f32; 3]; 3]>>,
    min: [f32; 3],
    max: [f32; 3],
    grid: Option<Arc<TriGrid>>,
}
/// Uniform XZ bucket index over one mesh's triangles. Pure culling: queries
/// return the same triangles (in ascending original order) as a linear scan
/// filtered by the same per-triangle bounds test.
#[derive(Debug)]
struct TriGrid {
    origin: [f64; 2],
    cell: f64,
    dims: [usize; 2],
    cells: Vec<Vec<u32>>,
}
impl TriGrid {
    const MIN_TRIANGLES: usize = 48;
    const CELL: f64 = 4.0;
    fn build(tris: &[[[f32; 3]; 3]], min: [f32; 3], max: [f32; 3]) -> Option<Self> {
        if tris.len() < Self::MIN_TRIANGLES {
            return None;
        }
        let origin = [min[0] as f64, min[2] as f64];
        let span = [(max[0] - min[0]) as f64, (max[2] - min[2]) as f64];
        let cell = Self::CELL.max(span[0].max(span[1]) / 256.0);
        let dims = [
            ((span[0] / cell).floor() as usize + 1).min(256),
            ((span[1] / cell).floor() as usize + 1).min(256),
        ];
        let mut cells = vec![Vec::new(); dims[0] * dims[1]];
        for (i, t) in tris.iter().enumerate() {
            let lo = [
                t.iter().map(|v| v[0]).fold(f32::INFINITY, f32::min) as f64,
                t.iter().map(|v| v[2]).fold(f32::INFINITY, f32::min) as f64,
            ];
            let hi = [
                t.iter().map(|v| v[0]).fold(f32::NEG_INFINITY, f32::max) as f64,
                t.iter().map(|v| v[2]).fold(f32::NEG_INFINITY, f32::max) as f64,
            ];
            let (x0, x1) = (
                Self::index(lo[0], origin[0], cell, dims[0]),
                Self::index(hi[0], origin[0], cell, dims[0]),
            );
            let (z0, z1) = (
                Self::index(lo[1], origin[1], cell, dims[1]),
                Self::index(hi[1], origin[1], cell, dims[1]),
            );
            for z in z0..=z1 {
                for x in x0..=x1 {
                    cells[z * dims[0] + x].push(i as u32);
                }
            }
        }
        Some(Self {
            origin,
            cell,
            dims,
            cells,
        })
    }
    fn index(v: f64, origin: f64, cell: f64, dim: usize) -> usize {
        (((v - origin) / cell).floor().max(0.0) as usize).min(dim - 1)
    }
    /// Candidate triangle indices overlapping the XZ box, ascending and unique.
    fn query(&self, xmin: f64, xmax: f64, zmin: f64, zmax: f64, out: &mut Vec<u32>) {
        out.clear();
        let (x0, x1) = (
            Self::index(xmin, self.origin[0], self.cell, self.dims[0]),
            Self::index(xmax, self.origin[0], self.cell, self.dims[0]),
        );
        let (z0, z1) = (
            Self::index(zmin, self.origin[1], self.cell, self.dims[1]),
            Self::index(zmax, self.origin[1], self.cell, self.dims[1]),
        );
        for z in z0..=z1 {
            for x in x0..=x1 {
                out.extend_from_slice(&self.cells[z * self.dims[0] + x]);
            }
        }
        out.sort_unstable();
        out.dedup();
    }
}
#[derive(Debug, Clone, Copy)]
pub struct BridgeSpan {
    pub ux: f32,
    pub uz: f32,
    pub min_u: f32,
    pub max_u: f32,
    pub min_v: f32,
    pub max_v: f32,
    pub y0: f64,
    pub y1: f64,
}
#[derive(Debug, Default, Clone)]
pub struct Scene {
    meshes: Vec<Mesh>,
    owned_nodes: BTreeSet<ColliderKey>,
    object_owners: BTreeMap<crate::ObjectId, Handle>,
    disabled_owners: BTreeSet<Handle>,
    walkable_owners: BTreeSet<Handle>,
    crushable_owners: BTreeSet<Handle>,
    pub ground: Option<Arc<Ground>>,
    pub bridge_spans: Vec<BridgeSpan>,
    /// Lazily built XZ index over `meshes`; reset by every mesh mutation.
    index: std::sync::OnceLock<Arc<MeshIndex>>,
}
/// Coarse XZ bucket index of mesh indices. Pure culling: queries return a
/// superset of AABB-overlapping meshes in ascending original order.
#[derive(Debug, Default)]
struct MeshIndex {
    cells: std::collections::HashMap<(i32, i32), Vec<u32>>,
    large: Vec<u32>,
    count: u32,
}
impl MeshIndex {
    const CELL: f64 = 16.0;
    const MAX_CELLS: i64 = 256;
    fn cell(v: f64) -> i32 {
        (v / Self::CELL).floor().clamp(-1e6, 1e6) as i32
    }
    fn build(meshes: &[Mesh]) -> Self {
        let mut index = Self {
            count: meshes.len() as u32,
            ..Self::default()
        };
        for (i, m) in meshes.iter().enumerate() {
            let (x0, x1) = (Self::cell(m.min[0] as f64), Self::cell(m.max[0] as f64));
            let (z0, z1) = (Self::cell(m.min[2] as f64), Self::cell(m.max[2] as f64));
            if (x1 - x0 + 1) as i64 * (z1 - z0 + 1) as i64 > Self::MAX_CELLS {
                index.large.push(i as u32);
                continue;
            }
            for z in z0..=z1 {
                for x in x0..=x1 {
                    index.cells.entry((x, z)).or_default().push(i as u32);
                }
            }
        }
        index
    }
    fn query(&self, xmin: f64, xmax: f64, zmin: f64, zmax: f64, out: &mut Vec<u32>) {
        out.clear();
        out.extend_from_slice(&self.large);
        let (x0, x1) = (Self::cell(xmin), Self::cell(xmax));
        let (z0, z1) = (Self::cell(zmin), Self::cell(zmax));
        if (x1 - x0 + 1) as i64 * (z1 - z0 + 1) as i64 > Self::MAX_CELLS * 4 {
            out.clear();
            out.extend(0..self.count);
            return;
        }
        for z in z0..=z1 {
            for x in x0..=x1 {
                if let Some(v) = self.cells.get(&(x, z)) {
                    out.extend_from_slice(v);
                }
            }
        }
        out.sort_unstable();
        out.dedup();
    }
}
thread_local! {
    static MESH_CANDIDATES: std::cell::RefCell<Vec<u32>> = const { std::cell::RefCell::new(Vec::new()) };
}
#[derive(Debug, Clone, Copy)]
pub struct Other {
    pub position: [f32; 3],
    pub body: Body,
}
#[derive(Debug)]
pub struct Movement {
    pub position: [f32; 3],
    pub blocked: bool,
    pub mesh_contacts: u64,
    pub unit_contacts: u64,
}
#[derive(Clone, Copy)]
struct Hit {
    t: f64,
    n: V2,
    unit: bool,
}
fn choose(best: &mut Option<Hit>, hit: Option<Hit>) {
    if let Some(h) = hit
        && best.is_none_or(|b| h.t < b.t)
    {
        *best = Some(h);
    }
}
fn circle(p: V2, d: V2, c: V2, r: f64) -> Option<Hit> {
    let q = sub(p, c);
    let a = dot(d, d);
    if a < 1e-16 {
        return None;
    }
    let length = len(q);
    if length > 1e-12 && length <= r + 1e-5 {
        let n = mul(q, 1.0 / length);
        if dot(d, n) < -1e-10 {
            return Some(Hit {
                t: 0.0,
                n,
                unit: false,
            });
        }
    }
    let b = dot(q, d);
    let disc = b * b - a * (dot(q, q) - r * r);
    if disc < 0.0 {
        return None;
    }
    let t = (-b - disc.sqrt()) / a;
    if !(-1e-8..=1.0).contains(&t) {
        return None;
    }
    let normal = sub(add(p, mul(d, t.max(0.0))), c);
    let l = len(normal);
    if l < 1e-12 {
        return None;
    }
    let n = mul(normal, 1.0 / l);
    if dot(d, n) >= -1e-10 {
        return None;
    }
    Some(Hit {
        t: t.max(0.0),
        n,
        unit: false,
    })
}
fn edge(p: V2, d: V2, a: V2, b: V2, r: f64) -> Option<Hit> {
    let mut hit = None;
    choose(&mut hit, circle(p, d, a, r));
    choose(&mut hit, circle(p, d, b, r));
    let e = sub(b, a);
    let length = len(e);
    if length < 1e-10 {
        return hit;
    }
    let tangent = mul(e, 1.0 / length);
    let n = [-tangent[1], tangent[0]];
    for sign in [-1.0, 1.0] {
        let n = mul(n, sign);
        let offset = dot(sub(p, a), n);
        let velocity = dot(d, n);
        if velocity >= -1e-10 {
            continue;
        }
        let at_start = dot(sub(p, a), tangent);
        if (0.0..=r + 1e-5).contains(&offset) && (0.0..=length).contains(&at_start) {
            choose(
                &mut hit,
                Some(Hit {
                    t: 0.0,
                    n,
                    unit: false,
                }),
            );
            continue;
        }
        let t = (r - offset) / velocity;
        if !(-1e-8..=1.0).contains(&t) {
            continue;
        }
        let along = dot(sub(add(p, mul(d, t.max(0.0))), a), tangent);
        if (0.0..=length).contains(&along) {
            choose(
                &mut hit,
                Some(Hit {
                    t: t.max(0.0),
                    n,
                    unit: false,
                }),
            );
        }
    }
    hit
}
fn distance_segment(p: V2, a: V2, b: V2) -> f64 {
    let e = sub(b, a);
    let length = dot(e, e);
    let t = if length > 1e-16 {
        (dot(sub(p, a), e) / length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    len(sub(p, add(a, mul(e, t))))
}
fn polygon_overlap(p: V2, poly: &[V2], radius: f64) -> bool {
    if poly.is_empty() {
        return false;
    }
    let mut positive = false;
    let mut negative = false;
    let mut area = 0.0;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        if distance_segment(p, a, b) < radius - 1e-5 {
            return true;
        }
        let e = sub(b, a);
        let q = sub(p, a);
        let cross = e[0] * q[1] - e[1] * q[0];
        positive |= cross > 1e-8;
        negative |= cross < -1e-8;
        area += a[0] * b[1] - a[1] * b[0];
    }
    area.abs() > 1e-8 && !(positive && negative)
}
fn clipped(triangle: [[f32; 3]; 3], low: f64, high: f64) -> Vec<V2> {
    let mut vertices = triangle.map(|p| p.map(|v| v as f64)).to_vec();
    for (height, lower) in [(low, true), (high, false)] {
        let mut out = Vec::new();
        for i in 0..vertices.len() {
            let a = vertices[i];
            let b = vertices[(i + 1) % vertices.len()];
            let inside = if lower {
                a[1] >= height
            } else {
                a[1] <= height
            };
            let next = if lower {
                b[1] >= height
            } else {
                b[1] <= height
            };
            if inside {
                out.push(a);
            }
            if inside != next {
                let t = (height - a[1]) / (b[1] - a[1]);
                out.push(std::array::from_fn(|j| a[j] + (b[j] - a[j]) * t));
            }
        }
        vertices = out;
    }
    vertices.into_iter().map(|v| [v[0], v[2]]).collect()
}
impl Scene {
    pub fn add_mesh(&mut self, triangles: Vec<[[f32; 3]; 3]>) -> Result<()> {
        self.insert_mesh(None, triangles)
    }
    /// Register a complete object batch atomically. Handles are supplied by the scene's
    /// allocation owner, never derived from authored IDs. An explicit empty batch is known empty.
    pub fn register_object_meshes(
        &mut self,
        object: crate::ObjectId,
        owner: Handle,
        nodes: Vec<(usize, Vec<[[f32; 3]; 3]>)>,
    ) -> Result<()> {
        if object.0 == 0
            || self.object_owners.contains_key(&object)
            || self.object_owners.values().any(|h| *h == owner)
            || self.owned_nodes.iter().any(|k| k.owner == owner)
        {
            return Err("Invalid/duplicate collision object registration".into());
        }
        // Prepare only the new batch; do not clone the growing scene per placement.
        let mut staged = Self::default();
        for (node, triangles) in nodes {
            staged.add_owned_mesh(ColliderKey { owner, node }, triangles)?;
        }
        self.meshes.extend(staged.meshes);
        self.index = std::sync::OnceLock::new();
        self.owned_nodes.append(&mut staged.owned_nodes);
        self.object_owners.insert(object, owner);
        Ok(())
    }
    /// Binding in this collision snapshot only; not general object liveness.
    pub fn collision_owner(&self, object: crate::ObjectId) -> Option<Handle> {
        self.object_owners.get(&object).copied()
    }
    /// Rust scene presence: keep geometry for later respawn, but exclude all queries.
    pub fn set_object_present(&mut self, object: crate::ObjectId, present: bool) -> Result<()> {
        let owner = self
            .collision_owner(object)
            .ok_or("Unknown collision object presence")?;
        if present && !self.walkable_owners.contains(&owner) {
            self.disabled_owners.remove(&owner);
        } else {
            self.disabled_owners.insert(owner);
        }
        Ok(())
    }
    pub fn set_object_walkable(&mut self, object: crate::ObjectId, walkable: bool) -> Result<()> {
        let owner = self
            .collision_owner(object)
            .ok_or("Unknown collision object walkable")?;
        if walkable {
            self.walkable_owners.insert(owner);
            self.disabled_owners.insert(owner);
        } else {
            self.walkable_owners.remove(&owner);
        }
        Ok(())
    }
    pub fn set_object_crushable(&mut self, object: crate::ObjectId, crushable: bool) -> Result<()> {
        let owner = self
            .collision_owner(object)
            .ok_or("Unknown collision object crushable")?;
        if crushable {
            self.crushable_owners.insert(owner);
        } else {
            self.crushable_owners.remove(&owner);
        }
        Ok(())
    }
    /// Records even an empty/degenerate node so absence of geometry is not unknown ownership.
    pub fn add_owned_mesh(
        &mut self,
        key: ColliderKey,
        triangles: Vec<[[f32; 3]; 3]>,
    ) -> Result<()> {
        if self.owned_nodes.contains(&key) {
            return Err("Duplicate owned collision node".into());
        }
        self.insert_mesh(Some(key), triangles)?;
        self.owned_nodes.insert(key);
        Ok(())
    }
    /// Replacement collision removal primitive, NOT native leftover-node selection.
    /// Caller must resolve the exact retained node identities; a count alone is insufficient.
    /// Validate the complete request before changing geometry. Anonymous meshes are untouched.
    /// Returns nonempty mesh batches removed, not the number of registered nodes.
    pub fn remove_owned_meshes(
        &mut self,
        owner: Handle,
        retained_nodes: &[usize],
    ) -> Result<usize> {
        let nodes: BTreeSet<_> = self
            .owned_nodes
            .iter()
            .filter(|k| k.owner == owner)
            .map(|k| k.node)
            .collect();
        if nodes.is_empty() && !self.object_owners.values().any(|h| *h == owner) {
            return Err("Unknown collision owner".into());
        }
        let retained: BTreeSet<_> = retained_nodes.iter().copied().collect();
        if retained.len() != retained_nodes.len() || !retained.is_subset(&nodes) {
            return Err("Invalid retained collision nodes".into());
        }
        let before = self.meshes.len();
        self.index = std::sync::OnceLock::new();
        self.meshes.retain(|mesh| {
            mesh.owner
                .is_none_or(|key| key.owner != owner || retained.contains(&key.node))
        });
        self.owned_nodes
            .retain(|key| key.owner != owner || retained.contains(&key.node));
        if retained.is_empty() {
            self.disabled_owners.remove(&owner);
            self.walkable_owners.remove(&owner);
            self.crushable_owners.remove(&owner);
            self.object_owners.retain(|_, h| *h != owner);
        }
        Ok(before - self.meshes.len())
    }
    pub fn has_owned_node(&self, key: ColliderKey) -> bool {
        self.owned_nodes.contains(&key)
    }

    fn insert_mesh(
        &mut self,
        owner: Option<ColliderKey>,
        mut triangles: Vec<[[f32; 3]; 3]>,
    ) -> Result<()> {
        if triangles.is_empty() {
            return Ok(());
        }
        if triangles.iter().flatten().flatten().any(|v| !v.is_finite()) {
            return Err("Nonfinite collision geometry".into());
        }
        // Degenerate GX strip entries draw no surface and must not become invisible blockers.
        triangles.retain(|t| {
            let a: [f64; 3] = std::array::from_fn(|i| t[1][i] as f64 - t[0][i] as f64);
            let b: [f64; 3] = std::array::from_fn(|i| t[2][i] as f64 - t[0][i] as f64);
            let n = [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ];
            n.iter().map(|v| v * v).sum::<f64>() > 1e-16
        });
        if triangles.is_empty() {
            return Ok(());
        }
        let min = std::array::from_fn(|i| {
            triangles
                .iter()
                .flatten()
                .map(|p| p[i])
                .fold(f32::INFINITY, f32::min)
        });
        let max = std::array::from_fn(|i| {
            triangles
                .iter()
                .flatten()
                .map(|p| p[i])
                .fold(f32::NEG_INFINITY, f32::max)
        });
        let grid = TriGrid::build(&triangles, min, max).map(Arc::new);
        self.index = std::sync::OnceLock::new();
        self.meshes.push(Mesh {
            owner,
            triangles: Arc::new(triangles),
            min,
            max,
            grid,
        });
        Ok(())
    }
    pub fn world_point(&self, position: [f32; 3], height: f32) -> Result<[f32; 3]> {
        if !height.is_finite() || position.iter().any(|v| !v.is_finite()) {
            return Err("Invalid world point/eye height".into());
        }
        Ok([
            position[0],
            self.floor(position).ok_or("Point outside terrain")? as f32 + height,
            position[2],
        ])
    }
    /// Raw terrain height without elevated bridge deck spans (used for water/shoreline checks).
    pub fn terrain_height(&self, x: f32, z: f32) -> Option<f32> {
        if !x.is_finite() || !z.is_finite() {
            return None;
        }
        if let Some(g) = &self.ground {
            let [gx, gz] = original_scene::source_to_grid(x, z);
            Some(g.height(gx, gz)? * 2.0)
        } else {
            Some(0.0)
        }
    }
    /// Two-sided scene ray plus sampled terrain occlusion; actors are queried by combat separately.
    pub fn line_clear(&self, from: [f32; 3], to: [f32; 3]) -> Result<bool> {
        Ok(self.first_hit(from, to)?.is_none())
    }
    /// Closest static/terrain contact; fraction is along the finite segment.
    pub fn first_hit(
        &self,
        from: [f32; 3],
        to: [f32; 3],
    ) -> Result<Option<(f64, Option<crate::ObjectId>)>> {
        self.first_hit_options(from, to, true)
    }
    /// Closest static/terrain contact with optional terrain heightmap occlusion.
    pub fn first_hit_options(
        &self,
        from: [f32; 3],
        to: [f32; 3],
        include_terrain: bool,
    ) -> Result<Option<(f64, Option<crate::ObjectId>)>> {
        self.first_hit_filtered(from, to, include_terrain, 0.0, 0.0, |_, _| false)
    }
    /// Closest static/terrain contact skipping ignored owners and optional endpoint terrain margins.
    pub fn first_hit_filtered(
        &self,
        from: [f32; 3],
        to: [f32; 3],
        include_terrain: bool,
        terrain_margin_start: f64,
        terrain_margin_end: f64,
        mut ignore_owner: impl FnMut(f64, Option<crate::ObjectId>) -> bool,
    ) -> Result<Option<(f64, Option<crate::ObjectId>)>> {
        let mut nearest: Option<(f64, Option<crate::ObjectId>)> = None;
        if from.iter().chain(to.iter()).any(|v| !v.is_finite()) {
            return Err("Invalid sight segment".into());
        }
        fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
            [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ]
        }
        fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
            (0..3).map(|i| a[i] * b[i]).sum()
        }
        let origin = from.map(f64::from);
        let delta: [f64; 3] = std::array::from_fn(|i| to[i] as f64 - origin[i]);
        for mesh in &self.meshes {
            if mesh
                .owner
                .is_some_and(|key| self.disabled_owners.contains(&key.owner))
            {
                continue;
            }
            if (0..3).any(|i| from[i].min(to[i]) > mesh.max[i] || from[i].max(to[i]) < mesh.min[i])
            {
                continue;
            }
            let mut resolved_owner: Option<Option<crate::ObjectId>> = None;
            for t in mesh.triangles.iter() {
                let e1 = std::array::from_fn(|i| t[1][i] as f64 - t[0][i] as f64);
                let e2 = std::array::from_fn(|i| t[2][i] as f64 - t[0][i] as f64);
                let p = cross(delta, e2);
                let det = dot3(e1, p);
                if det.abs() < 1e-10 {
                    continue;
                }
                let s = std::array::from_fn(|i| origin[i] - t[0][i] as f64);
                let u = dot3(s, p) / det;
                if !(0.0..=1.0).contains(&u) {
                    continue;
                }
                let q = cross(s, e1);
                let v = dot3(delta, q) / det;
                if v < 0.0 || u + v > 1.0 {
                    continue;
                }
                let fraction = dot3(e2, q) / det;
                if fraction > 1e-5
                    && fraction < 1.0 - 1e-5
                    && nearest.is_none_or(|hit| fraction < hit.0)
                {
                    let owner = *resolved_owner.get_or_insert_with(|| {
                        mesh.owner.and_then(|key| {
                            self.object_owners
                                .iter()
                                .find_map(|(id, handle)| (*handle == key.owner).then_some(*id))
                        })
                    });
                    if !ignore_owner(fraction, owner) {
                        nearest = Some((fraction, owner));
                    }
                }
            }
        }
        if include_terrain && let Some(g) = &self.ground {
            let length = dot3(delta, delta).sqrt();
            let count = (length / 0.75).ceil().max(1.0);
            if count > 4096.0 {
                return Err("Sight query budget exceeded".into());
            }
            for i in 1..count as usize {
                let t = i as f64 / count;
                if nearest.is_some_and(|hit| t >= hit.0) {
                    break;
                }
                let dist = t * length;
                if dist <= terrain_margin_start || (length - dist) <= terrain_margin_end {
                    continue;
                }
                let p: [f32; 3] = std::array::from_fn(|j| (origin[j] + delta[j] * t) as f32);
                let [x, z] = original_scene::source_to_grid(p[0], p[2]);
                if g.height(x, z).is_none_or(|h| p[1] < h * 2.0 + 0.05) && !ignore_owner(t, None) {
                    nearest = Some((t, None));
                    break;
                }
            }
        }
        Ok(nearest)
    }
    pub fn object_aabb(&self, id: crate::ObjectId) -> Option<([f32; 3], [f32; 3])> {
        let owner = *self.object_owners.get(&id)?;
        let mut low = [f32::INFINITY; 3];
        let mut high = [f32::NEG_INFINITY; 3];
        let mut found = false;
        for mesh in &self.meshes {
            if mesh.owner.is_some_and(|key| key.owner == owner) {
                found = true;
                for i in 0..3 {
                    low[i] = low[i].min(mesh.min[i]);
                    high[i] = high[i].max(mesh.max[i]);
                }
            }
        }
        found.then_some((low, high))
    }
    pub fn object_center(&self, id: crate::ObjectId) -> Option<[f32; 3]> {
        let owner = *self.object_owners.get(&id)?;
        if self.disabled_owners.contains(&owner) {
            return None;
        }
        let (low, high) = self.object_aabb(id)?;
        Some(std::array::from_fn(|i| (low[i] + high[i]) * 0.5))
    }
    pub fn mesh_count(&self) -> usize {
        self.meshes.len()
    }
    pub fn registered_object_count(&self) -> usize {
        self.object_owners.len()
    }
    pub fn owned_node_count(&self) -> usize {
        self.owned_nodes.len()
    }
    fn floor(&self, p: [f32; 3]) -> Option<f64> {
        if p.iter().any(|v| !v.is_finite()) {
            return None;
        }
        if let Some(g) = &self.ground {
            let [x, z] = original_scene::source_to_grid(p[0], p[2]);
            let mut base = g.height(x, z)? as f64 * 2.0;
            for span in &self.bridge_spans {
                let pu = p[0] * span.ux + p[2] * span.uz;
                let pv = -p[0] * span.uz + p[2] * span.ux;
                if pu >= span.min_u && pu <= span.max_u && pv >= span.min_v && pv <= span.max_v {
                    let t = ((pu - span.min_u) / (span.max_u - span.min_u).max(1e-3))
                        .clamp(0.0, 1.0) as f64;
                    base = base.max(span.y0 + (span.y1 - span.y0) * t);
                }
            }
            Some(base + p[1] as f64)
        } else {
            Some(p[1] as f64)
        }
    }
    fn polygons(
        &self,
        p: V2,
        d: V2,
        body: Body,
        low: f64,
        high: f64,
        mut visit: impl FnMut(&[V2]),
    ) {
        let end = add(p, d);
        if self.bridge_spans.iter().any(|span| {
            [p, end].into_iter().all(|pt| {
                let pu = pt[0] as f32 * span.ux + pt[1] as f32 * span.uz;
                let pv = -pt[0] as f32 * span.uz + pt[1] as f32 * span.ux;
                pu >= span.min_u - 4.0
                    && pu <= span.max_u + 4.0
                    && pv >= span.min_v - 0.5
                    && pv <= span.max_v + 0.5
            })
        }) {
            return;
        }
        let xmin = p[0].min(end[0]) - body.radius;
        let xmax = p[0].max(end[0]) + body.radius;
        let zmin = p[1].min(end[1]) - body.radius;
        let zmax = p[1].max(end[1]) + body.radius;
        let index = self
            .index
            .get_or_init(|| Arc::new(MeshIndex::build(&self.meshes)));
        let mut candidates = MESH_CANDIDATES.with(|c| std::mem::take(&mut *c.borrow_mut()));
        index.query(xmin, xmax, zmin, zmax, &mut candidates);
        let vehicle = body.radius > Body::INFANTRY.radius;
        for mesh in candidates.iter().map(|&i| &self.meshes[i as usize]) {
            if mesh.owner.is_some_and(|key| {
                self.disabled_owners.contains(&key.owner)
                    || (vehicle && self.crushable_owners.contains(&key.owner))
            }) {
                continue;
            }
            if (mesh.max[0] as f64) < xmin
                || (mesh.min[0] as f64) > xmax
                || (mesh.max[2] as f64) < zmin
                || (mesh.min[2] as f64) > zmax
                || (mesh.max[1] as f64) < low
                || (mesh.min[1] as f64) > high
            {
                continue;
            }
            let mut test = |tri: &[[f32; 3]; 3]| {
                if (0..3).any(|axis| {
                    let (min, max) = match axis {
                        0 => (xmin, xmax),
                        1 => (low, high),
                        _ => (zmin, zmax),
                    };
                    tri.iter().all(|v| (v[axis] as f64) < min)
                        || tri.iter().all(|v| (v[axis] as f64) > max)
                }) {
                    return;
                }
                let poly = clipped(*tri, low, high);
                if !poly.is_empty() {
                    visit(&poly);
                }
            };
            if let Some(grid) = &mesh.grid {
                let mut c = CANDIDATES.with(|c| std::mem::take(&mut *c.borrow_mut()));
                grid.query(xmin, xmax, zmin, zmax, &mut c);
                for &i in c.iter() {
                    test(&mesh.triangles[i as usize]);
                }
                CANDIDATES.with(|slot| *slot.borrow_mut() = c);
            } else {
                for tri in mesh.triangles.iter() {
                    test(tri);
                }
            }
        }
        MESH_CANDIDATES.with(|slot| *slot.borrow_mut() = candidates);
    }
    pub fn overlaps(&self, position: [f32; 3], body: Body, others: &[Other]) -> Result<bool> {
        body.validate()?;
        let foot = self
            .floor(position)
            .ok_or("Invalid or void collision position")?;
        let p = [position[0] as f64, position[2] as f64];
        let mut overlap = false;
        self.polygons(
            p,
            [0.0; 2],
            body,
            foot + body.step,
            foot + body.height,
            |poly| overlap |= polygon_overlap(p, poly, body.radius),
        );
        for other in others {
            other.body.validate()?;
            if other.position.iter().any(|v| !v.is_finite()) {
                return Err("Invalid other body coordinates".into());
            }
            if len(sub(p, [other.position[0] as f64, other.position[2] as f64]))
                >= body.radius + other.body.radius - 1e-5
            {
                continue;
            }
            let y = self
                .floor(other.position)
                .ok_or_else(|| format!("Invalid other body position {:?}", other.position))?;
            if foot < y + other.body.height
                && y < foot + body.height
                && len(sub(p, [other.position[0] as f64, other.position[2] as f64]))
                    < body.radius + other.body.radius - 1e-5
            {
                overlap = true;
            }
        }
        Ok(overlap)
    }
    pub fn move_body(
        &self,
        start: [f32; 3],
        delta: V2,
        body: Body,
        others: &[Other],
    ) -> Result<Movement> {
        if delta.iter().any(|v| !v.is_finite()) {
            return Err("Invalid collision displacement".into());
        }
        if self.overlaps(start, body, others)? {
            return Err("Initial unit penetration; placement must be resolved explicitly".into());
        }
        let mut foot = self.floor(start).unwrap();
        let mut p = [start[0] as f64, start[2] as f64];
        let mut d = delta;
        let mut blocked = false;
        let mut mesh_contacts = 0;
        let mut unit_contacts = 0;
        for _ in 0..4 {
            if len(d) < 1e-8 {
                break;
            }
            let end = add(p, d);
            let target = [end[0] as f32, start[1], end[1] as f32];
            let max_step = if self.ground.is_some() {
                (body.step * 2.5).max(len(d) * 3.0)
            } else {
                body.step
            };
            let Some(next_floor) = self.floor(target) else {
                blocked = true;
                break;
            };
            if (next_floor - foot).abs() > max_step {
                if self.ground.is_some() {
                    let tx = [(p[0] + d[0]) as f32, start[1], p[1] as f32];
                    let tz = [p[0] as f32, start[1], (p[1] + d[1]) as f32];
                    if d[0].abs() > 1e-6
                        && self
                            .floor(tx)
                            .is_some_and(|fx| (fx - foot).abs() <= max_step)
                    {
                        d = [d[0], 0.0];
                        blocked = true;
                        continue;
                    } else if d[1].abs() > 1e-6
                        && self
                            .floor(tz)
                            .is_some_and(|fz| (fz - foot).abs() <= max_step)
                    {
                        d = [0.0, d[1]];
                        blocked = true;
                        continue;
                    }
                }
                blocked = true;
                break;
            }
            let mut hit = None;
            self.polygons(
                p,
                d,
                body,
                foot.min(next_floor) + body.step,
                foot.max(next_floor) + body.height,
                |poly| {
                    for i in 0..poly.len() {
                        choose(
                            &mut hit,
                            edge(p, d, poly[i], poly[(i + 1) % poly.len()], body.radius),
                        );
                    }
                },
            );
            for other in others {
                let candidate = circle(
                    p,
                    d,
                    [other.position[0] as f64, other.position[2] as f64],
                    body.radius + other.body.radius,
                );
                if candidate.is_none() {
                    continue;
                }
                let y = self
                    .floor(other.position)
                    .ok_or_else(|| format!("Invalid other body position {:?}", other.position))?;
                if foot < y + other.body.height && y < foot + body.height {
                    choose(
                        &mut hit,
                        candidate.map(|mut h| {
                            h.unit = true;
                            h
                        }),
                    );
                }
            }
            let Some(h) = hit else {
                p = end;
                break;
            };
            blocked = true;
            if h.unit {
                unit_contacts += 1;
            } else {
                mesh_contacts += 1;
            }
            let fraction = (h.t - SKIN / len(d)).max(0.0);
            p = add(p, mul(d, fraction));
            if let Some(f) = self.floor([p[0] as f32, start[1], p[1] as f32]) {
                foot = f;
            }
            d = mul(d, 1.0 - fraction);
            let inward = dot(d, h.n);
            if inward < 0.0 {
                d = sub(d, mul(h.n, inward));
            }
        }
        let position = [p[0] as f32, start[1], p[1] as f32];
        if self.overlaps(position, body, others)? {
            return Ok(Movement {
                position: start,
                blocked: true,
                mesh_contacts,
                unit_contacts,
            });
        }
        Ok(Movement {
            position,
            blocked,
            mesh_contacts,
            unit_contacts,
        })
    }
    pub fn find_free(&self, preferred: [f32; 3], body: Body, others: &[Other]) -> Result<[f32; 3]> {
        self.find_free_near(preferred, body, others, 40.0)
    }
    /// Deterministic bounded spawn/depenetration search; does not cross arbitrary distances.
    pub fn find_free_near(
        &self,
        preferred: [f32; 3],
        body: Body,
        others: &[Other],
        radius: f64,
    ) -> Result<[f32; 3]> {
        body.validate()?;
        if preferred.iter().any(|v| !v.is_finite())
            || !radius.is_finite()
            || !(0.0..=40.0).contains(&radius)
        {
            return Err("Invalid placement search".into());
        }
        for ring in 0..=(radius * 4.0).floor() as usize {
            for i in 0..32 {
                let angle = i as f64 * std::f64::consts::TAU / 32.0;
                let p = [
                    preferred[0] + (angle.cos() * ring as f64 * 0.25) as f32,
                    preferred[1],
                    preferred[2] + (angle.sin() * ring as f64 * 0.25) as f32,
                ];
                if self.floor(p).is_some() && !self.overlaps(p, body, others)? {
                    return Ok(p);
                }
            }
        }
        Err("No collision-free diagnostic placement within search radius".into())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn wall() -> Scene {
        let mut s = Scene::default();
        s.add_mesh(vec![
            [[0.0, 0.0, -20.0], [0.0, 5.0, -20.0], [0.0, 5.0, 20.0]],
            [[0.0, 0.0, -20.0], [0.0, 5.0, 20.0], [0.0, 0.0, 20.0]],
        ])
        .unwrap();
        s
    }
    #[test]
    fn triangle_grid_matches_linear_scan() {
        // A pseudo-random field of small triangles large enough to get a grid.
        let mut seed = 12345u32;
        let mut r = || {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            (seed >> 8) as f32 / (1 << 24) as f32
        };
        let mut tris = Vec::new();
        for _ in 0..400 {
            let c = [r() * 120.0 - 60.0, r() * 4.0, r() * 90.0 - 45.0];
            tris.push([
                c,
                [c[0] + r() * 6.0, c[1] + 2.0, c[2] + r() * 3.0],
                [c[0] - r() * 3.0, c[1] + 1.0, c[2] + r() * 7.0],
            ]);
        }
        let mut gridded = Scene::default();
        gridded.add_mesh(tris).unwrap();
        assert!(gridded.meshes[0].grid.is_some());
        let mut linear = gridded.clone();
        linear.meshes[0].grid = None;
        let body = Body::INFANTRY;
        for _ in 0..200 {
            let p = [(r() * 140.0 - 70.0) as f64, (r() * 110.0 - 55.0) as f64];
            let d = [(r() * 20.0 - 10.0) as f64, (r() * 20.0 - 10.0) as f64];
            let collect = |s: &Scene| {
                let mut out = Vec::new();
                s.polygons(p, d, body, 0.5, 2.5, |poly| out.push(poly.to_vec()));
                out
            };
            assert_eq!(collect(&gridded), collect(&linear));
        }
    }
    #[test]
    fn mesh_index_matches_per_mesh_scan() {
        let mut seed = 777u32;
        let mut r = || {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            (seed >> 8) as f32 / (1 << 24) as f32
        };
        let mut scene = Scene::default();
        let mut singles = Vec::new();
        for _ in 0..300 {
            let c = [r() * 600.0 - 300.0, r() * 3.0, r() * 600.0 - 300.0];
            let w = if r() < 0.05 { 400.0 } else { r() * 8.0 + 0.5 };
            let tri = vec![[
                c,
                [c[0] + w, c[1] + 2.0, c[2]],
                [c[0], c[1] + 1.0, c[2] + w],
            ]];
            scene.add_mesh(tri.clone()).unwrap();
            let mut single = Scene::default();
            single.add_mesh(tri).unwrap();
            singles.push(single);
        }
        let body = Body::INFANTRY;
        for _ in 0..400 {
            let p = [(r() * 640.0 - 320.0) as f64, (r() * 640.0 - 320.0) as f64];
            let d = [(r() * 40.0 - 20.0) as f64, (r() * 40.0 - 20.0) as f64];
            let mut got = Vec::new();
            scene.polygons(p, d, body, 0.5, 2.5, |poly| got.push(poly.to_vec()));
            let mut want = Vec::new();
            for s in &singles {
                s.polygons(p, d, body, 0.5, 2.5, |poly| want.push(poly.to_vec()));
            }
            assert_eq!(got, want);
        }
    }
    #[test]
    fn placement_correction_is_bounded_and_validated() {
        let scene = wall();
        let body = Body::INFANTRY;
        assert!(scene.find_free_near([0.; 3], body, &[], 0.0).is_err());
        let p = scene.find_free_near([0.; 3], body, &[], 1.0).unwrap();
        assert!(p[0].hypot(p[2]) <= 1.0);
        assert!(!scene.overlaps(p, body, &[]).unwrap());
        assert!(
            scene
                .find_free_near([f32::NAN, 0., 0.], body, &[], 1.0)
                .is_err()
        );
    }
    #[test]
    fn cover_rays_are_two_sided_and_validate_inputs() {
        let s = wall();
        assert!(!s.line_clear([-3.0, 1.5, 0.0], [3.0, 1.5, 0.0]).unwrap());
        assert!(!s.line_clear([3.0, 1.5, 0.0], [-3.0, 1.5, 0.0]).unwrap());
        assert!(s.line_clear([-3.0, 6.0, 0.0], [3.0, 6.0, 0.0]).unwrap());
        assert!(s.line_clear([-3.0, 1.5, 0.0], [-1.0, 1.5, 0.0]).unwrap());
        assert!(s.line_clear([f32::NAN, 0.0, 0.0], [0.0; 3]).is_err());
        assert!(s.world_point([f32::INFINITY, 0.0, 0.0], 1.0).is_err());
        assert_eq!(
            s.world_point([1.0, 2.0, 3.0], 1.5).unwrap(),
            [1.0, 3.5, 3.0]
        );
    }
    #[test]
    fn thin_wall_no_tunneling_and_slide() {
        let s = wall();
        let r = s
            .move_body([-5.0, 0.0, 0.0], [100.0, 3.0], Body::INFANTRY, &[])
            .unwrap();
        assert!(r.blocked);
        assert!(r.position[0] <= -0.6);
        assert!(r.position[2] > 2.9);
        assert!(!s.overlaps(r.position, Body::INFANTRY, &[]).unwrap());
    }
    #[test]
    fn bodies_stop_and_allow_separating_motion() {
        let s = Scene::default();
        let other = Other {
            position: [0.0, 0.0, 0.0],
            body: Body::INFANTRY,
        };
        let r = s
            .move_body([-5.0, 0.0, 0.0], [100.0, 0.0], Body::INFANTRY, &[other])
            .unwrap();
        assert!(r.position[0] < -1.19);
        let away = s
            .move_body(r.position, [-1.0, 0.0], Body::INFANTRY, &[other])
            .unwrap();
        assert!(away.position[0] < r.position[0]);
    }
    #[test]
    fn degenerate_geometry_is_not_an_invisible_blocker() {
        let mut s = Scene::default();
        s.add_mesh(vec![[[0.0; 3]; 3]]).unwrap();
        assert_eq!(s.mesh_count(), 0);
        assert!(s.add_mesh(vec![[[f32::NAN; 3]; 3]]).is_err());
    }
    #[test]
    fn touching_surface_does_not_tunnel() {
        let s = wall();
        let p = s
            .move_body([-0.599995, 0.0, 0.0], [100.0, 0.0], Body::INFANTRY, &[])
            .unwrap();
        assert!(p.position[0] < 0.0);
        let other = Other {
            position: [0.0; 3],
            body: Body::INFANTRY,
        };
        let p = Scene::default()
            .move_body(
                [-1.199995, 0.0, 0.0],
                [100.0, 0.0],
                Body::INFANTRY,
                &[other],
            )
            .unwrap();
        assert!(p.position[0] < 0.0);
    }
    #[test]
    fn corner_and_swept_angles_remain_outside() {
        let mut s = wall();
        s.add_mesh(vec![
            [[-20.0, 0.0, 0.0], [20.0, 0.0, 0.0], [20.0, 5.0, 0.0]],
            [[-20.0, 0.0, 0.0], [20.0, 5.0, 0.0], [-20.0, 5.0, 0.0]],
        ])
        .unwrap();
        for i in 1..40 {
            let p = s
                .move_body(
                    [-5.0, 0.0, -5.0],
                    [i as f64, 40.0 - i as f64],
                    Body::INFANTRY,
                    &[],
                )
                .unwrap();
            assert!(p.position[0] <= -0.59 && p.position[2] <= -0.59);
            assert!(!s.overlaps(p.position, Body::INFANTRY, &[]).unwrap());
        }
    }
    #[test]
    fn overlap_rejected_and_high_obstacle_ignored() {
        let s = wall();
        assert!(
            s.move_body([0.0, 0.0, 0.0], [1.0, 0.0], Body::INFANTRY, &[])
                .is_err()
        );
        let p = s
            .move_body([-5.0, 6.0, 0.0], [10.0, 0.0], Body::INFANTRY, &[])
            .unwrap();
        assert_eq!(p.position[0], 5.0);
        assert!(
            s.move_body([-5.0, 0.0, 0.0], [f64::NAN, 0.0], Body::INFANTRY, &[])
                .is_err()
        );
    }
}
