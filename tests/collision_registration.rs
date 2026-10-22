use bw_rust::{
    ObjectId,
    mission_destruction::Handle,
    unit_collision::{ColliderKey, Scene},
};
use std::num::NonZeroU64;
fn owner(n: u64) -> Handle {
    Handle(NonZeroU64::new(n).unwrap())
}
fn triangle() -> Vec<[[f32; 3]; 3]> {
    vec![[[0., 0., 0.], [0., 2., 0.], [0., 0., 2.]]]
}
#[test]
fn grouped_geometry_preserves_flat_scene_queries() {
    let nodes: Vec<_> = [0., 5.]
        .into_iter()
        .enumerate()
        .map(|(i, x)| {
            (
                i,
                vec![
                    [[x, 0., -5.], [x, 10., -5.], [x, 10., 5.]],
                    [[x, 0., -5.], [x, 10., 5.], [x, 0., 5.]],
                ],
            )
        })
        .collect();
    let mut flat = Scene::default();
    flat.add_mesh(
        nodes
            .iter()
            .flat_map(|(_, ts)| ts.iter().copied())
            .collect(),
    )
    .unwrap();
    let mut grouped = Scene::default();
    grouped
        .register_object_meshes(ObjectId(50), owner(1), nodes)
        .unwrap();
    for x in [-4., -1., 1., 4., 6.] {
        for z in [-6., 0., 6.] {
            for delta in [[-3., 0.], [3., 0.], [0., 3.], [0., -3.], [2., 2.]] {
                let a = flat
                    .move_body(
                        [x, 0., z],
                        delta,
                        bw_rust::unit_collision::Body::INFANTRY,
                        &[],
                    )
                    .unwrap();
                let b = grouped
                    .move_body(
                        [x, 0., z],
                        delta,
                        bw_rust::unit_collision::Body::INFANTRY,
                        &[],
                    )
                    .unwrap();
                assert_eq!((a.position, a.blocked), (b.position, b.blocked));
                let ray = ([x, 1., z], [x + delta[0] as f32, 1., z + delta[1] as f32]);
                assert_eq!(
                    flat.line_clear(ray.0, ray.1).unwrap(),
                    grouped.line_clear(ray.0, ray.1).unwrap()
                );
            }
        }
    }
}
#[test]
fn complete_batches_validate_before_committing() {
    let mut s = Scene::default();
    for batch in [
        vec![(0, triangle()), (0, triangle())],
        vec![(0, triangle()), (1, vec![[[f32::NAN; 3]; 3]])],
    ] {
        assert!(
            s.register_object_meshes(ObjectId(50), owner(1), batch)
                .is_err()
        );
        assert_eq!(s.mesh_count(), 0);
        assert_eq!(s.collision_owner(ObjectId(50)), None);
        assert!(!s.has_owned_node(ColliderKey {
            owner: owner(1),
            node: 0
        }));
    }
    s.register_object_meshes(
        ObjectId(50),
        owner(1),
        vec![(0, triangle()), (1, triangle())],
    )
    .unwrap();
    assert_eq!(s.mesh_count(), 2);
    assert_eq!(s.collision_owner(ObjectId(50)), Some(owner(1)));
    assert!(
        s.register_object_meshes(ObjectId(50), owner(2), vec![])
            .is_err()
    );
    assert!(
        s.register_object_meshes(ObjectId(51), owner(1), vec![])
            .is_err()
    );
    assert!(
        s.register_object_meshes(ObjectId(0), owner(2), vec![])
            .is_err()
    );
    assert_eq!(s.mesh_count(), 2);
}
#[test]
fn retained_bindings_and_snapshot_lookup_follow_geometry() {
    let mut s = Scene::default();
    s.register_object_meshes(
        ObjectId(50),
        owner(1),
        vec![(0, triangle()), (1, triangle())],
    )
    .unwrap();
    let original = s.clone();
    s.remove_owned_meshes(owner(1), &[1]).unwrap();
    assert_eq!(s.collision_owner(ObjectId(50)), Some(owner(1)));
    assert_eq!(s.mesh_count(), 1);
    assert_eq!(original.mesh_count(), 2);
    s.remove_owned_meshes(owner(1), &[]).unwrap();
    assert_eq!(s.collision_owner(ObjectId(50)), None);
    assert_eq!(original.collision_owner(ObjectId(50)), Some(owner(1)));
    assert!(original.has_owned_node(ColliderKey {
        owner: owner(1),
        node: 0
    }));
}
#[test]
fn known_empty_object_is_distinct_from_unknown_and_unbound_owner_cannot_alias() {
    let mut s = Scene::default();
    s.register_object_meshes(ObjectId(50), owner(1), vec![])
        .unwrap();
    assert_eq!(s.collision_owner(ObjectId(50)), Some(owner(1)));
    assert!(s.remove_owned_meshes(owner(1), &[0]).is_err());
    assert_eq!(s.remove_owned_meshes(owner(1), &[]).unwrap(), 0);
    assert_eq!(s.collision_owner(ObjectId(50)), None);
    assert!(s.remove_owned_meshes(owner(1), &[]).is_err());
    s.add_owned_mesh(
        ColliderKey {
            owner: owner(2),
            node: 0,
        },
        triangle(),
    )
    .unwrap();
    assert!(
        s.register_object_meshes(ObjectId(51), owner(2), vec![])
            .is_err()
    );
}
