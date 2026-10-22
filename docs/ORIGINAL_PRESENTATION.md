# Original battlefield presentation checkpoint

## Launch

On macOS, double-click `Play Battalion Wars Rust.command`. It builds the locked Rust/Bevy original viewer and opens SP_0.1 directly in presentation mode. This is **not Dolphin, Fort Assault, or restored mission gameplay**. Original extracted files remain required and private.

The former default encounter is preserved in `Play Fort Assault Prototype.command`. The custom JSON sandbox remains separate. Explicit JSON arguments to the main launcher retain the previous sandbox route.

## What you can inspect

- Original terrain and authored model placements, including troops, vehicles, ruins and bridges.
- Three selectable viewpoints: staff bearer at its source placement, an authored battlefield landmark, and a wide overview.
- Keyboard/controller view navigation, zoom, preset reset and connected-controller status.
- An always-visible statement that poses are static and mission startup is still blocked at ArtInitialisation / Kill.

| Action | Keyboard | Controller |
|---|---|---|
| Move inspection view | WASD | Left stick |
| Look/orbit | Arrow keys | Right stick |
| Zoom in/out | Q / E | Right / left bumper |
| Next viewpoint | Tab | South / A |
| Reset viewpoint | R | North / Y |
| Close | Escape | Keyboard Escape |

These controls move the **inspection camera**, not the soldier. No motion controls are required. They are currently fixed presentation bindings; `config/controls.json` continues to describe the separate gameplay prototypes. Physical Xbox-controller verification remains outstanding.

## Validation and captures

- Locked viewer build, 179 library tests and 2 presentation input tests passed.
- Viewer-feature all-target Clippy with warnings denied and formatting passed.
- Both launchers passed `bash -n` and retain executable permission.
- Native Bevy/Metal offscreen smoke visited all three presets and emitted `ORIGINAL_PRESENTATION_VERIFY_PASS views=3 mission_runtime=false`.
- Rendered captures were visually reviewed; the initial obstructed landmark angle was corrected and unsupported HUD glyphs replaced.
- Private captures: `local/analysis/presentation-{player,landmark,overview}.png`; player capture predates the ASCII-only label cleanup.
- Logs: `local/analysis/presentation-{build,input-tests,library-tests,clippy,smoke,overview}.log`.

This does not validate physical gamepad input, full keyboard event injection, audio, Windows/Linux execution, original animation, mission activation rules, combat, cinematics, victory or defeat. Missing/unsupported source materials and the existing static-scene omissions remain visible in the loader audit. Inspection camera motion is not native player/camera parity and has no collision guarantee.

## Next gameplay milestone

Finish live spatial registration and the remaining explosion construction/runtime contracts, bind transactional Kill into World, then run the unmodified shared startup. The presentation mode deliberately does not bypass that failure, run replacement combat, or fabricate a victory.
