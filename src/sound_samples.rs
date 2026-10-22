//! Bounded BW2 DNOS sound-bank reader. HPSD names pair with standard DSP ADPCM DPSD payloads.
use crate::{
    Result,
    archive::{chunks, named_payload},
    dsp_audio::Audio,
};
use std::collections::BTreeMap;
pub struct Samples<'a> {
    entries: BTreeMap<&'a str, &'a [u8]>,
}
impl<'a> Samples<'a> {
    pub fn from_archive(bytes: &'a [u8]) -> Result<Self> {
        let containers = chunks(bytes)?;
        let banks: Vec<_> = containers.iter().filter(|c| c.tag == *b"DNOS").collect();
        let [bank] = banks.as_slice() else {
            return Err("Missing/ambiguous DNOS sound bank".into());
        };
        let (_, body) = named_payload(bank.payload)?;
        let records = chunks(body)?;
        let header = records.first().ok_or("Missing sound-bank count")?;
        if header.tag != *b"HFSB" || header.payload.len() != 4 {
            return Err("Invalid sound-bank header".into());
        }
        let count = u32::from_le_bytes(header.payload.try_into().unwrap()) as usize;
        if count > 4096 || records.len() != 1 + count * 2 {
            return Err("Invalid sound-bank count".into());
        }
        let mut entries = BTreeMap::new();
        for pair in records[1..].chunks_exact(2) {
            if pair[0].tag != *b"HPSD" || pair[1].tag != *b"DPSD" || pair[0].payload.len() != 32 {
                return Err("Unsupported sound-bank record pair".into());
            }
            let raw = pair[0].payload;
            let end = raw.iter().position(|v| *v == 0).unwrap_or(raw.len());
            if end == 0 || raw[end..].iter().any(|v| *v != 0) {
                return Err("Invalid sound name padding".into());
            }
            let name =
                std::str::from_utf8(&raw[..end]).map_err(|_| "Invalid sound name encoding")?;
            if entries.insert(name, pair[1].payload).is_some() {
                return Err("Duplicate sound name".into());
            }
        }
        Ok(Self { entries })
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn names(&self) -> impl Iterator<Item = &'a str> + '_ {
        self.entries.keys().copied()
    }
    pub fn decode(&self, name: &str) -> Result<Audio> {
        Audio::decode(
            self.entries
                .get(name)
                .ok_or_else(|| format!("Missing sound sample {name}"))?,
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn chunk(tag: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        [
            tag.as_slice(),
            &(payload.len() as u32).to_le_bytes(),
            payload,
        ]
        .concat()
    }
    fn bank(count: u32, sound_tag: &[u8; 4]) -> Vec<u8> {
        let mut body = chunk(b"HFSB", &count.to_le_bytes());
        let mut name = [0; 32];
        name[0] = b'x';
        body.extend(chunk(b"HPSD", &name));
        body.extend(chunk(sound_tag, &[0; 96]));
        let mut payload = vec![1, 0, 0, 0, b'b'];
        payload.extend(body);
        chunk(b"DNOS", &payload)
    }
    #[test]
    fn malformed_counts_tags_and_payloads_fail_explicitly() {
        assert!(Samples::from_archive(&bank(2, b"DPSD")).is_err());
        assert!(Samples::from_archive(&bank(1, b"????")).is_err());
        let bytes = bank(1, b"DPSD");
        let samples = Samples::from_archive(&bytes).unwrap();
        assert_eq!(samples.len(), 1);
        assert!(samples.decode("absent").is_err());
        assert!(samples.decode("x").is_err());
        for len in 0..bytes.len() {
            assert!(Samples::from_archive(&bytes[..len]).is_err());
        }
    }
}
