# Modding and controller foundation

## Goal confirmed by William

Reimplement the whole game as a native Rust/Bevy runtime for macOS, Linux and Windows, with custom levels, characters and remappable conventional controls. Wii motion input must not be a dependency. The temporary skirmish is a development harness, not the final product or replacement for original campaigns.

## Available now

### Two launchable prototype levels

- **Play Battalion Wars Rust.command** — now runs the original-asset Fort Assault battle; see `PLAYABLE_BATTLE.md`. The JSON scenario interfaces below still apply to the separate `bw-play` prototype, not this encounter.
- **Play Custom Sandbox.command** — an ORIGINAL custom rolling-hills heightfield defined in JSON, using texture references to the locally owned resource archive; same temporary skirmish.

These are not two restored campaign missions. Neither runs original mission scripts. The custom sandbox demonstrates a data-driven custom level independent of Wii repacking.

### Controller mapping

Standard Bevy gamepad input, no Wii controller/motion requirement:
- Left stick: movement.
- Right stick: camera yaw/pitch.
- RT/R2 (`RightTrigger2`): fire.
- LB/L1 (`LeftTrigger`): cycle lock.
- Left-stick click (`LeftThumb`): sprint.
- South face button (A/Cross): start.
- Start/Menu: pause/resume.
- North face button (Y/Triangle): restart.

The HUD shows whether a controller is detected. Uses the first connected Bevy gamepad. Physical hardware has NOT been verified; synthetic Gamepad axis/button tests pass. Platform drivers and button naming can differ. No vibration, multi-controller selection or controller mapping UI yet.

Keyboard controls still work simultaneously. Up/down arrows now adjust camera pitch.

### Remapping

Edit `config/controls.json`, then restart. Supports all letter keys, arrows, Space/Tab/Enter/Escape/left-right Shift; multiple keys per action. Gamepad buttons, analog deadzone, sensitivity and vertical inversion are configurable. Stick roles and left mouse firing are fixed for now. Unknown/missing actions, invalid keys and invalid deadzone/sensitivity are rejected rather than silently ignored.

### Editable level manifests

See `mods/training/level.json` and `mods/sandbox/level.json`:

- name: displayed title.
- terrain: repository-relative original `.out` or native custom `.json` heightfield.
- resources: repository-relative decompressed original `.res` texture archive.
- arena_center: explicit `[x,z]`, or null for automatic low-relief selection.
- player_color / enemy_color: RGB components in 0..1.

Paths reject absolute and parent traversal components, but this is a local trusted mod format, not a sandbox for untrusted packages or filesystem symlinks. Manifests contain data only, no shell commands or script execution.

Launch a copy with:

```sh
cargo run --locked --features viewer --bin bw-play -- mods/my-level/level.json
```

### Custom terrain

`mods/sandbox/terrain.json` is editable source data, not a game dump:
- width, height: 4..256 vertices, each dimension must be `3n+1`.
- heights: row-major `z * width + x`, exactly width*height finite floats.
- textures: two original texture resource names. At present generated custom terrain uses the first layer (blend=0).
- Grid spacing: 1 prototype unit. Keep a contiguous approximately 47x47 area for the current fixed-radius combat harness.

Change heights, colors or center, duplicate the manifest, and relaunch. No visual terrain editor, arbitrary objectives, customizable spawn list, character model import or mod packaging UI yet.

## Textures implemented

Original terrain uses LTAM material pairs and per-subtile material IDs; all 14 material definitions resolve to six named textures in the sample. Both CMPR/DXT1 and Nintendo tiled RGBA8 top-mip formats decode. Primary UVs interpolate the four stored corners; secondary UVs use the 16 explicit pairs; normalized by 4096.

Custom WGSL blends two sampled textures using stored vertex alpha. Blend direction, brightness, lighting, wrap semantics and original rendering parity remain provisional. Top mip only; no mip chain yet. Geometry normals are generated; original normal handling is not recovered. Original global material flags remain uninterpreted.

## Validation

- 33 Rust tests (including four control-input tests with viewer feature) and 15 Python tests pass.
- Warning-denying Clippy passes.
- Both original-terrain and custom-sandbox gameplay verification pass movement, victory, defeat and restart.
- Both levels complete a native Metal automated victory run, exit 0, with no ERROR log lines.
- Render captures reviewed under ignored `local/analysis/`.
- First texture test caught an asset-root error despite a successful gameplay result. Fixed AssetPlugin to use the repository assets directory; subsequent tests explicitly reject ERROR logs. A successful simulation alone does not establish a successful render.

## Next milestones — not yet implemented

1. Recover original model mesh/material/animation paths and replace capsule proxies.
2. Establish stable asset IDs and an override registry, then support custom character imports.
3. Verify original world-coordinate scale/origin and object placement.
4. Implement original Lua 5.0 runtime semantics and engine binding compatibility.
5. Restore one entire original mission: objectives, AI, squad commands, vehicles, audio and saving.
6. Expand mission-by-mission across the campaign; document multiplayer separately.
7. Visual level tools, import/export, versioned mod packages and per-action in-game control remapping.
8. Native Windows/Linux tests, packaging and hardware controller validation.
