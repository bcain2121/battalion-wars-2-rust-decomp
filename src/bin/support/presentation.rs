//! Original-scene presentation controls. No replacement combat or mission simulation.
use super::{CameraState, Loaded};
use bevy::prelude::*;

#[derive(Resource)]
pub struct Presentation {
    views: Vec<(&'static str, Vec3, f32)>,
    index: usize,
    initialized: bool,
}
#[derive(Component)]
pub struct Status;
impl Presentation {
    pub fn new() -> Self {
        Self {
            views: vec![],
            index: 0,
            initialized: false,
        }
    }
}
fn axis(pad: Option<&Gamepad>, axis: GamepadAxis) -> f32 {
    let v = pad.and_then(|p| p.get(axis)).unwrap_or(0.0);
    if v.abs() < 0.18 {
        0.0
    } else {
        v.signum() * ((v.abs() - 0.18) / 0.82).min(1.0)
    }
}
#[allow(clippy::too_many_arguments)]
pub fn update(
    mut presentation: Option<ResMut<Presentation>>,
    loaded: Res<Loaded>,
    mut camera: ResMut<CameraState>,
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    time: Res<Time>,
    mut labels: Query<&mut Text, With<Status>>,
    mut frame: Local<u32>,
) {
    let Some(ref mut p) = presentation else {
        return;
    };
    *frame += 1;
    let pad = pads.iter().next();
    let mut reset = !p.initialized;
    if !p.initialized {
        for (id, name, distance) in [
            (550030791, "STAFF BEARER | ORIGINAL START POSITION", 24.0),
            (550034232, "ANCIENT BATTLEFIELD | AUTHORED LANDMARK", 150.0),
        ] {
            let placement = loaded
                .scene
                .placements
                .iter()
                .find(|v| v.id.0 == id)
                .expect("Presentation landmark in original scene");
            let mut center = Vec3::new(
                placement.matrix[12],
                placement.matrix[13],
                placement.matrix[14],
            );
            if placement.stick_to_floor {
                let [x, z] = bw_rust::original_scene::source_to_grid(center.x, center.z);
                center.y += loaded.ground.height(x, z).expect("Landmark terrain") * 2.0;
            }
            center.y += 3.0;
            p.views.push((name, center, distance));
        }
        p.views.push((
            "SP_0.1 | BATTLEFIELD OVERVIEW",
            Vec3::new(0.0, 20.0, 150.0),
            400.0,
        ));
        p.initialized = true;
    }
    let verify = std::env::var_os("BW_PRESENTATION_VERIFY").is_some();
    if keys.just_pressed(KeyCode::Tab)
        || pad.is_some_and(|v| v.just_pressed(GamepadButton::South))
        || (verify && (*frame == 61 || *frame == 121))
    {
        p.index = (p.index + 1) % p.views.len();
        reset = true;
    }
    if keys.just_pressed(KeyCode::KeyR) || pad.is_some_and(|v| v.just_pressed(GamepadButton::North))
    {
        reset = true;
    }
    if reset {
        let (_, center, distance) = p.views[p.index];
        camera.center = center;
        camera.distance = distance;
        camera.yaw = if p.index == 1 { -1.8 } else { 0.55 };
        camera.pitch = if p.index == 1 { 0.65 } else { 0.28 };
        println!(
            "ORIGINAL_PRESENTATION_VIEW index={} center={:?} distance={} mission_runtime=false",
            p.index, center, distance
        );
    }
    let dt = time.delta_secs().min(0.05);
    let digital =
        |positive, negative| f32::from(keys.pressed(positive)) - f32::from(keys.pressed(negative));
    camera.yaw -= (digital(KeyCode::ArrowRight, KeyCode::ArrowLeft)
        + axis(pad, GamepadAxis::RightStickX))
        * dt
        * 1.8;
    camera.pitch = (camera.pitch
        + (digital(KeyCode::ArrowUp, KeyCode::ArrowDown) + axis(pad, GamepadAxis::RightStickY))
            * dt)
        .clamp(-1.2, 1.3);
    let forward = -Vec3::new(camera.yaw.sin(), 0.0, camera.yaw.cos());
    let right = Vec3::new(camera.yaw.cos(), 0.0, -camera.yaw.sin());
    let movement = forward
        * (digital(KeyCode::KeyW, KeyCode::KeyS) + axis(pad, GamepadAxis::LeftStickY))
        + right * (digital(KeyCode::KeyD, KeyCode::KeyA) + axis(pad, GamepadAxis::LeftStickX));
    let speed = camera.distance.clamp(12.0, 200.0) * dt;
    camera.center += movement.clamp_length_max(1.0) * speed;
    let zoom = digital(KeyCode::KeyE, KeyCode::KeyQ)
        + f32::from(pad.is_some_and(|v| v.pressed(GamepadButton::LeftTrigger)))
        - f32::from(pad.is_some_and(|v| v.pressed(GamepadButton::RightTrigger)));
    camera.distance = (camera.distance * (zoom * dt).exp()).clamp(4.0, 1500.0);
    for mut text in &mut labels {
        **text = format!(
            "{}\nVIEW {} / {}   |   {}\n\nWASD / LEFT STICK  move view    ARROWS / RIGHT STICK  look\nQ E / BUMPERS  zoom    TAB / A  next view    R / Y  reset    ESC  close\n\nRESTORATION PREVIEW - original assets; static poses, no mission simulation.\nStartup blocked at ArtInitialisation / Kill. Controller hardware not yet verified.",
            p.views[p.index].0,
            p.index + 1,
            p.views.len(),
            if pad.is_some() {
                "CONTROLLER CONNECTED"
            } else {
                "KEYBOARD READY"
            }
        );
    }
    if verify && *frame == 170 {
        assert_eq!(p.index, 2);
        assert!(camera.center.is_finite() && camera.distance.is_finite());
        println!("ORIGINAL_PRESENTATION_VERIFY_PASS views=3 mission_runtime=false");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disconnected_and_deadzone_are_neutral() {
        assert_eq!(axis(None, GamepadAxis::LeftStickX), 0.0);
        let mut pad = Gamepad::default();
        for value in [-0.17, 0.0, 0.17] {
            pad.analog_mut().set(GamepadAxis::LeftStickX, value);
            assert_eq!(axis(Some(&pad), GamepadAxis::LeftStickX), 0.0);
        }
    }
    #[test]
    fn both_sticks_have_symmetric_full_range() {
        let mut pad = Gamepad::default();
        for channel in [
            GamepadAxis::LeftStickX,
            GamepadAxis::LeftStickY,
            GamepadAxis::RightStickX,
            GamepadAxis::RightStickY,
        ] {
            for value in [-1.0, 1.0] {
                pad.analog_mut().set(channel, value);
                assert_eq!(axis(Some(&pad), channel), value);
            }
        }
    }
}
