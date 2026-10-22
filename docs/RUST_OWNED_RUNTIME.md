# Rust-owned runtime — approved 2026-10-09

William approved tested replacement collision, movement and effects where native internals are unrecovered. Original mission scripts, assets, objectives and cinematics remain the target. No success-only bindings, skipped scenes or invented victory.

## Current playable-first extension

See [PLAYABLE_FIRST.md](PLAYABLE_FIRST.md) for the subsequently approved policy and current integrated build. Classified cosmetic gaps can warn instead of aborting; real radial damage, all 97 scripts, movement/recruitment, chase camera, scene visibility and LightningX are connected through `first_mission::Session` and `bw-first-mission`. The original intro now completes in simulation. Full combat/objective/victory acceptance remains outstanding.

The sections below preserve the **earlier strict-rewrite checkpoint**, including its then-current blockers; they are not the latest mission status.

## Earlier connected capabilities

`World::configure_rewrite(graph)` explicitly enables the replacement; default/reference execution stays strict. Gameplay RNG and the separate effect RNG have defined fresh-session cursor-zero initialization, not claimed captured Wii state.

- **Kill:** validates scenery/building profiles, constructs supported effects, retires owned collision, changes health/presence/activity and cancels movement orders transactionally. Repeated retirement is idempotent. All owned geometry is removed; native allocation/destructor and leftover-node protocols are not emulated. Unsupported effect channels still reject the entire operation.
- **Spawn/Despawn:** Rust-owned presence toggles keep static collision geometry for later respawn but exclude absent owners from sweep/overlap and sight queries. Capture-point despawn clears progress/contenders. Clone isolation and failed-registration rollback are tested. This is lifecycle handling, **not implemented capture gameplay**; dead-unit resurrection remains unsupported.
- **Scenery lights:** source light radius/color/duration and positive-delay children run in a bounded 60 Hz replacement lifecycle. Native wave-state and allocator lifetime parity are not asserted. Unsupported damage, sound, particles, rubble and dead models still fail.
- **PlayEffect / LightningX:** the original five-argument call resolves the original resource and creates a named Rust-authored camera-local ribbon effect. Flags 1/3 are supported when requested sound is null. Source timeout is honored; zero timeout means a looping replacement with a soft flash every four seconds. The shape, color and cadence are explicitly Rust-authored, not decoded Tequila behavior. Unknown resources, anchors and channel combinations fail.

The Bevy lightning consumer updates geometry/materials, hides quiet instances, restores repeated flashes and releases instances. It is verified separately offscreen; the complete mission presenter is not connected. A fade test exposed a renderer slab-allocation error when uploading an empty mesh. The consumer now hides the resident mesh instead and lets Bevy release assets after entity ownership ends. The subsequent fade/repeat/cleanup run has no ERROR/WARN logs. Earlier failed logs remain preserved.

## Real scene and kinematic vehicles

`mission_rewrite_scene::configure` builds static collision from original model bind geometry and the extracted terrain, without texture decoding or fabricated empty grids. It uses the existing viewer's node filtering and floor placement policy. Dynamic positions retain their authored height offsets; the existing collision solver adds terrain height.

Current original data produces **782 static registrations and 30 kinematic ground-vehicle profiles**. Vehicle cylinders conservatively enclose source chassis boxes; height includes turret boxes, and waypoint speed uses the selected source profile's patrol speed. This is the existing cylinder sweep/planner applied to ground vehicles, **not Ageia, wheel dynamics, acceleration or steering parity**.

Important limitations:
- The source scene loader reports 100 omitted placements; clusters are not expanded.
- Five floor-bound placements are outside available terrain and are reported, not silently registered as empty: 250009036, 250009037, 550030793, 550035253, 550035279.
- Air/water actors are separated from the ground solver; requesting their movement still fails explicitly.
- Zero-duration startup ticks do not advance rewrite physics. Full-population positive-time motion is **not accepted**: off-terrain active actors and placement/overlap handling remain integration work.
- One original ground vehicle has been tested moving in the real terrain/model collision scene **in isolation**, with other units removed and placement resolved through `find_free`. This is not unmodified mission movement acceptance.
- The scene installer honors already-retired objects. Full rendering/input integration still must honor presence/retirement and coordinate conventions. The existing static preview and launchers are untouched.

## Staff sound recovery

The next effect references `DE_StaffPower`. `sound_samples` now reads DNOS/HFSB/HPSD/DPSD banks and feeds the existing DSP ADPCM decoder. Original SP_0.1 contains 365 samples; the staff sample decodes to 59,648 non-silent mono samples at 22,050 Hz, without a DSP loop flag. WAV construction is tested in memory.

`mission_effect_sound::Voice` now resolves a single-sample sound profile, applies an explicit pitch-variation choice, renders stereo PCM with Rust linear distance attenuation, advances a bounded playhead, and expires at the decoded duration. The original staff profile is tested for audible near-field PCM, silence beyond falloff, invalid-delta rollback and expiry. Native sound flags, weighted multi-sample selection, volume variation and loop control are not emulated.

This is **software voice rendering, not mission-connected or hardware-verified playback**. The staff explosion still needs voice scheduling/commit integration and the `768_Spawn T Medium` visual consumer. The Kill binding remains blocked until those channels actually exist; no silent-success binding was installed.

## Current executable evidence

```sh
cargo test --locked --features viewer --test mission_rewrite -- --include-ignored
cargo run --locked --features viewer --bin bw-mission-audit -- \
  --shared-startup --fresh-attempt --rust-rewrite
cargo run --locked --features viewer --bin bw-lightning-check -- NEW_PRIVATE_OUTPUT.png
```

The unmodified shared startup now passes the Art kills/LightningX, ground waypoint setup and capture-point despawn, then stops at:

`Gambit_Part_1_Staff_Effect pc 5: Kill: Rewrite effect mBoomSoundBase backend not configured`

**The whole tick still rolls back. Frame zero remains uncommitted. No playable original mission or authored victory is claimed.**

Validation: 189 viewer library tests + 31 selected integration tests; formatting and all-target viewer Clippy pass. Offscreen Metal render, faded image, repeated flash and instance cleanup were checked (`rewrite-lightning-gpu-03*`). Private original data, inspection outputs and screenshots remain under ignored `local/`.

Menu/profile flow, full mission presentation, hardware audio/controller acceptance and Windows/Linux execution remain unverified.
