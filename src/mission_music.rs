//! Decoded original stereo music and a deterministic PCM transport for mission scripts.
//! Linear fades/one-shot intro playback are explicit replacement presentation policies.
//! This is a PCM backend, not a claim that a hardware audio device consumed the output.
use crate::{ObjectGraph, Result, dsp_audio::Audio, lua50::Value, original_scene::field};
use std::{
    collections::BTreeMap,
    path::{Component, Path},
    sync::Arc,
};
#[derive(Clone)]
pub struct Stereo {
    pub left: Audio,
    pub right: Audio,
}
impl Stereo {
    pub fn new(left: Audio, right: Audio) -> Result<Self> {
        if left.sample_rate == 0
            || left.sample_rate > 192000
            || left.sample_rate != right.sample_rate
            || left.pcm.len() != right.pcm.len()
            || left.pcm.is_empty()
            || left.looping != right.looping
            || left.loop_nibbles != right.loop_nibbles
        {
            return Err("Mismatched stereo DSP channels".into());
        }
        let stereo = Self { left, right };
        stereo.loop_range()?;
        Ok(stereo)
    }
    fn loop_range(&self) -> Result<Option<(usize, usize)>> {
        if !self.left.looping {
            return Ok(None);
        }
        let sample = |n: u32| -> Result<usize> {
            if n % 16 < 2 {
                return Err("DSP loop points into frame header".into());
            }
            Ok((n as usize / 16) * 14 + n as usize % 16 - 2)
        };
        let start = sample(self.left.loop_nibbles[0])?;
        let end = sample(self.left.loop_nibbles[1])? + 1;
        if start >= end || end > self.left.pcm.len() {
            return Err("Invalid DSP music loop range".into());
        }
        Ok(Some((start, end)))
    }
    pub fn duration(&self) -> f64 {
        self.left.duration()
    }
}
#[derive(Clone)]
pub struct Bank {
    pub tracks: BTreeMap<String, Arc<Stereo>>,
    pub level_gain: f64,
}
fn load_channel(root: &Path, folder: &str, name: &str) -> Result<Audio> {
    let relative = Path::new(folder).join(name);
    if !relative.starts_with("Data/Audio/Music")
        || folder.contains('\\')
        || name.contains(['/', '\\'])
        || relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("Unsafe music path".into());
    }
    let path = root
        .join(relative)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if !path.starts_with(root) {
        return Err("Music path escapes asset root".into());
    }
    if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 64 * 1024 * 1024 {
        return Err("Music file exceeds limit".into());
    }
    Audio::decode(&std::fs::read(path).map_err(|e| e.to_string())?)
}
impl Bank {
    pub fn load(root: &Path, graph: &ObjectGraph) -> Result<Self> {
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        let mut levels = graph
            .objects
            .values()
            .filter(|o| o.kind == "cLevelSettings");
        let level = levels.next().ok_or("Missing level music settings")?;
        if levels.next().is_some() {
            return Err("Ambiguous level music settings".into());
        }
        let level_gain = field(level, "mLevelMusicVolume")
            .ok_or("Missing level music volume")?
            .parse::<f64>()
            .map_err(|_| "Invalid music volume")?;
        if !level_gain.is_finite() || !(0.0..=1.0).contains(&level_gain) {
            return Err("Invalid level music gain".into());
        }
        let mut tracks = BTreeMap::new();
        let mut cache: BTreeMap<(String, String), Arc<Stereo>> = BTreeMap::new();
        for (kind, path_key, file_key) in [
            ("INTRO", "mLevelIntroPath", "mLevelIntroFile"),
            ("MAIN_MUSIC", "mLevelMusicPath", "mLevelMusicFile"),
            ("OUTRO", "mLevelOutroPath", "mLevelOutroFile"),
        ] {
            let folder = field(level, path_key).ok_or("Missing music folder")?;
            let name = field(level, file_key).ok_or("Missing music filename")?;
            let key = (folder.to_owned(), name.to_owned());
            let stereo = if let Some(s) = cache.get(&key) {
                s.clone()
            } else {
                let right = if let Some(p) = name.strip_suffix("_L.adp") {
                    format!("{p}_R.adp")
                } else if let Some(p) = name.strip_suffix(" L.adp") {
                    format!("{p} R.adp")
                } else {
                    return Err("Unsupported stereo music naming".into());
                };
                let s = Arc::new(Stereo::new(
                    load_channel(&root, folder, name)?,
                    load_channel(&root, folder, &right)?,
                )?);
                cache.insert(key, s.clone());
                s
            };
            tracks.insert(kind.into(), stereo);
        }
        Ok(Self { tracks, level_gain })
    }
}
#[derive(Clone, Debug)]
struct Fade {
    start: f64,
    target: f64,
    seconds: f64,
    elapsed: f64,
}
#[derive(Clone)]
pub struct Transport {
    pub bank: Arc<Bank>,
    pub playing: Option<String>,
    pub cursor: usize,
    pub gain: f64,
    /// Interleaved normalized stereo PCM for the last committed simulation interval.
    /// A renderer/sink must consume this; simulation tests explicitly use an offline sink.
    pub block: Vec<f32>,
    pub sample_rate: u32,
    pub sequence: u64,
    fractional: f64,
    fade: Option<Fade>,
}
impl Transport {
    pub fn new(bank: Arc<Bank>) -> Self {
        Self {
            bank,
            playing: None,
            cursor: 0,
            gain: 1.0,
            block: Vec::new(),
            sample_rate: 24000,
            sequence: 0,
            fractional: 0.0,
            fade: None,
        }
    }
    pub fn call(&mut self, name: &str, args: &[Value]) -> Result<Vec<Value>> {
        match (name, args) {
            // DOL default at r2-0x7af8 is 5.0. This backend has no dialogue playback;
            // active-voice ducking must be integrated before enabling PhoneMessage.
            ("SetMusicVolume", [Value::Number(gain)]) => {
                return self.call(name, &[Value::Number(*gain), Value::Number(5.0)]);
            }
            ("SetMusicVolume", [Value::Number(gain), Value::Number(seconds)]) => {
                if !gain.is_finite()
                    || !(0.0..=1.0).contains(gain)
                    || !seconds.is_finite()
                    || !(0.0..=3600.0).contains(seconds)
                {
                    return Err("Invalid music gain/fade".into());
                }
                if *seconds == 0.0 {
                    self.gain = *gain;
                    self.fade = None;
                } else {
                    self.fade = Some(Fade {
                        start: self.gain,
                        target: *gain,
                        seconds: *seconds,
                        elapsed: 0.0,
                    });
                }
            }
            ("StartMusic", [Value::Enum(kind), rest @ ..])
                if matches!(kind.as_str(), "INTRO" | "MAIN_MUSIC" | "OUTRO")
                    && (rest.is_empty()
                        || matches!(rest, [Value::Number(start)] if *start == 0.0)) =>
            {
                let track = self
                    .bank
                    .tracks
                    .get(kind)
                    .ok_or("Missing requested music")?;
                track.loop_range()?;
                self.sample_rate = track.left.sample_rate;
                self.playing = Some(kind.clone());
                self.cursor = 0;
                self.fractional = 0.0;
            }
            ("StopMusic", []) => {
                self.playing = None;
                self.cursor = 0;
                self.fractional = 0.0;
            }
            _ => return Err(format!("Unsupported music call/signature: {name}")),
        }
        Ok(Vec::new())
    }
    /// No hardware time is inferred: this generates PCM at the simulation clock.
    /// Caller commits the output only after its complete transaction succeeds.
    pub fn advance(&mut self, dt: f64) -> Result<()> {
        if !dt.is_finite() || !(0.0..=1.0).contains(&dt) {
            return Err("Invalid music timestep".into());
        }
        let exact = self.fractional + dt * self.sample_rate as f64;
        let count = (exact + 1e-9).floor() as usize;
        self.fractional = (exact - count as f64).max(0.0);
        self.block.clear();
        self.block.reserve(count * 2);
        let track = self
            .playing
            .as_ref()
            .and_then(|k| self.bank.tracks.get(k))
            .cloned();
        let loop_range = track
            .as_ref()
            .map(|t| t.loop_range())
            .transpose()?
            .flatten();
        for _ in 0..count {
            if let Some((start, end)) = loop_range
                && self.cursor >= end
            {
                self.cursor = start + (self.cursor - end) % (end - start);
            }
            let gain = self.gain * self.bank.level_gain;
            let frame = track.as_ref().and_then(|t| {
                Some((
                    *t.left.pcm.get(self.cursor)?,
                    *t.right.pcm.get(self.cursor)?,
                ))
            });
            if let Some((l, r)) = frame {
                self.block.push(l as f32 / 32768.0 * gain as f32);
                self.block.push(r as f32 / 32768.0 * gain as f32);
                self.cursor += 1;
            } else {
                self.block.extend([0.0, 0.0]);
            }
            if let Some(f) = &mut self.fade {
                f.elapsed = (f.elapsed + 1.0 / self.sample_rate as f64).min(f.seconds);
                self.gain = f.start + (f.target - f.start) * (f.elapsed / f.seconds);
                if f.elapsed >= f.seconds {
                    self.gain = f.target;
                    self.fade = None;
                }
            }
        }
        if track
            .as_ref()
            .is_some_and(|t| !t.left.looping && self.cursor >= t.left.pcm.len())
        {
            self.playing = None;
        }
        self.sequence = self.sequence.wrapping_add(1);
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Transport {
        let a = Audio {
            sample_rate: 8,
            pcm: vec![16384; 16],
            looping: false,
            loop_nibbles: [2, 17],
        };
        let mut tracks = BTreeMap::new();
        tracks.insert("INTRO".into(), Arc::new(Stereo::new(a.clone(), a).unwrap()));
        Transport::new(Arc::new(Bank {
            tracks,
            level_gain: 0.8,
        }))
    }
    #[test]
    fn stereo_fade_pause_and_natural_end() {
        let mut t = fixture();
        t.call("SetMusicVolume", &[Value::Number(0.0), Value::Number(0.0)])
            .unwrap();
        t.call("StartMusic", &[Value::Enum("INTRO".into())])
            .unwrap();
        t.call("SetMusicVolume", &[Value::Number(1.0), Value::Number(1.0)])
            .unwrap();
        t.advance(1.0).unwrap();
        assert_eq!(t.block.len(), 16);
        assert_eq!(t.block[0], 0.0);
        assert!((t.block[14] - 0.35).abs() < 1e-6);
        assert_eq!(t.gain, 1.0);
        assert_eq!(t.cursor, 8);
        t.advance(0.0).unwrap();
        assert_eq!(t.cursor, 8);
        t.advance(1.0).unwrap();
        assert!(t.playing.is_none());
        assert!(t.block.iter().all(|v| (*v - 0.4).abs() < 1e-6));
        t.advance(0.5).unwrap();
        assert!(t.block.iter().all(|v| *v == 0.0));
    }
    #[test]
    fn split_intervals_and_invalid_requests() {
        let mut a = fixture();
        a.call("StartMusic", &[Value::Enum("INTRO".into())])
            .unwrap();
        let mut b = a.clone();
        a.advance(1.0).unwrap();
        let mut samples = Vec::new();
        for _ in 0..10 {
            b.advance(0.1).unwrap();
            samples.extend_from_slice(&b.block);
        }
        assert_eq!(a.block, samples);
        assert_eq!(a.cursor, b.cursor);
        assert!(
            b.call("SetMusicVolume", &[Value::Number(2.0), Value::Number(0.0)])
                .is_err()
        );
        assert!(
            b.call("StartMusic", &[Value::Enum("MAIN_MUSIC".into())])
                .is_err()
        );
        b.call("SetMusicVolume", &[Value::Number(0.0)]).unwrap();
        b.advance(1.0).unwrap();
        assert!((b.gain - 0.8).abs() < 1e-9);
        let cursor = b.cursor;
        assert!(b.advance(f64::NAN).is_err());
        assert_eq!(b.cursor, cursor);
        b.call("StopMusic", &[]).unwrap();
        assert!(b.playing.is_none());
    }
    #[test]
    fn main_music_honors_inclusive_dsp_loop_end() {
        let audio = Audio {
            sample_rate: 8,
            pcm: (0..8).map(|i| i * 1000).collect(),
            looping: true,
            loop_nibbles: [4, 9],
        };
        let track = Arc::new(Stereo::new(audio.clone(), audio).unwrap());
        let mut transport = Transport::new(Arc::new(Bank {
            tracks: [("MAIN_MUSIC".into(), track)].into_iter().collect(),
            level_gain: 1.0,
        }));
        transport
            .call(
                "StartMusic",
                &[Value::Enum("MAIN_MUSIC".into()), Value::Number(0.0)],
            )
            .unwrap();
        transport.advance(1.0).unwrap();
        transport.advance(1.0).unwrap();
        assert_eq!(transport.playing.as_deref(), Some("MAIN_MUSIC"));
        assert_eq!(transport.cursor, 4);
        let expected = [2, 3, 4, 5, 6, 7, 2, 3];
        for (frame, value) in transport.block.chunks_exact(2).zip(expected) {
            assert!((frame[0] - value as f32 * 1000.0 / 32768.0).abs() < 1e-6);
            assert_eq!(frame[0], frame[1]);
        }
    }
    #[test]
    fn mismatched_channels_rejected() {
        let a = Audio {
            sample_rate: 8,
            pcm: vec![1; 8],
            looping: false,
            loop_nibbles: [2, 17],
        };
        let mut b = a.clone();
        b.sample_rate = 16;
        assert!(Stereo::new(a, b).is_err());
    }
}
