# Research log

## 2026-10-03 — Infantry waypoint orders

`UNIT_WAYPOINT_RUNTIME.md` records authored local-offset paths feeding shared movement/collision/A*. A wall blocks progression without skipping nodes; planned navigation detours and continues to subsequent nodes. The original Sstrikebazooka01/WP01 four-node path with 15/15 offsets completes in an explicitly isolated, empty-geometry fixture—not the original terrain or a full mission. **159 tests**, Clippy, locked builds and battle restart regression pass. Full-scene navigation and vehicle profiles remain blockers. Vehicle instances specify ODE, but old/new physics-profile selection and collision hulls still need verification; inertia-box metadata was not repurposed as a collider. Work remains local and uncommitted.

## 2026-10-03 — Intro music and authored control state

See `INTRO_RUNTIME.md` and `evidence/intro-runtime-validation.json`: **141 tests pass**, Clippy/builds pass, and the replacement battle still passes victory/restart/defeat. The original StartIntro helper now produces decoded stereo PCM; pauses and failed-tick rollback preserve audio state. Authored player camera 550030276 targets Sstaffbearer03 (550030791), so the first original transfer is redundant—not evidence of switching. A separately labelled altered-target fixture tests real infantry transfer and camera-cut state. Configured isolated probes now reach SetCamera at intro instruction 80. No hardware audio, cinematic rendering, full squad ownership, restored animation fidelity or complete original mission is claimed.

## 2026-10-03 — Original startup foundations, not mission completion

See `MISSION_STARTUP.md` and `evidence/mission-startup-validation.json`. Recovered the 6/40/57 authored initializer/global/unit bindings; all six initializers now execute with shared captured closures (26 closures, 3 cells). Added explicit presence/activity separation, damageable scenery, bounded single-argument lifecycle support, all 151 localized messages and decoding of 57 original voice assets. STR duration data contains 146 zeros and five -1 values, not usable audio durations; the first voice's decoded sample count establishes 8.743 seconds. Unsupported APIs remain named errors, with nested helper call paths visible.

Validation: **133 passing tests**, warning-denying Clippy, formatting and native builds; existing Fort Assault victory → restart → defeat still passes with prior deterministic results. No original startup rendering, complete gameplay, animation-fidelity restoration or original end-to-end playthrough is claimed. The launcher remains the replacement battle. Work is uncommitted, local media ignored, and no publication or subagents were used.

## 2026-10-02 — Initial read-only reconnaissance

### Verified locally

- User-provided media: `Battalion Wars 2 (USA)/Battalion Wars 2 (USA).wbfs`.
- WBFS file size: 2,787,115,008 bytes.
- Disc ID: `RBWE01`; embedded title: `Battalion Wars 2`.
- Wii header magic: `0x5d1c9ea3`.
- Host sector size: 512 bytes; WBFS block size: 2,097,152 bytes.
- 1,328 mapped blocks. One populated disc slot, slot zero.
- Partition table lists one partition, type 0 (game data), at logical disc offset 260,046,848.
- Partition data starts 131,072 bytes after its header. Declared data span: 4,424,597,504 bytes. This is a logical span, not the physical WBFS file size.
- TMD size 520 bytes; certificate chain size 2,560 bytes.
- `tools/inspect_disc.py` successfully reads the WBFS mapping and partition metadata without expanding the image or modifying it.
- Cargo is available. `wit` and `dolphin-tool` were not on PATH; no Dolphin-named app was found directly in /Applications. This does not establish absence everywhere.
- System Python has neither `cryptography` nor `Crypto` available.

### Not yet established at initial checkpoint (superseded below)

- Executable size, symbols/debug strings, exact engine structure, scripting version, asset counts/formats, mission list, and content integrity.
- Full partition hash verification has NOT been performed.
- No decrypted filesystem inventory, executable disassembly, or Rust/Bevy runtime yet.
- Community search results suggest XML levels and Lua scripts, but this has NOT been verified against this image.

### Next investigation

