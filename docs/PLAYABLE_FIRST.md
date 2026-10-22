# Playable-first reverse engineering

William approved prioritizing the original first playable mission over native-engine parity and cosmetic completeness. Menus, profiles and other-platform acceptance are deferred, not abandoned.

## Rules

- Preserve original scripts, objectives and cinematics. No fabricated victories, hand-set progress globals or Fort Assault substitution.
- Use real, tested Rust gameplay systems. Missing gameplay functionality, malformed data and unknown signatures remain errors with whole-tick rollback.
- Explicitly classified cosmetic gaps may produce warnings instead of blocking the mission. This is opt-in; strict reference execution remains available.
- Demonstrate integrated builds and objective progression, not just individual module counts.

## Latest milestone

The first authored combat objective now passes through actual movement, aim, tutorial confirmation and rifle fire. See [FIRST_COMBAT.md](FIRST_COMBAT.md) for current controls, evidence and replacement policies. Camp arrival now advances through the original bomber ambush, staff reveal and return of player control. Anti-air missiles and bomb impacts run from the original orders. The faded two-argument staff-player transfer now works; a subsequent battle waypoint order blocks the still-frozen staff cutscene. See [STAFF_TRANSFER.md](STAFF_TRANSFER.md) for the current 319-test checkpoint. See [AMBUSH_COMPLETION.md](AMBUSH_COMPLETION.md) for the 318-test checkpoint, full-sequence GPU replay and explicit replacement/presentation limits. The startup-build description below is preserved as an earlier checkpoint.

## Earlier integrated build

`cargo run --locked --features viewer --bin bw-first-mission`

Also available through `Preview First Mission Runtime.command`. Existing launchers are unchanged.

This is a **movement/squad integration build, not a completed playable mission**. It loads the original map and models, runs all six initializers and 97 global/unit coroutines, executes the opening scripts, switches to a Rust chase camera, and accepts movement/look/recruitment input. Source presence and retirement control mesh visibility; source camera fades, radio captions and the existing LightningX renderer are connected. R restores the initial session snapshot.

Controls: WASD/left stick move; arrows/right stick look; E/Xbox X calls nearby friendly ground units, subject to original controller enable masks; R restarts; Escape closes. Hardware gamepad acceptance remains unverified. Shooting, capture, objective-marker presentation, full animation and hardware audio are not connected in this build.

## Declared Rust behavior

- All authored unit coroutines start with globals in stable object-ID order. Their own guards control execution, including pre-spawn invulnerability. This is an explicit scheduling policy, not recovered native ordering.
- Blast damage uses authored channel amounts and target resistance tables, linear 3D radial falloff, invulnerability and player-only damage restrictions. Native cover, blast occlusion and impulse parity are not claimed. Damage and resulting deaths commit together.
- Cosmetic omissions: unsupported sound/Tequila/scorch/glow resources, dead-model presentation, and fragment trails whose incidental base cannot burst into another explosion. Extra explosions, tagged effects and unknown potentially damaging channels remain blockers.
- Scripted view-distance/water presentation requests and command-bar reset are recorded; unavailable graphics produce warnings. Command selection really resets.
- `GetActivity` uses a decaying score from actual applied blast damage, rather than a constant success value.
- `IsInRange` uses the same horizontal ground-command plane as movement/recruitment. `PlayerAngleToTarget` uses the current normalized aim and grounded target geometry.
- Recruitment creates ownership and real follow orders. Orders can queue while an actor is unspawned. Dead-unit GoToArea requests return false and are recorded as rejected, not accepted as successful movement. Despawning a corpse is allowed; resurrection is not.
- Initial overlap correction searches only within `min(2 * body radius + 1, 8)` world units and records every correction. Unrelated distant bodies outside terrain no longer poison local collision queries. No arbitrary relocation or bypass of collision.
- Main music supports validated DSP loop points; PCM generation is not hardware playback.
- Scheduler avoids redundant per-thread world clones inside its existing whole-tick transaction. The public per-thread transaction remains intact. Only this package is optimized in development builds; dependency versions are unchanged.

## Acceptance path

- [x] Shared original startup commits with explicit cosmetic warnings.
- [x] All 97 coroutines run for 120 simulated seconds without an error.
- [x] Original opening script completes and releases the player without setting progress globals externally.
- [x] Session test verifies movement after intro, invalid-input rollback and restart.
- [x] Owned offscreen render verifies visible original player geometry and movement.
- [x] Render gold-star guidance and connect source-profile rifle shooting and nearby rifle return fire.
- [x] Complete the original movement/look tutorials and first barrel-destruction combat objective, including the authored pause/confirmation flow.
- [ ] Complete subsequent original squad/attack objectives and heavy weapons.
- [ ] Complete original capture/player replacement/defeat behavior.
- [ ] Play through the authored objectives to original victory or defeat and restart.
- [ ] Validate hardware controls/audio, then return to menus/profiles/platform work.

`BW_MISSION_VERIFY=1` is a diagnostic-only pre-roll: it executes 45 seconds of original scripts before rendering, then synthesizes keyboard input. Normal launches do not pre-roll or skip the opening. It does not certify the complete rendered cinematic or human/device gameplay acceptance.
