//! Render-derived static collision and kinematic ground vehicles for the approved rewrite.
//! No Ageia dynamics, wheel simulation, destructible-node parity or cluster expansion claim.
use crate::{
    ObjectGraph, ObjectId, Result,
    mission_destruction::Handle,
    mission_world::{Navigation, World},
    model, original_scene,
    unit_collision::{Body, Scene},
};
use bevy::prelude::{Mat4, Vec3};
use std::{collections::BTreeMap, num::NonZeroU64, sync::Arc};
#[derive(Debug, serde::Serialize)]
pub struct Report {
    pub static_registrations: usize,
    pub omitted_placements: usize,
    pub kinematic_vehicles: usize,
    /// Same terrain clipping as the existing viewer; not silently registered empty.
    pub outside_terrain: Vec<ObjectId>,
}
pub fn configure(
    graph: &ObjectGraph,
    archive: &[u8],
    terrain: &crate::terrain::Terrain,
    engine: crate::vehicle_profile::Override,
    world: &mut World,
) -> Result<Report> {
    let runtime = world
        .rewrite
        .as_ref()
        .ok_or("Rewrite runtime must be configured first")?;
    let placements = original_scene::load(graph)?;
    let ground = Arc::new(crate::skirmish::Ground::from_terrain(terrain));
    let mut scene = Scene::default();
    scene.ground = Some(ground.clone());
    let mut models = BTreeMap::new();
    let mut model_bounds = BTreeMap::new();
    for placement in &placements.placements {
        if placement.kind == "cDestroyableObject" && !model_bounds.contains_key(&placement.model) {
            if !models.contains_key(&placement.model) {
                models.insert(
                    placement.model.clone(),
                    model::from_archive(archive, &placement.model)?,
                );
            }
            model_bounds.insert(
                placement.model.clone(),
                crate::model_geometry::hierarchy_bounds(&models[&placement.model])?,
            );
        }
    }
    let (resolved_matrices, outside_set) = placements.resolve_placements(&ground, &model_bounds)?;
    let mut staged = world.clone();
    // Dynamic positions remain untouched: Scene::floor adds terrain to their height offsets.
    let mut registrations = 0;
    let mut air_shapes = BTreeMap::new();
    let mut outside_terrain = Vec::new();
    let bridge_centers: Vec<[f32; 2]> = placements
        .placements
        .iter()
        .enumerate()
        .filter(|(_, p)| {
            if outside_set.contains(&p.id) {
                return false;
            }
            let u = p.model.to_ascii_uppercase();
            u.contains("BRIDGE")
                && !u.contains("C4SMBRIDGEIRON")
                && !u.contains("POST")
                && !u.contains("DEBRIS")
                && !u.contains("GATE")
        })
        .map(|(i, _)| {
            let root = Mat4::from_cols_array(&resolved_matrices[i]);
            [root.w_axis.x, root.w_axis.z]
        })
        .collect();
    let mut raw_bridge_boxes = Vec::new();
    for (index, placement) in placements.placements.iter().enumerate() {
        let root = Mat4::from_cols_array(&resolved_matrices[index]);
        if graph
            .objects
            .get(&placement.id)
            .is_some_and(|o| matches!(o.kind.as_str(), "cAirVehicle" | "cWaterVehicle"))
            // Effect-carrier dummy has a degenerate planar mesh, not an aircraft hull.
            && placement.model != "SFXdummyRoot"
        {
            if !models.contains_key(&placement.model) {
                models.insert(
                    placement.model.clone(),
                    model::from_archive(archive, &placement.model)?,
                );
            }
            let model = &models[&placement.model];
            let transforms = crate::model_geometry::bind_transforms(model)?;
            let scale = root.to_scale_rotation_translation().0;
            let mut min = Vec3::splat(f32::INFINITY);
            let mut max = Vec3::splat(f32::NEG_INFINITY);
            for (node, transform) in model.nodes.iter().zip(transforms) {
                if node.name.contains("NODRAW")
                    || node.name.contains("COLLIDE")
                    || node.name.contains("COLLISION")
                {
                    continue;
                }
                for surface in &node.surfaces {
                    for vertex in &surface.vertices {
                        let p =
                            transform.transform_point3(Vec3::from_array(vertex.position)) * scale;
                        min = min.min(p);
                        max = max.max(p);
                    }
                }
            }
            let shape = crate::mission_missile::AirShape {
                min: min.to_array(),
                max: max.to_array(),
                heading: root.z_axis.x.atan2(root.z_axis.z),
            };
            shape
                .validate()
                .map_err(|e| format!("{e}: {} {} {:?}", placement.id.0, placement.model, shape))?;
            air_shapes.insert(placement.id, shape);
        }
        if outside_set.contains(&placement.id) {
            outside_terrain.push(placement.id);
            continue;
        }
        if staged.units.contains_key(&placement.id) || runtime.retired.contains(&placement.id) {
            continue;
        }
        if !models.contains_key(&placement.model) {
            models.insert(
                placement.model.clone(),
                model::from_archive(archive, &placement.model)?,
            );
        }
        let model = &models[&placement.model];
        let transforms = crate::model_geometry::bind_transforms(model)?;
        let mut nodes = Vec::new();
        for (node, (source, transform)) in model.nodes.iter().zip(transforms).enumerate() {
            if placement.model == "C4SMIRONTOWER_H" {
                if !source.name.contains("NODRAW") {
                    continue;
                }
            } else if source.name.contains("NODRAW")
                || source.name.contains("COLLIDE")
                || source.name.contains("COLLISION")
            {
                continue;
            }
            let mut triangles = Vec::new();
            for surface in &source.surfaces {
                for indices in surface.indices.chunks_exact(3) {
                    let mut triangle = [[0.; 3]; 3];
                    for (point, index) in triangle.iter_mut().zip(indices) {
                        let vertex = surface
                            .vertices
                            .get(*index as usize)
                            .ok_or("Invalid collision vertex")?;
                        *point = (root * transform)
                            .transform_point3(Vec3::from_array(vertex.position))
                            .to_array();
                    }
                    triangles.push(triangle);
                }
            }
            nodes.push((node, triangles));
        }
        let upper = placement.model.to_ascii_uppercase();
        if upper.contains("BRIDGE")
            && !upper.contains("C4SMBRIDGEIRON")
            && !upper.contains("DMGD")
            && !upper.contains("POST")
            && !upper.contains("DEBRIS")
            && !upper.contains("GATE")
        {
            let mut ux = root.z_axis.x;
            let mut uz = root.z_axis.z;
            let ul = ux.hypot(uz);
            if ul > 1e-4 {
                ux /= ul;
                uz /= ul;
            } else {
                ux = 0.0;
                uz = 1.0;
            }
            let mut min_u = f32::INFINITY;
            let mut max_u = f32::NEG_INFINITY;
            let mut min_v = f32::INFINITY;
            let mut max_v = f32::NEG_INFINITY;
            for (_, tris) in &nodes {
                for tri in tris {
                    for pt in tri {
                        let pu = pt[0] * ux + pt[2] * uz;
                        let pv = -pt[0] * uz + pt[2] * ux;
                        min_u = min_u.min(pu);
                        max_u = max_u.max(pu);
                        min_v = min_v.min(pv);
                        max_v = max_v.max(pv);
                    }
                }
            }
            if min_u <= max_u && min_v <= max_v {
                let corners = vec![
                    [ux * min_u - uz * min_v, uz * min_u + ux * min_v],
                    [ux * min_u - uz * max_v, uz * min_u + ux * max_v],
                    [ux * max_u - uz * min_v, uz * max_u + ux * min_v],
                    [ux * max_u - uz * max_v, uz * max_u + ux * max_v],
                ];
                raw_bridge_boxes.push((vec![[ux, uz]], corners));
            }
        }
        scene.register_object_meshes(
            placement.id,
            Handle(NonZeroU64::new(index as u64 + 1).unwrap()),
            nodes,
        )?;
        let bridge_adjacent_wall = upper.contains("WALLRIVER")
            && if let Some((low, high)) = scene.object_aabb(placement.id) {
                bridge_centers.iter().any(|b| {
                    b[0] >= low[0] - 8.0
                        && b[0] <= high[0] + 8.0
                        && b[1] >= low[2] - 12.0
                        && b[1] <= high[2] + 12.0
                })
            } else {
                bridge_centers
                    .iter()
                    .any(|b| (root.w_axis.x - b[0]).hypot(root.w_axis.z - b[1]) <= 28.0)
            };
        let walkable_span = upper.contains("BRIDGE")
            || upper.contains("RAMP")
            || upper.contains("STAIR")
            || upper.contains("HELIPAD")
            || upper.contains("DOCKS01")
            || upper.contains("JETTY")
            || upper.contains("RUINVINES")
            || upper.contains("RUINEDROAD")
            || upper.contains("RUBBLE")
            || upper.contains("CLIFFCLAD")
            || upper.contains("CLIFFARCH")
            || upper.contains("CLIFFLEDGE")
            || upper.contains("CLIFFWATERARCH")
            || upper.contains("MPB_")
            || upper.contains("ENTRANCE")
            || upper.contains("ICEARCH")
            || upper.contains("WALLVEHICLE")
            || bridge_adjacent_wall
            || matches!(placement.id.0, 50022415 | 50022417);
        if walkable_span {
            scene.set_object_walkable(placement.id, true)?;
        }
        if staged
            .props
            .get(&placement.id)
            .is_some_and(|u| u.active == Some(false))
            || walkable_span
        {
            scene.set_object_present(placement.id, false)?;
        }
        if staged.crushable_props.contains(&placement.id) {
            scene.set_object_crushable(placement.id, true)?;
        }
        registrations += 1;
    }
    let mut merged = true;
    while merged {
        merged = false;
        let mut i = 0;
        while i < raw_bridge_boxes.len() {
            let mut j = i + 1;
            while j < raw_bridge_boxes.len() {
                let a_axis = raw_bridge_boxes[i].0[0];
                let b_axis = raw_bridge_boxes[j].0[0];
                let sep_axes = [
                    a_axis,
                    [-a_axis[1], a_axis[0]],
                    b_axis,
                    [-b_axis[1], b_axis[0]],
                ];
                let overlaps = sep_axes.iter().all(|&[ax, az]| {
                    let (mut a0, mut a1) = (f32::INFINITY, f32::NEG_INFINITY);
                    for p in &raw_bridge_boxes[i].1 {
                        let proj = p[0] * ax + p[1] * az;
                        a0 = a0.min(proj);
                        a1 = a1.max(proj);
                    }
                    let (mut b0, mut b1) = (f32::INFINITY, f32::NEG_INFINITY);
                    for p in &raw_bridge_boxes[j].1 {
                        let proj = p[0] * ax + p[1] * az;
                        b0 = b0.min(proj);
                        b1 = b1.max(proj);
                    }
                    a0 - 6.0 <= b1 && b0 - 6.0 <= a1
                });
                if overlaps {
                    let (b_axes, b_corners) = raw_bridge_boxes.remove(j);
                    raw_bridge_boxes[i].0.extend(b_axes);
                    raw_bridge_boxes[i].1.extend(b_corners);
                    merged = true;
                } else {
                    j += 1;
                }
            }
            i += 1;
        }
    }
    for (mut axes, corners) in raw_bridge_boxes {
        axes.push([1.0, 0.0]);
        let mut best: Option<(f32, f32, f32, f32, f32, f32, f32)> = None;
        for [ax, az] in axes {
            let (mut min_u, mut max_u) = (f32::INFINITY, f32::NEG_INFINITY);
            let (mut min_v, mut max_v) = (f32::INFINITY, f32::NEG_INFINITY);
            for p in &corners {
                let pu = p[0] * ax + p[1] * az;
                let pv = -p[0] * az + p[1] * ax;
                min_u = min_u.min(pu);
                max_u = max_u.max(pu);
                min_v = min_v.min(pv);
                max_v = max_v.max(pv);
            }
            let du = (max_u - min_u).max(1e-3);
            let dv = (max_v - min_v).max(1e-3);
            let area = du * dv;
            let cand = if du >= dv {
                (area, ax, az, min_u, max_u, min_v, max_v)
            } else {
                // Rotate 90 degrees so u is the longer span axis and v is the width.
                let ux = -az;
                let uz = ax;
                let (mut u0, mut u1) = (f32::INFINITY, f32::NEG_INFINITY);
                let (mut v0, mut v1) = (f32::INFINITY, f32::NEG_INFINITY);
                for p in &corners {
                    let pu = p[0] * ux + p[1] * uz;
                    let pv = -p[0] * uz + p[1] * ux;
                    u0 = u0.min(pu);
                    u1 = u1.max(pu);
                    v0 = v0.min(pv);
                    v1 = v1.max(pv);
                }
                (area, ux, uz, u0, u1, v0, v1)
            };
            if best.is_none_or(|b| cand.0 < b.0) {
                best = Some(cand);
            }
        }
        if let Some((_, ux, uz, mut min_u, mut max_u, min_v, max_v)) = best {
            min_u -= 2.0;
            max_u += 2.0;
            let mid_v = (min_v + max_v) * 0.5;
            let p0 = [ux * min_u - uz * mid_v, 0.0, uz * min_u + ux * mid_v];
            let p1 = [ux * max_u - uz * mid_v, 0.0, uz * max_u + ux * mid_v];
            let y0 = scene
                .world_point(p0, 0.0)
                .map(|p| p[1] as f64)
                .unwrap_or(0.0);
            let y1 = scene
                .world_point(p1, 0.0)
                .map(|p| p[1] as f64)
                .unwrap_or(0.0);
            scene.bridge_spans.push(crate::unit_collision::BridgeSpan {
                ux,
                uz,
                min_u,
                max_u,
                min_v,
                max_v,
                y0,
                y1,
            });
        }
    }
    let vehicles = crate::vehicle_profile::Bank::load(graph, engine)?;
    for (id, binding) in &vehicles.units {
        if !models.contains_key(&binding.model) {
            models.insert(
                binding.model.clone(),
                model::from_archive(archive, &binding.model)?,
            );
        }
        let groups = crate::vehicle_boxes::groups(&models[&binding.model])?;
        if groups.chassis.is_empty() {
            return Err("Vehicle missing chassis boxes".into());
        }
        let radius = groups
            .chassis
            .iter()
            .map(|b| (b.position[0].abs() + b.dimensions[0] * 0.5) as f64)
            .fold(0.0_f64, f64::max)
            .max(1.4);
        let height = groups
            .chassis
            .iter()
            .chain(&groups.turret)
            .map(|b| (b.position[1] + b.dimensions[1] * 0.5) as f64)
            .fold(0.0_f64, f64::max);
        let body = Body {
            radius,
            height,
            step: 0.6_f64.min(height * 0.45).max(0.25),
        };
        body.validate()?;
        staged.bodies.insert(*id, body);
        staged
            .units
            .get_mut(id)
            .ok_or("Missing ground vehicle state")?
            .forward_speed = Some(
            binding
                .profile
                .attack_speed
                .max(binding.profile.patrol_speed),
        );
    }
    staged.air_shapes = Arc::new(air_shapes);
    staged.collision = Some(Arc::new(scene));
    staged.navigation = Navigation::PlannedInfantry; // existing cylinder planner, now also kinematic chassis cylinders
    *world = staged;
    Ok(Report {
        static_registrations: registrations,
        omitted_placements: placements.omitted.len(),
        kinematic_vehicles: vehicles.units.len(),
        outside_terrain,
    })
}
