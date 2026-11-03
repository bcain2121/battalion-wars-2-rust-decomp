# Where we are — Battalion Wars Rust

**Agent handoff:** read `HANDOFF.md` for safety constraints, exact reproduction commands, code/data locations and the next bounded task; `docs/ROADMAP.md` contains the prioritized forward plan. The latest section below is authoritative. Earlier checkpoints retain historical blockers/test counts and must not be mistaken for current failures. This checkout is largely untracked, with no commits on `main`; preserve all work.

## Latest: Authentic Wii Cold-Start Boot Sequence (`BootLogos` -> `BWii_Intro.thp` -> Wii Title Screen -> 3-Card Save Slot Select -> On-Screen Keyboard -> 2x2 Main Menu -> 3D Satellite Globe Campaign & Level Select -> Campaign Opening `.THP` Cutscenes), Interactive Keybindings & Controller Remapping Settings Menu (`FrontendStage::Keybindings`), all 20 single-player campaign levels (`SP_0.1`, `SP_1.1..SP_5.4`), all 16 multiplayer maps (`MP1..MP15`, `MP17`), all 96 `Frontend2_Level` Lua scripts + on-screen `cGUIPage`/`cGUIWidget` Bevy UI canvas, live `FEQT` Tequila 3D particle bursts, dual-viewport 2-Player (`PLAYER_TWO`) split-screen + UDP `NetPeer` sync, matrix-palette normal+position skinning, live `cMorphingBuilding` mesh swapping, and per-wheel suspension integrated and verified

- **Authentic Wii Cold-Start Boot, Opening Intro Movie, Title Screen, Save Slot Cards, On-Screen Keyboard, 2x2 Main Menu, 3D Satellite Globe Campaign Select & Interactive Keybindings Menu (`src/bin/support/first_mission_view.rs`, `src/controls.rs`, `src/texture.rs`, `src/frontend_runtime.rs`, `src/frontend_gui.rs`):**
  - **Cold-Start Wii Boot & Opening Intro (`FrontendStage::BootLogos` -> `FrontendStage::IntroMovie`):** Cold-start launch displays the decoded `Frontend2_Level.res` Wii Remote wrist-strap safety screen (`startup_strap` / `MPLoadingWiiMenu8_US`) and Nintendo/Kuju ESRB screen (`startup_esrb`), then automatically streams and plays the full opening intro movie (`Data/Movies/BWii_Intro.thp` with decoded MJPEG video + THP ADPCM audio, skippable at any time via `A` / `B` / `Enter` / `Space` / `Esc` / mouse click).
  - **Authentic Wii Title Screen (`FrontendStage::Title`):** Renders the full-screen `bg_map03` navy world-map backdrop (`FrontendBackdrop`), the layered 3D rotating satellite globe (`Globe_sat` + `Globe_sat_clouds` + `Globe_atmos` from `Frontend2_Level.res` with mission terrain/water/actors hidden during frontend stages), 12 orbiting faction unit icons (`GlobeUnitRingIcon(0..12)`), centered `bwii_logo`, clickable `"PLEASE PRESS (A) & (B)"` prompt with `wii_abutton2` + `wii_bbutton2` icons, `"[ WATCH OPENING INTRO MOVIE (BWii_Intro.thp) ]"` replay button, and a live Wii Remote hand pointer (`WiiHandCursor` using `cursor_point` from `Shared.res`).
  - **Authentic Wii Save Slot Selection & On-Screen Keyboard (`FrontendStage::SaveSelect` & `FrontendStage::NameEntry`):** Displays 3 horizontal Wii save cards (`SAVE A`, `SAVE B`, `SAVE C`) with green header bars (`0%..100%` completion), gold active highlight, and bottom `"CHANGE NAME"` / `"CONTINUE"` / `"CLEAR SLOT [DEL]"` modal. Selecting an empty slot opens the interactive Wii On-Screen Keyboard (`NameEntry`) with clickable keypad buttons plus hardware keyboard typing and persists profiles to `local/saves/bw2_saves.json`.
  - **Authentic Wii 2x2 Main Menu & 3D Globe Campaign/Level Select (`FrontendStage::MainMenu`, `FrontendStage::CampaignSelect`, `FrontendStage::MissionCutsceneIn`):** Displays the 2x2 Wii button grid (`CAMPAIGN`, `NINTENDO WFC`, `OPTIONS`, `EXTRAS`) over the rotating 3D Globe. Entering `CAMPAIGN` reveals 6 clickable continent tabs (`PROLOGUE`, `C1: ANGLO ISLES`, `C2: SOLAR EMPIRE`, `C3: WESTERN FRONTIER`, `C4: TUNDRA TERRITORIES`, `C5: XYLVANIA`), an open center over the rotating 3D Globe, left mission list, right mission briefing card with `LAUNCH MISSION`, and automatic playback of each campaign's opening `.THP` cutscene (`0.1_Prologue_1.THP`, `1.1_Anglo_1_In.thp`, `2.1_Solar_1_In.thp`, `3.1_WF_1_In.thp`, `4.1_Tundra_1_In.thp`, `5.1_Xylvania_1_In.thp`) before deploying into battle.
  - **Interactive Keybindings & Controller Remapping Settings Menu (`FrontendStage::Keybindings`, `src/controls.rs`):** Accessible from `Options` (`"5. CUSTOMIZE KEYBINDINGS & GAMEPAD"`) or mid-mission via `[F4]` / Pause Menu (`"KEYBINDINGS & CONTROLLER REMAPPING (F4)"`). Lets players browse all 24 gameplay actions across 4 pages (Movement & Camera, Combat & Targeting, Squad Commands & Transfer, Squad Slots & Transport), press `[ENTER]` or click an action row to enter `"WAITING FOR INPUT..."` capture mode (`detect_pressed_key`, `detect_pressed_gamepad_button`), press any keyboard key or gamepad button to rebind live in `ResMut<Controls>`, reset all bindings to defaults with `[BACKSPACE / DELETE]`, and persist changes to `config/controls.json` (`Controls::to_json_pretty`).
  - `FrontendRuntime::init_shared_scripts` and `construct_all_pages` initialize and construct all 96 `Frontend2_Level` Lua scripts (`FrontendBW2`, `Vector`, `Matrix`, `Tools`, `Widgets`, `Spinning`, `Fade`, `Slide`, `GlobeSelect`, `Start`, `Title`, `LoadSave`, `Keyboard`, `ModeSelect`, `CampaignS`, `MissionS`, `MapSelect`, `Controls`, `Extras`, `Units`, `Concepts`, `Cutscenes`, `Credits`, `Stats`, etc.), verified by `tests/frontend_runtime.rs::all_frontend2_pages_construct_and_drive_page_stack`.
  - `GuiScene::from_graph` and `evaluate_active_page_canvas` (`src/frontend_gui.rs`) resolve all 54 `cGUIPage` hierarchies and 1,683 `cGUI*Widget` nodes (`cGUICustomWidget`, `cGUIButtonWidget`, `cGUITextBoxWidget`, `cGUITextureWidget`, `cGUISpriteWidget`, `cGUIRectWidget`, `cGUIDialogBox0..3Widget`, `cGUI3DObjectWidget`, and `sSpriteBasetype`) against live Lua `FromLabel`/`FromFunction`/`StartWidget`/`StopWidget` state and render them on-screen in `bw-first-mission` via a 48-node `FrontendGuiCanvasQuad` / `FrontendGuiCanvasText` Bevy UI pool (toggleable with `[F3]`).
