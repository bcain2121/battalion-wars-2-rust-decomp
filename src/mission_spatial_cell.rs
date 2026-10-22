//! Ordered cell traversal recovered from 0x8032b460..0x8032b770.
//! This is not the grid dispatcher, a collision-registration builder, or a Kill backend.
use crate::Result;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sphere {
    pub center: [f32; 3],
    pub radius: f32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min: [f32; 2],
    pub max: [f32; 2],
}
#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    /// Opaque record identity, not an authored object ID. Duplicates are retained.
    pub identity: u64,
    pub mask: u32,
    pub sphere: Sphere,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    /// Live aggregate mask; deriving this from XML flags is not supported here.
    pub mask: u32,
    pub contents: Contents,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Contents {
    Leaf(Vec<Record>),
    /// Native child order: low-x/low-z, high-x/low-z, low-x/high-z, high-x/high-z.
    Branch(Box<[Cell; 4]>),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Visit {
    pub callbacks: u32,
    pub stopped: bool,
}
impl Visit {
    pub fn native_result(self) -> u32 {
        if self.stopped {
            !self.callbacks
        } else {
            self.callbacks
        }
    }
}
fn sphere_valid(s: Sphere) -> bool {
    s.center.into_iter().all(f32::is_finite) && s.radius.is_finite() && s.radius >= 0.0
}
fn bounds_valid(b: Bounds) -> bool {
    b.min.into_iter().chain(b.max).all(f32::is_finite)
        && b.min[0] <= b.max[0]
        && b.min[1] <= b.max[1]
}
fn overlaps(a: Bounds, b: Bounds) -> bool {
    a.min[0] <= b.max[0] && a.min[1] <= b.max[1] && a.max[0] >= b.min[0] && a.max[1] >= b.min[1]
}
/// Native paired-single leaf test: rounded x²+y² followed by fused z*z+sum.
/// The CR0 LT bit is tested: tangency is NOT an intersection.
fn intersects(a: Sphere, b: Sphere) -> Result<bool> {
    if !sphere_valid(b) {
        return Err("Invalid spatial record sphere".into());
    }
    let x = b.center[0] - a.center[0];
    let y = b.center[1] - a.center[1];
    let z = b.center[2] - a.center[2];
    let xy = x * x + y * y;
    let distance = z.mul_add(z, xy);
    let radius = b.radius + a.radius;
    let squared = radius * radius;
    if !distance.is_finite() || !squared.is_finite() {
        return Err("Spatial sphere arithmetic overflow".into());
    }
    Ok(distance < squared)
}

/// The callback returns false to stop, including that callback in the native count.
/// Callers must transactionally enclose callback mutations/errors. Traversal itself
/// only reads the supplied tree. Bounds/depth/work limits are supported-subset guards,
/// not claims about native limits. No deduplication or sorting is performed.
pub fn visit(
    cell: &Cell,
    bounds: Bounds,
    query: Sphere,
    maximum_record_radius: f32,
    mask: u32,
    mut callback: impl FnMut(&Record) -> Result<bool>,
) -> Result<Visit> {
    if !sphere_valid(query)
        || !bounds_valid(bounds)
        || !maximum_record_radius.is_finite()
        || maximum_record_radius < 0.0
    {
        return Err("Invalid spatial cell query".into());
    }
    let expanded = query.radius + maximum_record_radius;
    let query_bounds = Bounds {
        min: [query.center[0] - expanded, query.center[2] - expanded],
        max: [query.center[0] + expanded, query.center[2] + expanded],
    };
    if !bounds_valid(query_bounds) {
        return Err("Spatial query expansion overflow".into());
    }
    let mut state = Walker {
        query,
        query_bounds,
        mask,
        work: 0,
        result: Visit {
            callbacks: 0,
            stopped: false,
        },
        callback: &mut callback,
    };
    state.cell(cell, bounds, 0)?;
    Ok(state.result)
}
struct Walker<'a, F> {
    query: Sphere,
    query_bounds: Bounds,
    mask: u32,
    work: usize,
    result: Visit,
    callback: &'a mut F,
}
impl<F: FnMut(&Record) -> Result<bool>> Walker<'_, F> {
    fn budget(&mut self) -> Result<()> {
        self.work += 1;
        if self.work > 1_000_000 {
            return Err("Spatial cell work bound exceeded".into());
        }
        Ok(())
    }
    fn cell(&mut self, cell: &Cell, bounds: Bounds, depth: usize) -> Result<()> {
        self.budget()?;
        if cell.mask & self.mask == 0 {
            return Ok(());
        }
        if depth > 32 {
            return Err("Spatial cell depth bound exceeded".into());
        }
        match &cell.contents {
            Contents::Leaf(records) => {
                // Native count is a nonnegative signed byte; negative means branch.
                if records.len() > 127 {
                    return Err("Spatial leaf count exceeds signed byte".into());
                }
                for record in records {
                    self.budget()?;
                    if record.mask & self.mask == 0 {
                        continue;
                    }
                    if !intersects(self.query, record.sphere)? {
                        continue;
                    }
                    self.result.callbacks += 1;
                    if !(self.callback)(record)? {
                        self.result.stopped = true;
                        break;
                    }
                }
            }
            Contents::Branch(children) => {
                let middle = [
                    (bounds.min[0] + bounds.max[0]) * 0.5,
                    (bounds.min[1] + bounds.max[1]) * 0.5,
                ];
                if !middle.into_iter().all(f32::is_finite) {
                    return Err("Spatial midpoint overflow".into());
                }
                let boxes = [
                    Bounds {
                        min: bounds.min,
                        max: middle,
                    },
                    Bounds {
                        min: [middle[0], bounds.min[1]],
                        max: [bounds.max[0], middle[1]],
                    },
                    Bounds {
                        min: [bounds.min[0], middle[1]],
                        max: [middle[0], bounds.max[1]],
                    },
                    Bounds {
                        min: middle,
                        max: bounds.max,
                    },
                ];
                for (child, child_bounds) in children.iter().zip(boxes) {
                    if overlaps(self.query_bounds, child_bounds) {
                        self.cell(child, child_bounds, depth + 1)?;
                        if self.result.stopped {
                            break;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
