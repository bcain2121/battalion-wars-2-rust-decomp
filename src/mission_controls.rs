//! Authored controller enable masks. Unassigned defaults remain unknown, not inferred.
use crate::{Result, lua50::Value};
use std::collections::BTreeMap;
pub const ITEMS: &[&str] = &[
    "CONTROL_CALL",
    "CONTROL_DISMISS",
    "CONTROL_FIRE",
    "CONTROL_GLOBAL_TOGGLE",
    "CONTROL_HUD_TRANSFER",
    "CONTROL_JUMP",
    "CONTROL_LOAD_VEHICLE",
    "CONTROL_LOCAL_CALL",
    "CONTROL_MAP_TRANSFER",
    "CONTROL_NEXT_UNIT",
    "CONTROL_NEXT_UNIT_TYPE",
    "CONTROL_NEXT_UNIT_TYPE_ALT",
    "CONTROL_PAUSE",
    "CONTROL_PREV_UNIT",
    "CONTROL_PREV_UNIT_TYPE",
    "CONTROL_ROLL",
    "CONTROL_SEND",
    "CONTROL_SKIP_CUTSCENE",
    "CONTROL_SKIP_CUTSCENE_ALT",
    "CONTROL_TOGGLE_MAP",
    "CONTROL_TRANSFER",
    "CONTROL_UNLOAD_VEHICLE",
    "GCONTROL_PAUSE_SELECT",
];
#[derive(Clone, Default, Debug, PartialEq)]
pub struct Controller {
    enabled: BTreeMap<String, bool>,
}
impl Controller {
    /// Explicit fresh-player initialization: native 0x800f1d64..0x800f1eb0
    /// zeroes disable bytes. Default intentionally remains unconfigured for probes.
    pub fn fresh_player() -> Self {
        Self {
            enabled: ITEMS.iter().map(|name| ((*name).into(), true)).collect(),
        }
    }
    /// No numeric control IDs are guessed; only explicitly supported symbolic controls.
    pub fn enabled(&self, item: &str) -> Result<Option<bool>> {
        if !ITEMS.contains(&item) {
            return Err("Unsupported controller item".into());
        }
        Ok(self.enabled.get(item).copied())
    }
    /// Consumer-side gate: an unset default is an error, not implicit permission.
    pub fn accepts(&self, item: &str) -> Result<bool> {
        self.enabled(item)?
            .ok_or_else(|| format!("Controller default not configured for {item}"))
    }
    pub fn call(&mut self, args: &[Value]) -> Result<Vec<Value>> {
        let (item, value) = match args {
            [Value::Enum(item), Value::Number(value)] => (item, *value),
            [Value::Enum(item), Value::Number(value), Value::Enum(player)]
                if player == "PLAYER_ONE" =>
            {
                (item, *value)
            }
            _ => return Err("Unsupported EnableControllerItem signature/player".into()),
        };
        if !ITEMS.contains(&item.as_str()) {
            return Err("Unsupported controller item".into());
        }
        // Native wrapper numerically reads the second argument and stores the inverted disable bit.
        // Restrict to the authored 0/1 domain instead of guessing numeric coercions.
        let enabled = match value {
            0.0 => false,
            1.0 => true,
            _ => return Err("Controller enable value must be 0 or 1".into()),
        };
        self.enabled.insert(item.clone(), enabled);
        Ok(vec![])
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_mask_and_unknown_defaults() {
        let mut c = Controller::default();
        assert!(c.accepts("CONTROL_FIRE").is_err());
        c.call(&[Value::Enum("CONTROL_FIRE".into()), Value::Number(0.0)])
            .unwrap();
        assert!(!c.accepts("CONTROL_FIRE").unwrap());
        c.call(&[
            Value::Enum("CONTROL_FIRE".into()),
            Value::Number(1.0),
            Value::Enum("PLAYER_ONE".into()),
        ])
        .unwrap();
        assert!(c.accepts("CONTROL_FIRE").unwrap());
        assert_eq!(c.enabled("CONTROL_CALL").unwrap(), None);
    }
    #[test]
    fn invalid_calls_do_not_mutate() {
        let mut c = Controller::default();
        let before = c.clone();
        for args in [
            vec![Value::Enum("CONTROL_FIRE".into()), Value::Number(f64::NAN)],
            vec![Value::Enum("CONTROL_FIRE".into()), Value::Number(0.5)],
            vec![
                Value::Enum("CONTROL_JUST_PRESSED".into()),
                Value::Number(0.0),
            ],
            vec![Value::Enum("CONTROL_FIRE".into()), Value::Boolean(true)],
            vec![
                Value::Enum("CONTROL_FIRE".into()),
                Value::Number(0.0),
                Value::Enum("PLAYER_TWO".into()),
            ],
        ] {
            assert!(c.call(&args).is_err());
            assert_eq!(c, before);
        }
    }
}