- **All 20 single-player campaign levels (`SP_0.1` and `SP_1.1` through `SP_5.4`) complete end-to-end through their full authored cutscenes, objectives, and `WinMission` (`MissionOutcome::Won`) via ordinary `Input` in `tests/campaign_levels_runtime.rs::autopilot_survey_campaign_levels`:**
  - **Prologue & Campaign 1 (6/6 `Won`):**
    - `SP_0.1` (*Prologue: A Flash in the Pan*): `Won` (`GRADE_S`, frame `11,069`; also verified with authored defeat `MissionOutcome::Lost { message_id: Some(196) }` and `restart()` in `tests/first_mission_staff.rs::first_mission_reaches_victory_and_defeat`).
    - `SP_1.1`: `Won` (`GRADE_S`, frame `13,128`, `objectives_done=9`).
    - `SP_1.2`: `Won` (`GRADE_S`, frame `17,676`, `objectives_done=38`).
    - `SP_1.3`: `Won` (`GRADE_S`, frame `18,863`, `objectives_done=16`).
    - `SP_1.4`: `Won` (`GRADE_S`, frame `13,866`, `objectives_done=14`).
    - `SP_1.5`: `Won` (`GRADE_S`, frame `18,695`, `objectives_done=43`).
  - **Campaign 2 (3/3 `Won`):**
    - `SP_2.1`: `Won` (`GRADE_S`, frame `13,441`, `objectives_done=9`).
    - `SP_2.2`: `Won` (`GRADE_A`, frame `24,995`, `objectives_done=9`).
    - `SP_2.3`: `Won` (`GRADE_S`, frame `24,522`, `objectives_done=9`).
  - **Campaign 3 (4/4 `Won`):**
    - `SP_3.1`: `Won` (`GRADE_S`, frame `15,436`, `objectives_done=10`).
    - `SP_3.2`: `Won` (`GRADE_A`, frame `14,705`, `objectives_done=4`).
    - `SP_3.3`: `Won` (`GRADE_S`, frame `19,356`, `objectives_done=15`).
    - `SP_3.4`: `Won` (`GRADE_A`, frame `11,176`, `objectives_done=24`).
  - **Campaign 4 (3/3 `Won`):**
    - `SP_4.1`: `Won` (`GRADE_A`, frame `21,679`, `objectives_done=14`).
    - `SP_4.2`: `Won` (`GRADE_A`, frame `35,642`, `objectives_done=14`).
    - `SP_4.3`: `Won` (`GRADE_B`, frame `33,221`, `objectives_done=7`).
  - **Campaign 5 (4/4 `Won`):**
    - `SP_5.1`: `Won` (`GRADE_A`, frame `28,284`, `objectives_done=6`).
    - `SP_5.2`: `Won` (`GRADE_S`, frame `24,524`, `objectives_done=5`).
    - `SP_5.3`: `Won` (`GRADE_B`, frame `45,568`, `objectives_done=5`).
    - `SP_5.4`: `Won` (`GRADE_A`, frame `35,285`, `objectives_done=8`).
- **Multiplayer Assault, Skirmish & Co-Op Maps (`MP1..MP15`, `MP17`) + Dual-Viewport 2-Player (`PLAYER_TWO`) Split-Screen & UDP `NetPeer` Sync (`src/net_sync.rs`, `src/mission_world.rs`, `src/mission_combat.rs`, `src/bin/support/first_mission_view.rs`):**
  - All 16 multiplayer levels (`MP1..MP6` Assault, `MP7..MP11` & `MP17` Skirmish, `MP12..MP15` Co-Op) load with authentic `frontend2English.str` (`1156..1179`) titles/briefings, complete their intro cutscenes and `ForceUnitTransfer` to Player 1 & Player 2 (`two_player_mode`, `player2`, `player2_roster`, `GetPlayerUnit(PLAYER_TWO)`, `IsUnderPlayerCommand(unit, PLAYER_TWO)`, `AllPlayerUnitsDead(PLAYER_TWO)`, `IsNetworkPlayer(PLAYER_TWO)`), run with `advance_multiplayer_skirmish_ai` (`src/mission_world.rs`), and support dual-viewport top/bottom split-screen (`Player2Camera3d` + `SplitScreenDivider`), full P2 controls (`IJKL` move, `U`/`O` aim yaw, `H` fire via `Combat::fire_unit_at_direction`, `Y` unit-switch via `World::cycle_player2_unit`), and non-blocking UDP `NetPeer` (`PlayerInputPacket`, `BW_NET_BIND`/`BW_NET_PEER`) alongside `SetNetworkMissionComplete` and `DeclareVictory`.
- **Visual, 38-Motion `cTroopAnimationSet` Animation Repertoire, Visible Seated Vehicle Crew, 1-to-1 Unit Piloting & Physics (`GroundVehiclePhysics`, `FlightProfile`, `WaterVehiclePhysics`, `UnitCameraProfile`), Live `FEQT` Tequila 3D Particle Bursts, Live `cMorphingBuilding` Mesh Swapping & 3D Positional Audio Fidelity:**
  - **Full 38-Motion `cTroopAnimationSet` Repertoire (`src/unit_animation.rs`, `src/original_scene.rs`, `src/bin/support/first_mission_view.rs`):** Expanded `Motion` from 5 clips to 38 authored `cTroopAnimationSet` clips (`Idle`, `AlertIdle`, `ContextIdle`, `Walk`, `Run`, `RunStop`, `WalkBack`, `RunBack`, `StrafeLeft`, `StrafeRight`, `StrafeLeftRun`, `StrafeRightRun`, `TurnLeft`, `TurnRight`, `RollForward`, `RollBack`, `RollLeft`, `RollRight`, `JumpRise`, `JumpPose`, `JumpLand`, `Fire`, `ForwardFire`, `Reload`, `FallFlail`, `HitGround`, `Getup`, `Swim`, `SwimBack`, `TreadWater`, `WaterDeath`, `SteerFront`, `SteerLeft`, `SteerRight`, `DriveIdle`, `GunnerFire`, `Death`, `Death2..5`, `Victory`) with fallback chains (`Player::select_with_fallback`) and `meHoldWeaponInLeftHand` weapon attachment to `BONE_L_HAND` vs `BONE_R_HAND`. Interactive `Space` triggers directional combat rolls (`RollLeft`/`RollRight`/`RollBack`/`RollForward` with planar burst velocity when locked on) or vertical jumps (`JumpRise` -> `JumpPose` -> `JumpLand` with parabolic height arc).
  - **Visible Seated Vehicle Crew (`mPassenger` & `cSeatBase` `DisplayType` in `src/vehicle_wheels.rs`, `src/bin/support/first_mission_view.rs`):** Extracts `mSeatData` (`SEATDISPLAY_SOLDER_SITTING`, `SEATDISPLAY_SOLDER_STANDING`, `SEATDISPLAY_SOLDER_TOSHOULDERS`, `SEATDISPLAY_SOLDER_HEADONLY`, `SEATDISPLAY_INVISIBLE`) and attaches visible driver, turret gunner, and open-top transport passengers (`host.world.passengers`) to their parent vehicle's world transform + turret rotation while playing `SteerLeft`/`SteerRight`/`SteerFront`/`DriveIdle`/`GunnerFire`.
  - **1-to-1 Piloting, Physics & Authored Chase Camera Profiles (`src/mission_air.rs`, `src/first_mission.rs`, `src/vehicle_wheels.rs`, `src/bin/support/first_mission_view.rs`):**
    - **Ground Vehicles (`GroundVehiclePhysics` & `SuspensionState::step_with_physics`):** Parses `cGroundVehiclePhysicsBase` and `cReflectedPhysicsParams` (`attackSpeed`, `reverseSpeed`, `driveAccln`, `brakingAccln`, `maxTurnRate`, `mChassisMass`, `mSuspensionSpringStiffness`, `mSuspensionSpringDamping`, `mFireImpulse`, `mSideSlideGripFactor`), raycasts terrain height at each of the 12 actual wheel world positions (`vehicle_wheel_offsets`), and applies main-gun recoil impulses (`mFireImpulse / mChassisMass`).
    - **Aircraft & Helicopters (`FlightProfile`):** Parses `cFlightData` (`mBackwardMaxSpeed`, `mMaxTiltForward`, `mCruiseTiltForward`, `mMaxTiltBackward`, `mMaxTiltSideways`, `mBankFactor`, `mBarrelRollTime`, `mBarrelRollRadius`), maintains continuous forward cruise speed (`min_speed`) for fixed-wing fighters/bombers/strato-destroyers while allowing helicopters/gunships to hover/strafe/reverse, and renders 3D pitch, bank, and interactive `Space` barrel-rolls.
    - **Naval Vessels & Submarines (`WaterVehiclePhysics` & `SubmarineState`):** Parses `cWaterVehiclePhysicsBase` (`mMaxSpeed`, `mMaxReverseSpeed`, `mBobRollAmount`, `mBobPitchAmount`, `mBobSpeed`, `mRollOnTurnCoefficient`, `mPitchOnAcclnCoefficient`) for wave bobbing and turn roll, plus interactive `Space` submarine dive/surface toggle (`submerged`), submerged depth rendering (`view.water_height - sub.depth`), and HUD depth/oxygen readout (`[SUBMERGED -12m | O2 94%]`).
    - **Per-Unit Chase Camera Profiles (`UnitCameraProfile`):** Parses `mCamPos`, `mCamLookPos`, `mCamPos2`, and `mCamLookPos2` across `cTroopBase`, `cGroundVehicleBase`, `cAirVehicleBase`, and `cWaterVehicleBase` to frame infantry, tanks, battlestations, aircraft, dreadnoughts, and submarines at their authored distances and heights.
  - Added fast matrix-palette normal+position skinning (`skinning::deform_positions_and_normals` in `src/skinning.rs`), upper-body spine/chest aim-pitch and torso yaw twist blending (`skinning::pose_with_aim` on `BONE_1_SPINE` and `BONE_2_SPINE`), native `cTequilaEffectResource` (`FEQT`) particle decoder & live 64-billboard `TequilaBillboard` emitter bursts (`LiveTequilaParticle`, `TequilaEffect::spawn_burst`, per-particle `ColourRange` keyframe tinting in `src/tequila.rs` and `src/bin/support/first_mission_view.rs`), live `cMorphingBuilding` intact/destroyed (`mpDeadModel`)/faction-swap mesh toggling (`MorphingBuildingModels`, `World::morphing_building_visual_state`, `MorphVariant`), radial infantry explosion knockback impulses, tank tread UV scroll, aircraft rotor spin, turret yaw/gun barrel pitch, 2-layer water shimmer, flying explosion debris fragments, authored `sExplodeBase` particle flipbooks, and 3D distance-attenuated positional audio.
  - Updated `Play Battalion Wars Rust.command` to launch `bw-first-mission` by default.