1. Establish a reproducible local partition/filesystem reader using documented disc formats or a maintained extraction tool; keep keys and outputs local.
2. Inventory filesystem paths, sizes, signatures, and content hashes. Avoid bulk extraction before understanding space needs.
3. Extract main.dol locally; map its sections, entry point, strings, and potential script registration tables.
4. Identify one mission's dependency graph and quantify the engine-to-script interface.
5. Only then pin Rust/Bevy dependencies and implement the first asset viewer/import path.

## 2026-10-02 — Filesystem and mission data unlocked

- Added partition decryption, accessed-cluster H0 checks, FST inventory, DOL section mapping, and selective non-overwriting extraction in `tools/inventory_filesystem.py`.
- Added mission XML/resource analysis in `tools/analyze_level.py`.
- Verified 7,257 filesystem entries; extracted main.dol and one mission's selected data locally.
- Recorded details in `FILESYSTEM_FINDINGS.md` and machine-readable evidence files.
- Seven synthetic parser tests pass with `.venv/bin/python -m unittest discover -s tests -v`.
- Source image hash is recorded in `docs/evidence/source-image.sha256`.
- Tooling detour: brew has no wit formula; official Dolphin 2609 macOS app lacks DolphinTool. Download was verified, mounted read-only, copied only under local/tools, and detached. No system application installation.
- Research Python dependencies pinned in requirements-research.txt; installed only in project .venv.
- All work remains uncommitted. No remote, publication, subagents, or running background tasks.
- Next: bounded Lua 5.0 prototype parsing and mission script interface inventory. No Rust runtime yet.

## 2026-10-02 — Lua structure and candidate native bindings

- Added bounded static Lua 5.0 parser; all 54 sampled scripts consume exactly, totaling 135 prototypes and 20,700 instructions. No bytecode executed.
- 87 external-global candidates; 84 match adjacent name/function-pointer patterns in DOL data. Unmatched: GRADE, constant, netvariable. These are not asserted to be functions.
- Initial PowerPC disassembly ties GetTime/WaitFor to a shared float location and EndFrame/WaitFor to a shared call target; scheduler semantics remain hypotheses.
- Combined level/preload: 6,635 unique objects, no duplicate IDs, 8,149 nonzero references, zero unresolved targets.
- All 53 cGameScriptResource names resolve in the archive. The additional archive script is EntityInitialise; its special role remains to be traced.
- Added `docs/SCRIPT_COMPATIBILITY.md`, symbol evidence, object schema inventory, and synthetic bytecode/binding tests. All 15 tests pass.
- Capstone pinned locally alongside cryptography; no system installation.
- Work still uncommitted; no external publication or subagents. Next: native scheduler confirmation and first Rust parser. No playable runtime yet.

## 2026-10-02 — First Rust implementation

- Added Cargo.toml, Cargo.lock, rust-toolchain.toml, src/lib.rs and src/main.rs. Native loader reads original uncompressed XML; typed stable IDs and loss-preserving unknown fields.
- Five Rust tests pass; cargo fmt and warning-denying Clippy pass.
- Ran Rust loader on real SP_0.1 level/preload and asserted exact summary parity against the Python evidence: 6,635 objects, 8,149 references, zero unresolved.
- 15 existing Python tests retained; no game code execution.
- Scheduler follow-up identifies Lua yield diagnostics in the shared target. Common apparent argument-count helper is a save/return stub in this build, so no validation semantics inferred.
- Detailed scope and limitations in RUST_LOADER.md and SCRIPT_COMPATIBILITY.md.
- Installed ctx7 CLI because absent, per dependency-research workflow; Rust toolchain components provisioned through rustup. No game/runtime GUI or service launched.
- All changes remain local/uncommitted. Next: Rust resource archive reader and terrain/model/texture investigation toward a real-asset Bevy viewer.

## 2026-10-02 — Original textures decoded in Rust

