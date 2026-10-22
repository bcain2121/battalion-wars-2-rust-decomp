# Active acceptance target: original menu to first mission victory

William's requested user journey is the completion criterion. Do not replace it with Fort Assault or an internal test count. Work solo; preserve original files, keep extracts ignored, no publishing/commits or forced window closures.

- [ ] Rebuild original startup/title and Start interaction using recovered assets/text.
- [ ] Rebuild required main-menu/profile/campaign/mission-selection navigation.
- [ ] Launch the actual SP_0.1 mission with authored initialization and required concurrent scripts.
- [ ] Render/play required cinematics and audio, without fabricated completion flags.
- [ ] Implement required infantry, enemies, vehicles, weapons, ownership/orders and objectives.
- [ ] Restore required animation/attachment fidelity.
- [ ] Play through the original level and observe the authored victory condition.
- [ ] Verify failure/restart and menu-return transitions; save reproducible evidence.
- [ ] Obtain human/controller acceptance separately from automation.

Current focus: actual SP_0.1 runtime blockers. [MISSION_SESSION.md](MISSION_SESSION.md) adds shared global scheduling, authored pause eligibility, independent wait clocks and command-HUD requests. The first shared unpaused tick stops transactionally at ArtInitialisation PC3 (Kill); the original pause global runs while the other 39 remain suspended. Original menu graph, page-stack execution and Lua arrays are tested foundations, not a rendered menu. Explicit history and retained-despawn state now let five previously blocked isolated startup probes yield (see [MISSION_DESPAWN_RETENTION.md](MISSION_DESPAWN_RETENTION.md)); these are not completed sequences. [MISSION_RANDOM.md](MISSION_RANDOM.md) adds the native floating random table with an explicitly supplied cursor, not an invented post-menu state; intro navigation, vehicles, lifecycle, roster and presentation remain blockers. See [MISSION_PROFILE_RUNTIME.md](MISSION_PROFILE_RUNTIME.md) for native research and [MISSION_CONTROLLER_RUNTIME.md](MISSION_CONTROLLER_RUNTIME.md) for the latest controller-mask execution/rollback tests. Original Controls_Disabled yields after applying its ten masks. [MISSION_INPUT.md](MISSION_INPUT.md) now validates Bevy digital snapshots consumed through an unmodified original script, native fresh-player defaults and transactional frame sampling; complete live mission-loop and human hardware acceptance remain pending. [VEHICLE_ASSETS.md](VEHICLE_ASSETS.md) resolves original/new-engine profile selection and preserves authored physics-node bounds for all three ground-vehicle models; vehicle dynamics and full-scene movement remain unsupported. Existing launcher still runs the replacement battle. First mission is not playable. There is no unattended worker/process implied by this document.
