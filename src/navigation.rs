//! Bounded replacement grid A*. Edges are tested with the live infantry collision solver.
//! Not an interpretation of the original game's navigation data.
use crate::{
    Result,
    unit_collision::{Body, Other, Scene},
};
use std::{
    cmp::{Ordering, Reverse},
    collections::{BTreeMap, BinaryHeap, VecDeque},
};
type Point = [f64; 2];
type Key = (i32, i32);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Found,
    NoRoute,
    Budget,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub status: Status,
    pub points: Vec<Point>,
    pub expanded: usize,
    pub probes: usize,
}
#[derive(Clone, Copy)]
pub struct Limits {
    pub expansions: usize,
    pub probes: usize,
    pub extent: i32,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            expansions: 256,
            probes: 8192,
            extent: 64,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Route {
    pub target: Point,
    pub radius: f64,
    pub points: VecDeque<Point>,
    pub pending: bool,
    pub retry: f64,
    pub planned: bool,
}
impl Route {
    pub fn new(target: Point, radius: f64) -> Self {
        Self {
            target,
            radius,
            points: VecDeque::new(),
            pending: true,
            retry: 0.0,
            planned: false,
        }
    }
}
#[derive(Clone, Copy, PartialEq)]
struct Entry {
    f: f64,
    g: f64,
    key: Key,
}
impl Eq for Entry {}
impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.f
            .total_cmp(&other.f)
            .then_with(|| self.g.total_cmp(&other.g))
            .then_with(|| self.key.cmp(&other.key))
    }
}
impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
fn distance(a: Point, b: Point) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
/// None means the shared probe budget was exhausted. False means collision/terrain blocked the edge.
fn trace(
    scene: &Scene,
    start: [f32; 3],
    goal: Point,
    body: Body,
    others: &[Other],
    probes: &mut usize,
    limit: usize,
) -> Result<Option<bool>> {
    let origin = [start[0] as f64, start[2] as f64];
    let length = distance(origin, goal);
    let step_size = if scene.ground.is_some() {
        (body.radius * 0.5).max(1.0)
    } else {
        body.radius * 0.25
    };
    let steps = (length / step_size).ceil().max(1.0);
    if !steps.is_finite() || steps > limit.saturating_sub(*probes) as f64 {
        return Ok(None);
    }
    let mut position = start;
    for i in 1..=steps as usize {
        let fraction = i as f64 / steps;
        let target = [
            origin[0] + (goal[0] - origin[0]) * fraction,
            origin[1] + (goal[1] - origin[1]) * fraction,
        ];
        *probes += 1;
        let movement = scene.move_body(
            position,
            [
                target[0] - position[0] as f64,
                target[1] - position[2] as f64,
            ],
            body,
            others,
        )?;
        if distance(
            [movement.position[0] as f64, movement.position[2] as f64],
            target,
        ) > 0.001
        {
            return Ok(Some(false));
        }
        position = movement.position;
    }
    if scene.overlaps([goal[0] as f32, start[1], goal[1] as f32], body, others)? {
        return Ok(Some(false));
    }
    Ok(Some(true))
}
pub fn plan(
    scene: &Scene,
    start: [f32; 3],
    target: Point,
    radius: f64,
    body: Body,
    others: &[Other],
    limits: Limits,
) -> Result<Plan> {
    plan_inner(scene, start, target, radius, body, others, limits, false, None)
}

pub fn plan_towards(
    scene: &Scene,
    start: [f32; 3],
    target: Point,
    radius: f64,
    body: Body,
    others: &[Other],
    limits: Limits,
) -> Result<Plan> {
    let primary = plan_inner(scene, start, target, radius, body, others, limits, true, None)?;
    if primary.status != Status::Found && scene.ground.is_some() {
        let fine = (body.radius * 2.0).clamp(1.5, 3.5);
        for retry_spacing in [fine, 8.0, 12.0] {
            if retry_spacing == fine && !primary.points.is_empty() {
                continue;
            }
            if let Ok(alt) = plan_inner(
                scene,
                start,
                target,
                radius,
                body,
                others,
                limits,
                true,
                Some(retry_spacing),
            ) {
                if alt.status == Status::Found
                    || (primary.points.is_empty() && !alt.points.is_empty())
                {
                    return Ok(alt);
                }
            }
        }
    }
    Ok(primary)
}

fn reconstruct_key(
    mut key: Key,
    parents: &BTreeMap<Key, Key>,
    at: impl Fn(Key) -> Point,
) -> Vec<Point> {
    let mut points = Vec::new();
    while key != (0, 0) {
        points.push(at(key));
        key = parents[&key];
    }
    points.reverse();
    points.dedup();
    points
}

