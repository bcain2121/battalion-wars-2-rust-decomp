# Shared global scheduling, pause clocks, and HUD requests

## Runtime changes

`Program::global_scheduler()` requires completed authored initialization and attaches the 40 original global bindings to **one shared VM/host transaction**. This is no longer 40 independent cloned probes. It preserves `m_UpdateWhenPaused`; SP_0.1 has one eligible global, `Gambit_Part_1_Pause_Tutorial`. Unit-script lifecycle is deliberately not guessed or attached by this constructor.

The deterministic existing object-ID scheduling policy remains a replacement policy, not recovered native coroutine ordering. One failing global rolls back the entire tick, including previous globals, world state, thread PCs, frames, and both clocks.

The scheduler now advances a separate no-pause clock on paused ticks. It leaves gameplay time, movement, cameras, audio and radio advancement frozen. Only explicitly eligible scripts resume while paused. An ineligible script using WaitForNoPause does **not** acquire permission to execute during pause; its elapsed wait is noticed when eligible again.

- `WaitFor` sleeps against gameplay time.
- `WaitForNoPause` sleeps against the no-pause clock.
- Both yield until at least the next scheduler frame, including a zero-duration call.
- Camera waits remain on gameplay time.
- Negative/nonfinite durations and overflowing clocks fail transactionally.
- Clocks use caller-supplied simulation deltas and existing f64 arithmetic, not OS wall-clock reads or a claim of bit-exact Wii float timing.

## Native evidence

RBWE01 main.dol SHA-256 remains `9e557a6c9d316de3a3db8e9790b8d8c54ffc9cd1656dee89a90a2d7f964df87f`.

- WaitFor at `0x800d6fc4`: deadline from r13-0x37c4; thread clock selector +0x18 = 0.
- WaitForNoPause at `0x800d7034`: deadline from r13-0x37c0; selector = 1.
- Wake test `0x800d11d0..0x800d1218` chooses the corresponding clock and compares the deadline before resuming.
- GetTime `0x800d4c50` reads the gameplay clock.
- Running-clock stores `0x8003ca5c`/`0x8003ca7c` update both clocks; other update paths at `0x8003cd90`/`0x8003cec4` update only the no-pause clock.

Traces remain ignored under `local/analysis/`: `WaitFor-clock-native.txt`, `WaitForNoPause-clock-native.txt`, `GetTime-clock-native.txt`, `script-wake-clock-native.txt`, `mission-clock-references.txt`, `mission-clocks-running.txt`, and `mission-clock-paused-{a,b}.txt`.

## HUD contract

`mission_hud::Hud` preserves independent **requested** states for COMMANDBAR and COMMANDSTACK, plus function-indicator enable. Unknown initial states remain None. It does not claim that these requests have been rendered or that UI animation has finished. Other widgets, flash timing, optional interpolation arguments and multiplayer remain explicitly unsupported.

Native SetHudState (`0x800d3a60`) reads player/item/state and dispatches at `0x80196b28`; flash state 2 has an additional duration argument and is not silently treated as ON. SetFunctionIndicatorState (`0x800e37cc`) resolves player zero and passes a boolean through `0x80197280` to the actual byte setter at `0x802409a0`. The adapter supports the authored boolean signature, not all Lua truthy coercions.

Original `Gambit_Part_1_Controller` now executes both OFF requests and disables the function indicator, then yields. Instruction-budget failure after the first HUD call rolls everything back.

## Actual integration frontier

The first shared, unpaused original-global tick currently fails at **ArtInitialisation PC3, Kill**. No earlier global or clock changes are committed. The paused path runs the unmodified Pause_Tutorial global over two ticks with gameplay time zero and no-pause time 1.0; the other 39 globals remain unexecuted. This is tested execution, not a completed mission or a skipped ArtInitialisation.

[MISSION_INPUT.md](MISSION_INPUT.md) adds frame-scoped digital sampling, native masks and `tick_with_input` transactional submission. An unchanged original Drained_Weapon script consumes a masked Bevy fire edge in a software-input test. The full interactive mission loop and human controller acceptance are still pending.

The separately configured fresh-player lifecycle now supports the proven not-defeated flag while the authored current player remains live, allowing the unmodified WinLose thread to yield without inventing a squad. Lethal damage, zero health and removal require the still-unsupported replacement/defeat transition; healing alone cannot silently reset that state. See [MISSION_PROFILE_RUNTIME.md](MISSION_PROFILE_RUNTIME.md).

[MISSION_DESPAWN_RETENTION.md](MISSION_DESPAWN_RETENTION.md) preserves the optional flag separately from physical presence; five configured isolated startup probes now yield. Capture-point lifecycle now fails explicitly rather than passing through generic building cleanup. [MISSION_RANDOM.md](MISSION_RANDOM.md) provides the recovered 1024-entry floating random stream with an explicit cursor and transactional rollback. Neither adds a roster, invents the post-menu random cursor, or bypasses the shared Kill blocker.

The runtime still needs effects/destruction, full collision/vehicle dynamics, ownership/death-transfer state, interactive input integration, complete HUD/cinematic/audio presentation and unit-script lifecycle. The original-menu-to-authored-victory checklist remains unchecked.
