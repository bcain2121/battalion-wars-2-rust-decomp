//! Real original cinematic playback; a reusable backend check, not the frontend launcher.
use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
};
use bw_rust::movie::{Decoder, Frame, Info};
use std::{path::PathBuf, sync::Mutex};

#[derive(Resource)]
struct Movie {
    decoder: Mutex<Decoder>,
    info: Info,
    wav: Option<Vec<u8>>,
    image: Handle<Image>,
    pending: Option<Frame>,
    started: bool,
    software_clock: bool,
    elapsed: f64,
    audio_wait: f64,
    last_frame: Option<u64>,
    captured: bool,
    target: Option<Handle<Image>>,
}
#[derive(Component)]
struct Soundtrack;
fn main() -> AppExit {
    let path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("local/extracted/files/Data/Video/BWii_Intro.thp")
        });
    let prepared = (|| {
        let info = Info::probe(&path)?;
        let wav = info.audio_wav()?;
        let decoder = Decoder::start(info.clone())?;
        Ok::<_, String>((info, wav, decoder))
    })();
    let (info, wav, decoder) = match prepared {
        Ok(v) => v,
        Err(e) => {
            eprintln!("MOVIE_ERROR {e}");
            return AppExit::error();
        }
    };
    let software_clock = std::env::var_os("BW_OFFSCREEN").is_some();
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Original cinematic playback — menu integration pending".into(),
                resolution: (960, 720).into(),
                visible: !software_clock,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(Movie {
            decoder: Mutex::new(decoder),
            info,
            wav: Some(wav),
            image: Handle::default(),
            pending: None,
            started: false,
            software_clock,
            elapsed: 0.0,
            audio_wait: 0.0,
            last_frame: None,
            captured: false,
            target: None,
        })
        .add_systems(Startup, setup)
        .add_systems(Update, tick)
        .run()
}
fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>, mut movie: ResMut<Movie>) {
    let image = Image::new_fill(
        Extent3d {
            width: movie.info.width,
            height: movie.info.height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::all(),
    );
    movie.image = images.add(image);
    let camera = commands.spawn(Camera2d).id();
    if movie.software_clock {
        let mut target = Image::new_target_texture(
            960,
            720,
            TextureFormat::Rgba8Unorm,
            Some(TextureFormat::Rgba8UnormSrgb),
        );
        target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
        let handle = images.add(target);
        commands
            .entity(camera)
            .insert(bevy::camera::RenderTarget::Image(handle.clone().into()));
        movie.target = Some(handle);
    }
    commands
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            UiTargetCamera(camera),
        ))
        .with_children(|p| {
            p.spawn((
                ImageNode::new(movie.image.clone()),
                Node {
                    width: px(960),
                    height: px(960.0 * movie.info.height as f32 / movie.info.width as f32),
                    ..default()
                },
            ));
        });
}
#[allow(clippy::too_many_arguments)]
fn tick(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut movie: ResMut<Movie>,
    mut images: ResMut<Assets<Image>>,
    mut audio: ResMut<Assets<AudioSource>>,
    sinks: Query<&AudioSink, With<Soundtrack>>,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        eprintln!("MOVIE_CANCELLED");
        exit.write(AppExit::Success);
        return;
    }
    let result = (|| -> Result<(), String> {
        if !movie.started {
            let frame = movie
                .decoder
                .lock()
                .map_err(|_| "Movie decoder mutex poisoned")?
                .try_frame()?;
            let Some(frame) = frame else {
                return Ok(());
            };
            movie.pending = Some(frame);
            movie.started = true;
            if !movie.software_clock {
                let wav = movie.wav.take().ok_or("Missing movie soundtrack")?;
                let handle = audio.add(AudioSource { bytes: wav.into() });
                commands.spawn((AudioPlayer::new(handle), Soundtrack));
            }
        }
        let done_audio;
        if movie.software_clock {
            // Verification clock only; no hardware audio claim.
            movie.elapsed += 1.0 / 60.0;
            done_audio = movie.elapsed >= movie.info.duration();
        } else if let Ok(sink) = sinks.single() {
            done_audio = sink.empty();
            movie.elapsed = if done_audio {
                movie.info.duration()
            } else {
                sink.position().as_secs_f64()
            };
        } else {
            movie.audio_wait += time.delta_secs_f64();
            if movie.audio_wait > 5.0 {
                return Err("Movie audio output did not become ready".into());
            }
            return Ok(());
        }
        // Drain due frames; retain one future frame. Queue memory remains bounded.
        for _ in 0..128 {
            if movie.pending.is_none() {
                let next = movie
                    .decoder
                    .lock()
                    .map_err(|_| "Movie decoder mutex poisoned")?
                    .try_frame()?;
                movie.pending = next;
            }
            let Some(frame) = &movie.pending else {
                break;
            };
            if frame.index as f64 / movie.info.frame_rate > movie.elapsed {
                break;
            }
            let frame = movie.pending.take().unwrap();
            images
                .get_mut(&movie.image)
                .ok_or("Movie texture missing")?
                .data = Some(frame.rgba);
            movie.last_frame = Some(frame.index);
        }
        if !movie.captured && movie.last_frame.is_some_and(|i| i >= movie.info.frames / 2) {
            if let Some(path) = std::env::var_os("BW_MOVIE_CAPTURE") {
                use bevy::render::view::screenshot::{Screenshot, save_to_disk};
                let shot = movie
                    .target
                    .clone()
                    .map(Screenshot::image)
                    .unwrap_or_else(Screenshot::primary_window);
                commands
                    .spawn(shot)
                    .observe(save_to_disk(PathBuf::from(path)));
            }
            movie.captured = true;
        }
        let finished = movie
            .decoder
            .lock()
            .map_err(|_| "Movie decoder mutex poisoned")?
            .finished();
        if finished && done_audio && movie.pending.is_none() {
            println!(
                "MOVIE_PLAYBACK_PASS frames={} last_frame={:?} audio_sink_playback={} duration={:.6}",
                movie.info.frames,
                movie.last_frame,
                !movie.software_clock,
                movie.info.duration()
            );
            exit.write(AppExit::Success);
        }
        Ok(())
    })();
    if let Err(e) = result {
        eprintln!("MOVIE_ERROR {e}");
        exit.write(AppExit::error());
    }
}