#[allow(clippy::too_many_arguments)]
fn plan_inner(
    scene: &Scene,
    start: [f32; 3],
    target: Point,
    radius: f64,
    body: Body,
    others: &[Other],
    limits: Limits,
    allow_partial: bool,
    spacing_override: Option<f64>,
) -> Result<Plan> {
    body.validate()?;
    if target
        .iter()
        .any(|v| !v.is_finite() || !(*v as f32).is_finite())
        || !radius.is_finite()
        || radius < 0.0
        || limits.extent <= 0
        || limits.extent > 1024
        || limits.expansions == 0
        || limits.probes == 0
    {
        return Err("Invalid route request/limits".into());
    }
    if scene.overlaps(start, body, others)? {
        return Err("Cannot plan from a penetrating body".into());
    }
    let origin = [start[0] as f64, start[2] as f64];
    let base_spacing = (body.radius * 2.0).clamp(1.5, 3.5);
    let dist_spacing = if allow_partial {
        (distance(origin, target) / 90.0).clamp(1.5, 6.0)
    } else {
        base_spacing
    };
    let spacing = spacing_override.unwrap_or_else(|| base_spacing.max(dist_spacing));
    if !spacing.is_finite() {
        return Err("Unsupported navigation grid spacing".into());
    }
    let at = |k: Key| {
        [
            origin[0] + k.0 as f64 * spacing,
            origin[1] + k.1 as f64 * spacing,
        ]
    };
    let position = |p: Point| [p[0] as f32, start[1], p[1] as f32];
    let heuristic = |p: Point| (distance(p, target) - radius).max(0.0);
    let mut best_key = (0, 0);
    let mut best_h = heuristic(origin);
    let mut heap = BinaryHeap::from([Reverse(Entry {
        f: best_h,
        g: 0.0,
        key: (0, 0),
    })]);
    let mut costs = BTreeMap::from([((0, 0), 0.0)]);
    let mut parents = BTreeMap::new();
    let mut edges = BTreeMap::new();
    let mut expanded = 0;
    let mut probes = 0;
    let mut clipped = false;
    while let Some(Reverse(entry)) = heap.pop() {
        if entry.g > costs[&entry.key] + 1e-9 {
            continue;
        }
        if expanded >= limits.expansions {
            return Ok(Plan {
                status: Status::Budget,
                points: if allow_partial {
                    reconstruct_key(best_key, &parents, at)
                } else {
                    vec![]
                },
                expanded,
                probes,
            });
        }
        expanded += 1;
        let here = at(entry.key);
        let d = distance(here, target);
        let h = (d - radius).max(0.0);
        if h < best_h - 1e-6 {
            best_h = h;
            best_key = entry.key;
        }
        let goal = if d <= radius {
            here
        } else {
            let travel = d - radius;
            [
                here[0] + (target[0] - here[0]) * travel / d,
                here[1] + (target[1] - here[1]) * travel / d,
            ]
        };
        let reached = if d <= radius + 1e-6 {
            true
        } else if d - radius <= spacing * 2.0 {
            match trace(
                scene,
                position(here),
                goal,
                body,
                others,
                &mut probes,
                limits.probes,
            )? {
                Some(clear) => clear,
                None => {
                    return Ok(Plan {
                        status: Status::Budget,
                        points: if allow_partial {
                            reconstruct_key(best_key, &parents, at)
                        } else {
                            vec![]
                        },
                        expanded,
                        probes,
                    });
                }
            }
        } else {
            false
        };
        if reached {
            let mut points = vec![goal];
            let mut key = entry.key;
            while key != (0, 0) {
                points.push(at(key));
                key = parents[&key];
            }
            points.reverse();
            points.dedup();
            return Ok(Plan {
                status: Status::Found,
                points,
                expanded,
                probes,
            });
        }
        for (dx, dz) in [
            (-1, -1),
            (-1, 0),
            (-1, 1),
            (0, -1),
            (0, 1),
            (1, -1),
            (1, 0),
            (1, 1),
        ] {
            let key = (entry.key.0 + dx, entry.key.1 + dz);
            if key.0.abs() > limits.extent || key.1.abs() > limits.extent {
                clipped = true;
                continue;
            }
            let next = at(key);
            let cost = entry.g + distance(here, next);
            if costs.get(&key).is_some_and(|old| cost >= *old - 1e-9) {
                continue;
            }
            // Directed cache: height-change and collision tolerances need not be symmetric.
            let edge = (entry.key, key);
            let clear = if let Some(clear) = edges.get(&edge) {
                *clear
            } else {
                let Some(clear) = trace(
                    scene,
                    position(here),
                    next,
                    body,
                    others,
                    &mut probes,
                    limits.probes,
                )?
                else {
                    return Ok(Plan {
                        status: Status::Budget,
                        points: if allow_partial {
                            reconstruct_key(best_key, &parents, at)
                        } else {
                            vec![]
                        },
                        expanded,
                        probes,
                    });
                };
                edges.insert(edge, clear);
                clear
            };
            if clear {
                costs.insert(key, cost);
                parents.insert(key, entry.key);
                heap.push(Reverse(Entry {
                    f: cost + heuristic(next),
                    g: cost,
                    key,
                }));
            }
        }
    }
    Ok(Plan {
        status: if clipped {
            Status::Budget
        } else {
            Status::NoRoute
        },
        points: if allow_partial {
            reconstruct_key(best_key, &parents, at)
        } else {
            vec![]
        },
        expanded,
        probes,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn wall(scene: &mut Scene, x: f32, z0: f32, z1: f32) {
        scene
            .add_mesh(vec![
                [[x, 0.0, z0], [x, 4.0, z0], [x, 4.0, z1]],
                [[x, 0.0, z0], [x, 4.0, z1], [x, 0.0, z1]],
            ])
            .unwrap();
    }
    #[test]
    fn routes_around_wall_and_replays_clear_edges() {
        let mut scene = Scene::default();
        wall(&mut scene, 0.0, -2.0, 2.0);
        let start = [-4.0, 0.0, 0.0];
        let p = plan(
            &scene,
            start,
            [4.0, 0.0],
            0.1,
            Body::INFANTRY,
            &[],
            Limits::default(),
        )
        .unwrap();
        assert_eq!(p.status, Status::Found);
        assert!(p.points.iter().any(|p| p[1].abs() > 2.5));
        let mut pos = start;
        let mut probes = 0;
        for point in &p.points {
            assert_eq!(
                trace(&scene, pos, *point, Body::INFANTRY, &[], &mut probes, 10000).unwrap(),
                Some(true)
            );
            pos = [point[0] as f32, 0.0, point[1] as f32];
        }
        assert!(distance([pos[0] as f64, pos[2] as f64], [4.0, 0.0]) <= 0.101);
        assert_eq!(
            p,
            plan(
                &scene,
                start,
                [4.0, 0.0],
                0.1,
                Body::INFANTRY,
                &[],
                Limits::default()
            )
            .unwrap()
        );
    }
    #[test]
    fn routes_around_live_body() {
        let scene = Scene::default();
        let other = Other {
            position: [0.0; 3],
            body: Body::INFANTRY,
        };
        let p = plan(
            &scene,
            [-3.0, 0.0, 0.0],
            [3.0, 0.0],
            0.1,
            Body::INFANTRY,
            &[other],
            Limits::default(),
        )
        .unwrap();
        assert_eq!(p.status, Status::Found);
        assert!(p.points.iter().any(|p| p[1].abs() >= 1.5));
    }
    #[test]
    fn blocked_and_budget_outcomes_are_not_arrivals() {
        let mut scene = Scene::default();
        wall(&mut scene, 2.0, -10.0, 10.0);
        wall(&mut scene, -2.0, -10.0, 10.0);
        let limits = Limits {
            extent: 1,
            ..Limits::default()
        };
        let p = plan(
            &scene,
            [0.0; 3],
            [6.0, 0.0],
            0.1,
            Body::INFANTRY,
            &[],
            limits,
        )
        .unwrap();
        assert_ne!(p.status, Status::Found);
        assert!(p.points.is_empty());
        let p = plan(
            &scene,
            [0.0; 3],
            [6.0, 0.0],
            0.1,
            Body::INFANTRY,
            &[],
            Limits {
                probes: 1,
                ..Limits::default()
            },
        )
        .unwrap();
        assert_eq!(p.status, Status::Budget);
        assert!(p.probes <= 1);
    }
    #[test]
    fn enclosed_start_reports_no_route() {
        let mut scene = Scene::default();
        wall(&mut scene, -2.0, -2.0, 2.0);
        wall(&mut scene, 2.0, -2.0, 2.0);
        for z in [-2.0, 2.0] {
            scene
                .add_mesh(vec![
                    [[-2.0, 0.0, z], [2.0, 0.0, z], [2.0, 4.0, z]],
                    [[-2.0, 0.0, z], [2.0, 4.0, z], [-2.0, 4.0, z]],
                ])
                .unwrap();
        }
        let p = plan(
            &scene,
            [0.0; 3],
            [6.0, 0.0],
            0.1,
            Body::INFANTRY,
            &[],
            Limits::default(),
        )
        .unwrap();
        assert_eq!(p.status, Status::NoRoute);
        assert!(p.points.is_empty());
    }
    #[test]
    fn rejects_invalid_inputs() {
        assert!(
            plan(
                &Scene::default(),
                [0.0; 3],
                [f64::NAN, 0.0],
                0.0,
                Body::INFANTRY,
                &[],
                Limits::default()
            )
            .is_err()
        );
    }
}
