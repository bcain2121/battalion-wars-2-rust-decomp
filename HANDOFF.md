# Agent handoff — Battalion Wars Rust

## Start here

Repository: `/Users/socialmedia/Apps/Battalion Wars Rust`

William Cain wants a **native Rust/Bevy reconstruction of Battalion Wars 2**, using the original assets, scripts, objectives and cinematics. Work directly in this existing project; do not switch to Godot or build a substitute mission. The approved priority is **finish the original first mission**, with explicit cosmetic omissions allowed, before frontend/campaign expansion and platform acceptance.

Read in this order:
1. This file: operational constraints and next task.
2. `where we are.md`: latest section is authoritative; subsequent checkpoints are historical.
3. `docs/STAFF_TRANSFER.md`: current implementation, evidence and blocker.
4. `docs/AMBUSH_COMPLETION.md`, `docs/FIRST_COMBAT.md`: accepted original progression and its limits.
5. `docs/PLAYABLE_FIRST.md`, `docs/ROADMAP.md`: policy and future priorities.

## Critical checkout / safety facts

- Branch is `main`, **with no commits yet**. The project is largely untracked. An empty `git diff` is NOT evidence that nothing changed. Do not reset, clean, delete untracked files, replace this checkout, or assume Git can recover it.
- Preserve existing assets, original media, research, recordings, launchers and user changes. No commits/pushes/publication/uploads without permission.
- Work solo: William's current constraint is **no subagents**. Do not inherit an unrelated worker/team.
- Private original assets, native disassembly, captures, logs and local tools stay in ignored `local/`. Do not copy them into distributable docs or source packages.
- Do not stop or adopt user-owned previews/processes. Use owned offscreen processes for checks. No current user-preview PID has been verified; historical PIDs are not authority.
- Do not change the primary launchers to point to a different experience. The first-mission launcher is separate from the static preview and Fort Assault.
- No forced progress globals, shortcut player teleports, skipped scenes, fabricated victory, or successful no-op gameplay APIs. Implement reusable engine behavior rather than rewriting scripted events.
- Unsupported gameplay/signatures and malformed data must remain errors with transactional rollback. Classify only known presentation omissions as cosmetic, require opt-in and report them.
- Keep the pinned Rust/Bevy setup. Read `rust-toolchain.toml`, `Cargo.toml`, `Cargo.lock` rather than upgrading dependencies as a side task.

## Current state, in one minute

- **Both `SP_0.1` (*Prologue: A Flash in the Pan*) and `SP_1.1` (*1. Anglo Attack* / *Flashback: A Prelude to War*) complete end-to-end through their full authored cutscenes, objectives, and `WinMission` (`MissionOutcome::Won`):**
  - `SP_0.1` completes through `Gambit_Part_1_Cutscene_PlayerWins` and `WinMission` at frame `23,115` (`tests/first_mission_staff.rs::first_mission_reaches_victory_and_defeat`), with authored defeat (`MissionOutcome::Lost { message_id: Some(196) }`) and `restart()` verified.
  - `SP_1.1` completes all 9 authored mission objectives (`260025754`, `280009207`, `280009344`, `280009356`, `280010295`, `280014581`, `260027438`, `280010544`, `280009570` `Defend_Airbase`), all 5 cutscenes (`JW_Cutscene01..05`), and `WinMission` (`MissionOutcome::Won { speed: 72.5, power: 83.03693570451436, technique: 100.0, rating: "GRADE_A" }`) at frame `34,013` (`566.9s`) via ordinary `Input` in `tests/campaign_levels_runtime.rs::autopilot_survey_campaign_levels`.
