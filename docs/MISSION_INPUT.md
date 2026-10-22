# Native digital input consumption

The new `mission_input::Input` consumes explicit frame snapshots for the four digital actions read by SP_0.1: FIRE, SKIP_CUTSCENE, SKIP_CUTSCENE_ALT and GCONTROL_PAUSE_SELECT. Missing/stale snapshots fail rather than becoming fabricated 'no input'. It supports JUST_PRESSED, HELD and JUST_RELEASED. PRESSED_THIS_LEVEL has different native storage and remains unsupported.

`Controller::fresh_player()` is now available explicitly. It initializes supported controls to enabled from recovered native initialization; ordinary Default still leaves masks unknown for capability probes. Authored disable calls override those initial values.

## Native evidence

- ReadControllerState: `0x800ec5c8`. Two or three numeric arguments; absent third argument selects player zero.
- Null player pointer returns false (`0x800ec78c`). `Input::single_player()` explicitly declares player two absent; this is not multiplayer support.
- Supported modes first read the disable byte at player+item+0x16fd. Disabled returns false. Otherwise +0xf44 supplies bit 1 for JUST_PRESSED, bit 0 for HELD, bit 2 for JUST_RELEASED.
- Mode zero reads the separate +0x1670 cumulative-level byte, without that mask test; it is deliberately not treated as HELD.
- Fresh-player initialization `0x800f1d64..0x800f1eb0` zeroes event, cumulative, and disable arrays. The unrolled 17×8 loop and remainder cover 141 control entries. This resolves the former unknown mask-default question for an explicitly fresh player.
- Original constant-table entries `0x804b25e8..0x804b2600`: PRESSED_THIS_LEVEL=0, JUST_PRESSED=1, HELD=2, JUST_RELEASED=3.
- Action values: FIRE=50, SKIP_CUTSCENE=139, SKIP_CUTSCENE_ALT=140, GCONTROL_PAUSE_SELECT=99. Rust still validates symbolic identities instead of accepting arbitrary numeric indices.

Evidence remains under `local/analysis/ReadControllerState-native.txt`, `controller-zero-initialization-native.txt`, `controller-read-constants.json`, and `controller-disable-store-*.txt`.

## Device adapter and transaction boundary

`Controls::mission_script_buttons()` reads Bevy keyboard, mouse and gamepad ButtonInput state. Portable binding policy (not Wii hardware mapping): configured fire/LMB → FIRE; configured start → skip and pause-select; configured quit/pad pause → alternate skip. As with the existing portable controls, alias edge events are OR-combined. Human hardware acceptance and short-event buffering across differing render/fixed-update rates remain unverified.

`Scheduler::tick_with_input()` stages a complete snapshot for the next scheduler frame and commits it with the entire tick. A failure in any script rolls back input publication, both clocks, script effects and thread execution. Reads do not consume edges, so multiple scripts see the same frame state. Same-frame pending samples can be replaced on retry; older samples are rejected.

Software Bevy input is tested through an **unmodified original** Gambit_Part_1_Drained_Weapon thread: the physical API snapshot contains a fire edge, the script's controller mask disables FIRE, and ReadControllerState returns false so the original script yields. This is not a synthetic replacement script or human hardware test. A separate VM test verifies an enabled fire edge and whole-tick rollback/retry.

The audit configures a fresh single-player controller but **does not invent device samples**. Isolated probes that require input consequently report a missing snapshot. The main launcher and complete interactive original-mission loop have not been replaced or accepted.
