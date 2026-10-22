# Follow-camera obstruction regression

2026-10-03. This improves the isolated original-script preview, not full original camera/physics parity.

**Later milestone:** [UNIT_COLLISION.md](UNIT_COLLISION.md) adds replacement infantry collision and changes the default fixture. Historical camera reproduction below explicitly disables unit collision to preserve its original path.

## Reproduced defect and cause

The previous death fixture at frame 80 left the camera behind/inside foreground scenery. Reproduced it with the same actor path, damage frame, requested orbit and capture frame 100; the before image is almost entirely blocked. The orbit simply placed the camera at its requested distance without querying scene geometry.

Ranked checks were scene-model obstruction, terrain penetration, and a follow point inside scenery. Model-triangle queries resolved the reproduced shot without relocating the actor or changing its damage time. Terrain and inside-geometry handling remain broader limitations.

## Implementation

`src/camera_collision.rs` adds reusable segment queries with shared per-model triangle data and transformed per-instance broad-phase bounds. Narrow-phase tests use two-sided rendered triangles, not just bounding boxes. Degenerate triangles are ignored, affine inverses are validated, and invalid inputs reject.

The original viewer builds the query data from the same successfully rendered primary-model parts and placement transforms. The selected moving actor is excluded. The current scene provides **1,003 static obstacle instances**. Other attachments and terrain are not included in this query set.

The follow camera retracts ahead of the nearest intervening surface with a small clearance. If retraction leaves less than three units of viewing room, it tries a bounded set of alternative directions. Requested orbit distance/direction remain unchanged so the camera can return when clear. Orientation follows the corrected position; degenerate zero-distance cases retain a finite direction.

This is explicit replacement camera policy. It is **not** a swept-sphere or full near-plane collision system, original camera behavior, terrain collision, navigation collision or alpha-aware visibility. Alternate views may snap. A follow target inside enclosed scenery, missing/omitted meshes, transparent texture regions and future moving obstacles still need work.

## Evidence

- Wrote a failing camera-retraction test, then implemented the correction; six geometry/camera tests now pass.
- Before: `local/analysis/camera-occlusion-before.png`.
- After, same frame-80 damage and frame-100 capture: `local/analysis/camera-occlusion-after.png`. Troop is visible; the blocking wall no longer covers the view.
- Moving-actor check: `local/analysis/camera-follow-running.png`.
- Native Metal death replay: 145 adjusted frames; original script terminates and death clip clamps.
- Native Metal return-home replay: 53 adjusted frames; actual arrival distance 5 and Idle state.
- `BW_REQUIRE_CAMERA_CLIP=1` asserts that obstruction handling was exercised and that corrected focus-to-camera segments do not still cross queried triangles.
- Static scene and SVET animation-gallery smoke regressions pass, as do both temporary skirmish verification profiles.
- Validation: 68 Rust viewer unit + 4 original-asset + 7 original-mission + 15 Python = **94 passing tests**; warning-denying Clippy passes.

An intermediate replay did not follow the canonical trajectory while smoke mode still accepted live keyboard inputs. Its image/log are preserved as `camera-noncanonical-replay.*`; it was not counted as a pass. Smoke mode now ignores live movement/orbit/pause/damage keys (Escape can still close the window). An attempted unfocused-window test produced a black capture; preserved as `camera-follow-unfocused.*` and reverted that window-setting experiment. Successful captures were visually inspected; exit status alone was not accepted as visual proof.

## Reproduction

```sh
cargo build --locked --features viewer --bin bw-original-viewer
BW_SCRIPT_PREVIEW=1 BW_UNIT_COLLISION_DISABLED=1 BW_SMOKE=1 BW_SCRIPT_SMOKE_DEATH=1 BW_SCRIPT_DAMAGE_FRAME=80 BW_REQUIRE_CAMERA_CLIP=1 BW_CAPTURE="$PWD/local/analysis/camera-occlusion-after.png" target/debug/bw-original-viewer
```

`BW_SCRIPT_DAMAGE_FRAME` defaults to 100. `BW_CAPTURE_FRAME` defaults to 100. These are diagnostic inputs, not game settings. Smoke windows self-exit; existing user windows are not forcibly closed. Captures remain ignored/local.

A testable camera-query boundary and the exact original-scene replay should prevent recurrence of this specific defect. At this milestone, missions, unit collision/pathfinding, original combat/vehicles and animation fidelity remained unfinished; launchers were unchanged. See the later unit-collision milestone for current progress.