- **Decompiled via Ghidra 12.1.4 / REA MCP (`local/analysis/main.elf`) and implemented in Rust:**
  - Cataloged all 268 native Lua C-function bindings (`0x804b1a60..0x804b22c0`) in `local/analysis/lua-bindings.json` and static Lua 5.0 bytecode inventories across all 20 SP levels in `local/analysis/lua-inv/`.
  - Implemented Lua object-handle numeric comparison parity (`GetPlayerUnit` `0x800e8258`, `Op::Eq`/`Op::Lt`/`Op::Le`), `cPickup` health/jerrycan collection (`0x800c39cc`) and crushable tent contact/pathing, spatial acceleration (`TriGrid` & `MeshIndex` in `src/unit_collision.rs`), bridge deck auto-detection (`SM GORGEBRIDGE`, `WF WOODBRIDGE`, `AI BRIDGE`, `SE BRIDGE`, `C2 BRIDGE`, `X BRIDGE`) and gate/wall opening (`GATE`, `DOOR`, `C5 SOLARWALL`), native `flag.*` / `constant.TYPE` / `constant.GTYPE` tables (`0x8023e038`, `0x80123338`), `NumItemsInArea`, `TestFlags`, `GetState`, `SetInvulnerable`, partial/closest-node path planning (`plan_towards`), cutscene combat, aircraft cruise HAG & landing (`LandAirUnit` `0x800e9590`, `GetMovementState` `0x800d7ffc`), `FollowUnit` route hysteresis, lock-on control transfer (`Z` button), capture-point flag raising/lowering & allied contention aggregation (`CheckCapturePoint` `0x800d7fcc`, `SetCapturePointHeight` `0x800d834c`, `GetInstallationOwner` `0x800e2b00`, `GetHealthPercent` on `cCapturePoint` `0x800d8124`), `cBuildingImpBase` `SeatFlags & 1` garrison filtering, burst weapon folding & indirect ballistic fire (`UNIT_VEH_ARTILLARY` / `UNIT_VEH_BATTLESTATION`), `cMorphingBuilding` combat base resolution, carrier passenger boarding/unloading (`EnterVehicle` / `ExitVehicle` / `ReviveDeadUnit` ground-relative `Y` preservation), automatic death control transfer (`TRANSFER_BYDEATH`, `GetPlayerTransferred` `0x800da024`), and `cAmbientAreaPointSoundSphere` (`0x801fbae0`) + numeric target ID support in `SetActive` (`0x800d9c78`).
- **221 library unit tests pass** (`cargo test --locked --features viewer --lib`), with `cargo fmt --check`, `cargo clippy --locked --features viewer --all-targets -- -D warnings`, `tests/frontend_runtime.rs`, and `BW_SMOKE=1 BW_MENU_SMOKE=1` clean.

## Earlier checkpoint: staff recovery cutscene completes; trench flame projectile blocks continuation

The original input route now completes the entire `Gambit_Part_1_Cutscene_StaffBearer` sequence and returns live player control on staff bearer **550035312** at approximately **122.38 game seconds** (`cutsceneStaffBearerBegin == 2`, `!frozen`). `Objective.PickupStaff` (`550041656`) completes and hides its marker (`550041658`), `Objective.Trench00` (`550035435`) becomes visible with marker `550035437` (`[92.827, 0.0, -12.297]`), `Troop.ILgrunt04` (`550030841`) relocates to `[63.339001, 0.0, 35.401001]`, and `Gambit_Part_1_SolarBattle` vehicles `550030859` and `550030356` advance along `Waypoint.BattleLeft02` and `Waypoint.BattleMainLeft01`. No scripts, progress globals or player-route positions were overridden.

Recovered `cWaypoint::GetNextWaypoint` (`0x8003dfcc`) and C-runtime `rand()` (`0x80258a60`, seeded to `1` per `0x805fc6a0`) for optional waypoint branches (`mOptionalNextWP1` / `mOptionalNextWP2`), enabled `FollowWaypoint` `ORDER_NORMAL` + dead-unit `false` return (`0x80145288`) under the rewrite runtime, and added collision- and terrain-validated non-player `cTroop` `Teleport` relocation (`0x800d4a18` / `0x80079638` / `0x8006ca4c`) while keeping player position relocation an explicit error.

**325 viewer tests / 194 overlapping library tests pass**, with formatting, Clippy and build clean. A full-tail GPU replay rendered **1,053 frames inside the staff cutscene** (`ORIGINAL_STAFF_RENDER_PASS`) through control return (`local/analysis/staff-complete.png` and `.frame-0320`, `.frame-0550`, `.frame-0950`, `.frame-1280`). Early `eCAMTYPE_GENERAL` shots (`frame-0320`, `frame-0550`) remain terrain-obscured, while `frame-0950` clearly frames the Iron Legion trench defense in front of the Iron Tower and `frame-1280` / `staff-complete.png` clearly frame the restored chase camera on `Sstaffbearer01` looking toward the `Trench00` Gold Star (68 m ahead) and Iron Tower.

Driving the unfrozen staff bearer 325 input frames (> 100 units north past `Objective_Marker.Trench00` into `StrikeForce01`) stops transactionally at **`Unsupported non-rifle projectile 200001224: DAMAGE_FIRE`**. See `docs/STAFF_TRANSFER.md` and `docs/evidence/staff-transfer-validation.json`. Existing user previews were untouched; nothing published.

## Earlier checkpoint: staff ownership transfer works; battle waypoint blocks continuation

The original scripts now transfer ownership to staff bearer **550035312** during the frozen black fade. This uses a narrowly scoped default-transfer adapter, not the explicit hard-cut path: no cut-revision increment, no route teleport, and no progression-global override. Native auxiliary dispatch was traced to the HUD0 sampled-audio path; its omission and hidden camera interpolation are explicitly reported cosmetic gaps. Visible default transfers still fail.

**319 viewer tests / 189 overlapping library tests pass**, with formatting, Clippy and build clean. The route advances 37 ticks beyond the previous blocker, then stops transactionally at **`Gambit_Part_1_SolarBattle pc 93: FollowWaypoint: Unsupported unit waypoint flags/branch selection`** (`Waypoint.BattleLeft02`, offsets 5/5, forced order). Staff cutscene phase remains 1 and player controls remain frozen: **staff recovery/cutscene completion is not accepted**.

See `docs/STAFF_TRANSFER.md`. The diagnostic `staff-handoff-prefix-inputs.json` can be rendered with `BW_MISSION_REPLAY_HOLD=1`, which holds the last successful recorded tick, not gameplay progression. Reviewed `local/analysis/staff-handoff-prefix.png` is terrain-obscured during the early fade; it is not character/cinematic visual acceptance. Next: resolve the original battle waypoint semantics, reach staff-cutscene control return and review its complete camera sequence. Existing user previews were untouched; nothing published.

## Earlier checkpoint: original ambush completes; staff handoff reached

The original bomber ambush now runs through its own camera/radio/staff-reveal sequence and returns player control at about 101.10 game seconds. Input-driven acceptance records **14 missile launches/aircraft contacts and 4 bomb impacts**. Camp arrival completes, the staff-recovery objective appears, and restart clears the new ordnance state. No scripts, progress globals or player-route positions were overridden.

