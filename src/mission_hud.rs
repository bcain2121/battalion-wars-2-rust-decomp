//! Authored single-player HUD presentation requests. Rendering remains a separate consumer.
use crate::{Result, lua50::Value};
use std::collections::BTreeMap;
pub const ITEMS: &[&str] = &["HUD_COMMANDBAR", "HUD_COMMANDSTACK", "HUD_PLAYERICON"];
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hud {
    requested: BTreeMap<String, bool>,
    pub function_indicator: Option<bool>,
    pub revision: u64,
}
impl Hud {
    pub fn requested(&self, item: &str) -> Result<Option<bool>> {
        if !ITEMS.contains(&item) {
            return Err("Unsupported HUD item".into());
        }
        Ok(self.requested.get(item).copied())
    }
    pub fn call(&mut self, name: &str, args: &[Value]) -> Result<Vec<Value>> {
        let revision = self
            .revision
            .checked_add(1)
            .ok_or("HUD revision overflow")?;
        match (name, args) {
            ("SetHudState", [Value::Enum(player), Value::Enum(item), Value::Enum(state)])
                if player == "PLAYER_ONE" && ITEMS.contains(&item.as_str()) =>
            {
                let shown = match state.as_str() {
                    "HUD_ITEM_OFF" => false,
                    "HUD_ITEM_ON" => true,
                    _ => return Err("Unsupported HUD state/transition".into()),
                };
                self.requested.insert(item.clone(), shown);
            }
            ("SetFunctionIndicatorState", [Value::Boolean(enabled)]) => {
                self.function_indicator = Some(*enabled)
            }
            _ => return Err("Unsupported HUD call/signature".into()),
        }
        self.revision = revision;
        Ok(vec![])
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn e(v: &str) -> Value {
        Value::Enum(v.into())
    }
    #[test]
    fn masks_are_distinct_and_unknown_defaults_remain_unknown() {
        let mut h = Hud::default();
        assert_eq!(h.requested("HUD_COMMANDBAR").unwrap(), None);
        assert_eq!(h.function_indicator, None);
        h.call(
            "SetHudState",
            &[e("PLAYER_ONE"), e("HUD_COMMANDBAR"), e("HUD_ITEM_OFF")],
        )
        .unwrap();
        assert_eq!(h.requested("HUD_COMMANDBAR").unwrap(), Some(false));
        assert_eq!(h.requested("HUD_COMMANDSTACK").unwrap(), None);
        h.call(
            "SetHudState",
            &[e("PLAYER_ONE"), e("HUD_COMMANDSTACK"), e("HUD_ITEM_ON")],
        )
        .unwrap();
        h.call("SetFunctionIndicatorState", &[Value::Boolean(false)])
            .unwrap();
        assert_eq!(h.revision, 3);
        assert_eq!(h.function_indicator, Some(false));
    }
    #[test]
    fn unsupported_transitions_and_players_do_not_mutate() {
        let mut h = Hud::default();
        for args in [
            vec![e("PLAYER_TWO"), e("HUD_COMMANDBAR"), e("HUD_ITEM_ON")],
            vec![
                e("PLAYER_ONE"),
                e("HUD_COMMANDBAR"),
                e("HUD_ITEM_FLASH"),
                Value::Number(1.0),
            ],
            vec![e("PLAYER_ONE"), e("HUD_UNKNOWN"), e("HUD_ITEM_ON")],
        ] {
            assert!(h.call("SetHudState", &args).is_err());
            assert_eq!(h, Hud::default());
        }
        assert!(
            h.call("SetFunctionIndicatorState", &[Value::Number(1.0)])
                .is_err()
        );
        assert_eq!(h, Hud::default());
        h.revision = u64::MAX;
        assert!(
            h.call("SetFunctionIndicatorState", &[Value::Boolean(true)])
                .is_err()
        );
        assert_eq!(h.function_indicator, None);
    }
}
