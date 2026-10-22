//! Recovered BAN track tables. Rotation decoding is experimental; root motion and
//! compressed translation are preserved, not substituted with invented animation.
use crate::{
    Result,
    archive::{chunks, named_payload},
};
use serde::Serialize;
#[derive(Debug, Serialize)]
pub struct Track {
    pub name: String,
    pub translation_scale: [f32; 3],
    pub translation_offset: [f32; 3],
    pub packed_positions: Vec<u32>,
    pub packed_rotations: Vec<[u16; 4]>,
    pub fps: u16,
}
#[derive(Debug, Serialize)]
pub struct Animation {
    pub name: String,
    pub duration: f32,
    pub flags: u32,
    pub tracks: Vec<Track>,
    pub trailing_data: Vec<u8>,
}
fn be32(b: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_be_bytes(
        b.get(offset..offset + 4)
            .ok_or("Truncated BAN")?
            .try_into()
            .unwrap(),
    ))
}
/// 14/14/15/15 magnitude bits plus sign bits. Conjugation maps the observed
/// animation orientation to model-node orientation; interpolation flags unconfirmed.
pub fn rotation_candidate(raw: [u16; 4]) -> [f32; 4] {
    let mut q = [0.0; 4];
    for i in 0..4 {
        let bits = if i < 2 { 14 } else { 15 };
        let sign = 1 << bits;
        let mask = sign - 1;
        q[i] = (raw[i] & mask) as f32 / mask as f32 * if raw[i] & sign != 0 { -1.0 } else { 1.0 };
        if i < 3 {
            q[i] = -q[i];
        }
    }
    q
}
impl Track {
    pub fn sample_position(&self, time: f32) -> Option<[f32; 3]> {
        if self.packed_positions.is_empty() || !time.is_finite() {
            return None;
        }
        let unpack = |raw: u32| {
            [
                ((raw >> 21) & 0x7ff) as f32 * self.translation_scale[0]
                    + self.translation_offset[0],
                ((raw >> 10) & 0x7ff) as f32 * self.translation_scale[1]
                    + self.translation_offset[1],
                (raw & 0x3ff) as f32 * self.translation_scale[2] + self.translation_offset[2],
            ]
        };
        if self.packed_positions.len() == 1 {
            return Some(unpack(self.packed_positions[0]));
        }
        let frame = (time.max(0.0) * self.fps as f32).min((self.packed_positions.len() - 1) as f32);
        let i = frame.floor() as usize;
        let j = (i + 1).min(self.packed_positions.len() - 1);
        let t = frame - i as f32;
        let a = unpack(self.packed_positions[i]);
        let b = unpack(self.packed_positions[j]);
        Some(std::array::from_fn(|k| a[k] * (1.0 - t) + b[k] * t))
    }
    pub fn sample_rotation(&self, time: f32) -> Option<[f32; 4]> {
        if self.packed_rotations.is_empty() || !time.is_finite() {
            return None;
        }
        let frame = (time.max(0.0) * self.fps as f32).min((self.packed_rotations.len() - 1) as f32);
        let i = frame.floor() as usize;
        let j = (i + 1).min(self.packed_rotations.len() - 1);
        let t = frame - i as f32;
        let a = rotation_candidate(self.packed_rotations[i]);
        let mut b = rotation_candidate(self.packed_rotations[j]);
        if a.iter().zip(b).map(|(a, b)| a * b).sum::<f32>() < 0.0 {
            b = b.map(|v| -v);
        }
        let q = std::array::from_fn::<_, 4, _>(|i| a[i] * (1.0 - t) + b[i] * t);
        let norm = q.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm < 0.001 {
            return None;
        }
        Some(q.map(|v| v / norm))
    }
}
pub fn parse(name: &str, b: &[u8]) -> Result<Animation> {
    if b.len() < 88 || b.len() > 64 * 1024 * 1024 {
        return Err("Invalid BAN size".into());
    }
    if be32(b, 0)? != 0x3ecccccd {
        return Err("Unsupported BAN revision".into());
    }
    let duration = f32::from_bits(be32(b, 72)?);
    if !duration.is_finite() || !(0.0..=3600.0).contains(&duration) {
        return Err("Invalid BAN duration".into());
    }
    let count = be32(b, 76)? as usize;
    if count > 10000 {
        return Err("BAN track limit".into());
    }
    let mut cursor = 88 + count * 64;
    if cursor > b.len() {
        return Err("Truncated BAN table".into());
    }
    let mut tracks = Vec::new();
    for i in 0..count {
        let record = &b[88 + i * 64..88 + (i + 1) * 64];
        let end = record[..16].iter().position(|b| *b == 0).unwrap_or(16);
        let track_name = std::str::from_utf8(&record[..end])
            .map_err(|e| e.to_string())?
            .into();
        let np = be32(record, 16)? as usize;
        let nr = be32(record, 20)? as usize;
        if np > 100000 || nr > 100000 {
            return Err("BAN sample limit".into());
        }
        let translation_scale =
            std::array::from_fn(|i| f32::from_bits(be32(record, 32 + i * 4).unwrap()));
        let translation_offset =
            std::array::from_fn(|i| f32::from_bits(be32(record, 44 + i * 4).unwrap()));
        if translation_scale
            .iter()
            .chain(&translation_offset)
            .any(|v| !v.is_finite())
        {
            return Err("Nonfinite translation quantization".into());
        }
        let fps = u16::from_be_bytes(record[60..62].try_into().unwrap());
        if fps == 0 || fps > 240 {
            return Err("Invalid BAN sample rate".into());
        }
        let end = cursor + np * 4 + nr * 8;
        let data = b.get(cursor..end).ok_or("Truncated BAN samples")?;
        cursor = end;
        let packed_positions = data[..np * 4]
            .chunks_exact(4)
            .map(|v| u32::from_be_bytes(v.try_into().unwrap()))
            .collect();
        let packed_rotations = data[np * 4..]
            .chunks_exact(8)
            .map(|v| {
                std::array::from_fn(|i| u16::from_be_bytes(v[i * 2..i * 2 + 2].try_into().unwrap()))
            })
            .collect::<Vec<_>>();
        for raw in &packed_rotations {
            let q = rotation_candidate(*raw);
            let n = q.iter().map(|v| v * v).sum::<f32>();
            if (n - 1.0).abs() > 0.002 {
                return Err(format!(
                    "Quaternion magnitude invariant failed in {track_name}"
                ));
            }
        }
        tracks.push(Track {
            name: track_name,
            translation_scale,
            translation_offset,
            packed_positions,
            packed_rotations,
            fps,
        });
    }
    Ok(Animation {
        name: name.into(),
        duration,
        flags: be32(b, 4)?,
        tracks,
        trailing_data: b[cursor..].into(),
    })
}
pub fn from_archive(b: &[u8], wanted: &str) -> Result<Animation> {
    for c in chunks(b)? {
        if c.tag == *b"MINA" {
            let (name, data) = named_payload(c.payload)?;
            if name.trim_end_matches('\0').eq_ignore_ascii_case(wanted) {
                return parse(wanted, data);
            }
        }
    }
    Err(format!("Animation missing: {wanted}"))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        let mut b = vec![0; 88 + 64];
        b[..4].copy_from_slice(&0.4_f32.to_be_bytes());
        b[72..76].copy_from_slice(&(1.0_f32 / 30.0).to_be_bytes());
        b[76..80].copy_from_slice(&1u32.to_be_bytes());
        b[88..92].copy_from_slice(b"BONE");
        b[104..108].copy_from_slice(&1u32.to_be_bytes());
        b[108..112].copy_from_slice(&1u32.to_be_bytes());
        b[148..150].copy_from_slice(&30u16.to_be_bytes());
        b.extend([0; 10]);
        b.extend(32767u16.to_be_bytes());
        b
    }
    #[test]
    fn complete_track_and_truncation() {
        let b = fixture();
        let a = parse("test", &b).unwrap();
        assert_eq!(a.tracks[0].packed_positions.len(), 1);
        assert_eq!(
            a.tracks[0].sample_rotation(0.01),
            Some([-0.0, -0.0, -0.0, 1.0])
        );
        assert!(a.tracks[0].sample_rotation(f32::NAN).is_none());
        for end in 0..b.len() {
            assert!(parse("test", &b[..end]).is_err());
        }
    }
    #[test]
    fn identity_rotation() {
        assert_eq!(
            rotation_candidate([0, 0, 0, 32767]),
            [-0.0, -0.0, -0.0, 1.0]
        );
    }
    #[test]
    fn rejects_short() {
        assert!(parse("bad", &[0; 16]).is_err());
    }
    #[test]
    fn empty_track_has_no_pose() {
        let track = Track {
            name: "empty".into(),
            translation_scale: [0.; 3],
            translation_offset: [0.; 3],
            packed_positions: vec![],
            packed_rotations: vec![],
            fps: 30,
        };
        assert!(track.sample_rotation(1.0).is_none());
    }
}
