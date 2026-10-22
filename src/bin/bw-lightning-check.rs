//! Owned offscreen GPU acceptance check, not the mission or a user-facing launcher.
use bevy::{
    app::ScheduleRunnerPlugin,
    prelude::*,
    render::{
        render_resource::{TextureFormat, TextureUsages},
        view::screenshot::{Screenshot, save_to_disk},
    },
    window::ExitCondition,
    winit::WinitPlugin,
};
use bw_rust::{
    ObjectGraph, ObjectId, mission_lightning_render::Renderer, mission_rewrite::Runtime,
    parse_objects,
};
#[derive(Resource)]
struct Check {
    runtime: Runtime,
    target: Option<Handle<Image>>,
    output: String,
    frame: u32,
}
fn main() {
    let output = std::env::args()
        .nth(1)
        .expect("Usage: bw-lightning-check NEW_OUTPUT.png");
    assert!(
        !std::path::Path::new(&output).exists(),
        "Refusing to overwrite existing output"
    );
    assert!(
        !std::path::Path::new(&format!("{output}.faded.png")).exists(),
        "Refusing to overwrite existing fade output"
    );
    assert!(
        !std::path::Path::new(&format!("{output}.repeat.png")).exists(),
        "Refusing to overwrite existing repeat output"
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("local/extracted/files/Data/CompoundFiles");
    let mut graph = ObjectGraph::default();
    for name in ["SP_0.1_Level.xml", "SP_0.1_Level_preload.xml"] {
        graph
            .merge(parse_objects(&std::fs::read_to_string(root.join(name)).unwrap()).unwrap())
            .unwrap();
    }
    let mut runtime = Runtime::new(&graph);
    runtime
        .play_effect(ObjectId(250011650), 3, [0.; 3])
        .unwrap();
    App::new()
        .add_plugins(
            DefaultPlugins
                .build()
                .disable::<WinitPlugin>()
                .set(WindowPlugin {
                    primary_window: None,
                    exit_condition: ExitCondition::DontExit,
                    ..default()
                }),
        )
        .add_plugins(ScheduleRunnerPlugin::run_loop(
            std::time::Duration::from_millis(16),
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(Check {
            runtime,
            target: None,
            output,
            frame: 0,
        })
        .init_resource::<Renderer>()
        .add_systems(Startup, setup)
        .add_systems(Update, update)
        .run();
}
fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>, mut check: ResMut<Check>) {
    let mut image = Image::new_target_texture(
        640,
        480,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    );
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let handle = images.add(image);
    commands.spawn((
        Camera3d::default(),
        Transform::IDENTITY,
        bevy::camera::RenderTarget::Image(handle.clone().into()),
    ));
    check.target = Some(handle);
}
fn update(
    mut commands: Commands,
    mut check: ResMut<Check>,
    mut renderer: ResMut<Renderer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut exit: MessageWriter<AppExit>,
) {
    check.frame += 1;
    if check.frame == 1 {
        renderer.sync(
            check.runtime.lightning(),
            &Transform::IDENTITY,
            &mut commands,
            &mut meshes,
            &mut materials,
        );
        assert_eq!(renderer.instance_count(), 1);
    }
    if check.frame == 45 {
        commands
            .spawn(Screenshot::image(check.target.clone().unwrap()))
            .observe(save_to_disk(check.output.clone()));
    }
    if check.frame == 70 {
        check.runtime.advance(0.25).unwrap();
        assert!(check.runtime.lightning()[0].frame().triangles.is_empty());
        renderer.sync(
            check.runtime.lightning(),
            &Transform::IDENTITY,
            &mut commands,
            &mut meshes,
            &mut materials,
        );
    }
    if check.frame == 85 {
        commands
            .spawn(Screenshot::image(check.target.clone().unwrap()))
            .observe(save_to_disk(format!("{}.faded.png", check.output)));
    }
    if check.frame == 95 {
        for _ in 0..3 {
            check.runtime.advance(1.0).unwrap();
        }
        check.runtime.advance(0.75).unwrap();
        assert!(!check.runtime.lightning()[0].frame().triangles.is_empty());
        renderer.sync(
            check.runtime.lightning(),
            &Transform::IDENTITY,
            &mut commands,
            &mut meshes,
            &mut materials,
        );
    }
    if check.frame == 110 {
        commands
            .spawn(Screenshot::image(check.target.clone().unwrap()))
            .observe(save_to_disk(format!("{}.repeat.png", check.output)));
    }
    if check.frame == 130 {
        renderer.sync(
            &[],
            &Transform::IDENTITY,
            &mut commands,
            &mut meshes,
            &mut materials,
        );
        assert_eq!(renderer.instance_count(), 0);
    }
    if check.frame == 150 {
        assert!(
            std::path::Path::new(&check.output).exists(),
            "Screenshot not completed"
        );
        assert!(
            std::path::Path::new(&format!("{}.faded.png", check.output)).exists(),
            "Fade screenshot not completed"
        );
        assert!(
            std::path::Path::new(&format!("{}.repeat.png", check.output)).exists(),
            "Repeat screenshot not completed"
        );
        println!(
            "LIGHTNING_GPU_CHECK_PASS source=250011650 render=true fade=true cleanup=true mission=false"
        );
        exit.write(AppExit::Success);
    }
}
