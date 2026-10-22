# Destruction effects: recovered kernels, not a working Kill backend

The unmodified shared startup still fails at **ArtInitialisation PC3 / Kill**. The code in this checkpoint does not register Kill, clear object health, suppress effects, or advance the mission. It supplies verified pieces needed for the actual implementation.

## Why Kill is not a health assignment

Native wrapper `0x800e9ca8` dispatches by target class. The destroyable path calls `0x8009fb70` with notification enabled. That implementation checks the already-destroyed flag, changes a lifecycle byte, handles notification/component records, launches the authored explosion when configured, and performs attachment cleanup. The component calls at +0x88 are NOT established model operations; the concrete vtable recovery below corrects that earlier assumption. The ArtInitialisation targets use explosion 250009268. It requests an orange effect light and a delayed self-chain even though damage is zero and sound/Tequila are absent.

The separate object-light virtual callback being a no-op for inspected target classes does **not** eliminate the effect-light channel. Staff_Effect has additional sound/Tequila requirements and cannot inherit a light-only shortcut.

## Effect-light channel

`src/mission_effect_light.rs` reads only `mEffectLightRadius`, `mEffectLightColour`, and `mEffectLightDuration` from an sExplodeBase. Other effect fields are not executed by this parser. Duplicate/missing fields, malformed colour, negative/nonfinite radius and overflowing native intermediate products fail.

Native evidence (same RBWE01 DOL hash recorded in VEHICLE_ASSETS.md):

- `0x800ba038..0x800ba0b0` supplies profile radius/colour and stores the duration counter.
- `0x8030f864..0x8030f8b4` initializes the light data and sets its enabled byte to one.
- `0x800bf138` tests enabled and nonzero radius.
- `0x800bc49c..0x800bc4fc` updates radius as **(initial radius × remaining counter) / initial counter**, preserving single-precision multiplication then division; it decrements the counter afterward.
- When the counter is already zero on entry, `0x800bf12c` clears enabled without writing radius.

Consequently the Art light (radius 80, raw colour bytes 255/77/0/129, duration 10) has update radii 80,72,...,8, then disables on the following update. Zero radius bypasses the counter update. Zero duration with positive radius disables on the first update. The duration is **native effect-update calls**, not seconds and not an assumed renderer frame rate. No alpha normalization is invented.

The kernel is not installed in the scheduler or renderer. Spatial registration at `0x800ba5cc..0x800ba61c` and callback `0x8030f83c -> 0x80310a4c` remain integration work; a light value alone is not a visible effect.

Further inspection: model association uses two pointers at +0x88/+0x8c. It takes the first empty or matching slot; when full it evaluates both existing scores and the candidate, replacing a weakest slot only on a strictly greater candidate score. `0x8030f6c4` computes a radius-minus-distance score using paired-single arithmetic and a reciprocal-square-root approximation. That calculation and slot lifetime management are not ported. The score receives model **matrix translation at +0x30**, not the header sphere center. Constructor `0x8030fa54..6c` copies the header tuple to +0x40/+0x48; the following identity-matrix initialization writes translation at +0x30/+0x38. Paired-single store words were checked directly rather than trusting Capstone's unrelated vector mnemonics. The GX light setup at `0x8030f768` uses radius × 0.3 and reference brightness 0.45, not guessed 0.5 constants. Its shader/channel integration remains to be established.

## Retained model header sphere

`model::Model::header_sphere` now preserves the four big-endian header floats previously skipped by the decoder. Native loader `0x80327330..4c` reads the 32-byte header, then `0x80327750..70` copies those four floats into model metadata +0/+4/+8/+0xc. They are separate from little-endian node XOBB, render-vertex scaling and vehicle body boxes. Nonfinite components are rejected; the format decoder does not silently fabricate replacement bounds or turn this sphere into a physics body.

Original SFXdummyRoot has center (0,-0.35,0), radius approximately 0.00148492; VUHTNKH has center approximately (0.0144157,2.55844,-0.246145), radius 5.3981595. Tests preserve these source values and byte order. Spatial transformation/query integration remains outstanding. Private traces: `model-header-{loader-native,bounds-consumer}.txt`, `render-model-constructor.txt`.

## Delayed chaining and integer randomness

`src/mission_effect_chain.rs` ports bounded setup selection at `0x800bb498..0x800bb4f8`, plus the delayed-chain tail reached at `0x800bcef0..0x800bcf98`.

Setup consumes one integer draw even for null/equal child references. For a non-null selection, it reads **that selected child's** +0xb4 delay. Zero (including negative zero) requires immediate child creation; this remains explicitly unsupported and rolls back the timer and draw. For a finite nonzero delay, it replaces the instance timer only when that timer equals zero. A null selection preserves the timer. Negative finite timers are retained rather than silently clamped. Profile resolution is a read-only callback, and lookup/validation errors roll back.

