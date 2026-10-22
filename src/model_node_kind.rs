//! Native node tag resolution and spatial dispatch classification.
//! Loads the complete RBWE01 static tag table from mapped DOL data; no partial default catalog.
use crate::Result;

#[derive(Debug, Clone)]
pub struct Tag {
    pub name: String,
    pub kind: u16,
    pub flag: u8,
}
#[derive(Debug, Clone)]
pub struct Catalog {
    tags: Vec<Tag>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    NameTag,
    AuthoredHeader,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolution {
    pub kind: u16,
    pub flag: u8,
    pub source: Source,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpatialDispatch {
    Box,
    Sphere,
    Capsule,
    Ignored,
}
impl Resolution {
    /// 0x8023a0e0..0x8023a140 dispatch only, NOT execution of volume updates.
    pub fn spatial_dispatch(self) -> SpatialDispatch {
        match self.kind {
            1 | 31 => SpatialDispatch::Box,
            150 => SpatialDispatch::Sphere,
            151 => SpatialDispatch::Capsule,
            _ => SpatialDispatch::Ignored,
        }
    }
}
impl Catalog {
    /// Read the complete static RBWE01 table, stopping at its real null-name terminator.
    /// Dynamic BONE_ allocation remains unsupported. Addresses/layout are version-specific;
    /// this validates table structure, not a whole-file authenticity hash.
    pub fn from_rbwe01_dol(bytes: &[u8]) -> Result<Self> {
        let mut tags = Vec::new();
        for index in 0..700u32 {
            let row = mapped(bytes, 0x804e_c2f0 + index * 8, 8)?;
            let pointer = u32::from_be_bytes(row[4..8].try_into().unwrap());
            if pointer == 0 {
                if tags.len() != 250 {
                    return Err("Unsupported RBWE01 static tag-table length".into());
                }
                let catalog = Self::new(tags)?;
                if catalog.tags[0].name != "UNTAGGED"
                    || catalog.tags[0].kind != 0
                    || catalog.tags[1].name != "NODRAW"
                    || catalog.tags[1].kind != 1
                {
                    return Err("Unsupported RBWE01 tag-table layout".into());
                }
                return Ok(catalog);
            }
            let mut name = Vec::new();
            for offset in 0..512u32 {
                let address = pointer.checked_add(offset).ok_or("Tag address overflow")?;
                let byte = mapped(bytes, address, 1)?[0];
                if byte == 0 {
                    break;
                }
                name.push(byte);
            }
            if name.len() == 512 {
                return Err("Unterminated native node tag".into());
            }
            let name = String::from_utf8(name)
                .map_err(|_| "Invalid native node-tag encoding")?
                .to_ascii_uppercase();
            tags.push(Tag {
                name,
                kind: u16::from_be_bytes([row[2], row[3]]),
                flag: row[0],
            });
        }
        Err("Unterminated native node-tag table".into())
    }
    pub fn static_tag_count(&self) -> usize {
        self.tags.len()
    }

    /// Preserve table order, including duplicates: equal-length matches keep the first row.
    /// Native table initialization uppercases names. This API expects that initialized form.
    /// Internal construction also supports bounded explicit test fixtures.
    fn new(tags: Vec<Tag>) -> Result<Self> {
        if tags.is_empty()
            || tags.len() > 700
            || tags.iter().any(|t| {
                t.name.is_empty()
                    || !t.name.is_ascii()
                    || t.name
                        .bytes()
                        .any(|b| b.is_ascii_control() || b.is_ascii_lowercase())
            })
        {
            return Err("Invalid initialized node-tag catalog".into());
        }
        Ok(Self { tags })
    }
    /// Native strcmp/strstr are case-sensitive. Do not uppercase the node name here.
    pub fn resolve(&self, name: &str, authored_kind: u16) -> Result<Resolution> {
        if name.is_empty() || !name.is_ascii() || name.bytes().any(|b| b.is_ascii_control()) {
            return Err("Unsupported native node name".into());
        }
        let exact = self.tags.iter().find(|t| t.name == name);
        let mut best = exact;
        if best.is_none() {
            for tag in &self.tags {
                if name.contains(&tag.name)
                    && best.is_none_or(|old| tag.name.len() > old.name.len())
                {
                    best = Some(tag);
                }
            }
        }
        if let Some(tag) = best {
            // 10000 is the native header-fallback sentinel, even if a table row returned it.
            return Ok(Resolution {
                kind: if tag.kind == 10000 {
                    authored_kind
                } else {
                    tag.kind
                },
                flag: tag.flag,
                source: if tag.kind == 10000 {
                    Source::AuthoredHeader
                } else {
                    Source::NameTag
                },
            });
        }
        if name.starts_with("BONE_") {
            return Err("Dynamic bone-tag allocation not configured".into());
        }
        Ok(Resolution {
            kind: authored_kind,
            flag: 0,
            source: Source::AuthoredHeader,
        })
    }
}

/// DOL has 18 file-backed sections. Refuse truncation, overflow and ambiguous mappings.
fn mapped(bytes: &[u8], address: u32, count: usize) -> Result<&[u8]> {
    if bytes.len() < 0x100 {
        return Err("Truncated DOL header".into());
    }
    let word = |offset| u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap());
    let requested_end = u64::from(address) + count as u64;
    let mut found = None;
    for index in 0..18 {
        let size = word(0x90 + index * 4);
        if size == 0 {
            continue;
        }
        let start = word(0x48 + index * 4);
        let end = start.checked_add(size).ok_or("DOL address overflow")?;
        let file = word(index * 4) as usize;
        let file_end = file
            .checked_add(size as usize)
            .ok_or("DOL file offset overflow")?;
        if file_end > bytes.len() {
            return Err("Truncated DOL section".into());
        }
        if start <= address && requested_end <= u64::from(end) {
            if found.is_some() {
                return Err("Ambiguous DOL address mapping".into());
            }
            let offset = file + (address - start) as usize;
            found = Some(&bytes[offset..offset + count]);
        }
    }
    found.ok_or_else(|| "Unmapped native node-tag address".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tag(name: &str, kind: u16, flag: u8) -> Tag {
        Tag {
            name: name.into(),
            kind,
            flag,
        }
    }
    fn dol_fixture() -> Vec<u8> {
        let mut bytes = vec![0; 0x100 + 251 * 8];
        for i in 0..250 {
            let name = match i {
                0 => "UNTAGGED".into(),
                1 => "NODRAW".into(),
                _ => format!("TAG_{i}"),
            };
            let pointer = 0x804e_c2f0u32 + (bytes.len() - 0x100) as u32;
            let row = 0x100 + i * 8;
            bytes[row + 2..row + 4].copy_from_slice(&(i as u16).to_be_bytes());
            bytes[row + 4..row + 8].copy_from_slice(&pointer.to_be_bytes());
            bytes.extend_from_slice(name.as_bytes());
            bytes.push(0);
        }
        let size = (bytes.len() - 0x100) as u32;
        bytes[..4].copy_from_slice(&0x100u32.to_be_bytes());
        bytes[0x48..0x4c].copy_from_slice(&0x804e_c2f0u32.to_be_bytes());
        bytes[0x90..0x94].copy_from_slice(&size.to_be_bytes());
        bytes
    }
    #[test]
    fn mapped_catalog_rejects_partial_truncated_unmapped_and_ambiguous_data() {
        let bytes = dol_fixture();
        assert_eq!(
            Catalog::from_rbwe01_dol(&bytes).unwrap().static_tag_count(),
            250
        );
        assert!(Catalog::from_rbwe01_dol(&bytes[..0xff]).is_err());
        assert!(Catalog::from_rbwe01_dol(&bytes[..bytes.len() - 1]).is_err());
        let mut bad = bytes.clone();
        bad[0x10c..0x110].fill(0);
        assert!(Catalog::from_rbwe01_dol(&bad).is_err());
        let mut bad = bytes.clone();
        bad[0x104..0x108].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(Catalog::from_rbwe01_dol(&bad).is_err());
        let mut bad = bytes.clone();
        bad[4..8].copy_from_slice(&bytes[..4]);
        bad[0x4c..0x50].copy_from_slice(&bytes[0x48..0x4c]);
        bad[0x94..0x98].copy_from_slice(&bytes[0x90..0x94]);
        assert!(Catalog::from_rbwe01_dol(&bad).is_err());
    }
    #[test]
    fn exact_longest_ties_and_case_sensitive_fallback() {
        let c = Catalog::new(vec![
            tag("A", 1, 2),
            tag("AA", 31, 3),
            tag("BB", 150, 4),
            tag("AA", 151, 5),
        ])
        .unwrap();
        assert_eq!(c.resolve("AA", 0).unwrap().kind, 31);
        assert_eq!(c.resolve("X_AABB", 0).unwrap().kind, 31);
        assert_eq!(
            c.resolve("aa", 62).unwrap(),
            Resolution {
                kind: 62,
                flag: 0,
                source: Source::AuthoredHeader
            }
        );
        assert_eq!(c.resolve("X_AA", 0).unwrap().flag, 3);
    }
    #[test]
    fn header_fallback_and_dynamic_tags_are_not_invented() {
        let c = Catalog::new(vec![tag("NODRAW", 1, 0)]).unwrap();
        assert_eq!(
            c.resolve("ROOT", 31).unwrap().spatial_dispatch(),
            SpatialDispatch::Box
        );
        assert_eq!(
            c.resolve("ROOT", 0).unwrap().spatial_dispatch(),
            SpatialDispatch::Ignored
        );
        assert!(c.resolve("BONE_CUSTOM", 0).is_err());
        assert_eq!(c.resolve("BONE_NODRAW", 0).unwrap().kind, 1);
        assert!(Catalog::new(vec![]).is_err());
        assert!(Catalog::new(vec![tag("lower", 1, 0)]).is_err());
        assert!(c.resolve("bad\0name", 0).is_err());
        let sentinel = Catalog::new(vec![tag("SPECIAL", 10000, 7)]).unwrap();
        assert_eq!(
            sentinel.resolve("SPECIAL", 31).unwrap(),
            Resolution {
                kind: 31,
                flag: 7,
                source: Source::AuthoredHeader,
            }
        );
        for (kind, expected) in [
            (150, SpatialDispatch::Sphere),
            (151, SpatialDispatch::Capsule),
            (62, SpatialDispatch::Ignored),
        ] {
            assert_eq!(
                c.resolve("ROOT", kind).unwrap().spatial_dispatch(),
                expected
            );
        }
    }
}