- Added src/archive.rs, src/texture.rs, bw-archive and bw-texture binaries.
- Resource archive fully framed: 603 chunks, 553 texture headers; chunk counts/types match independent Python inventory.
- Corrected initial texture dimension endianness using actual bytes; tests now capture mixed-endian layout.
- Initial CMPR top-mip decoder produced HUD_FLAG01 and C4TTROCKB previews. Both visually inspected; coherent flag and rocky terrain texture. Not pixel-parity verified.
- Extracted terrain sample, inventoried eight chunks, recorded unresolved layout hypotheses in ASSET_FORMATS.md.
- 13 Rust + 15 Python tests pass; Clippy warnings denied passes after fixing range-loop lint.
- No Bevy viewer/model geometry/runtime at that checkpoint. All outputs remain local, originals read-only; work uncommitted. Next: terrain geometry decoding and real-asset viewer.

## 2026-10-02 — First Bevy terrain rendering

- Independently recovered terrain grid organization through byte analysis and exhaustive shared-edge comparison. No external game editor code used.
- Added src/terrain.rs and optional bw-viewer with pinned Bevy 0.19.1. No prerelease selected.
- 734 terrain records; native mesh has 187,904 vertices and 211,392 triangles. Zero mismatches across 81,439 overlapping height samples.
- First native Bevy/Metal smoke test succeeded; window-only screenshot saved at local/analysis/terrain-bevy.png and visually inspected. Owned test process stopped and waited for.
- Scale/origin, triangle diagonals and color interpretation explicitly provisional. No gameplay, textured terrain, scenery or units yet.
- 16 Rust tests pass. Clippy nested-if lint fixed; final validation below. Full limitations and launch command in TERRAIN_VIEWER.md.
- Still local/uncommitted, no subagents or publication. Next: world coordinate calibration, material mapping and model geometry.

## 2026-10-02 — First playable prototype delivered

- William authorized autonomous work until playable; implemented a deliberately labeled temporary skirmish on recovered terrain rather than pretending original mission behavior is complete.
- Added Bevy-independent skirmish simulation: ground interpolation, collision limits, moving enemies, automatic target lock/cycle, damage/cooldowns, win/loss, restart. Added Bevy HUD/input/camera/tracer integration in bw-play.
- Created executable macOS launcher Play Battalion Wars Rust.command. Startup pauses for Enter; instructions in docs/FIRST_PLAYABLE.md.
- 24 Rust + 15 Python tests pass; warning-denying Clippy with viewer feature passes.
- Real-terrain non-graphical check passes movement, win, loss and restart. Automated Bevy playthrough reached victory with 15 hits and 79 health; native exit 0. Window screenshot inspected; unsupported font symbols changed to ASCII.
- Dependencies/docs initially suggested float font sizes; actual Bevy 0.19 requires FontSize::Px, fixed and compiled. Gameplay query-filter complexity documented with narrow Clippy allowance.
- No external publication, original files unchanged, no subagents. Full original game/mission compatibility remains unfinished. Temporary combat must eventually be replaced by measured original semantics.
- Final native replay passed after ready-screen/font fixes. An interactive bw-play process was launched for William and confirmed alive, paused at the ready screen. Captured and inspected local/analysis/play-ready.png. Ownership/PID details in ignored local/analysis/play-session.json; close window/Escape stops it. All other smoke-test processes exited.
- Delivery validation summary saved in docs/evidence/first-playable-validation.json. Work remains uncommitted.

## 2026-10-02 — Textures, controller bindings and native custom levels

