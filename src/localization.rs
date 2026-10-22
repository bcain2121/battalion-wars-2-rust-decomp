//! Bounded original STR reader. Layout informed by the attributed MIT editor reference.
//! Audio duration 0/-1 is unspecified, never permission to finish dialogue immediately.
use crate::Result;
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub audio_path: String,
    pub audio_name: String,
    pub text: String,
    pub declared_duration: Option<f32>,
    /// Preserve zero versus negative sentinel for native playback lifecycle recovery.
    pub raw_duration: f32,
}
#[derive(Debug, Clone)]
pub struct Language {
    pub messages: Vec<Message>,
}
fn u32_at(data: &[u8], at: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        data.get(at..at.checked_add(4).ok_or("STR offset overflow")?)
            .ok_or("Truncated STR integer")?
            .try_into()
            .unwrap(),
    ))
}
fn ascii(data: &[u8], offset: usize, start: usize) -> Result<String> {
    if offset < start {
        return Err("STR string overlaps slot table".into());
    }
    let tail = data.get(offset..).ok_or("STR string offset outside file")?;
    let end = tail
        .iter()
        .position(|&b| b == 0)
        .ok_or("Unterminated STR ASCII string")?;
    if !tail[..end].is_ascii() {
        return Err("Non-ASCII STR audio identifier".into());
    }
    String::from_utf8(tail[..end].to_vec()).map_err(|e| e.to_string())
}
fn utf16(data: &[u8], offset: usize, start: usize) -> Result<String> {
    if offset < start || !offset.is_multiple_of(2) {
        return Err("Invalid STR text offset".into());
    }
    let tail = data.get(offset..).ok_or("STR text outside file")?;
    let mut words = Vec::new();
    for pair in tail.chunks_exact(2) {
        let w = u16::from_le_bytes([pair[0], pair[1]]);
        if w == 0 {
            return String::from_utf16(&words)
                .map(|s| s.replace('\u{92}', "'"))
                .map_err(|e| e.to_string());
        }
        words.push(w);
    }
    Err("Unterminated STR UTF-16 text".into())
}
impl Language {
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() > 16 * 1024 * 1024 || data.len() < 16 {
            return Err("Invalid STR file size".into());
        }
        if u32_at(data, 0)? != 0x05a177 {
            return Err("Unsupported STR magic".into());
        }
        let count = u32_at(data, 4)? as usize;
        let size = u32_at(data, 8)? as usize;
        if size != data.len() {
            return Err("STR declared file size mismatch".into());
        }
        let start = 16usize
            .checked_add(count.checked_mul(56).ok_or("STR slot overflow")?)
            .ok_or("STR header overflow")?;
        if count > 10000 || start > data.len() {
            return Err("Invalid STR slot count".into());
        }
        let mut messages = Vec::with_capacity(count);
        for i in 0..count {
            let base = 16 + i * 56;
            if u32_at(data, base)? != 0x7f7f7f {
                return Err("Invalid STR slot magic".into());
            }
            let declared_duration = f32::from_bits(u32_at(data, base + 20)?);
            if !declared_duration.is_finite()
                || (declared_duration < 0.0 && declared_duration != -1.0)
            {
                return Err("Invalid STR audio duration".into());
            }
            let raw_duration = declared_duration;
            let declared_duration = (declared_duration > 0.0).then_some(declared_duration);
            let audio_path = ascii(data, u32_at(data, base + 4)? as usize, start)?;
            let audio_name = ascii(data, u32_at(data, base + 8)? as usize, start)?;
            ascii(data, u32_at(data, base + 12)? as usize, start)?;
            let text = utf16(data, u32_at(data, base + 28)? as usize, start)?;
            messages.push(Message {
                audio_path,
                audio_name,
                text,
                declared_duration,
                raw_duration,
            });
        }
        Ok(Self { messages })
    }
    pub fn message(&self, id: usize) -> Result<&Message> {
        self.messages
            .get(id)
            .ok_or_else(|| format!("Unknown localized message {id}"))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        let mut d = vec![0; 76];
        for (at, value) in [
            (0, 0x05a177u32),
            (4, 1),
            (16, 0x7f7f7f),
            (20, 72),
            (24, 72),
            (28, 72),
            (44, 74),
        ] {
            d[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        d[36..40].copy_from_slice(&2.5f32.to_le_bytes());
        d.truncate(74);
        for w in "Advance!".encode_utf16().chain([0]) {
            d.extend_from_slice(&w.to_le_bytes());
        }
        let size = d.len() as u32;
        d[8..12].copy_from_slice(&size.to_le_bytes());
        d
    }
    #[test]
    fn localized_text_and_duration() {
        let l = Language::parse(&fixture()).unwrap();
        assert_eq!(l.message(0).unwrap().text, "Advance!");
        assert_eq!(l.messages[0].declared_duration, Some(2.5));
        assert!(l.message(1).is_err());
    }
    #[test]
    fn every_truncation_and_invalid_offsets_fail() {
        let d = fixture();
        for i in 0..d.len() {
            assert!(Language::parse(&d[..i]).is_err(), "truncation {i}");
        }
        for offset in [0u32, 75, u32::MAX] {
            let mut bad = d.clone();
            bad[44..48].copy_from_slice(&offset.to_le_bytes());
            assert!(Language::parse(&bad).is_err());
        }
        let mut bad = d.clone();
        bad[36..40].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(Language::parse(&bad).is_err());
    }
}
