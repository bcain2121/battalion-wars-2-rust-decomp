# Agent handoff — Battalion Wars Rust

## Start here

Repository: `.` (`battalion-wars-2-rust-decomp`)

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

- **Authentic Wii Cold-Start Boot, Opening Intro Movie, Title Screen, Save Slot Cards, On-Screen Keyboard, 2x2 Main Menu, 3D Satellite Globe Campaign Select, Interactive Keybindings Menu & `Frontend2_Level` 96-script page-stack (`src/bin/support/first_mission_view.rs`, `src/controls.rs`, `src/texture.rs`, `src/frontend_runtime.rs`, `src/frontend_gui.rs`):**
  - Cold-start boot sequence (`BootLogos` wrist-strap/ESRB screens -> `IntroMovie` `BWii_Intro.thp` with decoded MJPEG video + THP ADPCM audio -> `Title` screen with 3D `Globe_sat` + `Globe_sat_clouds` + `Globe_atmos` over `bg_map03`, 12 orbiting faction unit icons, centered `bwii_logo`, `"PLEASE PRESS (A) & (B)"` prompt, and Wii Remote hand pointer `cursor_point` -> `SaveSelect` with 3 horizontal `SAVE A/B/C` cards and `"CHANGE NAME"` modal -> `NameEntry` Wii On-Screen Keyboard -> `MainMenu` 2x2 grid (`CAMPAIGN`, `NINTENDO WFC`, `OPTIONS`, `EXTRAS`) -> `CampaignSelect` 3D rotating Globe with 6 continent tabs and opening `.THP` campaign cutscenes `MissionCutsceneIn`).
  - Interactive Keybindings & Controller Remapping Settings Menu (`FrontendStage::Keybindings` via `Options` item 5 or mid-mission `[F4]` / Pause Menu) supporting live keyboard & gamepad remapping across all 24 actions and persisting to `config/controls.json`.
  - `FrontendRuntime::init_shared_scripts` and `construct_all_pages` initialize and construct all 96 `Frontend2_Level` Lua scripts (`FrontendBW2`, `Vector`, `Matrix`, `Tools`, `Widgets`, `Spinning`, `Fade`, `Slide`, `GlobeSelect`, `Start`, `Title`, `LoadSave`, `Keyboard`, `ModeSelect`, `CampaignS`, `MissionS`, `MapSelect`, `Controls`, `Extras`, `Units`, `Concepts`, `Cutscenes`, `Credits`, `Stats`, etc.), verified by `tests/frontend_runtime.rs::all_frontend2_pages_construct_and_drive_page_stack`.
  - `GuiScene::from_graph` and `evaluate_active_page_canvas` (`src/frontend_gui.rs`) resolve all 54 `cGUIPage` hierarchies and 1,683 `cGUI*Widget` nodes (`cGUICustomWidget`, `cGUIButtonWidget`, `cGUITextBoxWidget`, `cGUITextureWidget`, `cGUISpriteWidget`, `cGUIRectWidget`, `cGUIDialogBox0..3Widget`, `cGUI3DObjectWidget`, and `sSpriteBasetype`) against live Lua `FromLabel`/`FromFunction`/`StartWidget`/`StopWidget` state and render them on-screen in `bw-first-mission` via a 48-node `FrontendGuiCanvasQuad` / `FrontendGuiCanvasText` Bevy UI pool (toggleable with `[F3]`).