Added source-profile defensive anti-air missiles, model-derived aircraft hit bounds, real damage/expiry/cover handling, `EnableWeapon`, and the original cinematic's in-place player-facing reset. General ground relocation remains unsupported. Declared Rust solver/collision/damage/presentation policies are documented in `docs/AMBUSH_COMPLETION.md`; this is not native aircraft, weapon-balance or animation parity.

**318 viewer tests / 188 overlapping non-viewer library tests pass**, with formatting, Clippy and build clean. A full-tail GPU replay rendered **1,284 ambush frames** rather than only prerolling the scene. Reviewed original staff reveal and control-return captures: `local/analysis/ambush-full-render-final.png.frame-1450.png`, `.frame-1550.png`, and `ambush-full-render-final.png` (identical to the reviewed prior-run captures). First-shot aircraft framing/visibility, native FX, full animation and hardware audio remain unaccepted.

Walking another 70+ units triggers the staff-recovery cutscene, whose current blocker is **`Gambit_Part_1_Cutscene_StaffBearer pc 327: ForceUnitTransfer: Unsupported gameplay call/signature: ForceUnitTransfer`**. This is the two-argument default transfer, not the already-supported explicit hard-cut variant. The test verifies frame/player/position rollback; **staff recovery and recruitment are not complete**. Native wrapper defaults to flags 1; third-argument true adds 0x14. Resolve its default camera/auxiliary-notification behavior next; do not alias it blindly to the existing hard-cut path.

See `docs/AMBUSH_COMPLETION.md` and `docs/evidence/ambush-completion-validation.json`. Menus/profiles, heavy combat, capture, full mission victory/defeat, animation/hardware and other-platform acceptance remain open. User previews and launchers were left untouched.

## Earlier checkpoint: bomber deployment, flight and ordnance backend

Original input now completes the camp-arrival objective and executes aircraft `Teleport` plus the bomber's forced attack order. The original cinematic/radio prefix renders; no scripts or progress globals were rewritten. Isolated ordnance tests verify 7 releases, 6 impacts, real damage, invulnerability, burst/reload timing and repeated-order safety. **Those fixture impacts are not proof of completed mission bombing.**

The next real boundary is `Gambit_Part_1_Cutscene_Bombed pc 173: AttackTarget: AttackTarget ground weapons not configured`: anti-air troops `550035415`/`550035416` need missile/aircraft-contact support. The route test verifies whole-tick rollback, including aircraft positions/headings. Full ambush, staff recovery, actual recruitment, later objectives and mission victory/defeat remain unfinished.

**311 viewer tests passed**, including private assets; **185 overlapping non-viewer library tests passed**. Formatting, Clippy and build passed. Reviewed GPU captures `local/analysis/bomber-cinematic-prefix-02.png` and `bomber-combat-regression-02.png`; clean logs. See `docs/BOMBER_RUNTIME.md` for exact evidence and declared Rust policies, including guided descent, uniform STONE terrain impacts and procedural projectile presentation. General clearance searching, water/aircraft physics fidelity and full cinematic/animation/hardware acceptance remain open.

Next: implement the original defenders' anti-air weapons, then continue the unmodified ambush and staff/squad progression. Existing user previews and launchers were left alone; reopen Preview First Mission Runtime.command for the rebuilt version.

## Earlier checkpoint: next-objective handoff and ambush boundary verified

The Teleport boundary below is historical and superseded by the latest checkpoint above.

Completed barrel guidance now disappears and the next objective supplies its authored camp marker through `mParent`. This is an explicit Rust presentation policy: a visible parent enables its marker, and a completed parent hides it. Parent references are validated. The viewed post-combat GPU capture shows the original “Well done, Commander!” message and next-goal distance, rather than directing the player back to destroyed barrels.

Walking onward through actual input triggers the original bomber ambush **before squad recruitment**. The scene correctly freezes the player, but remains blocked at `Gambit_Part_1_Cutscene_Bombed pc 93: Teleport: Unsupported gameplay call/signature: Teleport`. A continuation test asserts that exact boundary and whole-tick rollback. **This is not completed cinematic or squad acceptance.** No scene was skipped and no script progress was forced.

310 viewer all-target tests passed including private assets. Final Clippy is clean. GPU replay passed with no ERROR/WARN lines; capture reviewed: `local/analysis/next-objective-guidance-01.png`. Logs: `ambush-boundary-tests-02.log`, `ambush-boundary-clippy-02.log`, `ambush-boundary-verified.log` under `local/analysis/`.

Next required work is authored bomber deployment and attack behavior, then staff recovery and subsequent squad progression. Native wrapper inspection confirms Teleport accepts 4–5 arguments, reads planar X/Z and converts heading; the optional fifth argument enters a placement query and still needs its contract resolved. Do not fake that call or bypass the ensuing aircraft AttackTarget.

## Previous: first original combat objective verified

The integration build now completes the original movement, aircraft-look and barrel-destruction tutorials through real input. Gold-star guidance, source-profile rifle firing/reload/damage, nearby rifle return fire, destructible aim assist, authored objective state, and the original pause-tutorial confirmation flow are connected. The pause really stops game time/movement/combat; Enter/Xbox A dismisses it through the original coroutine. Scripted aircraft flyaways and StopAndGuard now have explicit Rust behavior.

The original-data integration test destroys all three tutorial barrels and verifies objective `550032165` complete, without teleporting the player or changing progress globals. Owned windowless input-replay rendering also verifies that completion. This is **the first combat objective, not the complete mission**.

Validation: **309 viewer tests**, including private assets; **185 overlapping non-viewer library tests**; clean all-target Clippy and formatting. Final combat and synthetic keyboard/LMB GPU logs have no ERROR/WARN lines; captures inspected. See `docs/FIRST_COMBAT.md` and `docs/evidence/first-combat-validation.json`.

Use the existing `Preview First Mission Runtime.command`. New controls: LMB/RT fire, RMB/Tab/LT firing assist, Enter/A acknowledge tutorial. The already-running user window was left alone; reopening uses the new build.

Next: original squad/attack objectives and later tutorials, heavy weapons, capture, player replacement, victory/defeat and full-mission acceptance. Animation, hardware audio/controller checks, menus/profiles and other platforms remain unfinished. All earlier checkpoints below are historical.

## Approved implementation direction — 2026-10-09

William explicitly approved using tested Rust-owned collision, movement and effect systems where original engine internals remain unrecovered, while preserving the original mission scripts, objectives, assets and cinematics. Native allocator layout, spatial-grid parity and Ageia internals are no longer prerequisites for the playable rewrite. Keep recovered native kernels as reference evidence, not as mandatory execution machinery.

Replacement behavior must be named, tested and integrated, not hidden behind successful no-op bindings. No skipped scripts/cinematics, fabricated victory, or substitution of Fort Assault for the original mission. Work remains local; no subagents/publication or interference with the existing preview window. Platform/controller/audio acceptance still requires actual testing.

## Earlier playable-first startup checkpoint

William additionally approved prioritizing the original first mission over native parity and cosmetic completeness. Known nonessential graphics/audio gaps now generate explicit warnings; missing gameplay behavior and malformed input still fail transactionally. Menus/profiles/platform verification are deferred. See `docs/PLAYABLE_FIRST.md`.

**New:** `bw-first-mission` / `Preview First Mission Runtime.command` integrates the actual original map with all 97 authored global/unit coroutines. The original opening script now completes, the player is unfrozen and the chase camera activates. A 7,200-tick/120-second simulation has no script error. Movement, aim, source-driven visibility, captions, fades, LightningX, nearby recruitment/following and restart are connected. Blast damage is real, uses source resistance tables, and cascades transactionally; it was not waived as cosmetic. The remaining initial-overlap correction is bounded and recorded.

This is **not full mission playability**: shooting/enemy combat, capture, visible objective guidance, full animation and hardware audio still need integration and the original victory/defeat path remains unaccepted. Validation: all 302 viewer-enabled tests passed with ignored/private-asset tests included; the separate non-viewer library run passed 180 overlapping tests. All-target viewer Clippy and formatting are clean. Windowless movement verification uses synthetic keyboard input through the actual controls adapter, shows nine player model parts and real motion, and has no ERROR/WARN lines. A separate normal-start opening frame also renders without errors/warnings. These are not hardware or complete cinematic acceptance. Evidence: `docs/evidence/playable-first-validation.json`. Existing user preview and primary launchers remain untouched.

Earlier checkpoints below are historical; their staff-effect startup blocker is superseded by this policy and integrated progress.

## Bottom line

