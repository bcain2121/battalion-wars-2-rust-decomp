# Bounded infantry route planning — 2026-10-03

The original Stay_Here actor now routes around blockers and reaches its requested home radius rather than remaining stuck. This is replacement navigation, **not recovered original navigation behavior or a complete mission**.

## Run

The existing `Preview Original Unit Collision.command` now enables route planning too. R repeats the fixture, P pauses, K kills the moving actor; camera controls are unchanged. The moving Sgrunt02 and stationary Sgrunt03 are still explicit fixture inputs, not original mission activation.

```sh
cargo build --locked --features viewer --bin bw-original-viewer
BW_SCRIPT_PREVIEW=1 BW_UNIT_PAIR=1 target/debug/bw-original-viewer
# Native gates: actual search, planned-order arrival, home radius, collision checks.
BW_SCRIPT_PREVIEW=1 BW_UNIT_PAIR=1 BW_SMOKE=1 BW_REQUIRE_ROUTE=1 target/debug/bw-original-viewer
BW_SCRIPT_PREVIEW=1 BW_SMOKE=1 BW_REQUIRE_ROUTE=1 BW_REQUIRE_UNIT_CONTACT=1 target/debug/bw-original-viewer
# Preserve the earlier collision-only fixture:
BW_SCRIPT_PREVIEW=1 BW_UNIT_PAIR=1 BW_PATHFINDING_DISABLED=1 BW_SMOKE=1 target/debug/bw-original-viewer
```

`BW_REQUIRE_ROUTE` rejects configurations that disable collision or planning. `BW_UNIT_COLLISION_DISABLED` retains the older +100 straight-line/camera fixture. The launcher clears automated and disabling flags for human sessions.

## Implementation

- `src/navigation.rs`: deterministic eight-neighbor A*, with each edge subdivided and tested by the existing live cylinder/triangle/body movement solver. Diagonals cannot simply cut through obstacle corners. Directed edge results are cached only inside a single search.
- `Navigation::PlannedInfantry` initially attempts direct movement. When collision prevents the requested displacement, it queues a search. `CollisionAware` remains available without planning; the default unconfigured host still uses `Unavailable`.
- Successful searches supply waypoints. Every actual movement still runs collision checks against current bodies. A blocker moving into a cached route invalidates it and permits replanning; repeated identical script orders preserve valid routes. Changed destinations discard stale routes.
- At most one search per simulation tick, selected round-robin. Each search caps at 256 expansions, 8,192 movement probes and 64 grid cells from its origin. Grid spacing is max(1.5, 2.5 × body radius). Failed/invalidated searches have a 0.5 simulation-second retry cooldown.
- `Found`, `NoRoute`, and `Budget` are distinct outcomes. The latter two produce no accepted path and leave orders pending; they never declare arrival. Arrival uses the actual original order's destination/radius. Counters report searches, failures and planned-order arrivals.
- Scheduling/cache updates remain inside the existing transactional host update. Pause freezes pending retry clocks; frozen controlled units do not plan or move. Death/inactivity removes route eligibility. Invalid inputs and collision configuration still fail explicitly.

## Evidence

107 tests pass: 79 viewer Rust unit + 4 original-asset + 9 original-mission + 15 Python. Clippy with warnings denied, formatting, and launcher shell syntax pass.

New coverage includes finite-wall detours with collision-checked replay of every edge, deterministic results, live-body detours, enclosed/no-route and budget outcomes, invalid input, and original Stay_Here execution around a wall. That original-script fixture then moves a troop into its cached route, requires replanning and safe arrival, checks pause with a positive retry countdown, and checks destination-change invalidation.

Native Metal original-scene results:

| Fixture | Searches | Expansions / probes | Failures | Home distance | Reported overlap |
| --- | ---: | ---: | ---: | ---: | --- |
| Static scene snapshot | 1 | 3 / 162 | 0 | 4.9999943 | false |
| Active troop blocker | 1 | 8 / 365 | 0 | 5 | false |

Both have one planned-order arrival. The pair first records four live-body contact resolutions, then detours; the static snapshot records five mesh resolutions. Static meshes include troops, so mesh contacts are not exclusively building contacts.

Collision-only regression remains blocked at distance 13.200027 with 694 live-body resolutions and zero searches. Death still terminates the original coroutine and clamps its death clip. Historical camera replay still reports 145 adjusted frames with unit collision deliberately disabled. Static original rendering and both skirmish verification profiles pass.

Visually reviewed local captures: `navigation-detour.png` (frame 48, moving around the blocker), `navigation-pair.png` and `navigation-scene.png` (arrived). Logs/captures remain ignored under `local/analysis/`; structured evidence is `docs/evidence/unit-navigation-validation.json`.

## Limits / next work

This is a coarse, synchronous, bounded local planner—not a recovered navmesh, a complete global route service, predictive crowd avoidance, or squad formation logic. Long detours/targets beyond the search horizon or narrow passages may exhaust the budget or grid even when a physical route exists. No partial-path result is promoted to arrival. Dense-crowd performance/fairness is not validated beyond the bounded scheduling policy.

Collision/terrain approximations still apply: no water semantics, general bridge/climb/fall support, moving vehicle shapes, dynamic destructible mesh updates, or full original collision flags. Pose decoding, broader activation, player controls/combat, and complete original mission execution remain unfinished. No original content was published; all work remains uncommitted.