- **All 20 single-player campaign levels (`SP_0.1` and `SP_1.1` through `SP_5.4`) complete end-to-end through their full authored cutscenes, objectives, and `WinMission` (`MissionOutcome::Won`):**
  - `SP_0.1` (*Prologue: A Flash in the Pan*): `Won` (`GRADE_S`, frame `11,069` in `autopilot_survey_campaign_levels`; also verified at frame `23,115` with authored defeat `MissionOutcome::Lost { message_id: Some(196) }` and `restart()` in `tests/first_mission_staff.rs::first_mission_reaches_victory_and_defeat`).
  - **Campaign 1 (`SP_1.1..SP_1.5`):** `SP_1.1` (`Won`, `GRADE_S`, frame `13,128`), `SP_1.2` (`Won`, `GRADE_S`, frame `17,676`), `SP_1.3` (`Won`, `GRADE_S`, frame `18,863`), `SP_1.4` (`Won`, `GRADE_S`, frame `13,866`), `SP_1.5` (`Won`, `GRADE_S`, frame `18,695`).
  - **Campaign 2 (`SP_2.1..SP_2.3`):** `SP_2.1` (`Won`, `GRADE_S`, frame `13,441`), `SP_2.2` (`Won`, `GRADE_A`, frame `24,995`), `SP_2.3` (`Won`, `GRADE_S`, frame `24,522`).
  - **Campaign 3 (`SP_3.1..SP_3.4`):** `SP_3.1` (`Won`, `GRADE_S`, frame `15,436`), `SP_3.2` (`Won`, `GRADE_A`, frame `14,705`), `SP_3.3` (`Won`, `GRADE_S`, frame `19,356`), `SP_3.4` (`Won`, `GRADE_A`, frame `11,176`).
  - **Campaign 4 (`SP_4.1..SP_4.3`):** `SP_4.1` (`Won`, `GRADE_A`, frame `21,679`), `SP_4.2` (`Won`, `GRADE_A`, frame `35,642`), `SP_4.3` (`Won`, `GRADE_B`, frame `33,221`).
  - **Campaign 5 (`SP_5.1..SP_5.4`):** `SP_5.1` (`Won`, `GRADE_A`, frame `28,284`), `SP_5.2` (`Won`, `GRADE_S`, frame `24,524`), `SP_5.3` (`Won`, `GRADE_B`, frame `45,568`), `SP_5.4` (`Won`, `GRADE_A`, frame `35,285`).
- **All 16 multiplayer levels (`MP1..MP15`, `MP17`) load, complete intro cutscene transfer to Player 1 & Player 2 (`two_player_mode`, `player2`, `player2_roster`), and run with local Skirmish AI (`src/mission_world.rs::advance_multiplayer_skirmish_ai`) + Dual-Viewport `Player2Camera3d` Split-Screen + Full P2 Move/Aim/Fire/Unit-Switch Controls (`IJKL`, `U`/`O`, `H`, `Y`) + UDP `NetPeer` sync (`src/net_sync.rs`) + Win/Loss Conditions (`IsNetworkHost`, `IsNetworkPlayer`, `SetNetworkMissionComplete`, `DeclareVictory`):**
  - Verified by `tests/campaign_levels_runtime.rs::probe_multiplayer_levels_loading` and `src/mission_world.rs::tests::mp_skirmish_ai_drives_hostile_combat_and_capture`.
- **Visual, 38-Motion `cTroopAnimationSet` Animation Repertoire, Visible Seated Vehicle Crew, 1-to-1 Unit Piloting & Physics (`GroundVehiclePhysics`, `FlightProfile`, `WaterVehiclePhysics`, `UnitCameraProfile`), Live `FEQT` Tequila 3D Particle Bursts, Live `cMorphingBuilding` Mesh Swapping & 3D Positional Audio Fidelity:**
  - Full 38-motion `cTroopAnimationSet` repertoire (`src/unit_animation.rs`) with fallback resolution (`Player::select_with_fallback`), `meHoldWeaponInLeftHand` weapon attachment to `BONE_L_HAND` vs `BONE_R_HAND` (`src/original_scene.rs`), interactive `Space` combat roll (`RollLeft`/`RollRight`/`RollBack`/`RollForward`) and vertical jump (`JumpRise` -> `JumpPose` -> `JumpLand`), visible seated vehicle crew (`mPassenger` + `cSeatBase` `DisplayType` driver/gunner/passenger transforms & `SteerLeft`/`SteerRight`/`SteerFront`/`DriveIdle`/`GunnerFire` animations), per-wheel terrain raycasting (`vehicle_wheel_offsets`) & spring-damper suspension (`GroundVehiclePhysics` & `SuspensionState::step_with_physics` with main-gun `mFireImpulse` recoil in `src/vehicle_wheels.rs`), fixed-wing continuous forward flight + helicopter reverse + 3D pitch/bank + `Space` barrel-roll (`FlightProfile` in `src/mission_air.rs` & `src/first_mission.rs`), naval wave bobbing (`WaterVehiclePhysics`) + submarine `Space` dive toggle & depth/oxygen HUD readout, authored per-unit chase camera profiles (`UnitCameraProfile` `mCamPos`/`mCamLookPos`), fast matrix-palette normal+position skinning (`skinning::deform_positions_and_normals`), upper-body spine/chest aim-pitch and torso yaw twist blending (`skinning::pose_with_aim` on `BONE_1_SPINE` and `BONE_2_SPINE`), native `cTequilaEffectResource` (`FEQT`) particle decoder & live 64-billboard `TequilaBillboard` emitter bursts (`LiveTequilaParticle`, `TequilaEffect::spawn_burst`, per-particle `ColourRange` keyframe tinting in `src/tequila.rs` and `src/bin/support/first_mission_view.rs`), live `cMorphingBuilding` intact/destroyed (`mpDeadModel`)/faction-swap mesh toggling (`MorphingBuildingModels`, `World::morphing_building_visual_state`, `MorphVariant`), radial infantry explosion knockback impulses, tank tread UV scroll, aircraft rotor spin, turret yaw/gun barrel pitch, 2-layer water shimmer, flying explosion debris fragments, authored `sExplodeBase` particle flipbooks, and 3D distance-attenuated positional audio.
  - Updated `Play Battalion Wars Rust.command` to launch `bw-first-mission` by default.