- William clarified full-game modding goal: original campaign plus custom levels/characters, configurable controls and standard gamepads instead of motion hardware. Recorded in MODDING_AND_CONTROLLERS.md and roadmap.
- Recovered LTAM 48-byte material pairs, per-tile material IDs, primary corner UVs, explicit secondary UVs and vertex alpha. Added two-layer WGSL renderer; original six terrain textures decoded (CMPR and new RGBA8), 14 material definitions mapped. Lighting/blend semantics remain provisional.
- First native render failed because Bevy looked for assets beside the executable; visual review/log check caught it despite a passing gameplay replay. Fixed explicit repository asset root; final native logs have no ERROR lines.
- Added data-only level manifests, editable custom heightfield format, and a custom rolling-hills sandbox with separate launcher. This is not restored campaign progression.
- Added remappable keyboard/gamepad profile with analog deadzone, yaw/pitch and configurable buttons. Synthetic Gamepad tests pass; no physical controller was detected during screenshots, so hardware validation remains pending.
- 33 Rust tests with viewer feature, 15 Python tests, Clippy warnings denied all pass. Both profiles pass real terrain/custom terrain verification (move/win/lose/reset), and native Metal victory replays. Screenshots inspected; final HUD avoids unnecessary text rebuilds.
- Added config/controls.json, mods/training/level.json, mods/sandbox/level.json and terrain.json, src/controls.rs, scenario.rs, custom_terrain.rs, terrain_render.rs, assets/shaders/terrain.wgsl, Play Custom Sandbox.command.
- Final evidence docs/evidence/textures-and-mods-validation.json. Existing user game not forcibly closed; relaunch to pick up updated binary. Test processes exited. All work remains local/uncommitted; no publication/subagents.
- Next original fidelity priorities: model mesh/animation decoding, world-coordinate calibration, scenery/unit placement, original Lua engine bindings and complete mission restoration. Character import and visual level editor are not implemented yet.

## 2026-10-02 — Original models, authored scenery, skinning and script bootstrap

- William set the next acceptance gate to original characters/scenery/animation/mission behavior together, not terrain-material tweaks. That gate remains open; no new playable-original-mission claim or launcher was made.
- Added Rust model/GX parser and renderer: all 116 sampled models decode, 264,040 triangle entries including strip degenerates. Original SVET geometry/texture visually inspected on Metal.
- Added authored object/base/model resolution: 1,016 placements, 1,005 rendered in internal scene preview, 5 outside terrain, 6 omitted for palette-texture dependencies. 100 other world objects have no supported primary-model path, including unexpanded scenery clusters. Activation rules are not applied.
- Community MIT format readers informed layout/coordinates, not architecture. Reference notices/hashes retained; no clean-room claim. Candidate world transform aligns authored scenery visually; collision/runtime comparison still pending.
- Independently recovered RNOD inverse-bind tables and XBS2 bone palettes. Rest-pose reconstruction checked across 6 skinned models/16,555 emitted vertices: maximum coordinate error 7.1525574e-7.
- Parsed 184 BAN animation resources and 245,350 packed rotations. Candidate rotation decoding passes unit-magnitude checks and drives CPU skin preview, but visual joint/limb results remain imperfect. Translation/root motion/interpolation flags and original-runtime parity are unresolved; 84 clips retain uninterpreted trailing data.
- Added bounded Rust Lua 5.0 parser/bootstrap evaluator. 54 original root chunks initialize 42 entity tables and 53 function identities using 741 checked symbolic reflection-ID calls. 4,660 instructions executed. Mission function bodies and coroutine scheduler remain unimplemented; no permissive host stubs or OS capabilities.
- Tests: 50 Rust unit + 4 opt-in local-disc integration + 15 Python; warning-denying Clippy. Existing original/custom skirmish headless checks pass. Static model, full scene and experimental rotation playback rendered on Metal; screenshots reviewed. Test windows exited; existing user windows untouched.
- Details/commands/limitations: docs/ORIGINAL_RECOVERY.md. Evidence: docs/evidence/model-audit.json, script-bootstrap.json and original-recovery-validation.json. All work uncommitted and local; no subagents/publication.

## 2026-10-02 — Original initializer bodies and resumable scoring coroutine