**The original first mission completes its opening/tutorials, barrels, camp arrival, bomber ambush and staff-recovery cutscene through genuine player control return on the staff bearer (`550035312`), then stops 325 post-cutscene input frames later at `Unsupported non-rifle projectile 200001224: DAMAGE_FIRE` when flame troops engage past `Trench00`. It is not yet verified finishable.**

The main launcher opens the **static original SP_0.1 restoration preview**, not gameplay. Fort Assault remains separately available through `Play Fort Assault Prototype.command`. Its victory/restart tests do not demonstrate completion of Battalion Wars 2's original mission. There is no verified original victory or complete original menu flow. The separate first-mission build verifies first combat, ambush completion and staff-cutscene completion through control return, not trench flame combat or the entire mission. This is still a major engine/runtime integration project, not final polish; a reliable completion percentage or date is not available.

## Reconnection checkpoint — staff software voice

Verified no interrupted Cargo process remained. Retried the interrupted non-viewer validation: 178 library tests passed. Nine rewrite integration tests and viewer all-target Clippy also passed after reconnecting.

Added `mission_effect_sound::Voice`: source-profile/sample resolution, decoded PCM playhead, explicit pitch variation, Rust linear distance attenuation, stereo sample rendering and expiry. Extended the original staff-sample test to verify non-silent near playback, far-field silence, invalid-delta rollback and complete expiry; it passes, with Clippy clean. This voice is **not yet scheduled by the mission runtime or hardware-verified**. Staff Kill correctly remains blocked; its voice scheduling and spawn visual are next. No first shared tick/playable milestone claimed.

## Lightning, ground scene and lifecycle connected — 2026-10-09

Current approved rewrite now passes the original Art Kill/PlayEffect calls, ground-vehicle waypoint setup and capture-point despawn. It stops at **Gambit_Part_1_Staff_Effect pc 5: Kill: Rewrite effect mBoomSoundBase backend not configured**. Whole-tick rollback remains intact; **no successful shared startup tick or playable mission yet**.

- LightningX has a named Rust camera-local visual, timed fade/repeat, and Bevy mesh/material consumer. Actual offscreen Metal captures verify visible, faded and repeated states. Fixed a GPU slab-allocation error during zero-vertex fade; resident meshes are now hidden and assets released through normal ownership. Clean rerun: `local/analysis/rewrite-lightning-gpu-03*`; failed prior logs preserved.
- Original terrain/model-derived collision now installs 782 static registrations and 30 source-profile-driven kinematic vehicle cylinders. Verified an original vehicle moving in this real scene **in isolation**. This is not Ageia or unmodified mission movement. Five off-terrain placements and 100 loader omissions are explicitly reported. Full-population positive-time placement/movement remains unaccepted; zero-duration startup does not advance physics.
- Rust capture-point spawn/despawn toggles physical presence and clears capture progress/contenders transactionally; actual capture gameplay remains unimplemented. Air/water dynamics are explicitly separate and unimplemented.
- Recovered the embedded sound-bank reader: 365 DSP samples, including non-silent `DE_StaffPower` (59,648 samples at 22,050 Hz). Sound is decoded but not playback-connected; the staff visual `768_Spawn T Medium` also needs a consumer.

189 viewer library + 31 selected integration tests pass; viewer all-target Clippy and formatting clean. See `docs/RUST_OWNED_RUNTIME.md` and `docs/evidence/rewrite-lightning-validation.json`. No launcher/live-preview changes, commits, publishing or subagents.

## Rust-owned destruction connected — 2026-10-09 (earlier checkpoint)

Implemented opt-in `World::configure_rewrite` and `--rust-rewrite` startup auditing. The actual Lua Kill binding now retires supported scenery transactionally: health/presence/activity, movement orders, owned collision registrations and source-driven light-chain creation. Rust-owned effects run at 60 Hz with isolated RNG, bounded live instances, expiry and child creation. Recovered native internals remain available separately; they are not the replacement execution protocol.

The unmodified shared scheduler advances through the three Art kills to **ArtInitialisation pc 19: PlayEffect**. The whole tick still rolls back there; frame zero remains uncommitted. Strict reference mode still stops at Kill pc 3. Neither result is playable mission acceptance. The next concrete dependency is LightningX's camera-anchored scripted visual; its five-argument binding and source resource have been inspected. Implement its visual/lifetime consumer, not a no-op.

186 library + 27 selected integration tests pass, including five rewrite tests. Clippy/all targets and formatting pass. Actual Art light chains remain bounded through ten seconds of simulation; equivalent timestep partitions agree. Tests also verify collision snapshot isolation, failed registration rollback and original whole-tick rollback. Existing preview and launchers were not changed. See `docs/RUST_OWNED_RUNTIME.md`; evidence under `local/analysis/rewrite-*`.

## Original Start.Open completes — 2026-10-09

Recovered controller-error singleton initialization (disabled byte starts at 1), implemented its stateful Lua binding and transactional rollback. Added scoped light-userdata identities and bounded OpenSprite/CloseSprite table bindings from the native routines: cached opens do not reset the shared sprite; close clears only the caller table's pointer; reopen resets raw +0x5c/+0x70 fields.

The **unmodified original Start.Open now completes**, opening revConCrosshair (descriptor 2144913074 → cSprite 2144913068). The subsequent authored A/B event reaches LoadCampaignData. Tests sweep failing instruction budgets and verify sprite/stack rollback; aliases and close/reopen semantics are covered. This is actual script execution, not a no-op sprite binding. It does not render the menu, execute cold Logo/video flow, or implement profiles/RunLevel.

Validated 186 viewer library tests and 27 selected integration tests; formatting and viewer all-target Clippy clean. See `docs/FRONTEND_RUNTIME.md` and `docs/evidence/frontend-sprite-validation.json`. Temporary sprite diagnostics were removed. Prior failed assertions/logs are preserved: the initial assertion targeted the hand cursor rather than the crosshair; original bytecode upvalues and actual descriptor corrected that test.

Overall requested stopping goal is **not yet reached**: original mission still needs live registration construction and complete destruction/effect operations; menus still need the original driver, rendering/input/audio and save/profile integration. No first playable original level or menu acceptance is claimed. Open preview was left untouched.

## Connected destruction query — 2026-10-09

`remove_using_grids` now runs the real typed dispatcher/grid/cell traversal through destruction's neighbor eligibility filter, capacity stop and subsequent cleanup. A `GridBackend` must supply transaction-consistent live sphere/registration snapshots and record-owner resolution; missing data explicitly fails. No owner/neighbor writes happen before the query finishes. Backend failure rolls back the entire operation. The existing iterator-provider route shares the same filtering and cleanup functions.

The connected path is tested with synthetic registrations and a mock volume/instance backend, not a production scene: duplicate writes remain ordered, the 31st eligible candidate stops before a later unresolved record, and every backend-event failure rolls back. Validated 185 viewer library tests and 23 selected integration tests, with clean viewer all-target Clippy. Logs: `local/analysis/spatial-connected-*`.

Shared-startup audit rerun; production Kill is still unsupported. Remaining prerequisites are live registration construction/mutation and concrete explosion/volume operations. Original menus remain a subsequent integration target, not accepted functionality. Preview window was not touched.

## Goal and dispatcher continuation — 2026-10-09

Confirmed goal: original first level playable, then original menus/profile/campaign entry working. No replacement encounter is accepted as that milestone. Current work remains engine integration rather than preview polish.

Implemented the two-grid manager dispatcher in `mission_spatial_grid`: mode bit 1 before bit 2, ignored higher bits, selected-grid availability checks, preserved duplicate/callback order and lazy early stop. Corrected an important distinction from the inner traversal: the manager returns a **decoded normal count**, not a complemented stop marker. It does not inspect the second grid after a first-grid stop. This still needs real registration data and a destruction-backend connection; it does not advance original startup yet.

Validated 183 viewer library tests and 23 selected integration tests (including original frontend stack/startup), with clean viewer all-target Clippy. Logs: `local/analysis/spatial-dispatch-{tests,integration,clippy}.log`. No menu-rendering, profile persistence or gameplay acceptance is newly claimed.

## Gameplay backend continuation — 2026-10-09

Added `mission_spatial_grid`: the recovered three-layer 64x64 grid walk feeding the cell traversal. Preserves layer order, Z-row/X-column order, inclusive index ranges, duplicate callbacks and complemented early-stop counts. Verified DOL constants: cell size 64, origin -2048, reciprocal 1/64. Uses signed-16 quantization with floor correction; saturation/nonfinite cases explicitly reject. Missing sparse cells mean **known empty**, not unknown registrations.

