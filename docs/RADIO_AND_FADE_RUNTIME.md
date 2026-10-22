# Radio and camera fade/FOV runtime — in progress

Follow-up: `UNIT_WAYPOINT_RUNTIME.md` records actual infantry route/collision integration and 159-test validation. The configured intro still needs its navigation scene installed; vehicles remain unsupported.

This does **not** complete the first mission. No hardware playback or rendered cinematic acceptance is claimed.

## New native evidence

- ClearMessageQueue `0x800d6004` delegates through `0x80196720` to `0x8019b9f8`, clearing both pending entries and active state and stopping the audio stream.
- The queue's `+0x548` value is an active **state**, not a message pointer (values 5 and 7 occur). Count still includes one active entry plus pending entries.
- Enqueue `0x8019b094` obtains a language-specific duration through `0x801044f4`; start `0x8019b280` distinguishes negative, zero and positive durations. STR parsing now retains the raw duration so zero and -1 cannot be conflated during playback.
- Zero-duration playback update `0x801985d4` calls stream-status `0x8041ea7c`, then uses the later of the current clock and start+**4 seconds**. The constant at `0x80600fc0` is 4.0. The stream check compares read/end state and an inactive-stream indicator; it is not an invented timer.
- The fourth PhoneMessage argument is converted to deciticks by the wrapper, but those input ticks are **not used as a guessed audio duration**. They are retained as opaque metadata. Positive/negative-duration, voiceless, looping, alternative-rate and objective-bearing messages remain rejected.
- CameraSetFOV `0x800d34f8` converts both target and its fourth argument from degrees to radians. The fourth argument is **angular speed**, not seconds. WAIT computes absolute FOV delta divided by absolute speed.

## Implemented subset

- Radio queue clear/count/cancellation generation; capacity bounded to 20 pending entries.
- Restricted six-argument PhoneMessage for known assets, PLAYER_ONE, ID_NONE, Solar/Underworld speakers and 24 kHz nonlooping voices with raw duration zero.
- Mono PCM generation, full-clip progression, four-second minimum, sequential queue playback and source subtitle lookup. Scheduler cloning makes output transactional; pause stops its clock. PCM production is offline simulation, not hardware consumption.
- Linear camera fades with WAIT/NO_WAIT, explicit overlap rejection and black-overlay opacity state. Overlay rendering is not integrated.
- Immediate FOV assignment and smooth angular-rate progression; finite/bounded inputs and active-camera restrictions. The renderer remains unintegrated.

Panel entrance/exit transitions are currently instantaneous replacement presentation behavior, not original panel parity. Dialogue music ducking and the hardware mixer remain unfinished. These limitations must be resolved before cinematic fidelity acceptance.

## Original-script test

The **unmodified** original intro runs to its genuine CameraFade WAIT. The test checks half-fade, pause, and rollback when voiced playback is not configured. After enabling original voice PCM, the same thread resumes, emits nonzero original speech, switches authored cameras and configures FOV. It currently blocks at **instruction 177, FollowWaypoint**. No cutscene completion global is invented.

FollowWaypoint native wrapper `0x800e9238` is being traced. Its third/fourth arguments are written into X/Z fields of an order payload. Further tracing: order creation `0x801d93c0` → `0x8015efb4` → target helper `0x80144c40`; target position `0x80144da4` uses the waypoint transform's basis columns and translation when the vector is nonzero. This indicates **waypoint-local X/Z offsets**, not radii. The default separate arrival value loaded from r2−0x7fd4 is 1.0. The intro uses infantry and tanks; movement, waypoint flags, speed/pause, collision and vehicle handling still need implementation/verification rather than accepting calls without effects.

Local trace files include `follow-waypoint-native.txt`, `follow-event-copy.txt` (actually order creation), `follow-order-constructor.txt`, `follow-target-position-helper.txt`, `waypoint-order-dispatch.txt`, and `mission-follow-waypoint-calls.txt`. Capstone misidentifies some Wii paired-single operations as later ISA mnemonics; do not infer their semantics from those printed names alone.

## Validation

152 tests pass: 114 viewer-library, 10 startup, 4 original-assets, 9 original-mission, 15 Python. Formatting and warning-denying Clippy pass. Locked battle/original-viewer/play builds and the offscreen victory → restart → defeat regression pass. Logs: `local/analysis/radio-runtime-{unit-tests,original-tests,python-tests,clippy}.log`. This validation does not replace hardware/render/end-to-end acceptance. Work remains local and uncommitted.
