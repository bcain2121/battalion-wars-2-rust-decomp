use bw_rust::movie::{Decoder, Info};
use std::{
    path::Path,
    time::{Duration, Instant},
};
#[test]
#[ignore = "requires original local THP assets and FFmpeg"]
fn original_startup_and_prologue_decode_completely() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("local/extracted/files/Data/Video");
    for (name, count) in [("BWii_Intro.thp", 323), ("0.1_Prologue_1.THP", 2777)] {
        let info = Info::probe(&root.join(name)).unwrap();
        assert_eq!((info.width, info.height, info.frames), (640, 480, count));
        assert_eq!((info.audio_rate, info.audio_channels), (32000, 2));
        let wav = info.audio_wav().unwrap();
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(u16::from_le_bytes(wav[22..24].try_into().unwrap()), 2);
        assert!(((wav.len() - 44) as f64 / (32000.0 * 4.0) - info.audio_duration).abs() < 0.01);
        let mut decoder = Decoder::start(info).unwrap();
        let deadline = Instant::now() + Duration::from_secs(120);
        let mut frames = 0;
        while !decoder.finished() {
            assert!(Instant::now() < deadline, "Movie decoder timed out");
            match decoder.try_frame().unwrap() {
                Some(frame) => {
                    assert_eq!(frame.index, frames);
                    assert_eq!(frame.rgba.len(), 640 * 480 * 4);
                    assert!(frame.rgba.chunks_exact(4).all(|p| p[3] == 255));
                    frames += 1;
                }
                None => std::thread::sleep(Duration::from_millis(1)),
            }
        }
        assert_eq!(frames, count);
    }
}
#[test]
#[ignore = "requires original local THP and FFmpeg"]
fn dropping_full_decoder_queue_is_bounded() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("local/extracted/files/Data/Video/BWii_Intro.thp");
    let decoder = Decoder::start(Info::probe(&path).unwrap()).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let start = Instant::now();
    drop(decoder);
    assert!(start.elapsed() < Duration::from_secs(3));
}