Viewer library tests and selected cell/startup integrations passed, along with viewer all-target Clippy. Logs: `local/analysis/spatial-grid-{tests,integration,clippy}.log`. These are structural fixtures, not acceptance of original live memberships. Still missing the two-grid dispatcher, registration construction/mutation, and full explosion runtime needed for production Kill. No playable original mission yet. The open presentation window was left alone.

## Visible Rust checkpoint — 2026-10-09

William requested a presentable build rather than more isolated recovery modules. The main `Play Battalion Wars Rust.command` now opens **the original SP_0.1 restoration preview**, with three authored-scene viewpoints, keyboard/stick camera movement, zoom/reset and an honest on-screen startup status. Fort Assault is preserved under `Play Fort Assault Prototype.command`; it is no longer the default launcher destination. No original assets were changed.

This remains a static-scene presentation, **not playable original mission gameplay**. Controls move the view, not the soldier. Full script startup still stops at Kill. No new cinematic/gameplay acceptance is claimed. Details, controls and remaining visual limitations: `docs/ORIGINAL_PRESENTATION.md`.

Verified locked viewer build, 179 library tests, 2 synthetic controller-input tests, formatting, all-target viewer Clippy and launcher shell syntax. Native offscreen Metal smoke cycled all three views; rendered landmark/overview captures were visually checked, and a blocked camera angle corrected. Physical gamepad and full keyboard event injection remain unverified. Logs/captures: `local/analysis/presentation-*`.

## Spatial traversal continuation — 2026-10-09

Added the native **cell traversal** layer in `src/mission_spatial_cell.rs`: category masks, ordered quadrants/records, duplicate preservation, expanded XZ bounds, strict sphere intersection, and immediate callback-stop propagation with native complemented counts. Verified paired-single operand decoding rather than trusting Capstone's vector mnemonics. Details and supported arithmetic limits: `docs/NATIVE_SPATIAL_CELL.md`.

Validation: 179 viewer library tests and 21 selected integration tests passed; formatting and viewer all-target Clippy are clean. Logs: `local/analysis/spatial-cell-{tests,viewer-tests,integration-tests,clippy}.log`. These tests do not constitute whole-grid/native-scene acceptance. Outer grid traversal, live registration membership and production destruction/effect integration remain absent, so original shared startup is still blocked at Kill. Dolphin remains exited; no new reference process was launched.

## Previous continuation checkpoint — 2026-10-09

The saved Dolphin session now reports exit **-9**, not a paused live process. Its cause is unknown. No new emulator was launched; all previous captures are preserved.

Recovered and implemented the bounded explosion **setup** chain selection (0x800bb498..0x800bb4f8): draw once, read the selected child's delay, preserve a pre-existing nonzero timer, and select independently again at expiration. Null/equal children still consume the setup draw. Missing/invalid child data or immediate-spawn requests roll back; immediate creation remains explicitly unsupported. This fills a missing setup operation, not the full explosion backend.

Validation: 179 viewer library tests, 18 selected integration tests including original assets, formatting and viewer all-target Clippy passed. The shared-startup audit was rerun: still `ArtInitialisation pc 3: Kill: Unsupported gameplay call/signature: Kill`. Evidence: `docs/evidence/chain-setup-validation.json`. No successful first gameplay tick is claimed. Next remains concrete effect creation, native spatial traversal and production Kill integration; source recovery below is preserved historical context.

## Native-reference investigation (captured session has exited)

William explicitly approved using Dolphin as a local debugging reference, not as the replacement runtime. The isolated reference reached the original title, fresh-save creation, campaign prologue and SP_0.1 intro. This is **not Rust gameplay acceptance**. A full controller profile with explicit virtual IR axes makes pipe-driven inputs work; the earlier minimal profile did not. The precise missing-setting cause is not yet isolated.

Read-only GDB inspection works. JIT breakpoints require debugging mode enabled: the initial sessions could read RAM but did not stop at breakpoints. The current `native-reference-debug-20261008` session uses `-d` and `Interface.DebugModeEnabled=True`; a stop at 0x802c86d4 was independently verified, then that test breakpoint removed. Kill breakpoint 0x8009fb70 remains the target. Do not infer absent native Kill calls from the earlier missed breakpoints.

Private helpers: `local/analysis/native_reference.py` (owned Dolphin/profile/Unix-socket debugger), `native_client.py`, `capture_art_kill.py`. Set `BW_REFERENCE_SESSION=native-reference-debug-20261008` and `PYTHONPATH=local/analysis` when using the client. Ownership details are in that session's `owner.json`; inspect current status rather than trusting an old PID. Current owned tmux session: `bw-native-reference-debug-20261008` (use exact `=NAME:` targets). Earlier null/Metal/UI Dolphin processes exited normally. The current process must be paused or shut down deliberately before leaving it unattended.

Captured the first Art object's complete Kill entry/exit and the second object's entry. Both begin with lifecycle **5**, main flags 0x02000000, zero instance flags and no records/auxiliary/attachments. First Kill changes main flags to 0x02100000, spatial byte to 1, component byte to 1 and lifecycle to 1. Its removal query visits one observed neighbor, whose spatial byte stays zero. See `docs/LIVE_ART_KILL_REFERENCE.md` for evidence, corrected probe labels and limitations.

This exposed and fixed a Rust kernel API gap: explosion backends can now explicitly report the native owner-component activation callback; it applies before DEAD/tail cleanup and rolls back on later failure. Eight targeted tests and default-feature all-target Clippy passed. These are isolated tests, not a production explosion backend or a successful startup tick.

The second object's trace is incomplete: later expected stops were missed for an unresolved reason. The emulator was explicitly interrupted and verified paused at 0x802c86d4. Captures remain intact; inspect status/breakpoint behavior before further tracing. Next: resolve missed stops, finish effect creation/RNG evidence and native spatial traversal, then integrate the real transactional backend. No production Kill binding or successful Rust startup tick has been added.

## What we are building

A native Rust/Bevy reconstruction for macOS, Linux, and Windows. Acceptance is:

**Original startup/title → original menus/profile/campaign flow → actual SP_0.1 mission → authored victory → restart/menu return.**

Required cinematics, objectives, combat, vehicles, animation, audio, defeat, and restart must work.

**Explicit control-design requirement:** modern regular/Xbox-style controller support, with movement on the left stick, aiming/camera on the right stick, triggers/buttons for firing, targeting, commands and transfer. Wii pointing/motion gestures may be deliberately replaced by ordinary controls. Recover gameplay semantics, not the Wii's input-device restrictions. Native reference controls are only research tooling; no Xbox hardware acceptance is claimed. We must not substitute Fort Assault, bypass scripts, skip cinematics, fabricate victory, or pretend unsupported systems work.

The acceptance checklist remains unchecked in [docs/MENU_TO_FIRST_WIN.md](docs/MENU_TO_FIRST_WIN.md).

## What exists today

| Area | What is implemented | What it does not prove |
|---|---|---|
| Asset recovery | Read-only inventory of 7,257 files and selected asset extraction; terrain, models, textures, placements, animation clips, audio, original script data | Complete visual/animation fidelity or permission to redistribute original assets |
| Rendering/game prototype | Original-asset viewers and a playable replacement battle with combat/orders/capture, win/loss/restart | Original campaign gameplay |
| Lua runtime | Transactional execution, closures/upvalues, supported calls/results, yielding, numeric table keys and array writes | Complete Lua compatibility or all native game APIs |
| Mission startup | Original initialization, 40 shared global threads, pause eligibility, game/no-pause clocks and rollback | A successful shared gameplay tick; unit scripts are not attached |
| World state | Health, activity/presence, retained despawn, limited transfer, objectives, controller masks/input snapshots | Complete ownership, automatic replacement, capture lifecycle or combat |
| Infantry movement | Collision/navigation foundations and bounded waypoint following | Complete mission collision or vehicle movement |
| Camera/audio/HUD | Bounded camera paths/fades, voice decoding/radio queue, music transport and HUD request state | Integrated cinematic presentation, HUD rendering or hardware audio acceptance |
| Frontend | Shared complete Global root and Start constructor; XML A/B script callback drives original PageStack to LoadCampaignData | Page opening/rendering, live input, event sounds, full FrontendBW2 execution or profile loading |
| Cinematic backend | Original startup/prologue THP frames and soundtracks decode; Bevy/Metal offscreen playback reaches both final frames | Frontend video API integration, subtitles, hardware audio or menu-to-mission sequencing |
| Vehicles | Profile selection, model hierarchy order, wheel metadata and native box-group selection | Bodies, contacts, forces, suspension or a working vehicle solver |
| Random stream | Native floating table plus independent integer cursor; explicit initialization | Recovered post-menu random continuity |
| Destruction effects | Native light/chain kernels plus transactional destroyable Kill control flow, ordered record cleanup, component flags and ownership-aware attachment release | Concrete resource destruction, effect spawning, spatial/model association, rendering or a working Lua Kill |

