# Original mission gameplay-state slice

2026-10-03. Full Rust/Bevy game port remains the target. **Not a complete playable original mission.** This follows MISSION_EXECUTION.md; its earlier unsupported-feature list is historical.

**Later integration update:** [SCRIPT_RENDER_INTEGRATION.md](SCRIPT_RENDER_INTEGRATION.md) connects an isolated original movement script and per-unit animation state to the native original-model renderer. Full mission integration is still unfinished.

## What now executes

- Original `ScoreScreen` initializer, including its thirteen non-capturing helper closures, using explicit local `netvariable.player1/player2` tables. This is not networking implementation.
- Original `Gambit_Part_1_WinLose` logic responds to controlled world-state inputs:
  - Player positioned at the authored Tower01 zone center: original code sets MissionOver, freezes player state, hides the tower objective, calls the original StopScoringTimer helper, sets the win cinematic flag and waits for cinematic completion.
  - Staff-bearer health reaches zero: original code calls its local nested loss function, declares enemy victory, freezes player state, calls the original scoring helper, sets the loss cinematic flag and waits. Its nested Lua call stack survives subsequent yields.
- Original `Gambit_Part_1_Invincibility` sets the passed unit's invulnerability; subsequent raw damage is actually suppressed in the typed unit state.

Tests explicitly configure a player roster/activation, move positions, and apply damage. They do **not** claim that original navigation, combat AI, cinematics or full initialization ordering produced those inputs. Tests intentionally leave cinematic completion pending; final WinMission/LoseMission, grading, fades/effects and presentation remain incomplete. No fake completion callbacks or permissive no-op host bindings were added.

## Runtime changes

`src/mission_vm.rs` now supports bounded Lua-to-Lua calls, non-capturing nested closures, fixed parameter semantics, open argument/result calls and open returns. Return destinations and caller registers survive yields. Recursion depth is capped at 128. Failure still rolls back thread, shared Lua state and host effects. Varargs, captured upvalues, tail calls, numeric table keys and several other instructions remain unsupported and fail explicitly.

`src/lua50.rs` retains parameter/vararg metadata and stores executable prototypes behind bootstrap function identities. It does not treat function identity registration as execution.

`src/mission_world.rs` adds typed units, raw damage/health, invulnerability, controlled-player roster, explicit activation state, original map zones, objective visibility and player freeze/declaration state. Health maxima and army labels come from original base objects. Unit activation is deliberately unresolved (`None`) until configured; object presence is not equated with activation. Objective visibility is similarly unresolved until set.

Additional native adapters are GetPlayerUnit, IsDead, SetUnitInvulnerable, SetHealthPercent, AllPlayerUnitsDead, FreezePlayer, UnfreezePlayer, IsInArea, SetObjectiveData (visibility only) and DeclareVictory (supported enemy enum only). Unsupported signatures reject. Player freeze/objective state is not yet wired into the graphical renderer/input loop.

Symbolic enum tokens stand in for the named native constants required by this slice. Their original numerical ABI is **not** invented. Horizontal zone containment uses the inverse X/Z transform, full-size box extents or radius. Height, flags, exact edge policy and original ABI behavior still require comparison against original execution. Raw damage is explicitly not the complete damage-type/armor/weapon simulation.

## Animation progression

`src/unit_animation.rs` resolves each troop's own base → animation set → original animation resource, rather than selecting an arbitrary named clip. For this mission, all **183 troops** resolve all five implemented states: **915 bindings / 10 unique original clips**.

The replacement playback controller preserves phase when the same state is selected, resets on transitions, loops idle/walk/run, and clamps fire/death to the end rather than endlessly replaying death. Health reaching zero selects the authored death clip. Scheduler pause freezes these animation clocks. This selection/loop policy is explicit replacement behavior, not recovered original state-machine parity.

This is animation **binding and playback-state** progress, not a fix for the visual decoder. BAN quaternion signs/flags, packed translation/root motion, attachments and skinning fidelity are still experimental. The new per-unit playback state has not yet been connected to the graphical mission loop. No new visual-fidelity claim or screenshot acceptance is made.

## Reproduction

```sh
cargo test --locked --features viewer --lib
cargo test --locked --features viewer --test original_assets --test original_mission -- --ignored --nocapture
cargo clippy --locked --features viewer --all-targets -- -D warnings
.venv/bin/python -m unittest discover -s tests -q
```

Original data remains under ignored local/. No launcher changes, publication or user-window termination.

## Next integration work

1. Restore required remaining Lua semantics and host capabilities, starting with the mission setup/activation and cinematic control paths; do not mark a cinematic complete merely to unblock scripts.
2. Connect typed original units, movement/combat/orders and selected animation state to the original-model renderer, preserving the existing skirmish only as a regression harness.
3. Verify BAN decoding against the executable/original behavior and fix pose/translation fidelity before accepting character animation.
4. Finish the complete first-mission path, then expand campaign coverage. The complete game, custom content and cross-platform acceptance remain the project scope.
