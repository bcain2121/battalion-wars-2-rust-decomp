//! Frame-scoped input snapshots; no absent sample is treated as an unpressed button.
use crate::{Result, lua50::Value, mission_controls::Controller};
use std::collections::BTreeMap;
pub const ITEMS: &[&str] = crate::mission_controls::ITEMS;
pub const MODES: &[&str] = &[
    "CONTROL_JUST_PRESSED",
    "CONTROL_HELD",
    "CONTROL_JUST_RELEASED",
];
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Button {
    pub held: bool,
    pub just_pressed: bool,
    pub just_released: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Input {
    frame: Option<u64>,
    buttons: BTreeMap<String, Button>,
}
impl Input {
    /// Explicit local single-player policy: player one exists; player two does not.
    pub fn single_player() -> Self {
        Self {
            frame: None,
            buttons: BTreeMap::new(),
        }
    }
    /// Submit a complete snapshot for the scheduler frame about to execute.
    /// Same-frame retries may replace a pending sample; reads never consume its edges.
    pub fn submit(&mut self, frame: u64, buttons: BTreeMap<String, Button>) -> Result<()> {
        if self.frame.is_some_and(|old| frame < old) {
            return Err("Controller sample frame regressed".into());
        }
        if buttons.len() != ITEMS.len() || ITEMS.iter().any(|k| !buttons.contains_key(*k)) {
            return Err("Incomplete/unsupported controller snapshot".into());
        }
        self.frame = Some(frame);
        self.buttons = buttons;
        Ok(())
    }
    pub fn read(&self, args: &[Value], controller: &Controller, frame: u64) -> Result<Vec<Value>> {
        let (item, mode, player) = match args {
            [Value::Enum(item), Value::Enum(mode)] => (item.as_str(), mode.as_str(), "PLAYER_ONE"),
            [Value::Enum(item), Value::Enum(mode), Value::Enum(player)] => {
                (item.as_str(), mode.as_str(), player.as_str())
            }
            _ => return Err("Unsupported ReadControllerState signature".into()),
        };
        if !ITEMS.contains(&item) || !MODES.contains(&mode) {
            return Err("Unsupported controller read item/mode".into());
        }
        match player {
            "PLAYER_TWO" => return Ok(vec![Value::Boolean(false)]),
            "PLAYER_ONE" => {}
            _ => return Err("Unsupported controller read player".into()),
        }
        // Native reads the disable byte before the event bits (all three supported modes).
        if !controller.accepts(item)? {
            return Ok(vec![Value::Boolean(false)]);
        }
        if self.frame != Some(frame) {
            return Err("Controller input snapshot missing/stale for current frame".into());
        }
        let button = self
            .buttons
            .get(item)
            .ok_or("Missing controller button sample")?;
        let value = match mode {
            "CONTROL_JUST_PRESSED" => button.just_pressed,
            "CONTROL_HELD" => button.held,
            "CONTROL_JUST_RELEASED" => button.just_released,
            _ => unreachable!(),
        };
        Ok(vec![Value::Boolean(value)])
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn query(player: &str, mode: &str) -> Vec<Value> {
        ["CONTROL_FIRE", mode, player]
            .map(|s| Value::Enum(s.into()))
            .into()
    }
    #[test]
    fn masks_gate_real_snapshots_and_edges_are_not_consumed() {
        let mut input = Input::single_player();
        let mut c = Controller::fresh_player();
        assert!(
            input
                .read(&query("PLAYER_ONE", "CONTROL_HELD"), &c, 1)
                .is_err()
        );
        let mut buttons = ITEMS
            .iter()
            .map(|s| ((*s).into(), Button::default()))
            .collect::<BTreeMap<_, _>>();
        buttons.insert(
            "CONTROL_FIRE".into(),
            Button {
                held: true,
                just_pressed: true,
                just_released: false,
            },
        );
        input.submit(1, buttons).unwrap();
        for _ in 0..2 {
            assert_eq!(
                input
                    .read(&query("PLAYER_ONE", "CONTROL_JUST_PRESSED"), &c, 1)
                    .unwrap(),
                vec![Value::Boolean(true)]
            );
        }
        assert_eq!(
            input
                .read(&query("PLAYER_ONE", "CONTROL_JUST_RELEASED"), &c, 1)
                .unwrap(),
            vec![Value::Boolean(false)]
        );
        c.call(&[Value::Enum("CONTROL_FIRE".into()), Value::Number(0.0)])
            .unwrap();
        assert_eq!(
            input
                .read(&query("PLAYER_ONE", "CONTROL_HELD"), &c, 1)
                .unwrap(),
            vec![Value::Boolean(false)]
        );
        assert!(
            input
                .read(&query("PLAYER_ONE", "CONTROL_PRESSED_THIS_LEVEL"), &c, 1)
                .is_err()
        );
    }
    #[test]
    fn absent_second_player_is_explicit_but_missing_primary_defaults_fail() {
        let input = Input::single_player();
        let c = Controller::default();
        assert_eq!(
            input
                .read(&query("PLAYER_TWO", "CONTROL_JUST_PRESSED"), &c, 0)
                .unwrap(),
            vec![Value::Boolean(false)]
        );
        assert!(
            input
                .read(&query("PLAYER_ONE", "CONTROL_JUST_PRESSED"), &c, 0)
                .is_err()
        );
        assert!(
            input
                .read(&query("PLAYER_THREE", "CONTROL_HELD"), &c, 0)
                .is_err()
        );
    }
    #[test]
    fn incomplete_stale_or_regressed_samples_are_not_used() {
        let mut input = Input::single_player();
        assert!(input.submit(1, BTreeMap::new()).is_err());
        assert_eq!(input.frame, None);
        let buttons = ITEMS
            .iter()
            .map(|s| ((*s).into(), Button::default()))
            .collect();
        input.submit(3, buttons).unwrap();
        let before = input.clone();
        assert!(input.submit(2, input.buttons.clone()).is_err());
        assert_eq!(input, before);
        assert!(
            input
                .read(
                    &query("PLAYER_ONE", "CONTROL_HELD"),
                    &Controller::fresh_player(),
                    4
                )
                .is_err()
        );
    }
}