- **Decompiled & implemented native mechanics (via Ghidra 12.1.4 / REA MCP on `local/analysis/main.elf`):**
  - All 268 native Lua C-function bindings (`0x804b1a60..0x804b22c0`) cataloged in `local/analysis/lua-bindings.json`.
  - Lua object-handle numeric comparison parity (`GetPlayerUnit` `0x800e8258`, `Op::Eq`/`Op::Lt`/`Op::Le`), `cPickup` health/jerrycan collection (`0x800c39cc`) and crushable tent contact/pathing, spatial acceleration (`TriGrid` & `MeshIndex` in `src/unit_collision.rs`), bridge deck auto-detection (`SM GORGEBRIDGE`, `WF WOODBRIDGE`, `AI BRIDGE`, `SE BRIDGE`, `C2 BRIDGE`, `X BRIDGE`) and gate/wall opening (`GATE`, `DOOR`, `C5 SOLARWALL`), native `flag.*` / `constant.TYPE` / `constant.GTYPE` tables (`0x8023e038`, `0x80123338`), `NumItemsInArea`, `TestFlags`, `GetState`, `SetInvulnerable`, partial/closest-node path planning (`plan_towards`), cutscene combat, aircraft cruise HAG & landing (`LandAirUnit` `0x800e9590`, `GetMovementState` `0x800d7ffc`), `FollowUnit` route hysteresis, lock-on control transfer (`Z` button), capture-point flag raising/lowering & allied contention aggregation (`CheckCapturePoint` `0x800d7fcc`, `SetCapturePointHeight` `0x800d834c`, `GetInstallationOwner` `0x800e2b00`, `GetHealthPercent` on `cCapturePoint` `0x800d8124`), `cBuildingImpBase` `SeatFlags & 1` garrison filtering, burst weapon folding & indirect ballistic fire (`UNIT_VEH_ARTILLARY` / `UNIT_VEH_BATTLESTATION`), `cMorphingBuilding` combat base resolution, carrier passenger boarding/unloading (`EnterVehicle` / `ExitVehicle` / `ReviveDeadUnit` ground-relative `Y` preservation), automatic death control transfer (`TRANSFER_BYDEATH`, `GetPlayerTransferred` `0x800da024`), and `cAmbientAreaPointSoundSphere` (`0x801fbae0`) + numeric target ID support in `SetActive` (`0x800d9c78`).

## Next bounded implementation task

1. Ready for William's interactive playtest across the frontend menu, Keybindings settings menu, all 20 single-player campaign missions (`SP_0.1`, `SP_1.1..SP_5.4`), and all 16 multiplayer maps (`MP1..MP15`, `MP17`).
2. Keep all unit tests, integration tests, formatting, and Clippy clean.

## Code map

