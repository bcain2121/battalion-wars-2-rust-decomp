//! Original levels.xml entries, in source order. Parsing does not launch a level.
use crate::{
    Result,
    vehicle_profile::{Engine, Override},
};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Entry {
    pub name: String,
    pub filename: String,
    pub gametype: String,
    pub players: Option<u8>,
    pub physics: Override,
    pub physics_attribute: Option<u8>,
}

#[derive(Debug)]
pub struct Catalog {
    pub entries: Vec<Entry>,
}

impl Catalog {
    pub fn parse(xml: &str) -> Result<Self> {
        if xml.len() > 1_048_576 {
            return Err("Level catalog exceeds supported size".into());
        }
        let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
        let root = doc.root_element();
        if root.tag_name().name() != "levels"
            || root.tag_name().namespace().is_some()
            || root.attributes().len() != 0
        {
            return Err("Unsupported level catalog root".into());
        }
        let mut entries = Vec::new();
        for node in root.children() {
            if node.is_comment() || node.is_text() && node.text().unwrap_or("").trim().is_empty() {
                continue;
            }
            if !node.is_element()
                || node.tag_name().name() != "level"
                || node.tag_name().namespace().is_some()
                || node.children().any(|n| {
                    n.is_element() || n.is_text() && !n.text().unwrap_or("").trim().is_empty()
                })
            {
                return Err("Unsupported level catalog content".into());
            }
            for a in node.attributes() {
                if a.namespace().is_some()
                    || !matches!(
                        a.name(),
                        "name" | "filename" | "gametype" | "players" | "physics"
                    )
                {
                    return Err(format!("Unsupported level attribute {}", a.name()));
                }
            }
            let required = |key| -> Result<String> {
                let s = node
                    .attribute(key)
                    .ok_or_else(|| format!("Missing level attribute {key}"))?;
                if s.is_empty() || s.trim() != s || s.chars().any(char::is_control) {
                    return Err(format!("Invalid level attribute {key}"));
                }
                Ok(s.to_owned())
            };
            let name = required("name")?;
            let filename = required("filename")?;
            let tail = filename
                .strip_prefix("@os:Data/CompoundFiles/")
                .ok_or("Unsupported level filename namespace")?;
            if !tail.ends_with(".xml")
                || tail.len() <= 4
                || tail.contains(['/', '\\', ':'])
                || tail == "..xml"
            {
                return Err("Unsupported level filename".into());
            }
            let gametype = required("gametype")?;
            if !matches!(
                gametype.as_str(),
                "frontend"
                    | "frontend2"
                    | "singleplayer"
                    | "skirmish"
                    | "assault"
                    | "co-op"
                    | "mini-game"
            ) {
                return Err("Unsupported level game type".into());
            }
            let players = match node.attribute("players") {
                None => None,
                Some("1") => Some(1),
                Some("2") => Some(2),
                _ => return Err("Unsupported level player count".into()),
            };
            // 0x80189844..74: physics lookup uses fallback string "2" (0x806022e4).
            let raw_physics = node.attribute("physics");
            let (physics, value) = match raw_physics.unwrap_or("2") {
                "0" => (Override::Force(Engine::Ode), 0),
                "1" => (Override::Force(Engine::Ageia), 1),
                "2" => (Override::Authored, 2),
                _ => return Err("Unsupported level physics override".into()),
            };
            entries.push(Entry {
                name,
                filename,
                gametype,
                players,
                physics,
                physics_attribute: raw_physics.map(|_| value),
            });
            if entries.len() > 1024 {
                return Err("Too many level catalog entries".into());
            }
        }
        if entries.is_empty() {
            return Err("Empty level catalog".into());
        }
        Ok(Self { entries })
    }

    /// Preserve repeated source names (notably unused.*); never silently pick one.
    pub fn unique(&self, name: &str) -> Result<&Entry> {
        let mut matches = self.entries.iter().filter(|e| e.name == name);
        let entry = matches
            .next()
            .ok_or_else(|| format!("Level not found: {name}"))?;
        if matches.next().is_some() {
            return Err(format!("Ambiguous level name: {name}"));
        }
        Ok(entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const ENTRY: &str = r#"<level name="SP_0.1" filename="@os:Data/CompoundFiles/SP_0.1.xml" gametype="singleplayer" physics="1"/>"#;

    #[test]
    fn explicit_engine_and_source_names_are_preserved() {
        let catalog = Catalog::parse(&format!("<levels><!-- ignored -->{ENTRY}</levels>")).unwrap();
        let entry = catalog.unique("SP_0.1").unwrap();
        assert_eq!(entry.physics, Override::Force(Engine::Ageia));
        assert_eq!(entry.players, None);
        assert!(catalog.unique("sp_0.1").is_err());
        for (value, expected) in [
            ("0", Override::Force(Engine::Ode)),
            ("2", Override::Authored),
        ] {
            let xml = format!("<levels>{ENTRY}</levels>")
                .replace("physics=\"1\"", &format!("physics=\"{value}\""));
            assert_eq!(Catalog::parse(&xml).unwrap().entries[0].physics, expected);
        }
    }

    #[test]
    fn repeated_entries_remain_ordered_but_lookup_is_ambiguous() {
        let catalog = Catalog::parse(&format!("<levels>{ENTRY}{ENTRY}</levels>")).unwrap();
        assert_eq!(catalog.entries.len(), 2);
        assert!(catalog.unique("SP_0.1").unwrap_err().contains("Ambiguous"));
    }

    #[test]
    fn missing_physics_uses_recovered_catalog_fallback() {
        let xml = format!("<levels>{ENTRY}</levels>").replace(" physics=\"1\"", "");
        let catalog = Catalog::parse(&xml).unwrap();
        assert_eq!(catalog.entries[0].physics, Override::Authored);
        assert_eq!(catalog.entries[0].physics_attribute, None);
    }

    #[test]
    fn unsupported_values_fail_without_coercion() {
        let xml = format!("<levels>{ENTRY}</levels>");
        for bad in [
            xml.replace("physics=\"1\"", "physics=\"\""),
            xml.replace("physics=\"1\"", "physics=\"3\""),
            xml.replace("singleplayer", "guessed"),
            xml.replace("<level name", "<level players=\"4\" name"),
            xml.replace("CompoundFiles/SP_0.1.xml", "CompoundFiles/../secret.xml"),
            xml.replace("<levels>", "<levels>unexpected"),
            xml.replace("<level name", "<level skip=\"true\" name"),
            "<levels/>".into(),
        ] {
            assert!(Catalog::parse(&bad).is_err(), "accepted {bad}");
        }
    }
}