- Added src/mission_vm.rs: restricted Lua 5.0 body evaluator, explicit in-memory capability host, register-preserving resumable threads and proposed deterministic scheduler. Unknown operations fail closed; failed resumes/ticks roll back all staged state. No OS capabilities or permissive no-op host stubs.
- Original ArtVariables (23 instructions) and Gambit_Part_1_Global_Variables (342 instructions) now complete. Named unit X/Z reads checked against original object matrices; debug events and bounds-message configuration retained. No claim that this is full mission startup ordering.
- Original Pause_Scoring_Timer runs through waits and both active/disabled branches using explicitly seeded external scoring/cutscene globals. Preserved its one-final-increment-after-pending-wait behavior. Logical pause/resume and frame yield policies tested but not compared against original execution.
- Investigated animation palette/sign/translation hypotheses. Corrected earlier evidence wording: inverse-bind cancellation alone cannot validate vertex-to-bone palette assignment. SVET spatial groups support the candidate ordering; executable scans did not establish definitive decompression semantics. No unverified animation 'fix' was applied.
- 56 Rust unit + 6 opt-in original-data integration + 15 Python tests pass; Clippy warnings denied. Both existing skirmish profiles still pass movement/win/loss/restart. No new visual behavior was claimed and no user windows were closed.
- Current details: docs/MISSION_EXECUTION.md. Reports: docs/evidence/mission-initialization.json (metadata only) and mission-execution-validation.json. Full mission Lua calls/captures, gameplay host functions, AI/objectives and animation fidelity remain unfinished. No subagents/publication; all work uncommitted.

## 2026-10-03 — Original win/loss triggers and unit animation state

- Added bounded non-capturing Lua call stacks, nested closures, open calls/returns and fixed parameter metadata. Synthetic yield/resume, recursion rollback and argument conformance tests pass. Original ScoreScreen initializer now defines its helpers; StopScoringTimer executes original Lua rather than a replacement stub.
- Original Gambit_Part_1_WinLose reacts to controlled tower-entry and staff-death inputs and reaches its actual win/loss cinematic gates. Tests preserve the waits; cinematic completion is not fabricated. Nested loss function stays suspended correctly across ticks.
- Added mission_world typed units, base-derived health/army, raw damage/invulnerability, explicit roster/activation, zone queries, objective visibility and player freeze/declaration state. Original invincibility script actually affects damage. Numeric native enum ABI is not invented; symbolic tokens are labelled adapters. Region policies still require original-runtime verification.
- Added unit_animation: all 183 troops resolve 915 idle/walk/run/fire/death bindings to 10 original clips. Health death selects the authored clip, one-shot playback clamps, and pause freezes clocks. This is not a decoder fix or graphical integration; BAN pose defects/root motion remain unresolved.
- Validation: 62 Rust viewer unit + 4 original-asset + 6 original-mission + 15 Python = 87 passing tests; warning-denying Clippy and both existing skirmish regressions pass.
- Current evidence/limitations: docs/MISSION_GAMEPLAY.md and docs/evidence/mission-gameplay-validation.json. Existing launchers unchanged. Full port remains the scope; complete original mission is not yet playable. No publication, subagents or user-window termination; all work remains uncommitted.

## 2026-10-03 — Moving original script/actor in native scenery

- Added original circular IsInArea and restricted forced GoToArea adapters. Explicit straight-line diagnostic backend updates the same positions queried by Lua; unavailable navigation rejects atomically. Uses authored troop speed, radius stopping, run/idle animation state and scheduler pause. No collision/pathfinding or vehicle-steering claim.
- Original Stay_Here test captures home, issues return orders after displacement, observes real position changes, idles on arrival and terminates on death. Backend-disabled rollback and pause checks pass.
- Added opt-in BW_SCRIPT_PREVIEW to the existing original viewer, integrating original Sgrunt02 body/attachments with the original script, private deformable mesh copies and unit animation clocks. Fixture actor pairing/cutscene flag are explicit; this is not authored mission startup or a new full-game launcher.
- Native Metal scene-regression, return-home and death smokes pass. Return reached distance 5 and Idle; death ended the original coroutine and clamped the authored death clip. Reviewed screenshots show the original running/fallen actor in original scenery.
- Initial death screenshot was camera-occluded; preserved image/log with -occluded suffix. Later capture changes the controlled damage point, not camera collision. Pose/root-motion and scenery/material defects remain unresolved.
- Validation: 62 Rust viewer unit + 4 original-asset + 7 original-mission + 15 Python = 88 passing tests, warning-denying Clippy, both old skirmish regressions. Smoke windows self-exited; user windows untouched. Captures stay ignored/local.
- Current commands/limitations: docs/SCRIPT_RENDER_INTEGRATION.md; evidence docs/evidence/script-render-integration.json. Full original mission still not playable; no publication/subagents, all changes uncommitted.

