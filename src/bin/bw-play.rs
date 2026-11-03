//! Playable prototype scaffolding on recovered terrain. Not original game logic.
use bevy::{app::AppExit, prelude::*};
use bw_rust::{
    controls::Controls,
    scenario::Scenario,
    skirmish::{Game, Ground, Input, Outcome},
    terrain::{self, Terrain},
    terrain_render::{self, TerrainMaterial},
    texture::{self, DecodedTexture},
};
use std::{collections::BTreeMap, fs::File, io::Read};

#[derive(Resource)]
struct WorldData {
    ground: Ground,
    game: Game,
    terrain: Option<Terrain>,
    textures: Option<BTreeMap<String, DecodedTexture>>,
    scenario: Scenario,
    camera_pitch: f32,
    controller_connected: bool,
    paused: bool,
    smoke: bool,
    frames: u32,
    smoke_won: bool,
}
#[derive(Component)]
struct Player;
#[derive(Component)]
struct Foe(usize);
#[derive(Component)]
struct Hud;
#[derive(Component)]
struct Banner;
#[derive(Component)]
struct FollowCamera;

fn location(ground: &Ground, p: [f32; 2]) -> Vec3 {
    Vec3::new(p[0], ground.height(p[0], p[1]).unwrap_or(0.0), p[1])
}
fn setup(
    mut commands: Commands,
    mut world: ResMut<WorldData>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut terrain_materials: ResMut<Assets<TerrainMaterial>>,
    mut images: ResMut<Assets<Image>>,
    controls: Res<Controls>,
) {
    let t = world.terrain.take().expect("terrain exists");
    let textures = world.textures.take().expect("terrain textures loaded");
    terrain_render::spawn(
        &mut commands,
        t,
        textures,
        &mut meshes,
        &mut images,
        &mut terrain_materials,
    );
    let body = meshes.add(Capsule3d::new(0.35, 0.8));
    let blue = materials.add(StandardMaterial {
        base_color: Color::srgb(
            world.scenario.player_color[0],
            world.scenario.player_color[1],
            world.scenario.player_color[2],
        ),
        unlit: true,
        ..default()
    });
    let red = materials.add(StandardMaterial {
        base_color: Color::srgb(
            world.scenario.enemy_color[0],
            world.scenario.enemy_color[1],
            world.scenario.enemy_color[2],
        ),
        unlit: true,
        ..default()
    });
    commands.spawn((
        Player,
        Mesh3d(body.clone()),
        MeshMaterial3d(blue),
        Transform::from_translation(location(&world.ground, world.game.player) + Vec3::Y * 0.8),
    ));
    for (i, e) in world.game.enemies.iter().enumerate() {
        commands.spawn((
            Foe(i),
            Mesh3d(body.clone()),
            MeshMaterial3d(red.clone()),
            Transform::from_translation(location(&world.ground, e.position) + Vec3::Y * 0.8),
        ));
    }
    commands.spawn((FollowCamera, Camera3d::default(), Transform::default()));
    commands.spawn((
        Hud,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(22.0),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: px(20),
            left: px(24),
            ..default()
        },
        BackgroundColor(Color::srgba(0.02, 0.04, 0.07, 0.88)),
    ));
    commands.spawn((
        Text::new(controls.keyboard_help()),
        TextFont {
            font_size: FontSize::Px(14.0),
            ..default()
        },
        TextColor(Color::srgb(0.85, 0.9, 0.95)),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(20),
            left: px(24),
            ..default()
        },
        BackgroundColor(Color::srgba(0.02, 0.04, 0.07, 0.88)),
    ));
    commands.spawn((
        Banner,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(32.0),
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.85, 0.25)),
        Node {
            position_type: PositionType::Absolute,
            top: percent(35),
            left: percent(28),
            ..default()
        },
    ));
}
fn step(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    mut world: ResMut<WorldData>,
    mut exit: MessageWriter<AppExit>,
    controls: Res<Controls>,
    gamepads: Query<&Gamepad>,
) {
    world.frames += 1;
    let pad = gamepads.iter().next();
    world.controller_connected = pad.is_some();
    let actions = controls.sample(&keys, &mouse, pad);
    if actions.quit {
        exit.write(AppExit::Success);
        return;
    }
    if actions.start {
        world.paused = false;
    }
    if actions.pause {
        world.paused = !world.paused;
    }
    if actions.input.restart {
        world.paused = false;
    }
    let mut input = actions.input;
    if !world.paused {
        world.camera_pitch =
            (world.camera_pitch + actions.look_pitch * time.delta_secs()).clamp(0.15, 1.25);
    }
    let mut dt = time.delta_secs();
    if world.smoke {
        dt = 1.0 / 60.0;
        input.forward = if world.frames < 60 { 1.0 } else { 0.0 };
        input.turn = if world.frames > 60 && world.frames < 90 {
            0.6
        } else {
            0.0
        };
        input.fire = world.frames >= 90;
    }
    if !world.paused {
        let w = &mut *world;
        w.game.tick(&w.ground, input, dt);
    }
    if world.smoke && world.game.outcome == Outcome::Won && !world.smoke_won {
        println!(
            "SMOKE_WIN health={} shots={} hits={} frames={}",
            world.game.health, world.game.shots, world.game.hits, world.frames
        );
        world.smoke_won = true;
    }
    if world.smoke && world.frames == 720 {
        println!(
            "SMOKE_RESULT won={} shots={} hits={} outcome={:?}",
            world.smoke_won, world.game.shots, world.game.hits, world.game.outcome
        );
        exit.write(if world.smoke_won {
            AppExit::Success
        } else {
            AppExit::error()
        });
    }
}
// Explicit disjoint Bevy query filters prevent aliased mutable Transform access.
#[allow(clippy::type_complexity)]
fn present(
    world: Res<WorldData>,
    mut players: Query<&mut Transform, (With<Player>, Without<Foe>, Without<FollowCamera>)>,
    mut foes: Query<
        (&Foe, &mut Transform, &mut Visibility),
        (Without<Player>, Without<FollowCamera>),
    >,
    mut cameras: Query<&mut Transform, (With<FollowCamera>, Without<Player>, Without<Foe>)>,
    mut hud: Query<&mut Text, (With<Hud>, Without<Banner>)>,
    mut banners: Query<&mut Text, (With<Banner>, Without<Hud>)>,
    mut gizmos: Gizmos,
) {
    let game = &world.game;
    let p = location(&world.ground, game.player);
    for mut t in &mut players {
        t.translation = p + Vec3::Y * 0.8;
        t.rotation = Quat::from_rotation_y(-game.yaw);
    }
    for (foe, mut t, mut visible) in &mut foes {
        let e = &game.enemies[foe.0];
        t.translation = location(&world.ground, e.position) + Vec3::Y * 0.8;
        *visible = if e.health > 0.0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if e.health > 0.0 {
            let a = t.translation + Vec3::Y * 1.1;
            gizmos.line(
                a - Vec3::X * 0.6,
                a + Vec3::X * (1.2 * e.health / 60.0 - 0.6),
                Color::srgb(1.0, 0.25, 0.1),
            );
        }
    }
    let forward = Vec3::new(game.yaw.sin(), 0.0, -game.yaw.cos());
    let mut eye = p - forward * (12.0 * world.camera_pitch.cos())
        + Vec3::Y * (12.0 * world.camera_pitch.sin());
    if let Some(h) = world.ground.height(eye.x, eye.z) {
        eye.y = eye.y.max(h + 2.5);
    }
    for mut t in &mut cameras {
        *t = Transform::from_translation(eye).looking_at(p + forward * 4.0 + Vec3::Y, Vec3::Y);
    }
    if let Some(i) = game.target
        && game.enemies[i].health > 0.0
    {
        let target = location(&world.ground, game.enemies[i].position) + Vec3::Y * 2.5;
        gizmos.line(
            target - Vec3::X * 0.45,
            target + Vec3::X * 0.45,
            Color::srgb(1.0, 0.9, 0.1),
        );
        gizmos.line(
            target - Vec3::Y * 0.45,
            target + Vec3::Y * 0.45,
            Color::srgb(1.0, 0.9, 0.1),
        );
    }
    if game.flash > 0.0
        && let Some((a, b)) = game.last_shot
    {
        gizmos.line(
            location(&world.ground, a) + Vec3::Y,
            location(&world.ground, b) + Vec3::Y,
            Color::srgb(1.0, 0.9, 0.3),
        );
    }
    gizmos.line(
        p + Vec3::Y * 0.15,
        p + Vec3::Y * 0.15 + forward * 2.0,
        Color::srgb(0.1, 0.9, 1.0),
    );
    for i in 0..96 {
        let angle = i as f32 / 96.0 * std::f32::consts::TAU;
        let next = (i + 1) as f32 / 96.0 * std::f32::consts::TAU;
        let a = [
            game.center[0] + angle.cos() * 23.0,
            game.center[1] + angle.sin() * 23.0,
        ];
        let b = [
            game.center[0] + next.cos() * 23.0,
            game.center[1] + next.sin() * 23.0,
        ];
        if world.ground.height(a[0], a[1]).is_some() && world.ground.height(b[0], b[1]).is_some() {
            gizmos.line(
                location(&world.ground, a) + Vec3::Y * 0.12,
                location(&world.ground, b) + Vec3::Y * 0.12,
                Color::srgb(0.1, 0.8, 1.0),
            );
        }
    }
    for mut text in &mut hud {
        let label = format!(
            "BATTALION WARS RUST / MODDING PROTOTYPE\n{} | Gamepad {}\nHEALTH {:3.0}   TARGETS {} / 5\nLOCK {}   HITS {}   SHOTS {}",
            world.scenario.name,
            if world.controller_connected {
                "connected"
            } else {
                "not detected"
            },
            game.health,
            5 - game.alive(),
            game.target
                .map(|i| (i + 1).to_string())
                .unwrap_or_else(|| "none - move closer".into()),
            game.hits,
            game.shots
        );
        if text.0 != label {
            text.0 = label;
        }
    }
    for mut text in &mut banners {
        text.0 = if world.paused && game.elapsed == 0.0 {
            "READY TO DEPLOY\nUse your Start binding".into()
        } else if world.paused {
            "PAUSED - use Pause to resume".into()
        } else {
            match game.outcome {
                Outcome::Won => "AREA CLEARED\nUse Restart to play again".into(),
                Outcome::Lost => "UNIT LOST\nUse Restart to retry".into(),
                Outcome::Playing => String::new(),
            }
        };
    }
}
fn capture(mut commands: Commands, world: Res<WorldData>) {
    if world.frames == 197
        && let Some(path) = std::env::var_os("BW_CAPTURE")
    {
        use bevy::render::view::screenshot::{Screenshot, save_to_disk};
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(std::path::PathBuf::from(path)));
    }
}
fn main() {
    let manifest = std::env::args_os()
        .nth(1)
        .unwrap_or_else(|| "mods/training/level.json".into());
    let mut config = String::new();
    File::open(manifest)
        .expect("level manifest")
        .take(65537)
        .read_to_string(&mut config)
        .expect("read level manifest");
    let scenario = Scenario::parse(&config).expect("valid level manifest");
    let mut control_json = String::new();
    File::open("config/controls.json")
        .expect("controls.json")
        .take(65537)
        .read_to_string(&mut control_json)
        .expect("read controls");
    let controls = Controls::parse(&control_json).expect("valid control bindings");
    let mut bytes = Vec::new();
    File::open(&scenario.terrain)
        .expect("Terrain file missing: launch from repository root")
        .take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .expect("read terrain");
    assert!(bytes.len() <= 64 * 1024 * 1024);
    let terrain = if scenario.terrain.ends_with(".json") {
        bw_rust::custom_terrain::parse(std::str::from_utf8(&bytes).expect("UTF-8 heightfield"))
            .expect("valid custom terrain")
    } else {
        terrain::decode(&bytes).expect("valid original terrain")
    };
    assert_eq!(terrain.seam_mismatches, 0);
    let ground = Ground::from_terrain(&terrain);
    let center = scenario
        .arena_center
        .unwrap_or_else(|| ground.arena().expect("playable terrain patch"));
    let game = Game::new(center);
    assert!(
        ground.height(game.player[0], game.player[1]).is_some(),
        "player spawn outside terrain"
    );
    for e in &game.enemies {
        assert!(
            ground.height(e.position[0], e.position[1]).is_some(),
            "enemy spawn outside terrain"
        );
    }
    let mut archive = Vec::new();
    File::open(&scenario.resources)
        .expect("resource archive")
        .take(256 * 1024 * 1024 + 1)
        .read_to_end(&mut archive)
        .expect("read resource archive");
    assert!(archive.len() <= 256 * 1024 * 1024);
    let mut textures = BTreeMap::new();
    for pair in &terrain.materials {
        for name in pair {
            let key = name.to_ascii_lowercase();
            textures.entry(key).or_insert_with(|| {
                texture::decode_named(&archive, name).expect("terrain texture decoding")
            });
        }
    }
    println!(
        "Decoded {} terrain textures for {} material pairs",
        textures.len(),
        terrain.materials.len()
    );
    println!(
        "ARENA center={center:?} ground_height={:?}",
        ground.height(center[0], center[1])
    );
    if std::env::var_os("BW_VERIFY").is_some() {
        let mut check = Game::new(center);
        let start = check.player;
        for _ in 0..60 {
            check.tick(
                &ground,
                Input {
                    forward: 1.0,
                    ..Default::default()
                },
                1.0 / 60.0,
            );
        }
        assert_ne!(start, check.player, "movement on real terrain");
        for _ in 0..1200 {
            check.tick(
                &ground,
                Input {
                    fire: true,
                    ..Default::default()
                },
                1.0 / 60.0,
            );
        }
        assert_eq!(check.outcome, Outcome::Won, "real-terrain victory");
        let victory_health = check.health;
        check.tick(
            &ground,
            Input {
                restart: true,
                ..Default::default()
            },
            1.0 / 60.0,
        );
        assert_eq!(check.health, 100.0);
        assert_eq!(check.alive(), 5);
        for _ in 0..3600 {
            check.tick(&ground, Input::default(), 1.0 / 60.0);
        }
        assert_eq!(check.outcome, Outcome::Lost, "real-terrain defeat");
        check.tick(
            &ground,
            Input {
                restart: true,
                ..Default::default()
            },
            1.0 / 60.0,
        );
        assert_eq!(check.outcome, Outcome::Playing);
        println!(
            "REAL_TERRAIN_VERIFY PASS movement victory defeat restart; victory_health={victory_health}"
        );
        return;
    }
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                file_path: std::fs::canonicalize("assets")
                    .expect("local assets directory")
                    .to_string_lossy()
                    .into_owned(),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Battalion Wars Rust — first playable prototype".into(),
                    resolution: (1280, 800).into(),
                    ..default()
                }),
                ..default()
            }),
    );
    terrain_render::register_shader(&mut app);
    app.add_plugins(MaterialPlugin::<TerrainMaterial>::default())
        .insert_resource(ClearColor(Color::srgb(0.09, 0.13, 0.18)))
        .insert_resource(controls)
        .insert_resource(WorldData {
            ground,
            game,
            terrain: Some(terrain),
            textures: Some(textures),
            scenario,
            camera_pitch: 0.6,
            controller_connected: false,
            paused: std::env::var_os("BW_SMOKE").is_none(),
            smoke: std::env::var_os("BW_SMOKE").is_some(),
            frames: 0,
            smoke_won: false,
        })
        .add_systems(Startup, setup)
        .add_systems(Update, (step, present, capture).chain())
        .run();
}