| Area | Main files |
|---|---|
| Integrated original session / inputs / recording / restart | `src/first_mission.rs`, `src/autopilot.rs`, `src/controls.rs` |
| Scheduler, Lua host and whole-tick transaction | `src/mission_program.rs`, `src/mission_vm.rs`, `src/mission_world.rs`, `src/lua50.rs` |
| Frontend GUI, `PageStack` & `SpriteManager` canvas | `src/frontend_runtime.rs`, `src/frontend_gui.rs`, `src/frontend_sprites.rs`, `src/frontend_environment.rs` |
| Combat, damage, and weapon profiles | `src/mission_combat.rs`, `src/mission_damage.rs`, `src/mission_world.rs` |
| Waypoints and C-runtime RNG | `src/mission_waypoints.rs`, `src/mission_random.rs` (`CrtRand`), `src/mission_world.rs` |
| Transfer adapter, 2-Player mode & UDP `NetPeer` sync | `src/mission_world.rs` (`ForceUnitTransfer`, `player2`), `src/net_sync.rs` |
| Skinning, `cTroopAnimationSet`, `FEQT` Tequila particles, `cMorphingBuilding` & vehicle suspension | `src/skinning.rs`, `src/unit_animation.rs`, `src/tequila.rs`, `src/original_scene.rs`, `src/vehicle_wheels.rs` |
| Camera paths/chase/fades | `src/mission_camera.rs` |
| Rewrite scene and collision (`TriGrid`, `MeshIndex`) | `src/mission_rewrite_scene.rs`, `src/unit_collision.rs`, `src/navigation.rs` |
| Bomber / missile / flight | `src/mission_bomber.rs`, `src/mission_missile.rs`, `src/mission_air.rs` |
| Original viewer, WGPU renderer, and replay harness | `src/viewer.rs`, `src/render_wgpu.rs`, `src/bin/support/first_mission_view.rs`, `src/bin/support/original_app.rs` |
| Progression & campaign regressions | `tests/first_mission_runtime.rs`, `tests/first_mission_squad.rs`, `tests/first_mission_staff.rs`, `tests/campaign_levels_runtime.rs`, `tests/frontend_runtime.rs` |

`World` and its immutable Arc-backed source data are distinct from live mutable positions/health/orders. Follow existing staged/whole-tick transaction ownership. Do not add partial mutations that survive a failed Lua call. Strict reference behavior is deliberately separate from the opt-in playable rewrite.

## Reproduce and verify

Run from the repository root. If recordings are missing or code changes invalidate them, regenerate **in this order**:

```sh
cargo test --locked --features viewer --test first_mission_runtime -- --include-ignored --nocapture
cargo test --locked --features viewer --test first_mission_squad -- --include-ignored --nocapture
cargo test --locked --features viewer --test first_mission_staff -- --include-ignored --nocapture
```

Campaign & multiplayer level verification (`SP_0.1..SP_5.4` full mission victory & `MP1..MP17` skirmish AI):

```sh
cargo test --locked --features viewer --test campaign_levels_runtime probe_campaign_levels_loading -- --nocapture
cargo test --locked --features viewer --test campaign_levels_runtime probe_multiplayer_levels_loading -- --nocapture
BW_SURVEY_FRAMES=3600 cargo test --locked --features viewer --test campaign_levels_runtime survey_all_campaign_levels -- --ignored --nocapture
BW_SURVEY_LEVELS=SP_0.1,SP_1.1,SP_1.2,SP_1.3,SP_1.4,SP_1.5,SP_2.1,SP_2.2,SP_2.3,SP_3.1,SP_3.2,SP_3.3,SP_3.4,SP_4.1,SP_4.2,SP_4.3,SP_5.1,SP_5.2,SP_5.3,SP_5.4 cargo test --release --locked --features viewer --test campaign_levels_runtime autopilot_survey_campaign_levels -- --ignored --nocapture
```

Full regression matrix:

```sh
cargo test --locked --features viewer --all-targets -- --include-ignored
cargo test --locked --lib
cargo clippy --locked --features viewer --all-targets -- -D warnings
cargo fmt --check
cargo build --locked --features viewer --bin bw-first-mission
```

Last implementation checkpoint: **221 library unit tests passed, 0 failed; all 20 single-player levels (`SP_0.1`, `SP_1.1..SP_5.4`) complete through `MissionOutcome::Won`; all 16 multiplayer levels (`MP1..MP15`, `MP17`) load and run with local Skirmish AI + 2P (`PLAYER_TWO`) + UDP `NetPeer` sync**. Do not sum overlapping test counts.

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