## 2026-10-03 — Original-scene camera obstruction correction

- Reproduced the blocked frame-80 death/frame-100 capture. Cause: unconstrained orbit placed the camera behind scene geometry. Added camera_collision.rs with shared model triangles, transformed instance bounds and two-sided segment queries; used the actual rendered scene transforms, excluding the moving actor.
- Follow camera retracts with clearance and tries bounded alternative views when too close. Same actor location/damage/capture frames now show the troop instead of an obscuring wall. This is replacement camera policy, not original camera parity, terrain/near-plane collision or unit navigation.
- Six camera/geometry tests, including observed red-to-green retraction test. Native corrected-ray assertions and both actor replays pass with 1,003 obstacles: death 145 adjusted frames, return 53, arrival distance 5/Idle. Static scene and animation-gallery regressions also pass. Before/after screenshots inspected.
- Hardened smoke input isolation after a noncanonical trajectory; preserved failed replay. Reverted an unfocused-window experiment after a black image, retaining that artifact. Native visual success requires inspected captures, not just exit status.
- Validation: 68 Rust viewer unit + 4 original-asset + 7 original-mission + 15 Python = 94 passing tests, Clippy warnings denied and both skirmish regressions pass. Details/evidence: docs/CAMERA_OBSTRUCTION.md and docs/evidence/camera-obstruction-validation.json.
- Full original mission remains unfinished; camera alternates can snap, transparent/missing geometry and inside-geometry targets remain limited. No changes to launchers, publication, subagents or forced user-window closure. All work uncommitted.

## 2026-10-03 — Replacement infantry collision

- Added independent unit_collision.rs: cylinder/height-clipped triangle and active-body sweeps, bounded sliding, finite validation, degenerate filtering, and touching-contact hardening. World CollisionAware backend runs shared bounded substeps; unknown active shapes fail closed and scheduler rollback remains atomic.
- Integrated actual original rendered primary geometry and the existing terrain-height adapter. Native default actor now stops against scene geometry; a second explicitly activated/placed original troop blocks it in the pair fixture. Both final screenshots inspected. Original pose fidelity and activation/startup remain unresolved.
- Final native scene: 1,003 instances, 675 mesh resolutions, distance 10.618307 (not arrival). Pair: 1,002 static instances, 694 active-body resolutions, distance 13.200027 (not arrival). Repeated resolutions are not unique collisions. No reported overlap under tested collision tolerances.
- Initial +100 fixture stopped near an obscured cliff; preserved unit-collision-cliff.*. Changed the collision preview to a visible +40 fixture, not a claim to repair that camera view. Preserved intermediate pre-contact-hardening results; only *-final captures/logs are current acceptance evidence.
- 101 tests pass (74 Rust viewer unit, 4 original-asset, 8 original-mission, 15 Python), Clippy warnings denied and formatting pass. Includes high-speed walls, corners, touching starts, opposing/crossing original units, pause/freeze/death and atomic budget errors. Native death, legacy camera (145 adjusted frames with unit collision disabled), static scene, and both skirmish profiles pass.
- Added Preview Original Unit Collision.command without changing existing play launchers. No publication, commits, subagents, or forced closure of user windows. Details/limits: UNIT_COLLISION.md and evidence/unit-collision-validation.json. Pathfinding, moving vehicle physics, and complete original mission acceptance remain unfinished.

## 2026-10-03 — Bounded replacement infantry routing

