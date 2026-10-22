# Infantry collision milestone — 2026-10-03

**Follow-up:** [UNIT_NAVIGATION.md](UNIT_NAVIGATION.md) adds bounded routing, now enabled by the same launcher. Use `BW_PATHFINDING_DISABLED=1` for this earlier collision-only behavior; the recorded results below are historical.

Working replacement infantry collision is now integrated with the original-script preview. This is **not recovered original collision behavior, vehicle physics, navigation, or a complete mission**.

**Destruction follow-up:** [DESTRUCTION_COLLISION.md](DESTRUCTION_COLLISION.md) adds owned-node removal and atomic object registration. The diagnostic loader now preserves source node groups and scene-local ownership. It still uses render-derived geometry, not native collision selection, and does not enable original mission Kill.

## Try it

Double-click `Preview Original Unit Collision.command` in the repository. It builds the existing native original viewer and enables a two-troop fixture. R replays the moving troop, P pauses, K kills the moving troop, arrows orbit, W/S zoom, Escape exits. The other original troop is a stationary, explicitly placed blocker, rendered in bind pose. No original mission startup/activation is implied. Existing skirmish launchers are unchanged.

CLI equivalents:

```sh
cargo build --locked --features viewer --bin bw-original-viewer
BW_SCRIPT_PREVIEW=1 BW_UNIT_PAIR=1 target/debug/bw-original-viewer
# One animated actor against the static original scene snapshot:
BW_SCRIPT_PREVIEW=1 target/debug/bw-original-viewer
```

## Implementation and boundaries

- `src/unit_collision.rs`: continuous horizontal cylinder sweeps against height-clipped, two-sided triangles and other living active bodies. Earliest contact, bounded sliding, broad-phase instance bounds, finite-input checks, and explicit initial surface-penetration rejection. Zero-area GX strip triangles are discarded rather than becoming invisible blockers.
- Replacement infantry profile: radius 0.6, height 2, lower static-surface tolerance 0.25, contact skin 0.002 source-world units. These are provisional policy values, **not recovered body dimensions**. Body collision does not promise that animated hands, weapons, or death poses never cross another mesh.
- `Navigation::CollisionAware` uses ordered shared substeps at most one quarter of the smallest active radius, preventing large frame steps from swapping opposing/crossing units. More than 4,096 substeps fails explicitly. Scene geometry is immutable/shared through `Arc`, so scheduler rollback does not copy scene triangles.
- Real terrain heights use the same provisional conversion as the renderer. Void destinations and excessive per-step height changes block movement. No gravity, falling, climbing, bridge support, or general slope/navigation parity is claimed.
- Original GoToArea orders remain queued when blocked; no fake arrival. Blocked motion selects Idle. Pause/freeze, death, and transactional failures are tested. Dead/inactive dynamic bodies do not block; corpses are not ragdolls.
- Active units without a configured body, invalid placements, or a missing collision scene fail closed. Moving vehicle/air/water body shapes are not implemented. Visible static vehicle hulls are included in the scene triangle snapshot.
- Viewer colliders use successfully rendered primary-model geometry/transforms, excluding the moving actor and optional active blocker: 1,003 snapshot instances, or 1,002 in the pair fixture. Static troop meshes also belong to that snapshot; mesh contacts must not be interpreted exclusively as building contacts. Attachments, missing models, unexpanded clusters, alpha/collision flags, destructible updates, and solid-volume classification are not covered.
- The pair fixture activates Sgrunt03 and places it 12 units along Sgrunt02's home X axis. The moving actor starts 40 units from home. Bounded surface-free placement search is explicit fixture setup, not an original spawn algorithm. Both final native tests needed zero initial actor adjustment. Scene membership must be rebuilt when additional units become dynamic.
- Camera queries exclude the two dynamic bodies; dynamic-body/terrain/near-plane camera collision remains unfinished.

## Verified

101 tests: 74 viewer Rust unit, 4 original-asset, 8 original-mission, 15 Python. Clippy denies warnings; formatting and new launcher shell syntax pass.

Collision regressions cover thin walls at high speed, sliding, corners, touching-surface tolerance, degenerate/invalid geometry, height separation, overlapping starts, separating motion, two original troops moving head-on/perpendicularly, pause/freeze, death filtering, unsupported bodies, and atomic budget failure. Tolerance handling was hardened to prevent an almost-touching start from missing its entry intersection.

Final native Metal runs:

- Scene snapshot: 675 mesh contact resolutions, no reported overlap, stops 10.618307 units from home. This is intentionally **not** reported as arrival.
- Active pair: 694 unit contact resolutions, no reported overlap, stops 13.200027 units from home. Counts include repeated blocked substeps, not distinct collisions.
- Death still terminates the original coroutine and clamps its original death clip.
- Historical camera replay still passes with collision deliberately disabled: 145 adjusted frames.
- Static original scene and both original/custom skirmish verification profiles pass.

Final screenshots were visually inspected. Evidence: `docs/evidence/unit-collision-validation.json`; captures/logs remain ignored under `local/analysis/unit-collision-*`.

The initial +100 displacement stopped near a cliff with an obscured shot (`unit-collision-cliff.*`, retained). The new +40 fixture is a different route chosen for visibility, not a repair of that camera case. Intermediate captures before contact-tolerance hardening reached home; they are not the final collision result.

Reproduce the new native gates:

```sh
BW_SCRIPT_PREVIEW=1 BW_PATHFINDING_DISABLED=1 BW_SMOKE=1 BW_REQUIRE_UNIT_CONTACT=1 target/debug/bw-original-viewer
BW_SCRIPT_PREVIEW=1 BW_PATHFINDING_DISABLED=1 BW_UNIT_PAIR=1 BW_SMOKE=1 target/debug/bw-original-viewer
BW_SCRIPT_PREVIEW=1 BW_PATHFINDING_DISABLED=1 BW_SMOKE=1 BW_SCRIPT_SMOKE_DEATH=1 target/debug/bw-original-viewer
# Historical straight-line/camera regression only:
BW_SCRIPT_PREVIEW=1 BW_UNIT_COLLISION_DISABLED=1 BW_SMOKE=1 BW_SCRIPT_SMOKE_DEATH=1 BW_SCRIPT_DAMAGE_FRAME=80 BW_REQUIRE_CAMERA_CLIP=1 target/debug/bw-original-viewer
```

`BW_UNIT_COLLISION_DISABLED` preserves the old +100 straight-line diagnostic and is incompatible with `BW_UNIT_PAIR`. Smoke ignores live control input and closes only its own window. Nothing was published or committed.

Next: route planning around blocked units/scenery, broader dynamic activation, original player/combat/vehicle integration, and animation fidelity. A complete original mission remains the acceptance gate.
