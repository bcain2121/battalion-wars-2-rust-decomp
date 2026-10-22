# Original intro runtime — partial, not a playable mission

**Later follow-up:** `SCRIPTED_CAMERA_RUNTIME.md` records camera-state integration and the 145-test validation; configured intro probes now reach ClearMessageQueue at instruction 96. Rendering remains unfinished.

Follow-up to `MISSION_STARTUP.md`. **The first original level remains unfinished.** The launcher remains Fort Assault. No hardware audio, cinematic rendering, animation-fidelity restoration or original full-mission playthrough is claimed.

## Implemented and verified

- Original stereo intro/main/outro resources are loaded from `cLevelSettings`, not filename guesses. Left/right naming is validated, paths are bounded to the asset root, stereo sample rates/counts/loop metadata must agree, and repeated resources share allocation. Intro/outro reference the same pair in SP_0.1. Intro has 650,272 samples/channel; main has 3,065,776; both are 24 kHz. Level gain is 0.8.
- `mission_music::Transport` implements the restricted intro `StartMusic(INTRO)`, `StopMusic`, and one-/two-argument `SetMusicVolume` signatures. It produces interleaved normalized stereo PCM during mission ticks. Pause holds its clock. Failed scheduler transactions do not publish PCM or advance playback/fades. Main/outro playback, seek signatures, voice ducking and hardware output remain unsupported/unintegrated.
- The original captured `StartIntro` helper executes without substitutions and generates nonzero stereo PCM through the scheduler. The single-argument music fade default is recovered as 5 seconds. Linear interpolation, immediate zero-duration fades and one-shot intro playback are explicit replacement presentation policies, not proven original mixer parity.
- Authored single-player startup resolves player camera **550030276** and its target **550030791**, with the declared Solar Empire army and live authored state validated. Ambiguous cameras/settings or absent targets fail. Squad membership is deliberately not inferred. `AllPlayerUnitsDead` rejects an unresolved roster rather than treating an empty roster as a defeat.
- Restricted same-army living infantry `ForceUnitTransfer(PLAYER_ONE, unit, true[, boolean])` updates control, clears manual drive and new-player AI movement, and updates the player-camera target/hard-cut revision. Other transfer signatures, vehicles and missing camera state fail explicitly. The presentation consumer is not integrated yet.

## Important distinction: the intro does not actually switch units here

The initial player-camera target is already **Troop.Sstaffbearer03**. Thus the original intro's first transfer is redundant. A test executes original instructions 0–74 and appends a return as an **explicit bounded prefix fixture**. It verifies original music startup, freeze and idempotent self-transfer; it does not establish a complete cinematic or a real unit change.

A separately labelled fixture changes the target to Sgrunt02 to test an actual switch, camera-cut revision and movement cleanup. Unsupported options roll back the earlier music, freeze and captured-cell effects. This altered-target fixture is not original mission playback.

## Current frontier

```sh
cargo run --locked --bin bw-mission-audit -- --authored-startup
```

The configured isolated probes report 22 yielded, 5 returned, and 13 blocked scripts. Intro now reaches **instruction 80, `SetCamera`**, rather than failing inside `StartIntro`. These are cloned probes, not simultaneously running mission scripts.

Next work: scripted cameras and waypoint movement, dialogue lifecycle/ducking and actual audio output, remaining global contracts, complete squad ownership/gameplay, and renderer integration. Do not bypass these with completed-cinematic flags or fabricated queue durations.

## Native research retained locally

- Music: StartMusic `0x800eab08`, SetMusicVolume `0x800eaf44`; r2 initialization gives `0x80608ec0`; default fade at r2−0x7af8 is 5.0. Native minimum fade is approximately 0.01666; the PCM adapter currently uses immediate zero-time fades instead.
- Transfer: `0x800e3100` sets bits 4 and 16 for a true third argument. Camera target forwarding reaches `0x800af0a4` and `0x800b042c`; hard-cut handling is visible at `0x800ae324`. Bit 16 skips an auxiliary notification call; its precise meaning has not been established and is not labelled as rumble.
- Dialogue: `PhoneMessage` converts its fourth numeric argument to integer then multiplies by 10 before forwarding. Queue count `0x801972b0` adds a current-entry indicator to the pending count. **Duration/priority, cancellation and completion semantics remain insufficiently recovered to enable dialogue calls.**

## Validation

**141 tests pass:** 105 viewer-library tests, 8 startup/original-media tests, 4 original-asset tests, 9 prior original-mission tests, 15 Python tests. Formatting, warning-denying Clippy and locked native builds pass. Existing Fort Assault victory → restart → defeat still passes deterministically. See `evidence/intro-runtime-validation.json`.

All edits remain uncommitted. Original audio and disassemblies remain ignored under `local/`; nothing was published.