- **All 20 single-player campaign levels (`SP_0.1` through `SP_5.4`) load and run 3,600 frames (`60s`) past their opening cutscenes with zero script errors** (`tests/campaign_levels_runtime.rs::survey_all_campaign_levels`).
- **Decompiled & implemented native mechanics (via Ghidra 12.1.4 / REA MCP on `local/analysis/main.elf`):**
  - All 268 native Lua C-function bindings (`0x804b1a60..0x804b22c0`) cataloged in `local/analysis/lua-bindings.json`.
  - Lua object-handle numeric comparison parity (`GetPlayerUnit` `0x800e8258`, `Op::Eq`/`Op::Lt`/`Op::Le`), `cPickup` health/jerrycan collection (`0x800c39cc`), spatial acceleration (`TriGrid` & `MeshIndex` in `src/unit_collision.rs`), native `flag.*` / `constant.TYPE` / `constant.GTYPE` tables (`0x8023e038`, `0x80123338`), `NumItemsInArea`, `TestFlags`, `GetState`, `SetInvulnerable`, partial/closest-node path planning (`plan_towards`), cutscene combat, aircraft cruise HAG & landing (`LandAirUnit` `0x800e9590`, `GetMovementState` `0x800d7ffc`), `FollowUnit` route hysteresis, lock-on control transfer (`Z` button), capture-point flag raising/lowering & squad dispatch (`CheckCapturePoint` `0x800d7fcc`, `SetCapturePointHeight` `0x800d834c`, `GetInstallationOwner` `0x800e2b00`, `GetHealthPercent` on `cCapturePoint` `0x800d8124`), `cMorphingBuilding` combat base resolution, carrier passenger boarding/unloading (`EnterVehicle` / `ExitVehicle` / `ReviveDeadUnit` ground-relative `Y` preservation), automatic death control transfer (`TRANSFER_BYDEATH`, `GetPlayerTransferred` `0x800da024`), and `cAmbientAreaPointSoundSphere` (`0x801fbae0`) + numeric target ID support in `SetActive` (`0x800d9c78`).

## Next bounded implementation task

1. Continue driving `SP_1.2..SP_1.5` (and subsequent campaign levels `SP_2.1..SP_5.4`) with `autopilot_survey_campaign_levels` (`BW_SURVEY_LEVELS=SP_1.2,... BW_SURVEY_FRAMES=36000 cargo test --locked --features viewer --test campaign_levels_runtime autopilot_survey_campaign_levels -- --ignored --nocapture`) to verify full end-to-end mission completion across the remaining campaign missions.
2. Decompile via REA (`local/analysis/main.elf`) and implement any mission-specific mechanics encountered in `SP_1.2..SP_5.4` (such as naval unit beaching / water movement in `SP_1.2`/`SP_1.3` if required by later objectives) while preserving whole-tick transactional rollback.
3. Keep all unit tests, integration tests, formatting, and Clippy clean.

## Code map

| Area | Main files |
|---|---|
| Integrated original session / inputs / recording / restart | `src/first_mission.rs`, `src/autopilot.rs` |
| Scheduler, Lua host and whole-tick transaction | `src/mission_program.rs`, `src/mission_vm.rs`, `src/mission_world.rs`, `src/lua50.rs` |
| Combat, damage, and weapon profiles | `src/mission_combat.rs`, `src/mission_damage.rs`, `src/mission_world.rs` |
| Waypoints and C-runtime RNG | `src/mission_waypoints.rs`, `src/mission_random.rs` (`CrtRand`), `src/mission_world.rs` |
| Transfer adapter and presentation authorization | `src/mission_world.rs` (`ForceUnitTransfer`), `src/mission_rewrite.rs` (`allow_faded_transfer_presentation`) |
| Camera paths/chase/fades | `src/mission_camera.rs` |
| Rewrite scene and collision (`TriGrid`, `MeshIndex`) | `src/mission_rewrite_scene.rs`, `src/unit_collision.rs`, `src/navigation.rs` |
| Bomber / missile / flight | `src/mission_bomber.rs`, `src/mission_missile.rs`, `src/mission_air.rs` |
| Original viewer and replay harness | `src/bin/support/first_mission_view.rs`, `src/bin/support/original_app.rs` |
| Progression & campaign regressions | `tests/first_mission_runtime.rs`, `tests/first_mission_squad.rs`, `tests/first_mission_staff.rs`, `tests/campaign_levels_runtime.rs` |

`World` and its immutable Arc-backed source data are distinct from live mutable positions/health/orders. Follow existing staged/whole-tick transaction ownership. Do not add partial mutations that survive a failed Lua call. Strict reference behavior is deliberately separate from the opt-in playable rewrite.

## Reproduce and verify

Run from the repository root. If recordings are missing or code changes invalidate them, regenerate **in this order**:

```sh
cargo test --locked --features viewer --test first_mission_runtime -- --include-ignored --nocapture
cargo test --locked --features viewer --test first_mission_squad -- --include-ignored --nocapture
cargo test --locked --features viewer --test first_mission_staff -- --include-ignored --nocapture
```

Campaign level verification & `SP_1.1` full mission victory:

```sh
cargo test --locked --features viewer --test campaign_levels_runtime probe_campaign_levels_loading -- --nocapture
BW_SURVEY_FRAMES=3600 cargo test --locked --features viewer --test campaign_levels_runtime survey_all_campaign_levels -- --ignored --nocapture
BW_SURVEY_LEVELS=SP_1.1 BW_SURVEY_FRAMES=36000 cargo test --locked --features viewer --test campaign_levels_runtime autopilot_survey_campaign_levels -- --ignored --nocapture
```

