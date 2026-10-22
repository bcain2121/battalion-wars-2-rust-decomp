# Staff recovery and cutscene completion checkpoint

The original input route now executes the default `ForceUnitTransfer(PLAYER_ONE, Sstaffbearer01)` during the frozen black fade, runs the concurrent `Gambit_Part_1_SolarBattle` branching waypoint orders and `Gambit_Part_1_Cutscene_StaffBearer` trench-troop setup, completes the staff-bearer cutscene (`cutsceneStaffBearerBegin == 2`), updates objective guidance to `Objective.Trench00` (`550035435`), and returns live player control on staff bearer **550035312** at approximately **122.38 game seconds** without modifying mission scripts, forcing progression globals or relocating the player along the route.

Driving the unfrozen staff bearer another 325 input frames (> 100 units north past `Objective_Marker.Trench00`) triggers `StrikeForce01` and reaches the next real combat boundary: **`Unsupported non-rifle projectile 200001224: DAMAGE_FIRE`**. Full mission completion, flame projectiles, squad recruitment/orders and heavy weapons remain open.

## Reusable engine support

- **Frozen fully-faded default infantry transfer (`ForceUnitTransfer`):** Two arguments, or an explicit false third argument, retain the native default option distinction (`0x800e3100`). Supported only with frozen controls, black alpha `1.0`, a rewrite backend and explicit cosmetic-gap opt-in. Eligible live infantry ownership, roster membership, chase target and player lifecycle move to the new unit without incrementing the camera hard-cut revision. Target movement/follow/selection/missile orders are cleared. Visible default transfers still error.
- **Waypoint optional branch selection (`cWaypoint::GetNextWaypoint` at `0x8003dfcc`):** `mission_waypoints.rs` loads and validates `mOptionalNextWP1` and `mOptionalNextWP2` across all reachable branches. When both optional pointers are null, `NextWP` is returned without consuming a random draw. When either optional pointer is present, non-null candidates `!= current_waypoint` are collected in slot order `[NextWP, mOptionalNextWP1, mOptionalNextWP2]` and selected by `CrtRand::next_u15() % count` (`0x80258a60`, seeded to `1` per `0x805fc6a0`, stored transactionally on `World::waypoint_rand`).
- **Rewrite `FollowWaypoint` `ORDER_NORMAL` and dead-unit return (`0x800e93fc..0x800e940c` / `0x80145288`):** When the rewrite runtime is active, `FollowWaypoint` accepts `ORDER_NORMAL` in addition to `ORDER_FORCED` and returns `false` without mutation when targeting a dead unit.
- **Collision-validated non-player `cTroop` `Teleport` (`0x800d4a18` / `0x80079638` / `0x8006ca4c`):** In addition to aircraft deployment and in-place ground facing resets, `World::teleport_scripted` now relocates active non-player `cTroop` units when the rewrite scene is configured and the destination passes terrain-height and static/dynamic cylinder collision checks (`margin == 0.0`). Relocating the player unit (`self.player`) remains an explicit error so route shortcuts are impossible.

## Native evidence

- `ForceUnitTransfer` at `0x800e3100` defaults to flags `1`. True third argument adds `0x14`; optional fourth true adds `0x10`. At `0x8007c058`, clear bit 16 dispatches event 8 through `0x800f9350` -> `0x800f93cc` -> **HUD0** (`0x48554430`) sampled-audio playback (`0x801f9450`).
- `cWaypoint::GetNextWaypoint` at `0x8003dfcc` (called from `cFollowWaypointOrder` arrival `0x8015f498` with `r4 = r3`) reads `+0x84` `NextWP`, `+0x88` `mOptionalNextWP1`, `+0x8c` `mOptionalNextWP2` and draws from `rand()` (`0x80258a60`, state at `r13 - 0x4ac0 = 0x805fc6a0`).
- `Teleport` at `0x800d4a18` queries placement via vtable `+0x438` (`cTroop` target `0x80079638`) and commits via vtable `+0x434` (`cTroop` target `0x8006ca4c`), clearing pathfinding/order state on the relocated troop.
- Private disassembly: `transfer-notification-native.txt`, `transfer-notification-dispatch.txt`, `transfer-notification-play.txt`, `transfer-audio-type.txt`, `waypoint-branch-selection.txt`, plus existing transfer/camera/waypoint traces under `local/analysis/`.

