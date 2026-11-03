# Compatibility roadmap

## Current execution priorities — authoritative over historical backlog wording

Start with `../HANDOFF.md` and `../where we are.md`. All 20 single-player campaign missions (`SP_0.1`, `SP_1.1..SP_5.4`) complete end-to-end through their authored Lua scripts to `MissionOutcome::Won`, all 16 multiplayer maps (`MP1..MP15`, `MP17`) load and run with a local Skirmish AI and win/loss conditions, and all 96 `Frontend2_Level` Lua scripts construct and step through the `PageStack`.

### 1. Finish original first-mission progression & full 20-mission campaign (complete)
- [x] Integrate the six initializers and all 97 original global/unit coroutines with transactional execution.
- [x] Complete original opening, movement/look/pause tutorials and three-barrel objective through real input.
- [x] Reach camp and complete the original bomber ambush through return of control; preserve original script/camera/objective calls.
- [x] Implement source-profile rifle, bomber and defensive anti-air missile behavior with disclosed replacement policies and tested damage/rollback.
- [x] Perform the staff-bearer ownership transfer under the original frozen black fade.
- [x] Resolve `BattleLeft02` route flags/branch semantics (`cWaypoint::GetNextWaypoint` and C-runtime `rand()`).
- [x] Complete the staff cutscene to real control return; verify camera framing and staff gameplay.
- [x] Reach actual squad recruitment/attack objectives and subsequent tutorials through input and original scripts.
- [x] Implement required heavy weapons, capture, player replacement, authored defeat/victory and complete-mission restart across all 20 single-player campaign missions (`SP_0.1`, `SP_1.1..SP_5.4`).
- [x] Verify end-to-end campaign mission completion (`MissionOutcome::Won`) across all 20 SP missions (`tests/campaign_levels_runtime.rs::autopilot_survey_campaign_levels`).

### 2. Fidelity and physical-device acceptance
- [x] Full 38-motion `cTroopAnimationSet` skeletal animation repertoire (`Idle`, `AlertIdle`, `ContextIdle`, `Walk`, `Run`, `RunStop`, `WalkBack`, `RunBack`, `StrafeLeft`, `StrafeRight`, `StrafeLeftRun`, `StrafeRightRun`, `TurnLeft`, `TurnRight`, `RollForward`, `RollBack`, `RollLeft`, `RollRight`, `JumpRise`, `JumpPose`, `JumpLand`, `Fire`, `ForwardFire`, `Reload`, `FallFlail`, `HitGround`, `Getup`, `Swim`, `SwimBack`, `TreadWater`, `WaterDeath`, `SteerFront`, `SteerLeft`, `SteerRight`, `DriveIdle`, `GunnerFire`, `Death`, `Death2..5`, `Victory` in `src/unit_animation.rs`), `meHoldWeaponInLeftHand` left/right hand weapon bone attachment (`BONE_L_HAND` / `BONE_R_HAND` in `src/original_scene.rs`), visible seated vehicle crew (`mPassenger` + `cSeatBase` `DisplayType` driver/gunner/passenger transforms & animations), fast matrix-palette normal+position skinning (`deform_positions_and_normals`), upper-body `pose_with_aim` spine pitch/yaw twist blending, per-wheel vehicle terrain raycasting & spring-damper suspension (`GroundVehiclePhysics` & `SuspensionState::step_with_physics` with main-gun `mFireImpulse` recoil), infantry explosion knockback impulses, and vehicle wheel/tread/rotor/turret articulation.
- [x] Cinematic framing, authored per-unit chase camera profiles (`UnitCameraProfile` `mCamPos`/`mCamLookPos`), camera transitions, 1-to-1 aircraft flight physics (`FlightProfile` continuous fixed-wing forward cruise, helicopter reverse, 3D pitch/bank, and interactive `Space` barrel-roll in `src/mission_air.rs` & `src/first_mission.rs`), naval wave bobbing (`WaterVehiclePhysics`) & submarine dive toggle (`Space`) with depth/oxygen HUD readout, 2-layer animated water shimmer, native `cTequilaEffectResource` (`FEQT`) particle decoder & live 3D `TequilaBillboard` emitter bursts with `ColourRange` keyframe tinting (`src/tequila.rs`, `src/bin/support/first_mission_view.rs`), live `cMorphingBuilding` intact/destroyed (`mpDeadModel`)/faction-swap mesh toggling (`MorphVariant`), and flying explosion debris fragments.
- [x] 3D distance-attenuated positional audio (weapons, explosions, `SpawnEffect`, capture-point flags, aircraft rotors, footsteps, CO radio voiceovers, and dynamic music ducking) plus mouse/keyboard, portable Xbox-style gamepad bindings, and interactive in-game Keybindings & Controller Remapping Settings UI (`FrontendStage::Keybindings` in `src/bin/support/first_mission_view.rs` & `src/controls.rs`, persisted to `config/controls.json`).
- [x] Performance and regression coverage without weakening malformed-data or unsupported-gameplay errors.

