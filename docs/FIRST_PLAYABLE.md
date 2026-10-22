# First playable: terrain skirmish

> Historical capsule-skirmish milestone. The main launcher now runs the original-asset battle documented in `PLAYABLE_BATTLE.md`. Commands and behavior below describe the earlier prototype; run it directly with `bw-play` or pass `mods/training/level.json` explicitly to the launcher.

## Play now

On this Mac, double-click **Play Battalion Wars Rust.command** in the repository root. It builds from the locked dependencies, sets the working directory, and opens the game. The local terrain data is already extracted on this machine.

Press **Enter** to deploy. You are the blue unit; eliminate the five red units. The blue perimeter bounds this temporary arena.

| Input | Action |
|---|---|
| WASD | Move relative to camera heading |
| Left/right arrows or Q/E | Orbit camera / change heading |
| Shift | Sprint |
| Space or left mouse, hold | Fire at the highlighted locked target |
| Tab | Cycle visible targets |
| P | Pause/resume |
| R | Restart, including after victory/defeat |
| Escape or close window | Quit |

Lock-on is automatic within range and terrain line-of-sight; it can select an offscreen target. Use Tab/orbit to inspect the current target. A yellow marker identifies it when visible. No ammunition limit. Three hits defeat each enemy.

Terminal equivalent:

```sh
cargo run --locked --features viewer --bin bw-play
```

## What this is—and is not

This is a playable **temporary skirmish on original recovered terrain**, not the original campaign and not a completed port. The objective, spawn selection, capsule units, movement speeds, autoaim, damage values, enemy AI, boundaries, camera and health rules are NEW scaffolding. They do not claim original game behavior.

Original mission scripts and model meshes are not executing/rendering. Textures decoded elsewhere are not yet applied to this terrain. World scale/origin and color interpretation remain provisional. Terrain-only collision samples match the displayed mesh triangles; this is not the original game's collision implementation.

Only this native macOS build has been run. Windows/Linux remain portability targets, not verified releases. No multiplayer, save/load, original vehicles, audio, squad orders or campaign progression yet.

## Architecture

- `src/skirmish.rs`: Bevy-independent temporary gameplay state machine and triangle-consistent ground sampler.
- `src/bin/bw-play.rs`: Bevy 0.19.1 rendering, keyboard/mouse input, HUD, lock marker, shot tracer, pause/start UI.
- `src/terrain.rs`: original-terrain reconstruction.
- `Play Battalion Wars Rust.command`: local macOS launcher. Requires Rust toolchain and local extracted terrain, no ROM download/upload.

The terrain patch is selected automatically from contiguous low-relief ground. Movement rejects voids, steep single steps and the 23-unit arena boundary. Enemies approach, then periodically inflict damage with terrain line-of-sight checks. There is no sophisticated navigation or unit-unit collision.

## Validation

- 24 Rust tests and 15 Python tests pass.
- Warning-denying Clippy passes with the viewer/playable feature enabled. A targeted allowance documents Bevy's explicit disjoint query filter types.
- Real-terrain simulation verification checks movement, victory, defeat, restart after both outcomes. Run `BW_VERIFY=1 target/debug/bw-play`.
- Bevy/Metal automated playthrough moved the player, turned, fired, took damage and won: 15 hits / 15 shots, 79 health remaining, victory at frame 342. Native process exited successfully.
- In-engine screenshot captured and visually inspected. Initial unsupported HUD symbols were replaced by ASCII.
- Automated playthrough injects simulation inputs; physical keyboard/mouse handling still needs William's hands-on check. No claim of human playtesting.
- Start screen keeps gameplay paused until Enter (P or R also starts/resumes). The game is not silently consuming health while waiting for William.

## Developer replay

- `BW_SMOKE=1 target/debug/bw-play`: 720-frame automated movement/fire replay; exits success only if victory occurs.
- `BW_CAPTURE=/absolute/new-image.png`: captures window at frame 197 in the current build. Use a fresh path; screenshot tool can overwrite.
- `BW_VERIFY=1`: verifies actual terrain gameplay state without launching a window.
- Launcher unsets all three so developer automation cannot leak into ordinary play.

## Next priorities

1. Align original world coordinates and recover real model geometry.
2. Replace capsule scaffolding with original unit model/animation rendering.
3. Restore original material/texture terrain rendering.
4. Implement measured script bindings and coroutine behavior, then a real mission.
5. Keep this skirmish as a regression harness, not a substitute for full-game compatibility.
