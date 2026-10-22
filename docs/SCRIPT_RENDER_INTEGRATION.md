# Original script → movement → original model integration

2026-10-03. **Internal diagnostic, not a complete original mission or replacement play launcher.** The full Rust port remains the project scope.

**Later milestones:** [CAMERA_OBSTRUCTION.md](CAMERA_OBSTRUCTION.md) corrects the reproduced blocked death shot. [UNIT_COLLISION.md](UNIT_COLLISION.md) adds replacement infantry collision, a +40 fixture, and an active blocker preview. [UNIT_NAVIGATION.md](UNIT_NAVIGATION.md) subsequently adds bounded routing and home arrivals around blockers; original navigation parity remains unfinished. The +100 straight-line commands/results below describe the earlier milestone and now require `BW_UNIT_COLLISION_DISABLED=1` for reproduction.

## Functional path now exercised

`Gambit_Part_1_Stay_Here` executes its original bytecode against changing unit positions:

1. Captures its actor's original home X/Z.
2. Waits for the opening-cutscene flag.
3. Checks distance using the circular IsInArea signature.
4. Issues the original GoToArea call when displaced.
5. Observes movement toward home, then yields while inside its area.
6. Exits when its actor dies.

The explicit diagnostic navigation backend executes these orders and updates the same position registry read by scripts. It uses the authored troop ForwardSpeed, does not overshoot the requested radius, switches run/idle animation state and respects scheduler pause. Death stops movement and selects the authored one-shot death clip.

Navigation defaults to **Unavailable**. Running queued orders without a configured backend fails atomically; it does not silently use straight-line motion. The only current backend is opt-in **StraightLineDiagnostic**: no pathfinding, unit/building collision, avoidance, slope/water rules or vehicle steering. GoToArea's supported signature is limited to the observed forced-order form with zero in its still-unverified scalar argument; all other forms reject. Speed units and arrival semantics remain replacement policies pending original-runtime comparison.

## Native visual integration

`bw-original-viewer` now has an opt-in `BW_SCRIPT_PREVIEW` mode. It integrates the isolated original script with the existing original-terrain/scenery renderer:

- Uses original Sgrunt02 model and its authored animation bindings.
- Updates actor position, candidate facing and terrain presentation height.
- Deforms **private actor mesh copies**, avoiding accidental animation of every instance sharing its model.
- Updates the actor's weapon/helmet attachment transforms from current candidate bone poses.
- Follows the actor with a diagnostic camera and displays actual movement/animation state.
- R displaces the live actor again; P pauses simulation; K applies lethal raw damage. Existing orbit/zoom/Esc controls remain.

The script/actor pairing, actor activation, initial displacement and `cutsceneStart=1` are **explicit fixture inputs**, not a restored authored startup sequence. The scene's other placements remain static with activation rules unapplied. K is a damage test, not restored weapon combat. None of this bypasses or completes the mission's win/loss cinematics.

```sh
BW_SCRIPT_PREVIEW=1 cargo run --locked --features viewer --bin bw-original-viewer
```

Existing play launchers were not changed or relabelled.

## Validation

- Opt-in original-data test verifies home capture, issued order, backend-disabled rollback, position changes, run/idle state, pause, radius arrival and script termination after death.
- Native macOS/Metal return-home smoke: 180 frames at a diagnostic 30 Hz; actual arrival distance 5 and Idle state asserted.
- Native macOS/Metal death smoke: injects damage at frame 100; verifies original script termination and death clip clamping by frame 180.
- Inspected local screenshots of the running and fallen original actor in original scenery.

Reproduction after building the viewer:

```sh
BW_SCRIPT_PREVIEW=1 BW_SMOKE=1 BW_CAPTURE="$PWD/local/analysis/original-script-return-home.png" target/debug/bw-original-viewer
BW_SCRIPT_PREVIEW=1 BW_SMOKE=1 BW_SCRIPT_SMOKE_DEATH=1 BW_CAPTURE_FRAME=140 BW_CAPTURE="$PWD/local/analysis/original-script-death.png" target/debug/bw-original-viewer
```

`BW_CAPTURE_FRAME` defaults to 100. Smoke windows exit themselves; no existing user windows were closed.

## Visual limitations observed

The first death capture was obscured by scenery because the camera lacks collision/occlusion handling. It is preserved as `local/analysis/original-script-death-occluded.png` and its log. The later death capture uses a different controlled damage point on the same diagnostic path; this is **not** a fix for camera obstruction.

BAN pose decoding, packed translations/root motion, attachment conventions, terrain/material gaps and some scene placements remain imperfect. Running/falling are now visibly connected to script-driven state, but original animation fidelity is not accepted. A single scripted actor in the restored scene is not the complete original mission.

Current evidence: `docs/evidence/script-render-integration.json`.