### 3. Original frontend, profiles and campaign flow (complete)
- [x] Complete `Frontend2_Level` runtime (`init_shared_scripts`, `construct_all_pages` across all 96 scripts, `PageStack` execution, `GetString`, `OpenSprite`/`CloseSprite`), 3-slot profile/save persistence (`local/saves/bw2_saves.json`), and cold-start route into all 20 campaign missions and 16 multiplayer maps.
- [x] Authentic Wii cold-start boot sequence (`BootLogos` wrist-strap/ESRB screens -> `IntroMovie` `BWii_Intro.thp` with skippable audio/video -> `Title` screen with 3D `Globe_sat` + `Globe_sat_clouds` + `Globe_atmos` over `bg_map03`, 12 orbiting faction unit icons, centered `bwii_logo`, `"PLEASE PRESS (A) & (B)"` prompt, and Wii Remote hand pointer `cursor_point` -> `SaveSelect` with 3 horizontal `SAVE A/B/C` cards and `"CHANGE NAME"` modal -> `NameEntry` Wii On-Screen Keyboard -> `MainMenu` 2x2 grid (`CAMPAIGN`, `NINTENDO WFC`, `OPTIONS`, `EXTRAS`) -> `CampaignSelect` 3D rotating Globe with 6 continent tabs and opening `.THP` campaign cutscenes `MissionCutsceneIn`).
- [x] Direct `cGUIPage` / `cGUIWidget` & `SpriteManager` screen-space canvas model & live on-screen Bevy UI rendering (`src/frontend_gui.rs` + `FrontendGuiCanvasQuad`/`FrontendGuiCanvasText` in `src/bin/support/first_mission_view.rs` resolving `cGUIPage`, `cGUICustomWidget`, `cGUIButtonWidget`, `cGUITextBoxWidget`, `cGUITextureWidget`, `cGUISpriteWidget`, `cGUIRectWidget`, `cGUIDialogBox0..3Widget`, `cGUI3DObjectWidget`, and `sSpriteBasetype` against live Lua `FromLabel`/`FromFunction`/`StartWidget`/`StopWidget` state).
- [x] Connect mission outcomes, grades (`S/A/B/C`), unlocks, persistence, 3D interactive `Globe_sat` + `Globe_sat_clouds` + `Globe_atmos`, CO briefing voiceovers, Unit Dossier, Concept Art Gallery, 15 `.THP` FMV movies, and Credits.

### 4. Expand native game coverage and portability
- [x] Implement and verify all 20 single-player missions (`SP_0.1`, `SP_1.1..SP_5.4`) and all 16 multiplayer maps (`MP1..MP15`, `MP17` Assault, Skirmish, and Co-Op modes with `IsNetworkHost`, `IsNetworkPlayer`, `SetNetworkMissionComplete`, `DeclareVictory`, real 2-player `PLAYER_TWO` roster/command/control, dual-viewport `Player2Camera3d` split-screen + full P2 move/aim/fire/unit-switch controls, and UDP `PlayerInputPacket` `NetPeer` sync in `src/net_sync.rs`).
- [ ] Validate native macOS, Linux and Windows execution, input/audio and saves on physical target hardware with William.
- [ ] Treat packaging and public distribution as separate milestones; publication requires William's authorization.

### 5. Later modding/editor ambitions
- [x] In-game Keybindings & Controller Remapping Settings UI (`FrontendStage::Keybindings`, accessible from `Options` and mid-mission via `F4` / Pause Menu, persisting all 24 actions to `config/controls.json`).
- [ ] Extend original/custom asset overrides, editable levels, and editor tooling after human playtest sign-off.

## Historical inventory and broader compatibility backlog

The sections below retain earlier investigations and long-term work. Old milestone counts and prototype results do not supersede the current execution priorities above. Fort Assault is not original-mission acceptance.

## A — Evidence and inventory
- [x] Identify local disc and partition layout.
- [x] Implement initial read-only WBFS inspection.
- [x] Hash source image.
- [ ] Verify complete partition integrity (accessed-cluster H0 checks implemented).
- [x] Inventory decrypted filesystem and executable.
- [x] Identify initial level/resource/script structures from actual bytes (full decoders pending).
- [ ] Create a mission dependency map and compatibility checklist.