The selected child is **not cached**: expiration makes a second independent selection. The original-data integration now checks both setup and expiration draws for the Art self-chain. This does not implement recursive spawning or the full constructor. The first live capture's integer cursor increment is consistent with this setup call; its full dynamic call attribution remains unverified.

The update tail:

1. A nonpositive remaining delay does nothing.
2. Subtract one supplied game delta, using f32 arithmetic.
3. On crossing zero, make one selection. Do not catch up with a loop or create a child implicitly.
4. `0x800ba648..0x800ba6a4` always draws an integer in the native 100-bound range, including when both child pointers are equal or null.
5. The signed-byte probability is sign-extended and then compared **unsigned**. First child is chosen for `draw <= threshold`, not `<`. Negative byte values therefore select the first child in the supported native arithmetic.

The return distinguishes not-due from a due null selection. Parent/source transform resolution, child creation, earlier explosion state-machine branches and pool/lifetime management remain absent. Callers must not treat this tail as the entire explosion update.

`mission_random::IntegerStream` ports `0x8003f834`: the same fixed native floating table, but an **independent** cursor at r13-0x3778, preincrement/wrap, unsigned bound converted to f32, multiplication and truncation to u32 (`0x80251c5c`). A zero bound still consumes a draw. Existing Lua GetRandom keeps its separate floating cursor. Tests supplying integer cursor zero are explicit isolated fixtures, not evidence of post-menu state. No default cursor is installed into World.

## Explosion update-entry gate (ported, not a complete lifecycle)

`Light::begin_effect_update` now follows `0x800bc420..0x800bc500`, using the explosion's flags at +0x1c, not a destroyable's main flags:

- Without bit 0x00080000: execute normal light decay, then return `RunState`.
- With that bit: disable the light without changing its radius or remaining-update counter. If 0x00200000 is also present, clear only that bit and return `RetainWithoutState`.
- On the following entry, the deletion bit remains but the hold bit does not: return `DestroyRequired`. The caller must perform the concrete destructor; the helper does not claim destruction happened.

The state-4 assignment ORs 0x00280000. Thus it leads to **one retained update before the destructor path**, not immediate destruction on the first subsequent update. Non-normal entries skip state, repeat and chain processing. Merely setting the hold bit without the deletion bit does not skip normal updates. `mark_native_deletion` ports only the flag assignment, not the preceding state-4 eligibility checks; marking does not retroactively cancel the remainder of the current update.

Tests verify unrelated flag preservation, no countdown/radius change during deferred removal, direct deletion without a hold, and the hold-bit-only case. The original Art light test now runs normal decay through this gate with an explicit flags fixture. These calls must happen on transaction-staged state and only when the native effect manager would update; scheduling, destructor/resource release and rendering are still unimplemented.

## Additional state-machine findings (remaining bodies not yet ported)

- Setup `0x800ba728..0x800ba858` uses explosion instance r28 and sets state byte +0xc9 to 2. Do not mix later fragment functions' r31 fields with this object just because offsets match.
- Wave-state entry `0x800bc748..0x800bc7a8` initializes +0xac to 1 (or 0.5 for the tested flag) and caches a per-update decrement at +0xb0 using the entry delta and profile wave time. It is not simply a newly recomputed elapsed-time fraction on every update.
- State 3 (`0x800bcb7c`) decrements those counters and separately advances the wave radius with the current delta and native scale factors. Do not guess its scale from the profile radius alone.
- State 4 first waits for repeat timer +0xc4 and chain timer +0xd0 to be nonpositive; its remaining owner/counter callbacks precede the 0x00280000 assignment at `0x800bce08`. Only the assignment and subsequent update-entry gate are ported above. This is not an immediate remove-on-wave-time shortcut.
- Delta helper `0x80096328` reads r13-0x7a10; update scheduling/pause ownership still needs tracing. Constructor `0x800ba268` reads scale r13-0x6e68, whose static initializer is 1.0 at 0x805fa2f8; that alone is not proof it remains unchanged throughout a live session.

## Destroyable control flow now ported, not installed in World

`src/mission_destruction.rs` implements the recovered control flow for the **concrete cDestroyableObject vtable**, with a required transactional backend. No production no-op backend or Lua Kill registration was added. Actual record destructors, spatial removal, explosion creation and auxiliary/attachment releases still require concrete implementations before shared startup can commit.

Important new evidence:

- Constructor `0x8009dd10` stores main vtable `0x804aeb90` and component vtable `0x804aed44` at object +0x88.
- Component virtual +0x90 is `0x8000e154`, adjusting this by +0x1c and calling `0x8004b708`. It clears the singly linked list rooted at object +0xac: cache next at record +0xc, call record virtual +8 with r4=1, then advance the head. It is not a model visibility operation. Record subclasses/destructors are still unresolved.
- Component virtual +0x4c is `0x8000f1d8`: OR 1 into component byte +0x18, which is object +0xa0. Do not substitute hiding/deleting a model or zeroing health.
- Object +0x130 is **not a boolean**: the constructor writes 4; Kill writes 1. The port retains a raw u8 rather than discarding other states. Constructor values do not prove post-load live values.
- Constructor initializes four embedded 0x30-byte slot records at +0x148 through +0x208, nulling each final pointer; runtime count +0x208 starts at zero. Authored cover references alone still do not establish post-load count or occupancy.
- `0x801f9754` invokes its auxiliary stop/release helpers and then clears the caller's pointer. Concrete resource semantics are not replaced by a guessed sound/model action.

The port preserves ordering: notification, optional spatial removal, record cleanup, explosion/defer/component-bit branch, main DEAD bit, auxiliary release, then attachment processing. Explosion flag 2 takes precedence over flag 16; a null explosion does not fall through to another branch. A null base returns after the lifecycle write/optional notification, without clearing records or marking DEAD. An already-DEAD object performs no callbacks. Only attachments owned by this live instance receive a release callback; foreign/unowned local links clear without destroying their resources. Ownership compares runtime allocation handles, not authored object IDs, so a different allocation using the same authored ID does not inherit old attachments. No health assignment is invented.

All state/backend changes stage on deep clones and commit together. Backends must be transaction-local and perform no external I/O; unsupported effects return errors. Tests inject failure at every effect position across all branch combinations, including after the DEAD/component flags were set, and verify complete rollback. A 4096-record safety cap is a replacement bound, not a native limit. Original-data tests verify Art's real flags/explosion and preserve its four non-null cover references without inventing empty runtime occupancy.

Private evidence: `destruction-component-vtable.json`, `destruction-constructor-native.txt`, `destruction-record-list-clear-native.txt`, `destruction-component-flag-native.txt`, `destruction-auxiliary-release-native.txt`, and `destruction-{unlink,collision-unregister}-native.txt` under ignored `local/analysis/`.

## Replacement collision service now available

`unit_collision::Scene` now supports owner/node removal, explicitly retained node identities and isolated scene snapshots. Movement, overlap and LOS tests verify the real queries change after removal without dropping unrelated meshes. This is not the complete native spatial callback, not native leftover-node selection, and not installed in Kill. See [DESTRUCTION_COLLISION.md](DESTRUCTION_COLLISION.md).

## Source node classification narrows Art spatial work

The previously skipped node-header kind is now preserved, and the full 250-entry native tag table is read from the original DOL. Art's source `SFXdummyRoot` resolves ROOT to kind 0 and EFFECT_A to kind 62; neither dispatches a volume operation at `0x8023a0b8`. This does not skip the surrounding spatial notifications or effects. A consumer selecting retained nodes from `mNumLeftBehindCollisionNodes` was not established. Follow the actual Art path rather than inventing a keep-first-N rule. See [NATIVE_NODE_KINDS.md](NATIVE_NODE_KINDS.md).

## Tests and remaining work

Unit tests cover exact decay/off-by-one behavior, disabled/zero cases, parsing errors, clone independence, integer wrapping/large unsigned bounds, inclusive branch selection, one-shot timing, null/self selection and rejection without mutation. `tests/mission_effect_light.rs` checks the actual original light/chain fields with an explicitly supplied integer cursor.

To unblock startup rather than merely test kernels:

- Recover remaining explosion initialization/state transitions, source/owner transform handling and chain spawn behavior.
- Establish actual update scheduling and random-state ownership/continuity.
- Implement spatial/model light association and presentation, plus lifecycle/attachment cleanup. Art's sDestroyBase 250009278 has four non-null cover-point references and `mNumLeftBehindCollisionNodes=1`; do not infer empty cleanup merely from the SFXdummyRoot model name or CollideType=0. These are authored references, not proof of runtime occupancy.
- Supply required effect-resource backends for other Kill targets instead of generalizing the Art subset.
- Wire the complete supported path into transactional World execution; prove unmodified shared startup commits and rendered effects remain present.

Private traces: `explosion-{constructor,update,create}-native.txt`, `kill-{native,destroyable-native}.txt`, `effect-light-{state-helpers,model-registration}.txt`, `effect-chain-random-native.txt`, `random-float-to-unsigned.txt`. No disassembly or original asset payload is copied into public source.