Full regression matrix:

```sh
cargo test --locked --features viewer --all-targets -- --include-ignored
cargo test --locked --lib
cargo clippy --locked --features viewer --all-targets -- -D warnings
cargo fmt --check
cargo build --locked --features viewer --bin bw-first-mission
```

Last implementation checkpoint: **201 library unit tests passed, 0 failed; all 20 single-player levels (`SP_0.1..SP_5.4`) pass 3,600-frame startup survey; `SP_0.1` and `SP_1.1` complete through `MissionOutcome::Won`**. Do not sum overlapping test counts.

Latest logs: `local/analysis/staff-complete-{full-tests,core-tests,clippy,build,render}.log`. Machine-readable evidence: `docs/evidence/staff-transfer-validation.json`. Previous accepted ambush evidence: `docs/evidence/ambush-completion-validation.json`.

Accepted staff-cutscene full-tail GPU replay:

```sh
BW_OFFSCREEN=1 BW_SMOKE=1 \
BW_MISSION_REPLAY="$PWD/local/analysis/staff-complete-inputs.json" \
BW_MISSION_REPLAY_TAIL=1300 BW_MISSION_EXPECT_STAFF=1 \
BW_CAPTURE="$PWD/local/analysis/staff-complete.png" BW_CAPTURE_FRAME=1340 \
BW_CAPTURE_FRAMES=320,550,950,1280 \
target/debug/bw-first-mission
```

For the accepted complete ambush replay, use the command in `docs/AMBUSH_COMPLETION.md`: tail 1800, `BW_MISSION_EXPECT_AMBUSH=1`, no HOLD. Keep full-scene and prefix evidence separate.

Interactive build: `Preview First Mission Runtime.command`, or `cargo run --locked --features viewer --bin bw-first-mission`. Controls: WASD/left stick move; arrows/right stick look; LMB/RT fire; RMB/Tab/LT assist; E/X recruit; Enter/A tutorial acknowledgment; R restart; Escape close. Hardware controller/audio acceptance remains pending.

## Private data / reference work

- Original level data: `local/extracted/files/Data/CompoundFiles/SP_0.1_Level.xml`, `SP_0.1_Level_preload.xml`, `SP_0.1_Level.res`, `SP_0.1.out`.
- Decoded scripts: `local/analysis/SP_0.1-decoded.json`. Relevant prototype is typically the named script's `children[0]`; instructions use `code`, lowercase `bx`, and `constant`. This decoder's RK constants use index `c - 250` for constant operands; inspect its format before applying Lua-version assumptions.
- `Value` is imported from `bw_rust::lua50::Value`, not the private VM import.
- Mission objectives store `(visible, complete)`; markers use `Marker::position(&Host)`, not a nonexistent `mission_guidance::visible` API.
- Terrain baseline queries must account for grounded versus world-space Y. `Scene::world_point` adds terrain height to supplied Y; do not double-ground aircraft or impacts.
- Read-only native reference tooling: `.venv/bin/python local/analysis/disasm_range.py ADDRESS SIZE`; DOL is `local/extracted/sys/main.dol`. r2=`0x80608ec0`, r13=`0x80601160`. **Capstone misdecodes paired-single PPC as VSX-like instructions; inspect raw words rather than trusting those mnemonics.**
- Dolphin was authorized for isolated read-only reference work, not interference with user processes or substitution for the Rust game.

## Deferred goals and reporting discipline

Use the prioritized plan in `docs/ROADMAP.md`. Full first-mission completion comes before frontend/profiles/campaign and cross-platform work. Native animation/material/camera/aircraft fidelity, hardware sound/controller tests and Windows/Linux runs remain unaccepted. Frontend recovery is a separate partial subsystem; see `docs/FRONTEND_RECOVERY.md`, `docs/FRONTEND_PAGE_STACK.md`, `docs/FRONTEND_ARRAYS.md`.

Never equate software PCM with hardware audio, moving bind-pose models with complete animation, isolated damage arenas with original mission completion, held prefixes with completed cinematics, or Fort Assault victories with the original mission. No reliable completion percentage/date has been established.

Pi skills, if using this environment: `ai-game-studio` for ownership/safety (this existing project stays Rust, not Godot); `mattpocock-diagnose` for regressions; `context7-check`/`context7-mcp` if changing library APIs. Do not invoke A2A/subagents under the current constraints. No root `AGENTS.md` was present at handoff preparation; check for newly supplied instructions before editing.
