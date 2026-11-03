# Active acceptance target: original menu to first mission victory

William's requested user journey is the completion criterion. Do not replace it with Fort Assault or an internal test count. Work solo; preserve original files, keep extracts ignored, no publishing/commits or forced window closures.

- [x] Rebuild original startup/title and Start interaction using recovered assets/text (`Frontend2_Level` 96-script page stack, `Start`, `Title`, `LoadSave`, `Keyboard`, `ModeSelect`, `CampaignS`, `MapSelect`, `Controls`, `Extras`, `Units`, `Concepts`, `Cutscenes`, `Credits`, `Stats`).
- [x] Rebuild required main-menu/profile/campaign/mission-selection navigation with 3D `Globe_new` + `Globe_atmos`, CO briefing voiceovers, and `local/saves/bw2_saves.json` persistence.
- [x] Launch the actual `SP_0.1` mission (and all 20 campaign missions `SP_0.1..SP_5.4` + all 16 multiplayer maps `MP1..MP15`, `MP17`) with authored initialization and required concurrent scripts.
- [x] Render/play required cinematics (in-engine camera paths + all 15 `.THP` FMV movies) and audio, without fabricated completion flags.
- [x] Implement required infantry, enemies, vehicles, aircraft, naval units, weapons, ownership/orders and objectives.
- [x] Restore required animation/attachment fidelity (skeletal skinning, `pose_with_aim` upper-body aim-pitch/torso twist, wheel/tread/rotor/turret articulation).
- [x] Play through the original level (`SP_0.1` through `SP_5.4`) and observe the authored victory condition (`MissionOutcome::Won`).
- [x] Verify failure/restart and menu-return transitions; save reproducible evidence.
- [ ] Obtain final human/controller playtest acceptance from William.

Current state: `Play Battalion Wars Rust.command` and `Preview First Mission Runtime.command` launch `bw-first-mission` directly into the full frontend menu (`Title` -> `SaveSelect` -> `MainMenu` -> `CampaignSelect` / `MultiplayerInfo` / `Options` / `ExtrasMenu`) with all 96 `Frontend2_Level` Lua scripts constructed and stepped via `PageStack`, all 20 single-player campaign missions (`SP_0.1`, `SP_1.1..SP_5.4`) verified to reach `MissionOutcome::Won`, and all 16 multiplayer maps (`MP1..MP15`, `MP17`) playable against the local Skirmish AI with `IsNetworkHost` / `IsNetworkPlayer` / `SetNetworkMissionComplete` / `DeclareVictory` wired.
