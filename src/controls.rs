//! Remappable action bindings. Controller path uses Bevy's standard gamepad API.
use crate::{Result, skirmish::Input};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

fn default_mouse_sensitivity() -> f32 {
    0.15
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    deadzone: f32,
    turn_sensitivity: f32,
    #[serde(default = "default_mouse_sensitivity")]
    mouse_sensitivity: f32,
    invert_right_stick_y: bool,
    #[serde(default)]
    invert_mouse_y: bool,
    keyboard: BTreeMap<String, Vec<String>>,
    gamepad: BTreeMap<String, String>,
}
#[derive(Resource, Clone)]
pub struct Controls {
    keyboard: BTreeMap<String, Vec<KeyCode>>,
    gamepad: BTreeMap<String, GamepadButton>,
    deadzone: f32,
    sensitivity: f32,
    mouse_sensitivity: f32,
    invert_y: bool,
    invert_mouse_y: bool,
}
pub struct Actions {
    pub input: Input,
    pub lock: bool,
    pub call: bool,
    pub call_held: bool,
    pub send: bool,
    pub send_held: bool,
    pub dismiss: bool,
    pub dismiss_held: bool,
    pub transfer: bool,
    pub load_vehicle: bool,
    pub unload_vehicle: bool,
    pub next_unit: bool,
    pub prev_unit: bool,
    pub next_unit_type: bool,
    pub prev_unit_type: bool,
    pub global_toggle: bool,
    pub toggle_map: bool,
    pub start: bool,
    pub confirm_held: bool,
    pub pause: bool,
    pub quit: bool,
    pub look_pitch: f32,
}
const KEY_ACTIONS: &[&str] = &[
    "forward",
    "back",
    "left",
    "right",
    "turn_left",
    "turn_right",
    "look_up",
    "look_down",
    "fire",
    "lock",
    "sprint",
    "call",
    "send",
    "dismiss",
    "transfer",
    "load_vehicle",
    "unload_vehicle",
    "next_unit",
    "prev_unit",
    "next_unit_type",
    "prev_unit_type",
    "global_toggle",
    "toggle_map",
    "cycle",
    "start",
    "pause",
    "restart",
    "quit",
];
const PAD_ACTIONS: &[&str] = &[
    "fire",
    "lock",
    "sprint",
    "call",
    "send",
    "dismiss",
    "transfer",
    "load_vehicle",
    "unload_vehicle",
    "next_unit",
    "prev_unit",
    "next_unit_type",
    "prev_unit_type",
    "global_toggle",
    "toggle_map",
    "cycle",
    "start",
    "pause",
    "restart",
];
const KEY_TABLE: &[(&str, KeyCode)] = &[
    ("A", KeyCode::KeyA),
    ("B", KeyCode::KeyB),
    ("C", KeyCode::KeyC),
    ("D", KeyCode::KeyD),
    ("E", KeyCode::KeyE),
    ("F", KeyCode::KeyF),
    ("G", KeyCode::KeyG),
    ("H", KeyCode::KeyH),
    ("I", KeyCode::KeyI),
    ("J", KeyCode::KeyJ),
    ("K", KeyCode::KeyK),
    ("L", KeyCode::KeyL),
    ("M", KeyCode::KeyM),
    ("N", KeyCode::KeyN),
    ("O", KeyCode::KeyO),
    ("P", KeyCode::KeyP),
    ("Q", KeyCode::KeyQ),
    ("R", KeyCode::KeyR),
    ("S", KeyCode::KeyS),
    ("T", KeyCode::KeyT),
    ("U", KeyCode::KeyU),
    ("V", KeyCode::KeyV),
    ("W", KeyCode::KeyW),
    ("X", KeyCode::KeyX),
    ("Y", KeyCode::KeyY),
    ("Z", KeyCode::KeyZ),
    ("Digit0", KeyCode::Digit0),
    ("Digit1", KeyCode::Digit1),
    ("Digit2", KeyCode::Digit2),
    ("Digit3", KeyCode::Digit3),
    ("Digit4", KeyCode::Digit4),
    ("Digit5", KeyCode::Digit5),
    ("Digit6", KeyCode::Digit6),
    ("Digit7", KeyCode::Digit7),
    ("Digit8", KeyCode::Digit8),
    ("Digit9", KeyCode::Digit9),
    ("ArrowLeft", KeyCode::ArrowLeft),
    ("ArrowRight", KeyCode::ArrowRight),
    ("ArrowUp", KeyCode::ArrowUp),
    ("ArrowDown", KeyCode::ArrowDown),
    ("Space", KeyCode::Space),
    ("Tab", KeyCode::Tab),
    ("Enter", KeyCode::Enter),
    ("Escape", KeyCode::Escape),
    ("Backspace", KeyCode::Backspace),
    ("ShiftLeft", KeyCode::ShiftLeft),
    ("ShiftRight", KeyCode::ShiftRight),
    ("ControlLeft", KeyCode::ControlLeft),
    ("ControlRight", KeyCode::ControlRight),
    ("AltLeft", KeyCode::AltLeft),
    ("AltRight", KeyCode::AltRight),
    ("Comma", KeyCode::Comma),
    ("Period", KeyCode::Period),
    ("BracketLeft", KeyCode::BracketLeft),
    ("BracketRight", KeyCode::BracketRight),
];
const PAD_TABLE: &[(&str, GamepadButton)] = &[
    ("South", GamepadButton::South),
    ("East", GamepadButton::East),
    ("West", GamepadButton::West),
    ("North", GamepadButton::North),
    ("Start", GamepadButton::Start),
    ("Select", GamepadButton::Select),
    ("LeftTrigger", GamepadButton::LeftTrigger),
    ("RightTrigger", GamepadButton::RightTrigger),
    ("LeftTrigger2", GamepadButton::LeftTrigger2),
    ("RightTrigger2", GamepadButton::RightTrigger2),
    ("LeftThumb", GamepadButton::LeftThumb),
    ("RightThumb", GamepadButton::RightThumb),
    ("DPadUp", GamepadButton::DPadUp),
    ("DPadDown", GamepadButton::DPadDown),
    ("DPadLeft", GamepadButton::DPadLeft),
    ("DPadRight", GamepadButton::DPadRight),
];
fn key(name: &str) -> Result<KeyCode> {
    for &(label, code) in KEY_TABLE {
        if label == name {
            return Ok(code);
        }
    }
    Err(format!(
        "Unsupported key {name}; use A-Z, Digit0-9, arrows, Space/Tab/Enter/Escape/Backspace/Shift/Control/Alt/Comma/Period/Brackets"
    ))
}
pub fn key_name(code: KeyCode) -> Option<&'static str> {
    KEY_TABLE
        .iter()
        .find_map(|&(label, c)| (c == code).then_some(label))
}
pub fn detect_pressed_key(keys: &ButtonInput<KeyCode>) -> Option<KeyCode> {
    KEY_TABLE
        .iter()
        .find_map(|&(_, code)| keys.just_pressed(code).then_some(code))
}
fn button(name: &str) -> Result<GamepadButton> {
    for &(label, btn) in PAD_TABLE {
        if label == name {
            return Ok(btn);
        }
    }
    Err(format!("Unsupported gamepad button {name}"))
}
pub fn button_name(btn: GamepadButton) -> Option<&'static str> {
    PAD_TABLE
        .iter()
        .find_map(|&(label, b)| (b == btn).then_some(label))
}
pub fn detect_pressed_gamepad_button(pad: &Gamepad) -> Option<GamepadButton> {
    PAD_TABLE
        .iter()
        .find_map(|&(_, btn)| pad.just_pressed(btn).then_some(btn))
}
impl Controls {
    pub fn defaults() -> Self {
        Self::parse(include_str!("../config/controls.json")).expect("Default controls.json")
    }
    pub fn reset_defaults(&mut self) {
        *self = Self::defaults();
    }
    pub fn keys_label(&self, action: &str) -> String {
        self.keyboard
            .get(action)
            .map(|list| {
                list.iter()
                    .filter_map(|&k| key_name(k))
                    .collect::<Vec<_>>()
                    .join(" / ")
            })
            .unwrap_or_else(|| "-".into())
    }
    pub fn gamepad_label(&self, action: &str) -> String {
        self.gamepad
            .get(action)
            .and_then(|&b| button_name(b))
            .unwrap_or("-")
            .to_string()
    }
    pub fn rebind_key(&mut self, action: &str, new_key: KeyCode) -> Result<()> {
        if key_name(new_key).is_none() {
            return Err("Unsupported key for rebinding".into());
        }
        let Some(slot) = self.keyboard.get_mut(action) else {
            return Err(format!("Unknown keyboard action {action}"));
        };
        *slot = vec![new_key];
        Ok(())
    }
    pub fn rebind_gamepad(&mut self, action: &str, new_btn: GamepadButton) -> Result<()> {
        if button_name(new_btn).is_none() {
            return Err("Unsupported gamepad button for rebinding".into());
        }
        let Some(slot) = self.gamepad.get_mut(action) else {
            return Err(format!("Unknown gamepad action {action}"));
        };
        *slot = new_btn;
        Ok(())
    }
    pub fn to_json_pretty(&self) -> Result<String> {
        let mut keyboard = BTreeMap::new();
        for (action, codes) in &self.keyboard {
            let names: Vec<String> = codes
                .iter()
                .filter_map(|&c| key_name(c).map(str::to_string))
                .collect();
            keyboard.insert(action.clone(), names);
        }
        let mut gamepad = BTreeMap::new();
        for (action, &btn) in &self.gamepad {
            if let Some(name) = button_name(btn) {
                gamepad.insert(action.clone(), name.to_string());
            }
        }
        let cfg = Config {
            deadzone: self.deadzone,
            turn_sensitivity: self.sensitivity,
            mouse_sensitivity: self.mouse_sensitivity,
            invert_right_stick_y: self.invert_y,
            invert_mouse_y: self.invert_mouse_y,
            keyboard,
            gamepad,
        };
        serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())
    }
    pub fn parse(json: &str) -> Result<Self> {
        if json.len() > 65536 {
            return Err("Controls configuration too large".into());
        }
        let c: Config = serde_json::from_str(json).map_err(|e| e.to_string())?;
        if !(0.0..0.8).contains(&c.deadzone)
            || !(0.1..=4.0).contains(&c.turn_sensitivity)
            || !(0.01..=4.0).contains(&c.mouse_sensitivity)
        {
            return Err("Invalid deadzone/sensitivity".into());
        }
        if c.keyboard.len() != KEY_ACTIONS.len()
            || KEY_ACTIONS.iter().any(|a| !c.keyboard.contains_key(*a))
            || c.gamepad.len() != PAD_ACTIONS.len()
            || PAD_ACTIONS.iter().any(|a| !c.gamepad.contains_key(*a))
        {
            return Err("Missing or unknown action binding".into());
        }
        let mut keyboard = BTreeMap::new();
        for (action, names) in c.keyboard {
            if names.is_empty() || names.len() > 8 {
                return Err("Each action needs 1..8 keys".into());
            }
            keyboard.insert(
                action,
                names.iter().map(|s| key(s)).collect::<Result<Vec<_>>>()?,
            );
        }
        let gamepad = c
            .gamepad
            .into_iter()
            .map(|(a, s)| Ok((a, button(&s)?)))
            .collect::<Result<_>>()?;
        Ok(Self {
            keyboard,
            gamepad,
            deadzone: c.deadzone,
            sensitivity: c.turn_sensitivity,
            mouse_sensitivity: c.mouse_sensitivity,
            invert_y: c.invert_right_stick_y,
            invert_mouse_y: c.invert_mouse_y,
        })
    }
    pub fn mouse_look(&self, delta: Vec2) -> [f64; 2] {
        let scale = self.mouse_sensitivity as f64 * 0.02;
        let y_sign = if self.invert_mouse_y { 1.0 } else { -1.0 };
        [-(delta.x as f64) * scale, (delta.y as f64) * y_sign * scale]
    }
    pub fn sample(
        &self,
        keys: &ButtonInput<KeyCode>,
        mouse: &ButtonInput<MouseButton>,
        pad: Option<&Gamepad>,
    ) -> Actions {
        let pressed = |action: &str| {
            self.keyboard[action].iter().any(|k| keys.pressed(*k))
                || pad.is_some_and(|p| self.gamepad.get(action).is_some_and(|b| p.pressed(*b)))
        };
        let just = |action: &str| {
            self.keyboard[action].iter().any(|k| keys.just_pressed(*k))
                || pad.is_some_and(|p| self.gamepad.get(action).is_some_and(|b| p.just_pressed(*b)))
        };
        let axis = |a: GamepadAxis| {
            let value = pad.and_then(|p| p.get(a)).unwrap_or(0.0);
            if value.abs() <= self.deadzone {
                0.0
            } else {
                value.signum() * (value.abs() - self.deadzone) / (1.0 - self.deadzone)
            }
        };
        let digital = |a: &str, b: &str| pressed(a) as u8 as f32 - pressed(b) as u8 as f32;
        Actions {
            input: Input {
                forward: (digital("forward", "back") + axis(GamepadAxis::LeftStickY))
                    .clamp(-1.0, 1.0),
                strafe: (digital("right", "left") + axis(GamepadAxis::LeftStickX)).clamp(-1.0, 1.0),
                turn: ((digital("turn_right", "turn_left") + axis(GamepadAxis::RightStickX))
                    * self.sensitivity)
                    .clamp(-1.0, 1.0),
                fire: pressed("fire") || mouse.pressed(MouseButton::Left),
                sprint: pressed("sprint"),
                cycle: just("cycle"),
                restart: just("restart"),
            },
            lock: pressed("lock") || mouse.pressed(MouseButton::Right),
            call: just("call"),
            call_held: pressed("call"),
            send: just("send") || mouse.just_pressed(MouseButton::Middle),
            send_held: pressed("send") || mouse.pressed(MouseButton::Middle),
            dismiss: just("dismiss"),
            dismiss_held: pressed("dismiss"),
            transfer: just("transfer"),
            load_vehicle: just("load_vehicle"),
            unload_vehicle: just("unload_vehicle"),
            next_unit: just("next_unit"),
            prev_unit: just("prev_unit"),
            next_unit_type: just("next_unit_type"),
            prev_unit_type: just("prev_unit_type"),
            global_toggle: just("global_toggle"),
            toggle_map: just("toggle_map"),
            start: just("start"),
            confirm_held: pressed("start"),
            pause: just("pause"),
            quit: just("quit"),
            look_pitch: (digital("look_up", "look_down")
                + axis(GamepadAxis::RightStickY) * if self.invert_y { -1.0 } else { 1.0 })
                * self.sensitivity,
        }
    }
    /// Digital queries used by the original mission scripts across all 21 symbolic Wii controls.
    /// This does not inject a snapshot or drive gameplay unless the caller submits it.
    pub fn mission_script_buttons(
        &self,
        keys: &ButtonInput<KeyCode>,
        mouse: &ButtonInput<MouseButton>,
        pad: Option<&Gamepad>,
    ) -> BTreeMap<String, crate::mission_input::Button> {
        use crate::mission_input::Button;
        let button = |keyboard: &str, gamepad: &str, extra_mouse: Option<MouseButton>| {
            let key = &self.keyboard[keyboard];
            let pad_button = self.gamepad[gamepad];
            Button {
                held: key.iter().any(|k| keys.pressed(*k))
                    || pad.is_some_and(|p| p.pressed(pad_button))
                    || extra_mouse.is_some_and(|m| mouse.pressed(m)),
                just_pressed: key.iter().any(|k| keys.just_pressed(*k))
                    || pad.is_some_and(|p| p.just_pressed(pad_button))
                    || extra_mouse.is_some_and(|m| mouse.just_pressed(m)),
                just_released: key.iter().any(|k| keys.just_released(*k))
                    || pad.is_some_and(|p| p.just_released(pad_button))
                    || extra_mouse.is_some_and(|m| mouse.just_released(m)),
            }
        };
        BTreeMap::from([
            ("CONTROL_CALL".into(), button("call", "call", None)),
            ("CONTROL_LOCAL_CALL".into(), button("call", "call", None)),
            (
                "CONTROL_SEND".into(),
                button("send", "send", Some(MouseButton::Middle)),
            ),
            ("CONTROL_DISMISS".into(), button("dismiss", "dismiss", None)),
            (
                "CONTROL_FIRE".into(),
                button("fire", "fire", Some(MouseButton::Left)),
            ),
            (
                "CONTROL_GLOBAL_TOGGLE".into(),
                button("global_toggle", "global_toggle", None),
            ),
            (
                "CONTROL_HUD_TRANSFER".into(),
                button("transfer", "transfer", None),
            ),
            (
                "CONTROL_MAP_TRANSFER".into(),
                button("transfer", "transfer", None),
            ),
            (
                "CONTROL_TRANSFER".into(),
                button("transfer", "transfer", None),
            ),
            ("CONTROL_JUMP".into(), button("start", "start", None)),
            ("CONTROL_ROLL".into(), button("lock", "lock", None)),
            (
                "CONTROL_LOAD_VEHICLE".into(),
                button("load_vehicle", "load_vehicle", None),
            ),
            (
                "CONTROL_UNLOAD_VEHICLE".into(),
                button("unload_vehicle", "unload_vehicle", None),
            ),
            (
                "CONTROL_NEXT_UNIT".into(),
                button("next_unit", "next_unit", None),
            ),
            (
                "CONTROL_PREV_UNIT".into(),
                button("prev_unit", "prev_unit", None),
            ),
            (
                "CONTROL_NEXT_UNIT_TYPE".into(),
                button("next_unit_type", "next_unit_type", None),
            ),
            (
                "CONTROL_NEXT_UNIT_TYPE_ALT".into(),
                button("next_unit_type", "next_unit_type", None),
            ),
            (
                "CONTROL_PREV_UNIT_TYPE".into(),
                button("prev_unit_type", "prev_unit_type", None),
            ),
            (
                "CONTROL_TOGGLE_MAP".into(),
                button("toggle_map", "toggle_map", None),
            ),
            ("CONTROL_PAUSE".into(), button("pause", "pause", None)),
            (
                "CONTROL_SKIP_CUTSCENE".into(),
                button("start", "start", None),
            ),
            (
                "CONTROL_SKIP_CUTSCENE_ALT".into(),
                button("quit", "pause", None),
            ),
            (
                "GCONTROL_PAUSE_SELECT".into(),
                button("start", "start", None),
            ),
        ])
    }
    pub fn keyboard_help(&self) -> String {
        format!(
            "Remap config/controls.json | Move {:?}/{:?}/{:?}/{:?} | Mouse/Arrows look | Fire {:?}/LMB | Lock {:?}/RMB\nSquad: Call {:?} | Send {:?}/MMB | Dismiss {:?} | Transfer {:?} | Unit {:?}/{:?} | Type {:?}/{:?} | Map {:?}\nStart {:?} | Pause {:?} | Restart {:?} | Quit {:?}",
            self.keyboard["forward"],
            self.keyboard["back"],
            self.keyboard["left"],
            self.keyboard["right"],
            self.keyboard["fire"],
            self.keyboard["lock"],
            self.keyboard["call"],
            self.keyboard["send"],
            self.keyboard["dismiss"],
            self.keyboard["transfer"],
            self.keyboard["prev_unit"],
            self.keyboard["next_unit"],
            self.keyboard["prev_unit_type"],
            self.keyboard["next_unit_type"],
            self.keyboard["toggle_map"],
            self.keyboard["start"],
            self.keyboard["pause"],
            self.keyboard["restart"],
            self.keyboard["quit"]
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mission_sampler_preserves_edges_and_native_masks_gate_them() {
        use crate::{
            lua50::Value, mission_controls::Controller, mission_input::Input as ScriptInput,
        };
        let controls = Controls::parse(include_str!("../config/controls.json")).unwrap();
        let mut keys = ButtonInput::default();
        let mut mouse = ButtonInput::default();
        keys.press(KeyCode::Enter);
        keys.press(KeyCode::KeyC);
        keys.press(KeyCode::KeyX);
        mouse.press(MouseButton::Left);
        mouse.press(MouseButton::Middle);
        let buttons = controls.mission_script_buttons(&keys, &mouse, None);
        assert_eq!(buttons.len(), crate::mission_controls::ITEMS.len());
        assert!(buttons["CONTROL_FIRE"].just_pressed);
        assert!(buttons["CONTROL_CALL"].just_pressed);
        assert!(buttons["CONTROL_SEND"].just_pressed);
        assert!(buttons["CONTROL_DISMISS"].just_pressed);
        assert!(buttons["GCONTROL_PAUSE_SELECT"].just_pressed);
        let mut input = ScriptInput::single_player();
        input.submit(1, buttons).unwrap();
        let query =
            ["CONTROL_FIRE", "CONTROL_JUST_PRESSED", "PLAYER_ONE"].map(|s| Value::Enum(s.into()));
        let mut controller = Controller::fresh_player();
        assert_eq!(
            input.read(&query, &controller, 1).unwrap(),
            vec![Value::Boolean(true)]
        );
        controller
            .call(&[Value::Enum("CONTROL_FIRE".into()), Value::Number(0.0)])
            .unwrap();
        assert_eq!(
            input.read(&query, &controller, 1).unwrap(),
            vec![Value::Boolean(false)]
        );
        keys.clear();
        mouse.clear();
        let buttons = controls.mission_script_buttons(&keys, &mouse, None);
        assert!(buttons["CONTROL_FIRE"].held);
        assert!(!buttons["CONTROL_FIRE"].just_pressed);
        mouse.release(MouseButton::Left);
        let buttons = controls.mission_script_buttons(&keys, &mouse, None);
        assert!(!buttons["CONTROL_FIRE"].held);
        assert!(buttons["CONTROL_FIRE"].just_released);
        let mut pad = Gamepad::default();
        pad.digital_mut().press(GamepadButton::South);
        assert!(
            controls.mission_script_buttons(&keys, &mouse, Some(&pad))["CONTROL_SKIP_CUTSCENE"]
                .just_pressed
        );
    }
    #[test]
    fn gamepad_sticks_buttons_and_deadzone() {
        let c = Controls::parse(include_str!("../config/controls.json")).unwrap();
        let mut pad = Gamepad::default();
        pad.analog_mut().set(GamepadAxis::LeftStickY, 0.1);
        let sample = c.sample(&ButtonInput::default(), &ButtonInput::default(), Some(&pad));
        assert_eq!(sample.input.forward, 0.0);
        pad.analog_mut().set(GamepadAxis::LeftStickY, 1.0);
        pad.analog_mut().set(GamepadAxis::RightStickX, -1.0);
        pad.digital_mut().press(GamepadButton::RightTrigger2);
        pad.digital_mut().press(GamepadButton::South);
        let sample = c.sample(&ButtonInput::default(), &ButtonInput::default(), Some(&pad));
        assert_eq!(sample.input.forward, 1.0);
        assert_eq!(sample.input.turn, -1.0);
        assert!(sample.input.fire);
        assert!(sample.start);
    }
    #[test]
    fn defaults_and_keyboard_and_mouse_wii_mapping() {
        let c = Controls::parse(include_str!("../config/controls.json")).unwrap();
        let mut k = ButtonInput::default();
        let mut m = ButtonInput::default();
        k.press(KeyCode::KeyW);
        k.press(KeyCode::Space);
        k.press(KeyCode::KeyC);
        k.press(KeyCode::KeyG);
        k.press(KeyCode::KeyX);
        k.press(KeyCode::KeyT);
        k.press(KeyCode::KeyV);
        k.press(KeyCode::KeyZ);
        k.press(KeyCode::KeyK);
        k.press(KeyCode::KeyJ);
        k.press(KeyCode::KeyN);
        k.press(KeyCode::KeyB);
        k.press(KeyCode::KeyH);
        k.press(KeyCode::KeyM);
        m.press(MouseButton::Right);
        let a = c.sample(&k, &m, None);
        assert_eq!(a.input.forward, 1.0);
        assert!(a.input.fire);
        assert!(a.lock);
        assert!(a.call && a.call_held);
        assert!(a.send && a.send_held);
        assert!(a.dismiss && a.dismiss_held);
        assert!(a.transfer);
        assert!(a.load_vehicle);
        assert!(a.unload_vehicle);
        assert!(a.next_unit);
        assert!(a.prev_unit);
        assert!(a.next_unit_type);
        assert!(a.prev_unit_type);
        assert!(a.global_toggle);
        assert!(a.toggle_map);
        let look = c.mouse_look(Vec2::new(10.0, -5.0));
        assert!(look[0] < 0.0);
        assert!(look[1] > 0.0);
    }
    #[test]
    fn rejects_typo_and_invalid_deadzone() {
        let json = include_str!("../config/controls.json");
        assert!(Controls::parse(&json.replace("\"W\"", "\"Bogus\"")).is_err());
        assert!(Controls::parse(&json.replace("0.18", "1.0")).is_err());
    }
    #[test]
    fn remapped_fire_works() {
        let c =
            Controls::parse(&include_str!("../config/controls.json").replace("\"Space\"", "\"F\""))
                .unwrap();
        let mut k = ButtonInput::default();
        k.press(KeyCode::KeyF);
        assert!(c.sample(&k, &ButtonInput::default(), None).input.fire);
    }
    #[test]
    fn rebind_key_and_gamepad_roundtrips_json() {
        let mut c = Controls::defaults();
        c.rebind_key("fire", KeyCode::KeyE).unwrap();
        c.rebind_gamepad("fire", GamepadButton::South).unwrap();
        assert_eq!(c.keys_label("fire"), "E");
        assert_eq!(c.gamepad_label("fire"), "South");
        let serialized = c.to_json_pretty().unwrap();
        let reparsed = Controls::parse(&serialized).unwrap();
        assert_eq!(reparsed.keys_label("fire"), "E");
        assert_eq!(reparsed.gamepad_label("fire"), "South");
        let mut k = ButtonInput::default();
        k.press(KeyCode::KeyE);
        assert!(
            reparsed
                .sample(&k, &ButtonInput::default(), None)
                .input
                .fire
        );
    }
}
