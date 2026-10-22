//! Data-only prototype level manifests. No scripts or commands are executed.
use crate::Result;
use serde::Deserialize;
use std::path::{Component, Path};
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub name: String,
    pub terrain: String,
    pub resources: String,
    pub arena_center: Option<[f32; 2]>,
    pub player_color: [f32; 3],
    pub enemy_color: [f32; 3],
}
fn safe_relative(path: &str) -> bool {
    !path.is_empty()
        && !path.contains('\\')
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}
impl Scenario {
    pub fn parse(json: &str) -> Result<Self> {
        if json.len() > 65536 {
            return Err("Level manifest exceeds 64 KiB".into());
        }
        let s: Self = serde_json::from_str(json).map_err(|e| e.to_string())?;
        if s.name.is_empty() || s.name.len() > 100 {
            return Err("Level name must be 1..100 bytes".into());
        }
        if !safe_relative(&s.terrain) || !safe_relative(&s.resources) {
            return Err("Level paths must be repository-relative without parent components".into());
        }
        if s.player_color
            .iter()
            .chain(s.enemy_color.iter())
            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        {
            return Err("Colors must be finite in 0..1".into());
        }
        if s.arena_center
            .is_some_and(|c| c.iter().any(|v| !v.is_finite()))
        {
            return Err("Non-finite arena center".into());
        }
        Ok(s)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn base() -> String {
        r#"{"name":"test","terrain":"mods/test.json","resources":"local/game.res","arena_center":null,"player_color":[0,0.5,1],"enemy_color":[1,0,0]}"#.into()
    }
    #[test]
    fn valid_manifest() {
        assert!(Scenario::parse(&base()).is_ok());
    }
    #[test]
    fn rejects_parent_absolute_and_unknown() {
        for value in ["../outside", "/outside", "C:\\outside"] {
            assert!(Scenario::parse(&base().replace("mods/test.json", value)).is_err());
        }
        assert!(Scenario::parse(&base().replace("\"name\"", "\"typo\"")).is_err());
    }
}
