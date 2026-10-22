//! Bounded chunk-container decoding, based on the sampled BW2 resource/terrain files.
//! Chunk sizes are little-endian even though many payload fields are big-endian.
use crate::Result;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct Chunk<'a> {
    pub tag: [u8; 4],
    pub offset: usize,
    pub payload: &'a [u8],
}

pub fn chunks(data: &[u8]) -> Result<Vec<Chunk<'_>>> {
    let mut offset = 0usize;
    let mut result = Vec::new();
    while offset < data.len() {
        let header = data
            .get(offset..offset.saturating_add(8))
            .ok_or("Truncated chunk header")?;
        let tag: [u8; 4] = header[..4].try_into().unwrap();
        let size = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
        let start = offset.checked_add(8).ok_or("Offset overflow")?;
        let end = start.checked_add(size).ok_or("Size overflow")?;
        let payload = data
            .get(start..end)
            .ok_or("Chunk payload exceeds container")?;
        result.push(Chunk {
            tag,
            offset,
            payload,
        });
        if result.len() > 1_000_000 {
            return Err("Chunk-count limit exceeded".into());
        }
        offset = end;
    }
    Ok(result)
}

pub fn named_payload(data: &[u8]) -> Result<(&str, &[u8])> {
    let length = u32::from_le_bytes(
        data.get(..4)
            .ok_or("Missing resource name length")?
            .try_into()
            .unwrap(),
    ) as usize;
    let end = 4usize.checked_add(length).ok_or("Name length overflow")?;
    let name = std::str::from_utf8(data.get(4..end).ok_or("Name exceeds payload")?)
        .map_err(|e| e.to_string())?;
    if name.is_empty() || name.contains('\0') {
        return Err("Empty or NUL-containing resource name".into());
    }
    Ok((name, &data[end..]))
}

#[derive(Debug, Serialize)]
pub struct Entry {
    pub tag: String,
    pub offset: usize,
    pub size: usize,
    pub name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Texture {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub format_tag: String,
    pub payload_size: usize,
}

#[derive(Debug, Serialize)]
pub struct Inventory {
    pub size: usize,
    pub chunk_count: usize,
    pub chunk_types: BTreeMap<String, usize>,
    pub entries: Vec<Entry>,
    pub textures: Vec<Texture>,
}

fn tag_text(tag: &[u8]) -> String {
    String::from_utf8_lossy(tag).into_owned()
}

/// Decode only headers observed in the sample. Unknown tags remain inventoried.
pub fn inventory(data: &[u8]) -> Result<Inventory> {
    let parsed = chunks(data)?;
    let mut out = Inventory {
        size: data.len(),
        chunk_count: parsed.len(),
        chunk_types: BTreeMap::new(),
        entries: Vec::new(),
        textures: Vec::new(),
    };
    for chunk in parsed {
        let tag = tag_text(&chunk.tag);
        *out.chunk_types.entry(tag.clone()).or_default() += 1;
        let mut name = None;
        if matches!(&chunk.tag, b"PRCS" | b"LDOM" | b"RXET") {
            let (resource_name, body) = named_payload(chunk.payload)?;
            name = Some(resource_name.to_owned());
            if chunk.tag == *b"RXET" {
                let groups = chunks(body)?;
                for group in groups {
                    if group.tag != *b"FTBG" {
                        continue;
                    }
                    let count = u32::from_le_bytes(
                        group
                            .payload
                            .get(..4)
                            .ok_or("Missing texture count")?
                            .try_into()
                            .unwrap(),
                    ) as usize;
                    let textures = chunks(&group.payload[4..])?;
                    if textures.len() != count {
                        return Err("Texture count mismatch".into());
                    }
                    for texture in textures {
                        if texture.tag != *b"DXTG" {
                            return Err("Unknown texture entry tag".into());
                        }
                        let p = texture.payload;
                        if p.len() < 64 {
                            return Err("Truncated texture header".into());
                        }
                        let end = p[..32].iter().position(|b| *b == 0).unwrap_or(32);
                        let name = std::str::from_utf8(&p[..end])
                            .map_err(|e| e.to_string())?
                            .to_owned();
                        out.textures.push(Texture {
                            name,
                            width: u32::from_be_bytes(p[32..36].try_into().unwrap()),
                            height: u32::from_be_bytes(p[36..40].try_into().unwrap()),
                            format_tag: tag_text(&p[52..56]),
                            payload_size: p.len(),
                        });
                    }
                }
            }
        }
        out.entries.push(Entry {
            tag,
            offset: chunk.offset,
            size: chunk.payload.len(),
            name,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn chunk(tag: &[u8; 4], data: &[u8]) -> Vec<u8> {
        [tag.as_slice(), &(data.len() as u32).to_le_bytes(), data].concat()
    }
    #[test]
    fn parses_empty_and_unknown_chunks() {
        let bytes = [chunk(b"WHAT", &[]), chunk(b"TEST", b"abc")].concat();
        let found = chunks(&bytes).unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found[1].offset, 8);
        assert_eq!(found[1].payload, b"abc");
    }
    #[test]
    fn rejects_truncation() {
        let bytes = chunk(b"TEST", b"abc");
        for end in 1..bytes.len() {
            assert!(chunks(&bytes[..end]).is_err());
        }
    }
    #[test]
    fn rejects_huge_size() {
        assert!(chunks(b"TEST\xff\xff\xff\xff").is_err());
    }
    #[test]
    fn named_resource() {
        let bytes = [3u32.to_le_bytes().as_slice(), b"abcDATA"].concat();
        assert_eq!(named_payload(&bytes).unwrap(), ("abc", b"DATA".as_slice()));
        assert!(named_payload(&[255, 255, 255, 255]).is_err());
    }
    #[test]
    fn texture_headers() {
        let mut p = vec![0u8; 64];
        p[..3].copy_from_slice(b"tex");
        p[32..36].copy_from_slice(&64u32.to_be_bytes());
        p[36..40].copy_from_slice(&32u32.to_be_bytes());
        p[52..56].copy_from_slice(b"1TXD");
        let group = chunk(
            b"FTBG",
            &[1u32.to_le_bytes().as_slice(), &chunk(b"DXTG", &p)].concat(),
        );
        let data = chunk(
            b"RXET",
            &[1u32.to_le_bytes().as_slice(), b"x", &group].concat(),
        );
        let i = inventory(&data).unwrap();
        assert_eq!(i.textures[0].width, 64);
        assert_eq!(i.textures[0].height, 32);
        assert_eq!(i.textures[0].name, "tex");
    }
}
