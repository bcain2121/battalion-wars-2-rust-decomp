# First original combat objective — verified

The first-mission integration build now reaches and completes the original barrel/unexploded-shell tutorial objective. This is **not full mission completion**.

## What is connected

- Gold stars follow authored marker visibility and attachment positions. Initial visibility comes from `mFlags & OBJECTIVE_MARKER_VISIBLE` (bit 1), consistent with the preserved editor enum reference. Stars use a procedural Rust mesh, with distance/bearing guidance.
- Original movement and aircraft-look tutorials advance through actual player movement/aim. No progress globals are overwritten.
- Scripted aircraft GoToArea uses the source flight profile's `mCruiseSpeed`, preserving authored altitude. This narrowly supports scripted flyaways, not aircraft combat, terrain avoidance, banking, player flight or water movement.
- Rifle combat uses source damage channels/resistances, cadence, clip size, reload time and range. Rust-owned deterministic hitscan replaces native projectile travel, scatter and ricochet. Actor cylinders and the nearest static/terrain hit determine contact. Allies intercept rounds without friendly damage.
- Nearby active ground rifle units acquire opposing units and return fire. This is a limited Rust AI policy, not recovered native AI. Unsupported weapon profiles still error; heavy weapons and full squad attack behavior are not silently substituted.
- Aim assist includes eligible destructible geometry as well as opposing ground units. It is a firing assist, not a completed native target-lock/reticle system.
- The original pause-tutorial coroutine controls the pause and dismissal. Game time, movement and combat stop while no-pause time, tutorial processing and radio playback continue. Enter/Xbox A supplies the real confirmation input; there is no automatic dismissal in normal play.
- Tutorial radio with the source -1 duration sentinel stays visible until the authored clear call. Objective notifications and visible/completed objective flags are retained and consumed by presentation.
- Portable tutorial text replaces Wii control artwork/layout. Native sound/visual omissions are disclosed; hardware audio and full character animation remain pending.
- `StopAndGuard` actually cancels movement/follow orders while rifle guard combat remains available.
- Third-person framing is offset over the shoulder. Collision checks still shorten the camera from the player anchor, not from a distant aim point.

The shared tutorial functions also configure/clear inactive player-two controller/message state. That is bookkeeping for original single-player scripts, **not multiplayer support**.

## Controls

- WASD / left stick: move
- Arrow keys / right stick: look
- LMB / configured right trigger: fire
- RMB / Tab / left trigger: firing aim assist
- E / Xbox X: recruit nearby friendly ground units
- Enter / Xbox A: acknowledge the pause tutorial
- R: restart; Escape: close

Hardware controller testing remains pending. Normal launches execute the original opening; diagnostic pre-roll is not enabled by the launcher.

## Acceptance evidence

`tests/first_mission_runtime.rs::original_intro_movement_look_pause_and_first_combat_objective`:

1. Runs all 97 authored global/unit coroutines through the opening.
2. Walks to the first star through movement input and verifies the script hides it.
3. Aims at the original aircraft and verifies the original look tutorial completes.
4. Walks toward the shooting marker, acknowledges the tutorial using input, and verifies 193 paused frames preserve game time/player position while no-pause time advances.
5. Fires the source rifle with aim assist. All three tutorial barrels die, and authored objective `550032165` becomes complete.
6. Verifies invalid-input rollback and restores the initial session on restart.

The completed run records 4,641 input frames, 109 total rifle shots and 32 damaging contacts; these counters include all combat actors. It reaches the objective at approximately 74.13 seconds of game time. No test teleports the player, edits original script bytes or writes progress globals.

Separate original-data **flat-arena fixtures** test damage, cadence/reload, cover, invulnerability, retirement, enemy return fire and friendly interception. Those fixtures are not mission-completion evidence.

Validation:

- 309 viewer-enabled tests passed, including ignored/private-asset cases.
- 185 overlapping non-viewer library tests passed; do not sum these with the viewer total.
- Viewer all-target Clippy (`-D warnings`) and formatting passed.
- Owned windowless GPU runs verified original-objective completion and synthetic keyboard/LMB controls; both final logs contain no ERROR/WARN lines. Both captures were inspected.

Evidence: `docs/evidence/first-combat-validation.json`. Captures, input recordings and full logs remain private under ignored `local/analysis/`.

## Diagnostic replay

The integration test writes `local/analysis/first-combat-inputs.json`. `BW_MISSION_REPLAY` reads this bounded input recording, executes all but the final 120 inputs before rendering, then renders the last inputs and asserts the authored objective is complete. This is deterministic input replay, not full rendered-intro or hardware acceptance.

## Continuation: next marker and bomber ambush

The next marker is enabled by its visible parent objective, not an explicit marker call. The Rust presentation policy now reads `mParent`: parent visibility enables guidance and parent completion suppresses the retired marker. This fixes the stale barrel star and exposes the camp destination; it is not a claim of full native marker-effect parity.

`tests/first_mission_squad.rs` replays the recorded first objective, walks more than 50 units toward the next marker and verifies that original scripts trigger the bomber ambush. The test requires the private first-combat recording generated by the earlier integration test. It asserts the known unsupported boundary and unchanged frame/player position after rollback—not successful cinematic completion.

The earlier blocker was `Gambit_Part_1_Cutscene_Bombed pc 93: Teleport: Unsupported gameplay call/signature: Teleport`. Both it and the subsequent anti-air `AttackTarget` boundary are now superseded: the original ambush completes, reveals the staff and returns player control. The subsequent faded staff-transfer blocker is also resolved. The current blocker is `Gambit_Part_1_SolarBattle pc 93: FollowWaypoint: Unsupported unit waypoint flags/branch selection`, while the staff cutscene remains frozen. See [STAFF_TRANSFER.md](STAFF_TRANSFER.md) for the current 319-test checkpoint. No call is skipped. See [AMBUSH_COMPLETION.md](AMBUSH_COMPLETION.md) for the 318-test checkpoint and full-sequence replay; [BOMBER_RUNTIME.md](BOMBER_RUNTIME.md) preserves the earlier 311-test checkpoint. The ambush occurs before squad recruitment.

Historical continuation validation: 310 viewer all-target tests, clean Clippy; reviewed `local/analysis/next-objective-guidance-01.png` shows the original completion message and the new destination distance. Its GPU log has no ERROR/WARN lines. The previous 309-test evidence remains a historical checkpoint.

## Next acceptance slice

Resolve the battle waypoint semantics to continue the staff cutscene after the now-working faded player transfer, then finish staff recovery and squad objectives. See [STAFF_TRANSFER.md](STAFF_TRANSFER.md) for the current boundary. Subsequent tutorial cards, heavy weapons, capture, player replacement, authored defeat/victory and a complete mission restart remain unfinished. Menus, profiles, other platforms and visual/audio fidelity remain deferred.
