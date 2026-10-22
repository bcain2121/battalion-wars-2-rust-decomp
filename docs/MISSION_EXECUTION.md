# Original mission execution — bounded first slice

2026-10-02 continuation. This supersedes the earlier statement that no mission function bodies execute. It does **not** mean that the mission is playable.

**Current follow-up:** [MISSION_GAMEPLAY.md](MISSION_GAMEPLAY.md) adds executable Lua calls, original win/loss triggers, typed unit state and authored animation bindings. The unsupported-feature list below describes this preceding slice.

## Implemented

`src/mission_vm.rs` executes a restricted Lua 5.0 instruction subset against the existing original bytecode/object graph. It supports finite numeric arithmetic, comparisons, Lua truthiness (zero is true), conditional branches, string-keyed tables, globals, fixed-arity host calls, fixed returns and resumable logical-time waits.

Only seven explicitly installed host capabilities exist:

| Capability | Implemented effect |
|---|---|
| GetObjectXPosition / GetObjectZPosition | Reads the loaded object's authored Mat coordinates from an in-memory position registry |
| OutOfBoundsMessageInfo | Retains the four original arguments as typed configuration; does not pretend the UI is displayed |
| DebugOut | Records a bounded in-memory diagnostic event |
| GetTime | Reads the caller-controlled logical clock, never wall-clock time |
| EndFrame | Suspends until at least the next logical frame |
| WaitFor | Suspends until deadline and at least the next frame |

These are explicit replacement-host contracts, not a claim that original ABI, invalid-ID behavior, time units or resume order have been fully recovered. Authored positions are initial data, not simulated moving-unit positions. Bounds-message scalar argument labels are provisional. No filesystem/network/process capabilities exist.

Each resume commits Lua state, thread registers/PC and host effects only after reaching a yield or return. A failed/budget-exhausted slice rolls back. The proposed scheduler uses stable numeric thread ordering, freezes logical time when paused, and commits whole ticks atomically. Unknown host calls, unsupported signatures, unsupported opcodes and unsupported Lua-to-Lua calls produce errors rather than permissive stubs.

Not implemented: general Lua function calls/call stacks, captured closures/upvalues, metatables, string coercion, numeric table keys, generic iteration, open argument/result lists, complete numeric-loop semantics, original scheduler ordering, original gameplay host APIs. This is not a complete Lua VM.

## Original bodies actually executed

1. **ArtVariables:** all 23 instructions complete.
2. **Gambit_Part_1_Global_Variables:** all 342 instructions complete, including actual table lookups and object X/Z queries. Tests compare the resulting named spawn-coordinate globals to the corresponding original Troop object positions. Eleven debug events and the out-of-bounds configuration are retained.
3. **Pause_Scoring_Timer:** the original infinite coroutine executes across multiple yields with preserved registers. A controlled fixture supplies the scoring/cutscene globals that would normally be initialized by other scripts. Tests exercise active, waiting, paused-clock and disabled states. Those fixture writes are NOT a claim that the cinematic scripts are running.

A source behavior worth preserving: disabling the scoring timer while its one-second wait is already pending still produces one final increment when that wait returns, before the bytecode checks its active flag again. The test retains this behavior rather than changing it to an assumed design.

Reproduction:

```sh
cargo run --locked --bin bw-mission-init
cargo test --locked --features viewer --test original_mission -- --ignored --nocapture
cargo test --locked --features viewer --test original_assets -- --ignored --nocapture
cargo test --locked --features viewer --lib
cargo clippy --locked --features viewer --all-targets -- -D warnings
```

`docs/evidence/mission-initialization.json` contains the initializer report. All proprietary executable/bytecode inputs remain under ignored `local/`.

## Animation investigation: correction and unresolved work

The previous bind-pose reconstruction test proves consistency of inverse-bind transforms but **cannot prove the bone-palette assignment**: any matching bone/inverse pair cancels in the bind pose. Earlier wording suggesting that this alone validated GX palette mapping was too strong and has been corrected.

A spatial inspection of original SVET groups supports the ascending-bit palette candidate: assigned hand, foot, thigh and torso groups have corresponding bind-space locations. That is supporting evidence, not animation parity. Ranked hypotheses remain palette assignment, missing translation, and quaternion sign/flag decoding. Exploratory executable scans did not establish an authoritative decompression routine; no speculative decoding change was promoted to the renderer. Existing imperfect animation previews remain labelled experimental.

## Remaining acceptance gate

A complete original mission still requires additional Lua semantics, verified native bindings, activation rules, real combat/AI/orders, vehicles, objectives, cinematics, audio and correct animation. At this milestone play launchers remained the temporary skirmish. The main launcher now runs the replacement original-asset encounter in PLAYABLE_BATTLE.md, not restored original mission gameplay. No publication or subagents.

Reference consulted for conditional instruction behavior:
https://www.lua.org/source/5.0/lvm.c.html (OP_EQ / OP_TEST), alongside the earlier Lua 5.0 opcode reference.
