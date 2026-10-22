//! Portable presentation state for original pause-tutorial scripts. No automatic dismissal.
use crate::{Result, lua50::Value};
use std::collections::BTreeMap;
pub const ITEMS: &[&str] = &[
    "HUD_CONTROLIMAGE_ALL",
    "HUD_CONTROLIMAGE_DPAD",
    "HUD_CONTROLIMAGE_A",
    "HUD_CONTROLIMAGE_TRIGGER",
    "HUD_CONTROLIMAGE_RETURN",
    "HUD_CONTROLIMAGE_HOME",
    "HUD_CONTROLIMAGE_PAUSE",
    "HUD_CONTROLIMAGE_1",
    "HUD_CONTROLIMAGE_2",
    "HUD_CONTROLIMAGE_ANALOGUE",
    "HUD_CONTROLIMAGE_C",
    "HUD_CONTROLIMAGE_Z",
    "HUD_CONTROLIMAGE_FLICK",
    "HUD_CONTROLIMAGE_TWIST",
    "HUD_CONTROLIMAGE_CONTROL_STICK_ABOVE",
    "HUD_CONTROLIMAGE_NUNCHUK_FLICK_ABOVE",
    "HUD_CONTROLIMAGE_NUNCHUK_ABOVE",
];
pub const ENUMS: &[&str] = &[
    "HUD_ITEM_FLASH",
    "HUD_FUNC_INDICATOR_NONE",
    "HUD_FUNC_INDICATOR_ON",
    "HUD_FUNC_INDICATOR_OFF",
    "HUD_FUNC_INDICATOR_FLASH",
    "HUD_FUNC_INDICATOR_AUTO",
    "HUD_FUNC_INDICATOR_PAUSE_CONTINUE",
    "HUD_FUNC_INDICATOR_ATTACK",
    "HUD_FUNC_INDICATOR_FOLLOW",
    "HUD_FUNC_INDICATOR_GUARD",
    "HUD_FUNC_INDICATOR_TRANSFER",
    "HUD_FUNC_INDICATOR_TRANSFER_LOCK_ON",
    "HUD_FUNC_INDICATOR_CAPTURE",
    "HUD_FUNC_INDICATOR_WAIT",
    "HUD_FUNC_INDICATOR_STACK",
    "HUD_FUNC_INDICATOR_LOAD",
    "HUD_FUNC_INDICATOR_UNLOAD",
];
#[derive(Clone, Default, Debug, PartialEq)]
pub struct Tutorial {
    pub paused: bool,
    pub warning: Option<u8>,
    pub controls: BTreeMap<String, (String, Option<f64>)>,
    pub solid: BTreeMap<String, bool>,
    pub indicator_position: Option<[f64; 2]>,
    pub indicator_mode: Option<String>,
    pub indicator_action: Option<String>,
    pub stack_indicator: Option<(bool, String, bool, crate::ObjectId)>,
}
impl Tutorial {
    pub fn handles(name: &str, args: &[Value]) -> bool {
        matches!(
            name,
            "ActivateTutorialWarning"
                | "ToggleBlankPauseScreen"
                | "SetTutorialItemSolid"
                | "SetFunctionIndicatorPosition"
                | "SetFunctionIndicatorMode"
                | "SetFunctionIndicatorAction"
                | "SetupPauseTutorialStack"
        ) || (name == "SetHudState"
            && matches!(args.get(1),Some(Value::Enum(item)) if ITEMS.contains(&item.as_str())))
    }
    pub fn call(&mut self, name: &str, args: &[Value]) -> Result<Vec<Value>> {
        match (name, args) {
            ("ActivateTutorialWarning", [Value::Enum(p), Value::Number(mode)])
                if p == "PLAYER_ONE" && (*mode == 0. || *mode == 1.) =>
            {
                self.warning = Some(*mode as u8)
            }
            ("ToggleBlankPauseScreen", [Value::Boolean(paused)]) => {
                self.paused = *paused;
                if !paused {
                    self.warning = None;
                }
            }
            ("SetHudState", [Value::Enum(p), Value::Enum(item), Value::Enum(state)])
                if p == "PLAYER_ONE"
                    && ITEMS.contains(&item.as_str())
                    && matches!(state.as_str(), "HUD_ITEM_ON" | "HUD_ITEM_OFF") =>
            {
                self.controls.insert(item.clone(), (state.clone(), None));
            }
            (
                "SetHudState",
                [
                    Value::Enum(p),
                    Value::Enum(item),
                    Value::Enum(state),
                    Value::Number(duration),
                ],
            ) if p == "PLAYER_ONE"
                && ITEMS.contains(&item.as_str())
                && state == "HUD_ITEM_FLASH"
                && duration.is_finite()
                && (0.0..=100000.).contains(duration) =>
            {
                self.controls
                    .insert(item.clone(), (state.clone(), Some(*duration)));
            }
            (
                "SetTutorialItemSolid",
                [Value::Enum(p), Value::Enum(item), Value::Boolean(value)],
            ) if p == "PLAYER_ONE" && ITEMS.contains(&item.as_str()) => {
                self.solid.insert(item.clone(), *value);
            }
            ("SetFunctionIndicatorPosition", [Value::Number(x), Value::Number(y)])
                if x.is_finite() && y.is_finite() =>
            {
                self.indicator_position = Some([*x, *y])
            }
            ("SetFunctionIndicatorMode", [Value::Enum(mode)])
                if matches!(
                    mode.as_str(),
                    "HUD_FUNC_INDICATOR_ON"
                        | "HUD_FUNC_INDICATOR_OFF"
                        | "HUD_FUNC_INDICATOR_FLASH"
                        | "HUD_FUNC_INDICATOR_AUTO"
                ) =>
            {
                self.indicator_mode = Some(mode.clone())
            }
            ("SetFunctionIndicatorAction", [Value::Enum(action)])
                if matches!(
                    action.as_str(),
                    "HUD_FUNC_INDICATOR_NONE"
                        | "HUD_FUNC_INDICATOR_PAUSE_CONTINUE"
                        | "HUD_FUNC_INDICATOR_ATTACK"
                        | "HUD_FUNC_INDICATOR_FOLLOW"
                        | "HUD_FUNC_INDICATOR_GUARD"
                        | "HUD_FUNC_INDICATOR_TRANSFER"
                        | "HUD_FUNC_INDICATOR_TRANSFER_LOCK_ON"
                        | "HUD_FUNC_INDICATOR_CAPTURE"
                        | "HUD_FUNC_INDICATOR_WAIT"
                        | "HUD_FUNC_INDICATOR_STACK"
                        | "HUD_FUNC_INDICATOR_LOAD"
                        | "HUD_FUNC_INDICATOR_UNLOAD"
                ) =>
            {
                self.indicator_action = Some(action.clone())
            }
            (
                "SetupPauseTutorialStack",
                [
                    Value::Boolean(enable),
                    Value::Enum(action),
                    Value::Boolean(highlight),
                    rest @ ..,
                ],
            ) if matches!(
                action.as_str(),
                "HUD_FUNC_INDICATOR_NONE"
                    | "HUD_FUNC_INDICATOR_ATTACK"
                    | "HUD_FUNC_INDICATOR_FOLLOW"
                    | "HUD_FUNC_INDICATOR_GUARD"
                    | "HUD_FUNC_INDICATOR_TRANSFER"
                    | "HUD_FUNC_INDICATOR_TRANSFER_LOCK_ON"
                    | "HUD_FUNC_INDICATOR_CAPTURE"
                    | "HUD_FUNC_INDICATOR_WAIT"
                    | "HUD_FUNC_INDICATOR_STACK"
                    | "HUD_FUNC_INDICATOR_LOAD"
                    | "HUD_FUNC_INDICATOR_UNLOAD"
            ) =>
            {
                let unit = match rest {
                    [] => crate::ObjectId(0),
                    [Value::Object(unit)] => *unit,
                    _ => return Err(format!("Unsupported tutorial call/signature: {name}")),
                };
                self.stack_indicator = Some((*enable, action.clone(), *highlight, unit))
            }
            _ => return Err(format!("Unsupported tutorial call/signature: {name}")),
        }
        Ok(vec![])
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pause_requires_explicit_script_exit() {
        let mut t = Tutorial::default();
        t.call("ToggleBlankPauseScreen", &[Value::Boolean(true)])
            .unwrap();
        let before = t.clone();
        assert!(
            t.call("ToggleBlankPauseScreen", &[Value::Number(0.)])
                .is_err()
        );
        assert_eq!(t, before);
        t.call("ToggleBlankPauseScreen", &[Value::Boolean(false)])
            .unwrap();
        assert!(!t.paused);
    }
}
