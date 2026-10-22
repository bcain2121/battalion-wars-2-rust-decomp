//! Authored objective-marker targets. Visibility stays owned by the original scripts.
use crate::{ObjectGraph, ObjectId, Result, mission_vm::Host, original_scene::field};
#[derive(Clone, Debug)]
pub struct Marker {
    pub id: ObjectId,
    pub target: ObjectId,
    pub parent: Option<ObjectId>,
    pub height: f32,
}
pub fn load(graph: &ObjectGraph) -> Result<Vec<Marker>> {
    graph
        .objects
        .values()
        .filter(|o| o.kind == "cObjectiveMarker")
        .map(|o| {
            let target = field(o, "mAttachedTo")
                .ok_or("Marker missing attachment")?
                .parse::<u64>()
                .map_err(|_| "Invalid marker attachment")?;
            let height = field(o, "mHeightAboveTarget")
                .ok_or("Marker missing height")?
                .parse::<f32>()
                .map_err(|_| "Invalid marker height")?;
            if !height.is_finite() {
                return Err("Nonfinite marker height".into());
            }
            let target = if target == 0 { o.id } else { ObjectId(target) };
            if !graph.objects.contains_key(&target) {
                return Err("Unknown marker attachment".into());
            }
            let parent = field(o, "mParent")
                .ok_or("Marker missing parent")?
                .parse::<u64>()
                .map_err(|_| "Invalid marker parent")?;
            let parent = (parent != 0).then_some(ObjectId(parent));
            if parent.is_some_and(|id| {
                graph
                    .objects
                    .get(&id)
                    .is_none_or(|p| p.kind != "cObjective")
            }) {
                return Err("Invalid objective parent".into());
            }
            Ok(Marker {
                id: o.id,
                target,
                parent,
                height,
            })
        })
        .collect()
}
impl Marker {
    pub fn effective_target(&self, host: &Host) -> ObjectId {
        host.world
            .objective_attachment
            .get(&self.id)
            .copied()
            .map(|t| if t.0 == 0 { self.id } else { t })
            .unwrap_or(self.target)
    }
    pub fn position(&self, host: &Host) -> Result<Option<[f32; 3]>> {
        let parent = self
            .parent
            .and_then(|id| host.world.objectives.get(&id))
            .copied();
        if parent.is_some_and(|(_, complete)| complete)
            || host.world.objective_visibility.get(&self.id) != Some(&Some(true))
        {
            return Ok(None);
        }
        let target = self.effective_target(host);
        let mut p = *host
            .positions
            .get(&target)
            .ok_or("Visible marker target missing position")?;
        p[1] += self.height;
        if p.iter().any(|x| !x.is_finite()) {
            return Err("Invalid marker position".into());
        }
        Ok(Some(p))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn visibility_attachment_and_movement_are_live() {
        let mut h = Host::default();
        let m = Marker {
            id: ObjectId(1),
            target: ObjectId(2),
            parent: Some(ObjectId(3)),
            height: 4.,
        };
        assert_eq!(m.position(&h).unwrap(), None);
        h.world.objective_visibility.insert(m.id, Some(true));
        assert!(m.position(&h).is_err());
        h.positions.insert(m.target, [10., 2., 30.]);
        assert_eq!(m.position(&h).unwrap(), Some([10., 6., 30.]));
        h.positions.insert(m.target, [20., 2., 30.]);
        assert_eq!(m.position(&h).unwrap(), Some([20., 6., 30.]));
        h.world.objective_visibility.insert(m.id, Some(false));
        assert_eq!(m.position(&h).unwrap(), None);
        h.world.objectives.insert(ObjectId(3), (true, false));
        assert_eq!(m.position(&h).unwrap(), None);
        h.world.objective_visibility.insert(m.id, Some(true));
        assert_eq!(m.position(&h).unwrap(), Some([20., 6., 30.]));
        h.world.objectives.insert(ObjectId(3), (true, true));
        assert_eq!(m.position(&h).unwrap(), None);
    }
}
