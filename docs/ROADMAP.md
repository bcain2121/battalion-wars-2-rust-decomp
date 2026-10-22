# Compatibility roadmap

## Current execution priorities — authoritative over historical backlog wording

Start with `../HANDOFF.md`, `../where we are.md` and `STAFF_TRANSFER.md`. The current checkpoint is 319 viewer tests / 189 overlapping library tests. No full original mission victory is accepted. Work remains native Rust/Bevy, solo, local and unpublished.

### 1. Finish original first-mission progression (active priority)
- [x] Integrate the six initializers and all 97 original global/unit coroutines with transactional execution.
- [x] Complete original opening, movement/look/pause tutorials and three-barrel objective through real input.
- [x] Reach camp and complete the original bomber ambush through return of control; preserve original script/camera/objective calls.
- [x] Implement source-profile rifle, bomber and defensive anti-air missile behavior with disclosed replacement policies and tested damage/rollback.
- [x] Perform the staff-bearer ownership transfer under the original frozen black fade (not completed staff recovery).
- [ ] Resolve `BattleLeft02` route flags/branch semantics: current `Gambit_Part_1_SolarBattle pc 93: FollowWaypoint` blocker.
- [ ] Complete the staff cutscene to real control return; verify camera framing and staff gameplay rather than accepting a held prefix.
- [ ] Reach actual squad recruitment/attack objectives and subsequent tutorials through input and original scripts.
- [ ] Implement required heavy weapons, capture, player replacement, authored defeat/victory and complete-mission restart.
- [ ] Record and review an end-to-end original first-mission run without progression overrides or skipped cinematics.

### 2. Fidelity and physical-device acceptance
- [ ] Full locomotion/firing/death animation and correct actor orientation/material presentation.
- [ ] Full cinematic framing, camera transitions, aircraft/projectile behavior and presentation where currently approximated.
- [ ] Actual hardware audio, mixing and controller verification with portable Xbox-style controls; no required Wii motion.
- [ ] Performance and regression coverage without weakening malformed-data or unsupported-gameplay errors.

Known cosmetic omissions may remain disclosed while phase 1 advances; defects that prevent genuine gameplay or usable camera control are not cosmetic waivers.

### 3. Original frontend, profiles and campaign flow
- [ ] Finish the partial frontend runtime, profile/save paths and cold-start route into the original mission.
- [ ] Preserve page/widget/script behavior; address remaining userdata/metatable/descriptor/cursor and sprite-animation contracts.
- [ ] Connect mission outcomes, unlocks, persistence and campaign progression. Frontend inventory/recovery is not complete menu acceptance.

### 4. Expand native game coverage and portability
- [ ] Track and implement remaining missions, units, vehicles, weapons and cinematics with per-slice acceptance.
- [ ] Validate native macOS, Linux and Windows execution, input/audio and saves on the actual platforms.
- [ ] Treat multiplayer, full-game parity, packaging and distribution as separate milestones; publication requires William's authorization.

### 5. Later modding/editor ambitions
- [ ] Extend original/custom asset overrides, editable levels, remapping UI and editor tooling after the playable campaign foundation.

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
- [x] External keyboard/gamepad binding configuration.
- [x] Data-only prototype level manifests and native editable heightfield sandbox.
- [ ] Original models/animations and custom character import/override registry.
- [ ] Visual level editor and custom objectives/spawns.
- [ ] In-game remapping UI and controller hardware acceptance.

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