## Next boundary: flame projectile `DAMAGE_FIRE` (`200001224`)

After `Gambit_Part_1_Cutscene_StaffBearer` completes (`input_frames = 1276`, `time = 122.38s`) and returns control on `Troop.Sstaffbearer01` (`550035312`) at `[99.479, 0.0, -79.711]`, driving north along the guided route passes `Objective_Marker.Trench00` (`[92.827, 0.0, -12.297]`), triggers `StrikeForce01`, and after **325 post-cutscene input frames** (`pos = [79.47431, 0.0, 22.526493]`) stops transactionally at:

```text
Unsupported non-rifle projectile 200001224: DAMAGE_FIRE
```

The failing tick preserves frame, player ownership, all positions and `waypoint_rand` state. `s.restart()` resets the session cleanly.

## Tests and rendering

- Synthetic unit tests cover default/false transfer options, optional waypoint branch validation/selection/RNG draws, `ORDER_NORMAL` dead-unit handling, and collision-validated non-player troop teleport (including rejection of player relocation and obstructed destinations).
- Original-data integration tests verify all **15 `Battle*` waypoint chains** (`tests/mission_startup.rs::original_battle_waypoint_branches_validate_and_advance_with_crt_rng`) and full staff-cutscene completion + post-cutscene `DAMAGE_FIRE` rollback + restart (`tests/first_mission_staff.rs::original_route_completes_staff_bearer_cutscene_and_returns_player_control`).
- **325 viewer all-target tests passed**, zero failed/ignored; **194 overlapping library tests passed**, not additive. Formatting, warnings-denied Clippy and viewer build pass.
- Private recordings: `local/analysis/staff-complete-inputs.json` (complete cutscene through control return, 7,536 inputs), `local/analysis/staff-trench-shot-inputs.json`, and `local/analysis/staff-fire-boundary-prefix-inputs.json`.
- Full-tail GPU replay (`BW_MISSION_REPLAY_TAIL=1300 BW_MISSION_EXPECT_STAFF=1`) rendered **1,053 frames inside the staff cutscene** (`ORIGINAL_STAFF_RENDER_PASS rendered_staff_frames=1053 player=Some(ObjectId(550035312)) following=2`) with no renderer `ERROR`/`WARN`/`BLOCKED` log lines:
  - Early `eCAMTYPE_GENERAL` shots (`staff-complete.png.frame-0320.png`, `.frame-0550.png`) remain terrain-obscured (`eCAMTYPE_GENERAL` camera-mode fidelity remains an open presentation item).
  - Trench-defense shot (`staff-complete.png.frame-0950.png`) clearly frames the Iron Legion defenders in front of the Iron Tower (`Cutscene · 116.9s · 97 scripts`).
  - Control-return captures (`staff-complete.png.frame-1280.png`, `staff-complete.png`) clearly frame the restored chase camera on `Sstaffbearer01` (`Player control · 123.0s · 97 scripts`, `NEW_PRIMARY_OBJECTIVE`, `GOLD STAR: 68 m | ahead 4 deg`) looking north toward the `Trench00` Gold Star and Iron Tower.

```sh
BW_OFFSCREEN=1 BW_SMOKE=1 \
BW_MISSION_REPLAY="$PWD/local/analysis/staff-complete-inputs.json" \
BW_MISSION_REPLAY_TAIL=1300 BW_MISSION_EXPECT_STAFF=1 \
BW_CAPTURE="$PWD/local/analysis/staff-complete.png" BW_CAPTURE_FRAME=1340 \
BW_CAPTURE_FRAMES=320,550,950,1280 \
target/debug/bw-first-mission
```

Next: inspect and implement projectile `200001224` (`DAMAGE_FIRE`) for the trench flame engagement, then continue staff/squad progression toward `Trench00` and `StrikeForce01`. No existing user preview was stopped and nothing was published.