Metadata and isolated script tests are foundations, not substitutes for integrated gameplay.

## Exactly where execution is blocked

1. **Shared startup:** `ArtInitialisation pc 3: Kill: Unsupported gameplay call/signature: Kill`.
   - The effects-bearing destruction/cleanup path is not implemented. Simply setting health to zero would not reproduce it.
2. **Original intro:** PC177, `FollowWaypoint: Waypoint movement backend not configured`.
   - Real vehicle dynamics and the complete mission collision environment are missing. An infantry-only or empty-scene fixture is not acceptance.
3. **Capture lifecycle:** `Gambit_Part_1_Gameflow_2 pc 6: Despawn: Capture-point lifecycle backend not configured`.
4. **Player death:** original WinLose reaches `AllPlayerUnitsDead: Player replacement/defeat transition not configured` after lethal damage.
   - Same-army membership is not a recovered ownership/replacement rule.
5. **Integration:** unit-script lifecycle, original combat/orders, full presentation and actual frontend launch still need completion.

Shared startup was rerun and remains blocked as shown. The other entries are the last established isolated runtime frontiers.

## Important discovery immediately before the pause

**SP_0.1 uses the level catalog's Ageia override, not the vehicles' authored ODE setting.**

- Original `local/extracted/files/Data/levels.xml` contains 77 entries.
- SP_0.1 explicitly has `physics="1"`, selecting Ageia and the new-engine profile links.
- Frontend entries use `physics="0"`. That does not establish an operational ODE gameplay backend.
- Missing physics attributes use the recovered native fallback `"2"`, which preserves the authored setting.
- Repeated `unused.*` names are present in the original catalog. Preserve source order; do not silently select one ambiguous entry.
- Example: ground vehicle 550040155 selects profile 50034654 with attack speed 11 under the mission override, rather than old profile 2138048309 with speed 10.

New code: `src/level_catalog.rs`, `tests/level_catalog.rs`. `src/bin/bw-mission-audit.rs` now loads the original catalog to select vehicle profiles. This changes audit/profile data, **not the live gameplay backend**.

A previous default-setting claim used an unrelated constructor field at the same offset: `0x8001f970` writes a field identified as `mBloomBlurFilter`, not proof of the physics default. Do not revive that assumption. Native catalog parsing at `0x80189844..74`, the level-entry constructor store at `0x80186d4c`, and startup consumption at `0x8003c584..58c` provide the relevant evidence. Private traces remain in `local/analysis/actual-settings-*.txt` and `physics-manager-*.txt`.

`docs/VEHICLE_ASSETS.md` has now been reconciled with the catalog discovery. The current audit selects four new-engine profiles: 50034495, 50034654, 50034658 and 550041986. Old authored-policy tests remain useful as explicit policy tests, not mission defaults.

## Current validation and runtime checkpoint

- **173 viewer library tests, 81 selected integration tests and 15 Python tests passed (269 total).** Formatting and viewer/all-target Clippy passed.
- The catalog's fallback/provenance/count refinements are now validated with original assets.
- Launcher builds and replacement restart regression passed again after the collision lifecycle changes. Bevy remains pinned to 0.19.1. These are not original-mission acceptance.
- Startup movie: all 323 frames decoded and final frame 322 reached in Bevy. Prologue: all 2777 frames decoded and final frame 2776 reached. Captures were visually checked; offscreen runs did not play hardware audio.
- `bw-movie` is a standalone cinematic-backend viewer requiring local FFmpeg, not a menu or mission launcher. Original media stays ignored.
- The initial effects integration build timed out at 180 seconds. After confirming no owned Cargo process remained, a lower-parallelism retry passed; fresh Clippy also passed. Do not count the interrupted log as a successful run.
- The latest shared audit still rolls back frame zero at ArtInitialisation PC3 / Kill.
- `docs/evidence/session-runtime-validation.json` now records current results and retains prior checkpoint counts.
- Hardware controller/audio acceptance and Windows/Linux execution remain unverified.

New modules `mission_effect_light.rs` and `mission_effect_chain.rs` recover the Art light's native update-count decay and delayed child-selection tail. The light lasts ten decrementing updates, then disables on the following update; its duration must not be interpreted as ten seconds. Chaining uses an independent integer random cursor and consumes a draw even for identical/null child choices. These kernels are **not wired into World**. See [docs/MISSION_EFFECTS.md](docs/MISSION_EFFECTS.md) for exact scope and evidence.

The model decoder also retains the authored header sphere previously skipped; original SFXdummyRoot and tank values are tested separately from node/physics bounds. The immediate implementation gap is now the surrounding explosion initialization/state machine, model-light association, child spawning, source/owner lifetime and destruction cleanup. Native model association at `0x80310a4c` has two slots and replaces a weaker candidate only when the incoming score is strictly higher. Its scoring helper uses paired-single distance arithmetic and a reciprocal-square-root approximation; it has been inspected, not ported. Do not mistake the tested kernels for a complete effect backend.

### Explosion lifetime entry progress

`mission_effect_light::Light::begin_effect_update` now handles effect flags +0x1c before normal state/repeat/chain work. Native marking ORs 0x00280000; first subsequent entry disables the light, clears 0x00200000 and retains; second requests concrete destruction. Radius and countdown stay unchanged on both deletion entries. `DestroyRequired` is not a completed destructor. Marking does not itself run state-4 eligibility checks. Original Art light tests now exercise normal decay through the gate with explicit fixture flags. See `docs/MISSION_EFFECTS.md`.

This is not a complete explosion instance or installed Kill backend. State-2 ground/orientation work, wave progression and scale, state-4 conditions, repeat/child spawning, concrete lifetime cleanup and scheduling still require integration/recovery. Shared startup remains frame zero / ArtInitialisation PC3.

### Spatial notification progress

`mission_spatial_removal` ports `0x8023d13c` and query callback `0x8023d654`: known-null model short circuit; cWorldObject filtering; conditional raw flag mask 0x00900000; ordered 30-candidate collection (stop at the 31st eligible, preserve duplicates); volume update before owner byte +0x75; conditional neighbor byte writes afterward. Concrete destroyable virtual +0x78/+0x1a4 predicates are also ported from verified vtable targets. Required profile data cannot default to zero. Failure injection verifies complete rollback.

Follow-up: base +0x78/+0xa8/+0xb0 are now identified as mDeadModelDecayTime / DeadModel pointer / CollideType. Instance +0x12c is mInstanceFlags. Strict original-data bindings confirm Art's no-dead-model subset and false pre-query gate at authored values. Missing live flags are still rejected; do not infer runtime invariance from source XML. The audit exposes `art_spatial_profile` separately from execution.

Still missing: the selected `0x8023172c` operation if its gate is true, native broadphase membership/order, live model flags and instance-state tracking, non-null dead-model binding, actual applicable volume operations and production backend integration. No Kill handler or successful startup tick is claimed. See `docs/NATIVE_SPATIAL_REMOVAL.md`. Do not confuse +0x290/base +0xa8 with the unproven retained-count consumer at +0x28c/base +0x9c.

### Native node-type finding

`Node.authored_kind` was previously discarded. Native recognized tags override it; unmatched ordinary names retain it. `model_node_kind::Catalog` reads the complete RBWE01 DOL table (250 rows); the older 162-row research snapshot was partial and is now labeled accordingly. Exact/longest case-sensitive matching, ties, fallback, mapping failures and unsupported dynamic BONE_ allocation are tested. The shared audit now reports source-model classification separately from execution.

Art's SFXdummyRoot source nodes are ROOT (header/effective 0) and EFFECT_A (header 0, tag-resolved 62). Neither emits a volume update in the specific native spatial dispatcher. This is NOT permission to skip surrounding spatial notifications, live-state validation, lights or cleanup. The retained-count field/getter is identified but its selection consumer is not; do not implement a keep-first-N guess or make that unproven rule an Art prerequisite. Next focus: actual spatial side effects and explosion/light lifetime. See `docs/NATIVE_NODE_KINDS.md`.

### Collision lifecycle progress

