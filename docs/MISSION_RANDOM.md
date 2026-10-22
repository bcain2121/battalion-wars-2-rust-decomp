# Native floating random stream

`src/mission_random.rs` implements the recovered RBWE01 floating stream used by `GetRandom`. It does not substitute an arbitrary modern PRNG or silently choose a post-menu cursor.

## Native evidence

- Initialization `0x8003f654`, called with 1024 at `0x8003d048`: fixed seed `0xad789fe6`, wrapping recurrence `s = s*1664525 + 1013904223`, sample `((s >> 4) & 0x7fff)/32768`. The table is filled **backwards**.
- `0x8003f744` advances the unsigned cursor before reading, modulo table length, then multiplies the sample by the supplied float span. The table is at r13-0x3780, length at -0x377c, floating cursor at -0x377a.
- `GetRandom` wrapper `0x800d4fd4` supports 0, 1, 2 or 3 arguments: unit sample, scaled sample, lower-plus-scaled-span, and quantized result respectively. It converts numeric arguments to f32.
- Three-argument mode uses double remainder via `0x800d5124 -> 0x8026128c -> 0x8025db68`, converts back to f32, and subtracts. Quantization is toward zero **relative to zero**, not the lower bound. Thus `(5,9,10)` returns zero, not five.
- Integer helpers have a **separate** cursor at r13-0x3778. `IntegerStream` now implements `0x8003f834`: unsigned bound converted to f32, preincremented table lookup, multiplication and unsigned truncation via `0x80251c5c`. Zero bound consumes a draw. It is independent of Lua's floating cursor and is not installed into World by default. Other integer-helper signatures remain unimplemented.
- Scenery callers invoke cursor selection at `0x800a66c8`, `0x800a6c2c`, `0x800a6d70`. A fresh cursor of zero is not proof of the cursor after menu/level initialization.

Local source traces: `GetRandom-native.txt`, `random-{core-native,table-initialization,remainder-native,fmod-native,call-*}.txt`; original call-site hints in `mission-random-calls.txt` include SolarBattle `(1,3)` and MusicInit `(0,90,10)`.

## Explicit capability and validation

`World.random` defaults to unavailable. Callers must supply `Stream::at_cursor(index)`; indices outside 0..1024 fail. The audit does not invent a cursor. The supported numeric subset requires finite f32 bounds, ordered endpoints and a positive finite optional step; strings, booleans, reversed ranges, overflow and unsupported arities fail without consuming a draw.

Float multiplication/addition/remainder/subtraction remain separate. A zero-width range still consumes a draw. Cursor zero's first two outputs are exactly `0.74212646484375` and `0.711700439453125`; wrapping from 1023 produces table entry zero, `0.44720458984375`.

Unit tests cover table order, wrap, source music quantization, signed/zero-width ranges and invalid-call preservation. Integration tests verify absent-cursor failure, whole-script rollback after a successful random call followed by an unsupported state query, and retry consuming exactly one draw. The separate integer-stream tests also cover wrap, independent cursor state, zero bounds and u32::MAX. [MISSION_EFFECTS.md](MISSION_EFFECTS.md) documents its use by the recovered delayed explosion-chain selector, including unconditional draws for equal/null child pointers. This is runtime/API evidence, not integrated original gameplay, reconstructed scenery or menu-to-mission RNG continuity.
