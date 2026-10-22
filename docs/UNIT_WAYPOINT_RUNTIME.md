# Unit waypoint runtime — partial first-mission support

The original mission is **not yet playable**. This adds actual infantry route movement, not a completed-cinematic flag or an additional replacement encounter.

## Implementation

- `mission_waypoints.rs` loads authored matrices, primary/optional successors (`NextWP`, `mOptionalNextWP1`, `mOptionalNextWP2`), flags and pause times. All reachable primary and optional paths are validated before installing an order; unknown successors, nonfinite values, degenerate self-only optional branches and unsupported facing/camera flags fail explicitly.
- FollowWaypoint's X/Z arguments are transformed by **each waypoint's local basis plus translation**, rather than interpreted as radii or copied directly into world coordinates.
- Five-argument `FollowWaypoint(unit, waypoint, x, z, order)` delegates to `World::follow_waypoint`. Strict reference mode accepts `ORDER_FORCED` on living, present units with a supported body/speed and explicitly selected movement backend; the opt-in playable rewrite additionally accepts `ORDER_NORMAL` and returns `false` without mutation when targeting a dead unit (`0x80145288`). No automatic straight-line fallback is enabled.
- Route links feed the existing move-order/collision/A* systems. A blocked unit retains the current waypoint. A completed link advances to its successor using transactional `World::waypoint_rand` (`CrtRand`, seeded to `1` per `0x805fc6a0`); zero-length cycles are bounded to one link per tick.
- Optional successor selection follows `cWaypoint::GetNextWaypoint` (`0x8003dfcc`): when both optional slots are `0`, `NextWP` is returned without consuming a random draw; when either optional slot is nonzero, non-null candidates `!= current_waypoint` are collected in slot order `[NextWP, mOptionalNextWP1, mOptionalNextWP2]` and indexed by `CrtRand::next_u15() % count` (`0x80258a60`).
- Pause flag 0x10 controls pause time. Unflagged pause values are not applied. Pauses conservatively round to tick boundaries rather than reusing wait time for movement.
- Top-speed and road-category flags are accepted using the existing authored-forward-speed movement policy. Native acceleration, normal-versus-top-speed dynamics and exact radius adjustments are **not** claimed equivalent.
- Player freeze holds route progression. New GoToArea orders, direct control, death, despawn and transfer to player control cancel appropriate route state. Invalid replacement orders preserve existing orders.

## Tests

Synthetic collision tests verify a wall prevents node skipping, A* detours around a finite wall and continues onto the next link, and freeze/cancellation work. A rotated-basis test proves local offsets are transformed. Pause, invalid input, unsupported flags, cyclic routes and optional-branch candidate filtering/RNG consumption are tested.

Original-data fixtures resolve **Troop.Sstrikebazooka01**, **Waypoint.WP01** and the intro's **15/15 offsets**, plus all **15 `Battle*` branching waypoint chains** (`Waypoint.BattleLeft01..05`, `BattleRight01..04`, `BattleMainLeft01..03`, `BattleMainRight01..03`, `BattleLastLeft01`, `BattleLastRight01`), verifying `CrtRand` branch progression across `550030756 -> 550030757 -> 550030882`.

The configured intro runtime still fails at its first FollowWaypoint when no navigation scene has been installed, with `Waypoint movement backend not configured`.

## Native provenance

- Wrapper `0x800e9238`: writes third/fourth numeric arguments to X/Z order fields and defaults the separate arrival parameter to 1.0 (r2−0x7fd4); `0x800e93fc..0x800e940c` checks `0x80145288` for dead units and returns `false`.
- Order creation `0x801d93c0` → constructor `0x8015efb4` → relative-target helper `0x80144c40`.
- Position helper `0x80144da4`: combines target transform basis and translation when a nonzero local vector is present.
- Update beginning `0x8015f0a4`: top-speed flag influences speed selection. Arrival section `0x8015f338` checks pause/facing flags; `0x8015f368` reads pause time and `0x8015f498` calls `cWaypoint::GetNextWaypoint` (`0x8003dfcc`) with `r4 = r3` (current waypoint), which draws from C-runtime `rand()` (`0x80258a60`, state at `r13 - 0x4ac0 = 0x805fc6a0`, initial seed `1`) when optional branches exist. Facing flags stay rejected.
- Local traces: `follow-waypoint-native.txt`, `follow-event-copy.txt`, `follow-order-constructor.txt`, `follow-target-position-helper.txt`, `follow-order-update.txt`, `waypoint-branch-selection.txt`. Some Capstone mnemonics misidentify Wii paired-single instructions; printed later-ISA names must not be treated as authoritative.

## Vehicle follow-up findings, not an implementation

All 30 ground-vehicle instances in the extracted level declare `mPhysEngine=physEngODE`. The editor enum also lists physEngAgeia. Vehicle bases reference both `mPhysicsBase` and `mPhysicsBaseNewEngine`, with differing attack/patrol speeds; these must not be selected by guesswork. Example base 260015336 references 2138048309 (attack/patrol 10/10) and 50034654 (11/8). Both reference additional reflected physics parameters. **mInertiaBox is an inertia parameter, not a verified collision hull.** No fake tank cylinder/profile was installed.

## Validation

**159 tests pass:** 120 viewer-library, 11 startup, 4 original-assets, 9 original-mission, 15 Python. Formatting, warning-denying Clippy and locked viewer builds pass. The replacement battle's offscreen victory → restart → defeat regression still passes.

Evidence: `evidence/waypoint-runtime-validation.json`. Logs remain ignored under `local/analysis/waypoint-runtime-*.log`. No commits, publication, agents or forced window closures.