## Modding / input objective (William's clarified goal)
- [x] First standard-gamepad input path (synthetic tests; physical hardware pending).
- [x] External keyboard/gamepad binding configuration (`config/controls.json`).
- [x] In-game remapping UI (`FrontendStage::Keybindings` for all 24 keyboard & gamepad actions).
- [x] Data-only prototype level manifests and native editable heightfield sandbox.
- [ ] Original models/animations and custom character import/override registry.
- [ ] Visual level editor and custom objectives/spawns.
- [ ] Physical controller hardware acceptance.

## B — Native content inspection
- [x] Pin Rust toolchain and Bevy version after checking current official docs.
- [x] Build initial tested original-format readers with bounds checks and synthetic fixtures (not all formats).
- [x] Decode sampled original model geometry/hierarchy and render original textured units/scenery in an internal preview.
- [x] Numerically validate sampled inverse-bind reconstruction; identify candidate GX bone palettes (bind pose alone does not validate their assignment).
- [x] Parse sampled original animation tables; experimental rotation-only deformation preview (not accepted fidelity).
- [x] Execute limited original script bootstrap roots with a fail-closed symbolic-ID host adapter.
- [ ] Render real terrain and a unit with verified orientation, scale, complete animation and material behavior. See ORIGINAL_RECOVERY.md.

## C — First complete mission, not final scope
- [x] Execute original art/global initializer bodies with explicit in-memory host capabilities.
- [x] Exercise the original scoring coroutine across resumable waits and controlled fixture transitions; scheduler policy remains provisional.
- [x] Bounded non-capturing Lua calls/closures, open calls/returns and suspended nested call stacks.
- [x] Original tower-win and staff-bearer-loss triggers reach cinematic gates from controlled world inputs; original scoring helper executes.
- [x] Typed health/invulnerability, player roster, zone queries and objective/freeze state; activation stays explicit.
- [x] Resolve 183 troops' authored idle/walk/run/fire/death bindings and test playback clocks; decoder fidelity remains unfinished.
- [x] Connect isolated original Stay_Here script to actual movement and original actor run/idle/death rendering in original scenery. Initial straight-line diagnostic; now extended with replacement infantry collision below. Full mission integration/original navigation parity remain unfinished; bounded replacement routing is recorded below. See SCRIPT_RENDER_INTEGRATION.md.
- [x] Reproduce/fix the diagnostic follow-camera obstruction using original rendered model geometry; regression-tested without relocating the actor. See CAMERA_OBSTRUCTION.md. Terrain/swept-volume and full camera parity remain pending.
- [x] Replacement infantry collision against rendered scene triangles and living active unit bodies; stop/slide, bounded substeps, native original-actor and pair verification. See UNIT_COLLISION.md. Original collision parity and moving vehicle bodies remain pending; bounded route planning follows below.
- [x] Bounded collision-checked infantry A*, cached-waypoint invalidation/replanning, explicit search failures, and native original-script home arrivals around blockers. See UNIT_NAVIGATION.md. Full-world navigation, crowd/formation behavior, and original navigation parity remain pending.
- [x] Integrate original scene/troops into a small playable encounter: direct collision-aware control, enemy pursuit/fire, squad hold/follow, lock-on combat, replacement capture objective, victory/defeat and restart. This battle is now available only through its separate Fort Assault prototype launcher; the primary launcher is a static original-scene preview. See PLAYABLE_BATTLE.md; this does not complete the original mission or establish original gameplay/animation fidelity.
- [ ] Complete remaining Lua/host compatibility and verify remaining gameplay contracts. Shared/captured closure support already exists; the old MISSION_GAMEPLAY.md milestone is not the latest implementation state.
- [ ] Reproduce player camera, controls, locomotion, aiming, damage, and switching.
- [ ] Implement unit orders, targeting, navigation, faction relationships, and vehicles.
- [ ] Implement verified mission scripting semantics, triggers, objectives, audio, and UI.
- [ ] Compare controlled reference scenarios against original execution.

## D — Full-game coverage
- [ ] Track every mission, unit, vehicle, weapon, cinematic, unlock, and save behavior.
- [ ] Assess multiplayer separately; offline campaign completion is not full-game parity.
- [ ] Complete keyboard/mouse and controller support.
- [ ] Test native builds on macOS, Linux, and Windows (not merely cross-compile).
- [ ] Performance, persistence, replay/regression testing, and packaging.

## Architecture direction

Existing boundaries include original-format parsing, stable object IDs, Lua 5.0 execution/shared closures, staged mutable simulation, immutable Arc-backed source data, and Bevy rendering/input. The playable rewrite is opt-in and preserves strict reference behavior separately. Extend these systems rather than replacing them with a new framework. Exact native physics/allocator implementation is not a prerequisite: bounded Rust replacement policies are allowed when named, tested and integrated, while unknown gameplay still fails closed.
