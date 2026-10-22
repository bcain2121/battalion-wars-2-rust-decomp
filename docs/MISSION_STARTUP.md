# Original first-mission restoration — in progress

This is **not a completed mission or a new playable release**. The launcher still runs Fort Assault, the replacement encounter documented in `PLAYABLE_BATTLE.md`. The target remains original startup, required gameplay, animation fidelity, and a verified end-to-end original mission. None of those four milestones is declared complete here.

**Follow-up:** [INTRO_RUNTIME.md](INTRO_RUNTIME.md) records the 141-test music/control slice. With explicit authored startup configuration, the intro now reaches `SetCamera`; the baseline probe results below describe the earlier/unconfigured frontier. Hardware audio and cinematic rendering remain unintegrated.

## Implemented foundations

- `mission_program.rs` discovers the original XML bindings: 6 initializers, 40 global scripts, and 57 attached unit scripts. All six initializers execute transactionally. One global binding requests updates while paused. Stable object-ID initialization order is a replacement policy, not recovered engine ordering.
- Captured Lua closures share open/closed upvalue cells correctly across sibling functions, inherited captures, register reuse, returns and yields. Failed slices roll back cells, registers, callers and host effects. The original initializers produce 26 closures and 3 captured cells, including the actual music helpers. Nested failures now identify the helper rather than misleadingly naming only its owner script.
- The 84 candidate engine function names are declared as native values. **Declarations are not implementations**: unsupported calls still raise named errors; there are no permissive native stubs. This also prevents closures from permanently capturing nil for an engine function that later receives a backend.
- `SetActive` separates activity from presence and optionally changes weapon permission. The existing battle respects AI inactivity, weapon inhibition and player freeze. `GetHealthPercent` is available.
- Damageable state now includes 140 buildings, the Iron Tower capture point, and 579 destroyable objects, separate from 231 mobile units. Loading health is not recovery of destruction effects, capture or occupancy behavior.
- Explicit `configure_authored_units` reads `AI_STATE_NORMAL`/`AI_STATE_UNSPAWNED` and instance flag 4 (`AIUNIT_INACTIVE`). All 372 authored AI objects in this level have NORMAL state; 71 have the inactive flag. This distinguishes presence from AI activation. It does not infer player ownership, weapon state or destroyable-object visibility. The default loader remains unresolved until configured.
- Single-argument `Spawn`/`Despawn` support living mobile units and building/capture-point entities; health is retained and despawn clears movement. Dead transitions, optional despawn flags, pickup lifecycle and unsupported types still fail explicitly. Full native lifecycle side-effect parity is not established.
- `localization.rs` parses all 151 original English STR messages, including objective labels. Duration fields are **146 zeros and five -1 sentinels**, not usable dialogue durations.
- `dsp_audio.rs` decodes mono Nintendo DSP ADPCM, retaining loop metadata and exporting PCM WAV. `mission_audio.rs` safely resolves and decodes all **57 unique original voice assets** referenced by this STR. The first voice has 209,832 samples at 24 kHz: **8.743 seconds**. Media loading is not dialogue playback or a completed cinematic. Audio is not yet wired to the playable renderer.

## Current first-resume blockers

`bw-mission-audit` runs independent cloned probes, not a simultaneous mission. Twenty-one global probes yield, five return, and fourteen block at the current frontier. In particular:

- Intro reaches original `StartIntro`, then stops at unsupported `SetMusicVolume`.
- Art/staff effects require `Kill`; gameflow needs profile queries.
- Several scripts require the optional `Despawn` flag; its native message transport was traced but its complete contract is not yet implemented.
- HUD/controller APIs are missing. Player-dependent scripts have no configured player until real transfer/startup is integrated.

More dependencies lie beyond those first blockers: music/dialogue playback, camera trajectories/fades, vehicles, combat/orders, original objectives/events, input tutorials, scripting capabilities, occupancy, destruction, endings and grading. Existing BAN decoding/skin palettes remain experimental. Do not substitute internal probe results or manually changed script globals for full-mission acceptance.

## Reproduce

```sh
cargo run --locked --bin bw-mission-audit
cargo test --locked --lib
cargo test --locked --features viewer --lib
cargo test --locked --features viewer --test mission_startup --test original_assets --test original_mission -- --ignored
cargo clippy --locked --features viewer --all-targets -- -D warnings
```

The additional localization/audio test needs ignored local STR and all 57 voice extracts. `local/analysis/mission-voice-manifest.json` and `mission-voices-extraction.json` record those files. `bw-dsp-audio input.adp [new-output.wav]` never overwrites its WAV destination. The first preview is `local/analysis/mission-first-voice.wav`.

## Research provenance and limitations

- MIT Battalion level-editor reference: commit `c2b074cfac58af6e751c8cb7e9713fa6b7b795e5`; STR layout and BW2 instance-flag labels. License: `references/battalion-editor-MIT.txt`.
- DSP ADPCM equation cross-checked against vgmstream `src/coding/ngc_dsp_decoder.c`; notice in `references/vgmstream-ISC.txt`. FFmpeg THP was examined but uses different rounding; it is **not claimed as bit-exact DSP validation**. Source hashes are recorded separately. No human auditory comparison has been performed.
- Original DOL traces: SetActive 0x800d9c78; Spawn 0x800d4888; Despawn 0x800e9ed8; message handlers 0x8007d310/0x8007d374; local spawn/despawn implementations 0x80078160/0x800784e4. State offset 0x2f8 uses zero for unspawned and one for spawned normal state. Optional despawn flag is transported separately; network/general lifecycle semantics remain incomplete.
- Proprietary extracts, disassemblies and media remain ignored under `local/`. No publication, commits, subagents or user-window termination.
