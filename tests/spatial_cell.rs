use bw_rust::mission_spatial_cell::{Bounds, Cell, Contents, Record, Sphere, visit};
fn bounds() -> Bounds {
    Bounds {
        min: [-10.0; 2],
        max: [10.0; 2],
    }
}
fn sphere(x: f32, radius: f32) -> Sphere {
    Sphere {
        center: [x, 0.0, 0.0],
        radius,
    }
}
fn record(id: u64, x: f32) -> Record {
    Record {
        identity: id,
        mask: 1,
        sphere: sphere(x, 1.0),
    }
}
fn leaf(records: Vec<Record>) -> Cell {
    Cell {
        mask: 1,
        contents: Contents::Leaf(records),
    }
}
#[test]
fn duplicate_order_and_stop_include_stopping_callback() {
    let tree = Cell {
        mask: 1,
        contents: Contents::Branch(Box::new([
            leaf(vec![record(3, 0.0), record(3, 0.0)]),
            leaf(vec![record(1, 0.0)]),
            leaf(vec![record(9, 0.0)]),
            leaf(vec![record(8, 0.0)]),
        ])),
    };
    let mut seen = vec![];
    let result = visit(&tree, bounds(), sphere(0.0, 1.0), 1.0, 1, |r| {
        seen.push(r.identity);
        Ok(seen.len() < 3)
    })
    .unwrap();
    assert_eq!(seen, [3, 3, 1]);
    assert_eq!(result.callbacks, 3);
    assert!(result.stopped);
    assert_eq!(result.native_result(), !3_u32);
    seen.clear();
    let all = visit(&tree, bounds(), sphere(0.0, 1.0), 1.0, 1, |r| {
        seen.push(r.identity);
        Ok(true)
    })
    .unwrap();
    assert_eq!(seen, [3, 3, 1, 9, 8]);
    assert_eq!(all.native_result(), 5);
}
#[test]
fn strict_sphere_test_and_lazy_masking() {
    let mut invalid = record(99, f32::NAN);
    invalid.mask = 2;
    let tree = leaf(vec![record(1, 2.0), record(2, 1.999), invalid]);
    let mut seen = vec![];
    let result = visit(&tree, bounds(), sphere(0.0, 1.0), 1.0, 1, |r| {
        seen.push(r.identity);
        Ok(true)
    })
    .unwrap();
    assert_eq!(seen, [2]);
    assert_eq!(result.callbacks, 1);
    let result = visit(
        &leaf(vec![record(99, f32::NAN)]),
        bounds(),
        sphere(0.0, 1.0),
        1.0,
        2,
        |_| panic!("masked cell"),
    )
    .unwrap();
    assert_eq!(result.callbacks, 0);
}
#[test]
fn stop_never_reads_later_bad_records() {
    let tree = leaf(vec![record(1, 0.0), record(99, f32::NAN)]);
    assert!(
        visit(&tree, bounds(), sphere(0.0, 1.0), 1.0, 1, |_| Ok(false))
            .unwrap()
            .stopped
    );
    assert!(visit(&tree, bounds(), sphere(0.0, 1.0), 1.0, 1, |_| Ok(true)).is_err());
    assert!(
        visit(&tree, bounds(), sphere(0.0, 1.0), 1.0, 1, |_| Err(
            "predicate unsupported".into()
        ))
        .is_err()
    );
}
#[test]
fn quadrant_bounds_and_expansion_control_traversal() {
    let point = Sphere {
        center: [-5.0, 0.0, -5.0],
        radius: 1.0,
    };
    let good = Record {
        identity: 1,
        mask: 1,
        sphere: point,
    };
    let bad = || leaf(vec![record(99, f32::NAN)]);
    let tree = Cell {
        mask: 1,
        contents: Contents::Branch(Box::new([leaf(vec![good]), bad(), bad(), bad()])),
    };
    assert_eq!(
        visit(&tree, bounds(), point, 1.0, 1, |_| Ok(true))
            .unwrap()
            .callbacks,
        1
    );
    // Touching the midpoint with the expanded box includes the neighboring cell.
    assert!(visit(&tree, bounds(), point, 4.0, 1, |_| Ok(true)).is_err());
}
#[test]
fn invalid_geometry_and_supported_work_bounds_fail_closed() {
    let tree = leaf(vec![record(1, 0.0)]);
    for radius in [-1.0, f32::NAN, f32::INFINITY] {
        assert!(visit(&tree, bounds(), sphere(0.0, radius), 1.0, 1, |_| Ok(true)).is_err());
    }
    assert!(
        visit(
            &leaf(vec![record(1, 0.0); 128]),
            bounds(),
            sphere(0.0, 1.0),
            1.0,
            1,
            |_| Ok(true)
        )
        .is_err()
    );
    assert!(
        visit(&tree, bounds(), sphere(0.0, f32::MAX), f32::MAX, 1, |_| Ok(
            true
        ))
        .is_err()
    );
}
