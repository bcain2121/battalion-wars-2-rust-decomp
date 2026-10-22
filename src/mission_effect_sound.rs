//! Rust-owned one-shot positional effect voice. PCM decoding/playhead, not hardware output.
//! Single-sample profiles only; caller supplies a deterministic pitch-variation choice.
use crate::{
    ObjectGraph, ObjectId, Result, dsp_audio::Audio, original_scene::field, sound_samples::Samples,
};
use std::sync::Arc;
#[derive(Clone, Debug)]
pub struct Voice {
    pub source: ObjectId,
    pub position: [f32; 3],
    pub audio: Arc<Audio>,
    pub pitch: f64,
    gain: f64,
    falloff: [f64; 2],
    elapsed: f64,
}
impl Voice {
    pub fn from_profile(
        graph: &ObjectGraph,
        samples: &Samples<'_>,
        source: ObjectId,
        position: [f32; 3],
        variation: f64,
    ) -> Result<Self> {
        if !variation.is_finite()
            || !(-1.0..=1.0).contains(&variation)
            || position.iter().any(|x| !x.is_finite())
        {
            return Err("Invalid sound position/variation".into());
        }
        let object = graph.objects.get(&source).ok_or("Missing sound profile")?;
        if object.kind != "cSoundBase" {
            return Err("Expected cSoundBase".into());
        }
        let number = |key| -> Result<f64> {
            let value = field(object, key)
                .ok_or_else(|| format!("Missing sound {key}"))?
                .parse::<f64>()
                .map_err(|_| "Invalid sound scalar")?;
            if !value.is_finite() {
                return Err("Nonfinite sound scalar".into());
            }
            Ok(value)
        };
        if number("mVolumeVariation")? != 0.0 {
            return Err("Sound volume variation not configured".into());
        }
        let gain = number("mVolume")?;
        let pitch_range = number("mPitchMultiplierVariation")?;
        let pitch = number("mPitchMultiplier")? + variation * pitch_range;
        let falloff = [number("mFalloffMin")?, number("mFalloffMax")?];
        if !(0.0..=16.0).contains(&gain)
            || pitch_range < 0.0
            || !(0.01..=8.0).contains(&pitch)
            || falloff[0] < 0.0
            || falloff[1] <= falloff[0]
        {
            return Err("Unsupported sound scalar range".into());
        }
        let fields: Vec<_> = object
            .fields
            .iter()
            .filter(|f| f.attributes.get("name").is_some_and(|n| n == "mSample"))
            .collect();
        let [sample_field] = fields.as_slice() else {
            return Err("Missing/ambiguous sound samples".into());
        };
        let ids = sample_field
            .items
            .iter()
            .map(|v| {
                v.parse::<u64>()
                    .map_err(|_| "Invalid sample reference".to_string())
            })
            .collect::<Result<Vec<_>>>()?;
        let ids: Vec<_> = ids.into_iter().filter(|id| *id != 0).collect();
        let [id] = ids.as_slice() else {
            return Err("Sound sample selection requires exactly one non-null sample".into());
        };
        let sample = graph
            .objects
            .get(&ObjectId(*id))
            .ok_or("Missing sample resource")?;
        if sample.kind != "sSampleResource" {
            return Err("Invalid sample resource type".into());
        }
        let audio = samples.decode(field(sample, "mName").ok_or("Missing sample name")?)?;
        if audio.looping {
            return Err("Looping effect sound requires stop/loop semantics".into());
        }
        Ok(Self {
            source,
            position,
            audio: Arc::new(audio),
            pitch,
            gain,
            falloff,
            elapsed: 0.0,
        })
    }
    pub fn finished(&self) -> bool {
        self.elapsed >= self.audio.duration() / self.pitch
    }
    pub fn advance(&mut self, dt: f64) -> Result<()> {
        if !dt.is_finite() || !(0.0..=1.0).contains(&dt) {
            return Err("Invalid effect voice delta".into());
        }
        self.elapsed = (self.elapsed + dt).min(self.audio.duration() / self.pitch);
        Ok(())
    }
    /// Render a non-consuming stereo PCM window from the current playhead.
    /// Distance attenuation is Rust-authored linear falloff; native sound flags are not emulated.
    pub fn render(&self, listener: [f32; 3], frames: usize, rate: u32) -> Result<Vec<[f32; 2]>> {
        if listener.iter().any(|v| !v.is_finite())
            || !(8000..=192000).contains(&rate)
            || frames > rate as usize
        {
            return Err("Invalid effect voice render request".into());
        }
        if self.finished() {
            return Ok(vec![[0.; 2]; frames]);
        }
        let distance = self
            .position
            .iter()
            .zip(listener)
            .map(|(a, b)| (*a as f64 - b as f64).powi(2))
            .sum::<f64>()
            .sqrt();
        let attenuation = ((self.falloff[1] - distance) / (self.falloff[1] - self.falloff[0]))
            .clamp(0.0, 1.0)
            * self.gain;
        Ok((0..frames)
            .map(|i| {
                let sample = (self.elapsed + i as f64 / rate as f64)
                    * self.pitch
                    * self.audio.sample_rate as f64;
                let value = self.audio.pcm.get(sample as usize).copied().unwrap_or(0) as f64
                    / 32768.0
                    * attenuation;
                [value as f32; 2]
            })
            .collect())
    }
}