`unit_collision::Scene` now registers meshes by live owner/node, validates exact retained-node lists and removes only the requested owner's geometry. Clones share immutable geometry but isolate mutable tables. Tests exercise real movement, overlap and sight queries before/after removal, rejection without mutation, empty node identity, retained/foreign/static geometry and discarded transaction snapshots. The diagnostic viewer now registers object/node batches atomically, with explicit scene-lifetime allocation handles and source-node groups. Source IDs resolve to collision owners; full removal clears the binding and partial retention preserves it. Flat/grouped collision query equivalence is tested. No guessed native retention or new Kill binding was installed. Camera obstruction and native spatial side effects still require integration. See `docs/DESTRUCTION_COLLISION.md`.

### Latest destruction finding

`mission_destruction.rs` ports the concrete cDestroyableObject Kill sequence, requiring real backend implementations rather than no-op callbacks. Native component +0x90 clears an ordered linked list (head at object +0xac); +0x4c sets bit 0 in component byte +0xa0. Neither is a proven model operation. Lifecycle byte +0x130 is a raw state byte (constructor 4, Kill 1), not a bool. Notifications, branch precedence, auxiliary release, owned versus foreign attachments, duplicate kills and rollback are tested. The actual Art base/explosion reference is checked against original data; live occupancy is not invented from authored cover templates.

Concrete record destructors, spatial-removal effects, explosion spawning/presentation and live-state population still need implementation. No production backend is installed and the shared audit remains at frame-zero ArtInitialisation Kill. See `docs/MISSION_EFFECTS.md` for native addresses and ignored private traces.

## What we need to do next, in order

William approved building only engine capabilities needed by the first mission: **correct initialization → playable development launch through authored victory/defeat → connect original menus/profile/prologue → expand to other levels**. Do not rebuild a broad engine first or let menu work block direct mission testing. Report each change by what it unlocks for gameplay, not just test counts.

### 1. Re-establish a trustworthy baseline

- [x] Validate the paused catalog changes, formatting and existing suites.
- [x] Confirm the audit uses SP_0.1's Ageia profile override and still reports dynamics unsupported.
- [x] Update vehicle-selection documentation and validation evidence using actual new results.
- [x] Preserve all existing uncommitted/untracked work; no reset, commit or publication was performed.

This baseline is complete. The next priority is completing the destruction/effects backend so shared mission startup can execute, then advancing the direct mission development launch. Defer further menu work unless the mission genuinely needs it.

### Existing frontend groundwork — integrate after the playable mission

- [x] Execute complete original Global root and Start constructor in a shared VM.
- [x] Execute the authored Start A/B script callback through the original page stack.
- [x] Decode and render original startup/prologue movies with bounded local resources.
- [ ] Complete the remaining frontend roots/initializers, font/string/profile services and UI rendering/input/sound dispatch.
- [ ] Execute the original FrontendBW2 driver. Its cold-start branch selects **Logo**, then StartPre/Start—not directly Start.
- [ ] Connect movie frame/mode state and subtitles to the authored page callbacks; do not bypass Logo or prologue.
- [ ] Connect the actual profile/campaign/RunLevel transition to the mission session.

See `docs/FRONTEND_RUNTIME.md` and `docs/MOVIE_PLAYBACK.md` for precise scope. Start.Open still fails at EnableControllerErrorMessages; no working interactive menu is claimed.

### 2. Make original shared startup execute

- [ ] Recover and implement the required `Kill` effects/cleanup path, starting with ArtInitialisation's actual objects.
- [ ] Preserve transactional behavior; unknown effects/signatures must fail explicitly.
- [ ] Run the unmodified shared globals and address the next genuine blocker exposed by execution.
- [ ] Implement capture-point lifecycle separately rather than treating it as ordinary building despawn.

Success here means committed original startup ticks, not isolated first-resume yields.

### 3. Make the original intro move correctly

- [ ] Use the level-selected new-engine parameters.
- [ ] Finish native body/backend research: mass scaling, inertia, body initialization, contacts, drive/steering, wheel/terrain interaction.
- [ ] Implement the supported dynamics and complete terrain/obstacle/unit collision installation.
- [ ] Advance the unmodified intro beyond PC177 without omitting active vehicles or using an empty collision scene.

The current box/wheel descriptors are not a dynamics implementation. Do not choose an ODE library just because asset enums say ODE, or invent physics values to get motion on screen.

### 4. Complete original mission systems

- [ ] Attach unit scripts and implement their lifecycle.
- [ ] Recover ownership, transfer, automatic replacement and terminal defeat rules.
- [ ] Implement required original combat, weapons, enemies, orders and objective progression.
- [ ] Establish random-stream initialization/continuity for the actual session.
- [ ] Connect live input, camera/fades, HUD, audio and animation to the same mission session.

### 5. Complete the actual player journey

- [ ] Execute original frontend callbacks and required profile/campaign/prologue/loading transitions.
- [ ] Launch the real mission from that flow, not a test fixture or replacement battle.
- [ ] Play through to the authored victory condition.
- [ ] Verify defeat, restart, victory/menu return and repeatability.
- [ ] Perform human/controller/audio acceptance, then Windows/Linux checks separately.

Steps 2–4 will expose additional dependencies. Reprioritize from actual execution evidence, not test-count growth. Do not call a metadata or test slice completion of the game.

## Resume commands

Run from the repository. These commands describe the validation loop; current results and exact log paths are recorded above and in the evidence JSON.

```sh
cd <repo-root>

# First: paused catalog changes
cargo fmt --check
cargo test --locked --lib level_catalog
cargo test --locked --test level_catalog -- --include-ignored

# Broad baseline
cargo test --locked --features viewer --lib
cargo test --locked --features viewer \
  --test mission_startup --test original_assets --test original_mission \
  --test frontend_assets --test frontend_graph --test frontend_stack \
  --test lua_arrays --test mission_history --test mission_controller \
  --test vehicle_assets --test mission_session --test mission_random \
  --test level_catalog --test mission_effect_light --test mission_destruction --test collision_lifecycle --test collision_registration --test model_node_kind --test spatial_profile --test frontend_runtime --test movie -- --include-ignored
.venv/bin/python -m unittest discover -s tests -q
cargo clippy --locked --features viewer --all-targets -- -D warnings

# Observe real runtime frontiers; unsupported calls are expected until implemented
cargo run --locked --features viewer --bin bw-mission-audit -- --shared-startup --fresh-attempt
cargo run --locked --features viewer --bin bw-mission-audit -- --authored-startup --fresh-attempt

# Build and replacement regression: useful, but NOT original-mission acceptance
cargo build --locked --features viewer --bin bw-battle --bin bw-original-viewer --bin bw-play
BW_OFFSCREEN=1 BW_BATTLE_VERIFY=restart target/debug/bw-battle
```

If formatting fails, inspect the differences before applying `cargo fmt`. Keep fresh logs and record which code state they validate; do not overwrite historical evidence and imply it covered newer changes.

## Where to look

- `docs/MENU_TO_FIRST_WIN.md` — actual acceptance checklist.
- `docs/FRONTEND_RUNTIME.md` — shared frontend execution, native API findings and cold-start Logo route.
- `docs/MOVIE_PLAYBACK.md` — real original cinematic playback backend and its limitations.
- `docs/MISSION_SESSION.md` — shared scheduler and runtime boundaries.
- `docs/VEHICLE_ASSETS.md` — vehicle evidence and corrected catalog selection.
- `docs/MISSION_EFFECTS.md` — new effect kernels and remaining Kill integration gaps.
- `docs/MISSION_DESPAWN_RETENTION.md` — retention is not physical presence or ownership.
- `docs/MISSION_INPUT.md` — input and player-lifecycle boundaries.
- `docs/MISSION_RANDOM.md` — native random stream and unresolved continuity.
- `src/mission_world.rs`, `src/mission_program.rs`, `src/mission_vm.rs` — runtime core.
- `src/level_catalog.rs`, `src/vehicle_profile.rs`, `src/vehicle_boxes.rs`, `src/vehicle_wheels.rs` — current vehicle/catalog foundations.
- `local/analysis/` — ignored native disassembly, audit output and test logs.
- `local/extracted/` — ignored original assets; preserve them.

## Working rules for resumption

Work solo unless William changes that instruction. No commits, pushes, publication, original-asset uploads, destructive cleanup, or forced closure of user windows without permission. Much of the repository is untracked/uncommitted; it must not be treated as disposable. Keep proprietary assets and private research under ignored `local/`. No clean-room or redistribution-rights claim is established.

Continue toward the complete acceptance journey once development is resumed; routine test/metadata slices are checkpoints, not reasons to ask for renewed permission. Report genuine blockers honestly. This handoff does not start unattended work.
