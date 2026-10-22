//! Diagnostic original-terrain viewer. Not original world scale or final materials.
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use bw_rust::terrain::Terrain;
use std::{fs::File, io::Read};

#[derive(Resource)]
struct LoadedTerrain(Option<Terrain>);
#[derive(Resource)]
struct Orbit {
    center: Vec3,
    distance: f32,
    yaw: f32,
    pitch: f32,
}

fn setup(
    mut commands: Commands,
    mut terrain: ResMut<LoadedTerrain>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let t = terrain.0.take().expect("terrain loaded before startup");
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for p in &t.positions {
        let p = Vec3::from_array(*p);
        min = min.min(p);
        max = max.max(p);
    }
    let center = (min + max) * 0.5;
    let distance = (max - min).length() * 0.9;
    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, t.positions)
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        vec![[0.0, 1.0, 0.0]; t.colors.len()],
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, t.colors)
    .with_inserted_indices(Indices::U32(t.indices));
    commands.spawn((
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(StandardMaterial {
            unlit: true,
            cull_mode: None,
            ..default()
        })),
    ));
    let orbit = Orbit {
        center,
        distance,
        yaw: 0.8,
        pitch: 0.8,
    };
    commands.spawn((Camera3d::default(), camera_transform(&orbit)));
    commands.insert_resource(orbit);
}
fn camera_transform(o: &Orbit) -> Transform {
    let offset = Vec3::new(
        o.yaw.cos() * o.pitch.cos(),
        o.pitch.sin(),
        o.yaw.sin() * o.pitch.cos(),
    ) * o.distance;
    Transform::from_translation(o.center + offset).looking_at(o.center, Vec3::Y)
}
fn orbit(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut orbit: ResMut<Orbit>,
    mut cameras: Query<&mut Transform, With<Camera3d>>,
) {
    let dt = time.delta_secs();
    if keys.pressed(KeyCode::ArrowLeft) {
        orbit.yaw -= dt;
    }
    if keys.pressed(KeyCode::ArrowRight) {
        orbit.yaw += dt;
    }
    if keys.pressed(KeyCode::ArrowUp) {
        orbit.pitch = (orbit.pitch + dt).min(1.5);
    }
    if keys.pressed(KeyCode::ArrowDown) {
        orbit.pitch = (orbit.pitch - dt).max(0.05);
    }
    if keys.pressed(KeyCode::KeyW) {
        orbit.distance = (orbit.distance * (1.0 - dt.min(0.1))).max(1.0);
    }
    if keys.pressed(KeyCode::KeyS) {
        orbit.distance *= 1.0 + dt.min(0.1);
    }
    for mut transform in &mut cameras {
        *transform = camera_transform(&orbit);
    }
}
fn capture(mut commands: Commands, mut frames: Local<u32>) {
    *frames += 1;
    if *frames == 90
        && let Some(path) = std::env::var_os("BW_CAPTURE")
    {
        use bevy::render::view::screenshot::{Screenshot, save_to_disk};
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(std::path::PathBuf::from(path)));
    }
}
fn main() {
    let path = std::env::args_os()
        .nth(1)
        .expect("Usage: bw-viewer TERRAIN.out (uncompressed)");
    let mut bytes = Vec::new();
    File::open(path)
        .expect("open terrain")
        .take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .expect("read terrain");
    assert!(bytes.len() <= 64 * 1024 * 1024, "terrain size limit");
    let t = bw_rust::terrain::decode(&bytes).expect("decode terrain");
    assert_eq!(t.seam_mismatches, 0, "terrain seam validation failed");
    println!(
        "Terrain: {} vertices, {} triangles, {} matching seam samples",
        t.positions.len(),
        t.indices.len() / 3,
        t.seam_comparisons
    );
    App::new()
        .add_plugins(
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title:
                        "BW2 terrain diagnostic — provisional scale/colors | Arrows orbit, W/S zoom"
                            .into(),
                    ..default()
                }),
                ..default()
            }),
        )
        .insert_resource(LoadedTerrain(Some(t)))
        .add_systems(Startup, setup)
        .add_systems(Update, (orbit, capture))
        .run();
}
