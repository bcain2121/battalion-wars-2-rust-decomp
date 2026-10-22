//! Nintendo DSP ADPCM (mono .adp) decoding to signed PCM and a WAV container.
//! Loop metadata is retained; this module does not silently loop or finish playback.
use crate::Result;
#[derive(Debug, Clone)]
pub struct Audio {
    pub sample_rate: u32,
    pub pcm: Vec<i16>,
    pub looping: bool,
    pub loop_nibbles: [u32; 2],
}
fn u32_at(b: &[u8], i: usize) -> u32 {
    u32::from_be_bytes(b[i..i + 4].try_into().unwrap())
}
fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_be_bytes(b[i..i + 2].try_into().unwrap())
}
impl Audio {
    pub fn decode(b: &[u8]) -> Result<Self> {
        if b.len() < 96 || b.len() > 64 * 1024 * 1024 {
            return Err("Invalid DSP file size".into());
        }
        let samples = u32_at(b, 0) as usize;
        let nibbles = u32_at(b, 4) as usize;
        let rate = u32_at(b, 8);
        if samples == 0
            || samples > 32 * 1024 * 1024
            || !(8000..=192000).contains(&rate)
            || u16_at(b, 14) != 0
            || u16_at(b, 12) > 1
        {
            return Err("Unsupported DSP header".into());
        }
        let frames = samples.div_ceil(14);
        let used_nibbles = samples + frames * 2;
        if nibbles < used_nibbles || nibbles > frames * 16 || 96 + nibbles.div_ceil(2) > b.len() {
            return Err("Invalid/truncated DSP sample payload".into());
        }
        let mut coefficients = [0i16; 16];
        for (i, c) in coefficients.iter_mut().enumerate() {
            *c = u16_at(b, 28 + i * 2) as i16;
        }
        let mut h1 = u16_at(b, 64) as i16 as i64;
        let mut h2 = u16_at(b, 66) as i16 as i64;
        let looping = u16_at(b, 12) != 0;
        let loop_nibbles = [u32_at(b, 16), u32_at(b, 20)];
        if looping && (loop_nibbles[0] > loop_nibbles[1] || loop_nibbles[1] as usize >= nibbles) {
            return Err("Invalid DSP loop addresses".into());
        }
        let mut pcm = Vec::with_capacity(samples);
        for frame in 0..frames {
            let offset = 96 + frame * 8;
            let header = *b.get(offset).ok_or("Truncated DSP frame")?;
            let predictor = (header >> 4) as usize;
            if predictor >= 8 {
                return Err("Invalid DSP predictor".into());
            }
            let scale = 1i64 << (header & 15);
            let c1 = coefficients[predictor * 2] as i64;
            let c2 = coefficients[predictor * 2 + 1] as i64;
            for j in 0..14.min(samples - pcm.len()) {
                let byte = *b.get(offset + 1 + j / 2).ok_or("Truncated DSP nibble")?;
                let nibble = if j % 2 == 0 { byte >> 4 } else { byte & 15 };
                let signed = if nibble >= 8 {
                    nibble as i64 - 16
                } else {
                    nibble as i64
                };
                let value = (((signed * scale) << 11) + 1024 + c1 * h1 + c2 * h2) >> 11;
                let value = value.clamp(i16::MIN as i64, i16::MAX as i64) as i16;
                pcm.push(value);
                h2 = h1;
                h1 = value as i64;
            }
        }
        Ok(Self {
            sample_rate: rate,
            pcm,
            looping,
            loop_nibbles,
        })
    }
    pub fn duration(&self) -> f64 {
        self.pcm.len() as f64 / self.sample_rate as f64
    }
    pub fn wav(&self) -> Vec<u8> {
        let size = (self.pcm.len() * 2) as u32;
        let mut b = Vec::with_capacity(44 + size as usize);
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + size).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&self.sample_rate.to_le_bytes());
        b.extend_from_slice(&(self.sample_rate * 2).to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&size.to_le_bytes());
        for sample in &self.pcm {
            b.extend_from_slice(&sample.to_le_bytes());
        }
        b
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        let mut b = vec![0; 104];
        for (i, v) in [(0, 14u32), (4, 16), (8, 24000)] {
            b[i..i + 4].copy_from_slice(&v.to_be_bytes());
        }
        b[97] = 0x1f;
        b[98] = 0x78;
        b
    }
    #[test]
    fn signed_nibbles_and_wav() {
        let a = Audio::decode(&fixture()).unwrap();
        assert_eq!(&a.pcm[..4], &[1, -1, 7, -8]);
        assert_eq!(a.pcm.len(), 14);
        assert_eq!(a.duration(), 14.0 / 24000.0);
        let wav = a.wav();
        assert_eq!(wav.len(), 72);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[44..48], &[1, 0, 255, 255]);
    }
    #[test]
    fn prediction_histories_and_saturation() {
        let mut b = fixture();
        b[97..].fill(0);
        b[28..30].copy_from_slice(&2048i16.to_be_bytes());
        b[64..66].copy_from_slice(&1000i16.to_be_bytes());
        assert!(Audio::decode(&b).unwrap().pcm.iter().all(|v| *v == 1000));
        b[96] = 15;
        b[97] = 0x77;
        assert_eq!(Audio::decode(&b).unwrap().pcm[0], i16::MAX);
    }
    #[test]
    fn truncated_bad_predictor_and_header() {
        let b = fixture();
        for end in 0..b.len() {
            assert!(Audio::decode(&b[..end]).is_err());
        }
        let mut bad = b.clone();
        bad[96] = 0x80;
        assert!(Audio::decode(&bad).is_err());
        let mut bad = b.clone();
        bad[4..8].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(Audio::decode(&bad).is_err());
        let mut bad = b;
        bad[8..12].fill(0);
        assert!(Audio::decode(&bad).is_err());
    }
}