- Added navigation.rs: deterministic A* with collision-tested subdivided edges, bounded work/horizon, and distinct Found/NoRoute/Budget results. PlannedInfantry starts directly, queues a plan when blocked, follows waypoints under live collision checks, and invalidates blocked/stale routes. One search/tick, round-robin candidate order, 0.5-second retry cooldown; no fake arrival on failed searches.
- Original Stay_Here now returns home around static snapshot geometry and the explicit active troop blocker. Native scene: one search, 3 expansions/162 probes, distance 4.9999943; pair: one search, 8 expansions/365 probes, distance 5. Both report one planned-order arrival and no overlap. Detour and arrival captures visually inspected under local/analysis/navigation-*.
- Added original-script fixture with a finite wall, live troop moved into the cached path, required replanning and arrival, positive retry-clock pause, and changed-order invalidation. Five planner unit tests cover detours/replay, deterministic output, bodies, enclosed/budget outcomes, and invalid inputs. These are controlled replacement-behavior tests, not original navigation parity.
- 107 tests pass (79 viewer Rust unit + 4 original-asset + 9 original-mission + 15 Python), Clippy warnings denied, formatting and launcher shell syntax pass. Native collision-only pair still stops at 13.200027 with 694 contacts and no searches; death, legacy camera (145 adjustments), static scene and both skirmish profiles pass.
- Existing original-unit preview launcher now enables planning; BW_PATHFINDING_DISABLED preserves the preceding collision-only fixture. Existing skirmish launchers unchanged. No publication, commits, subagents, or forced closure of user windows. Details: UNIT_NAVIGATION.md and evidence/unit-navigation-validation.json.
- Coarse bounded grid can miss narrow/long routes; no original navmesh, predictive crowd avoidance, vehicle routing, water/bridge/climb semantics or full mission startup. Animation fidelity and complete original mission acceptance remain unfinished.

## 2026-10-03 — Playable original-asset battle

- Main launcher now builds/runs bw-battle: direct control of original Sgrunt02, two original squadmates, four original opposing riflemen, original textured terrain/scenery and weapon attachments. Added shared original_app/battle_view integration, retaining original viewer and separate custom sandbox. No second asset reconstruction path.
- Replacement gameplay includes camera-relative movement through existing collision, explicit manual authority, enemy pursuit/routing and sight-checked fire, squad hold/follow, target cycling, friendly shot interception, health/death, cyan uplink capture, win/loss, pause and a shared restart handler. These rules/roster/activation are explicitly new; the original mission script chain, animation fidelity and full-game compatibility remain unfinished.
- 114 tests pass (86 Rust viewer-feature unit + 4 original-asset + 9 original-mission + 15 Python), warning-denying Clippy, formatting, locked native builds and launcher syntax. New cover/manual-collision/squad tests pass; the squad test fixture was corrected to move beyond the documented six-unit follow threshold before expecting pursuit.
- Native Metal victory: 11.0667 simulated seconds, 72% player health, four enemies eliminated, 34 shots, 8 enemy attacks, all five animation states observed. Defeat: 7.8333 seconds, 29 enemy attacks, zero player health. Victory → shared restart → defeat also passes with explicit restored state assertions. Planned original-script pair arrival/death and both older skirmish scenarios remain green.
- Initial native screenshots were black while the desktop was locked, including the old viewer regression. Kept those unsuccessful captures; did not unlock or modify desktop security. Added internal offscreen image targeting with the same scene/UI and inspected final combat/victory/animated-death frames. Normal play launcher clears offscreen/replay flags. Reduced HUD obstruction and replaced unsupported punctuation.
- Documentation: PLAYABLE_BATTLE.md and evidence/playable-battle-validation.json. Mac GPU verification only; human input/controller acceptance and Windows/Linux remain unverified. No audio/vehicles/switching/saves/campaign yet. No commits, publication, subagents, or forced closure of user windows. Stopping this slice with a runnable original-asset battle for William to try, not claiming the complete original-level milestone.
