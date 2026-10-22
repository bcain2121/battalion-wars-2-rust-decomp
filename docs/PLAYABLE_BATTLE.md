# Playable original-asset battle

2026-10-03. The main launcher now runs **Fort Assault**, a small playable encounter using original SP_0.1 terrain, buildings, troop/weapon models, textures and animation resources in native Rust/Bevy. It is **not the restored original mission**. Full Battalion Wars 2 compatibility remains the goal; the complete original-level milestone remains open.

## Play

Double-click `Play Battalion Wars Rust.command`, then press **Enter**. It builds `bw-battle` from the locked dependencies and does not start combat while waiting at the briefing.

You control the central Solar Empire rifleman. Two squadmates follow and fire automatically. Eliminate the four Underworld riflemen, then approach the cyan uplink and stay within six world units for five seconds. Death loses the encounter; R restarts from the initial roster, health and positions.

| Action | Keyboard/mouse | Default gamepad |
|---|---|---|
| Begin / retry after result | Enter | South / A / Cross |
| Move relative to camera | WASD | Left stick |
| Sprint | Shift | Left stick press |
| Fire at locked target | Hold Space / left mouse | RT |
| Cycle enemy target | Tab | LB |
| Squad hold / follow | C | West / X / Square |
| Orbit / tilt camera | Q/E, arrows | Right stick |
| Zoom | Mouse wheel | — |
| Pause / resume | P | Start |
| Restart | R | North / Y / Triangle |
| Quit | Escape / window close | — |

Most bindings load from `config/controls.json`; squad C/West and mouse zoom are currently fixed. Physical gamepad acceptance and human keyboard/mouse playtesting remain pending. Lock-on can select an offscreen or obstructed enemy; shots require range and unobstructed sight. Use Tab and the camera to inspect targets. Allies intercept shots without friendly-fire damage.

`Play Custom Sandbox.command` retains the separate older editable prototype, not the new battle. Passing a JSON manifest explicitly to the main launcher also retains that prototype route.

## Implemented integration

- Direct movement of an original troop through the shared infantry collision solver; manual authority removes that actor's AI orders/routes rather than racing a script.
- Original textured scenery and seven original animated troop instances, with private deformable meshes and bone-attached weapons. Other authored troop placements are hidden, not activated as mission actors. Dynamic actors are excluded from static collision snapshots.
- Enemy pursuit, collision-checked routing, line-of-sight attacks, squad hold/follow and defensive fire, target cycling, health/death, shot tracers, capture objective, win/loss, pause and restart.
- Idle/walk/run/fire/death resource selection and clocks; movement takes priority over a stationary firing pose. Full animation decoding fidelity remains unresolved.
- Shared `World`, collision/navigation, original loaders and rendering are reused by `bw-battle` and internal `bw-original-viewer`; the separate capsule-skirmish implementation is not used for battle simulation. The existing controls module's input value type is shared.
- `battle.rs` stages simulation ticks transactionally. `battle_view.rs` owns presentation/input; `original_app.rs` loads the original scene for both executables.

## Explicit replacement behavior / missing original behavior

Roster selection, deployment positions, activation, team membership, movement speeds, weapon ranges/cooldowns/damage, unlimited ammunition, autoaim, cylinder hits, AI, squad following, camera and uplink objective are replacement rules—not recovered original mission execution. Authored unit health and animation/resource identities are loaded, but this encounter does not run the original startup/objective/cinematic script chain. Squad behavior is basic offset following, not formation parity. Vehicles/buildings remain static scenery. There is no troop switching, vehicle control, audio, save system, campaign progression or multiplayer.

The existing model/material omissions and unexpanded scenery clusters remain. Rotation decoding, root motion, palette accuracy and some attachments are provisional. Static primary triangles/cylinders do not reproduce all original collision flags, destructibility, water, bridges or locomotion. Navigation can exhaust its bounded search and stop safely. Camera alternatives can snap; terrain/near-plane occlusion is not fully handled. This is a short initial encounter, not campaign balancing or a complete level.

## Validation

**114 passing tests:** 86 Rust viewer-feature unit tests, 4 original-asset tests, 9 original-mission tests and 15 Python tests. Warning-denying Clippy, formatting, locked native builds and launcher shell syntax pass.

New regressions exercise manual collision and AI-order override, pause/invalid-input rollback, combat/win/loss, squad hold/follow, cover preventing fire, approaching obstructed targets, friendly interception, two-sided sight rays and camera-relative movement. Native Metal runs complete victory, defeat, and victory → shared restart handler → defeat. A 180-frame briefing smoke test confirms unchanged health/positions, zero elapsed combat time and no shots before starting; default-feature Rust tests also pass (76, not double-counted). Original-script planned pair arrival/death and both old skirmish scenarios still pass.

Native automated victory: 11.0667 simulated seconds, 72% player health, no enemies, 34 total shots, 8 enemy attacks; all five animation states observed. Defeat: 7.8333 seconds, 29 enemy attacks, player health zero. These inject normal battle inputs via an autopilot, not human input events or forced enemy damage. Restart assertions check positions, health, activation, timer, capture and cleared orders against the saved initial state.

The desktop was locked during capture: primary-window captures were black and **are not visual acceptance evidence**. Final frames were rendered with the same Bevy/Metal scene and UI to an offscreen image target and visually inspected. `BW_OFFSCREEN=1` is an internal rendering option, not a launcher default and not a workaround that unlocks or captures the desktop. The launcher clears all verification/offscreen flags before human play.

```sh
cargo test --locked --features viewer --lib
cargo test --locked --features viewer --test original_assets --test original_mission -- --ignored --nocapture
.venv/bin/python -m unittest discover -s tests -q
cargo clippy --locked --features viewer --all-targets -- -D warnings
cargo fmt --check
cargo build --locked --features viewer --bin bw-battle --bin bw-original-viewer --bin bw-play
BW_OFFSCREEN=1 BW_SMOKE=1 target/debug/bw-battle # Inert briefing, exits at frame 180
BW_OFFSCREEN=1 BW_BATTLE_VERIFY=win target/debug/bw-battle
BW_OFFSCREEN=1 BW_BATTLE_VERIFY=loss target/debug/bw-battle
BW_OFFSCREEN=1 BW_BATTLE_VERIFY=restart target/debug/bw-battle
```

`BW_CAPTURE` and `BW_CAPTURE_FRAME` save an in-progress frame; `BW_CAPTURE_RESULT` saves a result after one simulated second of animation. Verification exits automatically; existing user game windows were not closed. An initial briefing smoke attempt lacked the battle-mode exit condition and timed out; its termination was confirmed, evidence retained, and the bounded briefing mode was fixed and rerun successfully. Evidence: `docs/evidence/playable-battle-validation.json`. Proprietary frames/logs stay ignored under `local/analysis/battle-*`. No commits or publication.
