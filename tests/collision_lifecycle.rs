//! Replacement collision lifecycle checks, not native destruction acceptance.
use bw_rust::{
    mission_destruction::Handle,
    unit_collision::{Body, ColliderKey, Scene},
};
use std::{num::NonZeroU64, sync::Arc};
fn key(owner: u64, node: usize) -> ColliderKey {
    ColliderKey {
        owner: Handle(NonZeroU64::new(owner).unwrap()),
        node,
    }
}
fn wall(x: f32) -> Vec<[[f32; 3]; 3]> {
    vec![
        [[x, 0., -5.], [x, 10., -5.], [x, 10., 5.]],
        [[x, 0., -5.], [x, 10., 5.], [x, 0., 5.]],
    ]
}
fn blocked(s: &Scene, x: f32) -> bool {
    !s.line_clear([x - 2., 1., 0.], [x + 2., 1., 0.]).unwrap()
}
#[test]
fn removal_updates_real_queries_and_preserves_retained_foreign_and_anonymous_geometry() {
    let mut s = Scene::default();
    s.add_owned_mesh(key(1, 0), wall(0.)).unwrap();
    s.add_owned_mesh(key(1, 1), wall(5.)).unwrap();
    s.add_owned_mesh(key(2, 0), wall(10.)).unwrap();
    s.add_mesh(wall(15.)).unwrap();
    assert!(blocked(&s, 0.));
    assert!(s.overlaps([0., 0., 0.], Body::INFANTRY, &[]).unwrap());
    let before = s
        .move_body([-2., 0., 0.], [4., 0.], Body::INFANTRY, &[])
        .unwrap();
    assert!(before.blocked);
    assert_eq!(s.remove_owned_meshes(key(1, 0).owner, &[1]).unwrap(), 1);
    assert!(!blocked(&s, 0.));
    assert!(!s.overlaps([0., 0., 0.], Body::INFANTRY, &[]).unwrap());
    let after = s
        .move_body([-2., 0., 0.], [4., 0.], Body::INFANTRY, &[])
        .unwrap();
    assert!(!after.blocked);
    assert!(after.position[0] > 1.9);
    assert!(blocked(&s, 5.) && blocked(&s, 10.) && blocked(&s, 15.));
    assert!(!s.has_owned_node(key(1, 0)));
    assert!(s.has_owned_node(key(1, 1)));
    assert_eq!(s.remove_owned_meshes(key(1, 0).owner, &[]).unwrap(), 1);
    assert!(!blocked(&s, 5.));
    assert!(blocked(&s, 10.) && blocked(&s, 15.));
    assert!(s.remove_owned_meshes(key(1, 0).owner, &[]).is_err());
}
#[test]
fn invalid_requests_do_not_mutate_and_staged_scenes_do_not_leak_changes() {
    let mut s = Scene::default();
    s.add_owned_mesh(key(1, 0), wall(0.)).unwrap();
    assert!(s.add_owned_mesh(key(1, 0), wall(5.)).is_err());
    for retained in [vec![9], vec![0, 0]] {
        assert!(s.remove_owned_meshes(key(1, 0).owner, &retained).is_err());
    }
    assert!(s.remove_owned_meshes(key(2, 0).owner, &[]).is_err());
    assert_eq!(s.mesh_count(), 1);
    assert!(blocked(&s, 0.));
    let original = Arc::new(s);
    let mut staged = original.clone();
    Arc::make_mut(&mut staged)
        .remove_owned_meshes(key(1, 0).owner, &[])
        .unwrap();
    assert!(!blocked(&staged, 0.));
    assert!(blocked(&original, 0.));
    drop(staged);
    assert!(original.has_owned_node(key(1, 0)));
}
#[test]
fn empty_nodes_have_known_ownership_and_failed_geometry_does_not_reserve_keys() {
    let mut s = Scene::default();
    s.add_owned_mesh(key(1, 0), vec![]).unwrap();
    s.add_owned_mesh(key(1, 1), vec![[[0.; 3]; 3]]).unwrap();
    assert_eq!(s.mesh_count(), 0);
    assert!(s.has_owned_node(key(1, 1)));
    assert_eq!(s.remove_owned_meshes(key(1, 0).owner, &[1]).unwrap(), 0);
    assert!(s.has_owned_node(key(1, 1)));
    assert!(!s.has_owned_node(key(1, 0)));
    assert!(
        s.add_owned_mesh(key(2, 0), vec![[[f32::NAN; 3]; 3]])
            .is_err()
    );
    assert!(!s.has_owned_node(key(2, 0)));
    s.add_owned_mesh(key(2, 0), wall(0.)).unwrap();
    assert!(blocked(&s, 0.));
}
