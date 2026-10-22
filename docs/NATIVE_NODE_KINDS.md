# Native node types: source classification, not a Kill backend

## New implementation

`model::Node::authored_kind` preserves the first little-endian u16 in the 56-byte node header, previously discarded. This is not automatically the effective runtime type: recognized name tags override it.

`model_node_kind::Catalog::from_rbwe01_dol` reads the complete static tag table directly from the local original DOL. It validates file-backed section mappings, bounds, the null-name terminator, expected 250-row layout and initial entries. This is structural/version-layout validation, not a whole-file authenticity hash. It does not execute native code or mutate original data.

Resolution follows the recovered routine:

1. Exact case-sensitive match, first matching table row.
2. Otherwise longest case-sensitive substring; equal lengths retain the earlier row.
3. Ordinary unmatched names preserve the authored header kind.
4. Unmatched `BONE_` names require native dynamic allocation and explicitly fail here.
5. The native 10000 sentinel also selects the header kind. Output flag is retained separately.

Native initialization uppercases the **table**, not the input node name. No arbitrary partial catalog can be publicly installed. Small internal tables exist only as explicit unit-test fixtures.

## Important correction to research data

The earlier private `model-node-kind-table.json` contained only **162 rows**. It was adequate for particular recorded marker facts, but NOT for concluding that a name has no match. The complete static table has **250 rows**, terminates at `0x804ecac0`, and is separately preserved in `model-node-kind-table-complete.json`. The old snapshot is now explicitly labeled partial; no old rows were deleted. Production parsing and original-data tests read the DOL rather than trusting either research JSON file.

## Actual startup effect carrier

The original `SFXdummyRoot` source model has two nodes:

| Node | Authored header kind | Resolved kind | Resolution | Dispatch at 0x8023a0b8 |
|---|---:|---:|---|---|
| ROOT | 0 | 0 | Header fallback | Ignored |
| EFFECT_A | 0 | 62 | Exact name tag | Ignored |

That specific native spatial routine handles box paths for kinds 1/31, sphere for 150, capsule for 151, and returns without a volume operation for other kinds. Thus this **source model** has zero nodes requesting those volume operations. `bw-mission-audit` now reports this under `source_node_kinds`, separately from shared-script execution.

This does **not** eliminate the surrounding spatial query, cache/neighbor dirty marking, effect-light association, record cleanup, or lifetime handling. It is not proof that every live model hierarchy is unchanged, or that Art Kill is implemented. The shared audit still rolls back at ArtInitialisation PC3 / Kill.

## Native evidence

- Name resolution: `0x8034858c..0x80348728`.
- Static table: `0x804ec2f0`, 8-byte records (flag byte, unclassified byte, u16 kind, name pointer), null-name termination at `0x804ecac0`.
- Locale table used during initialization maps ASCII `a` to `A` and preserves `A`; strcmp/strstr helpers `0x8025a220` / `0x8025ab5c` are case-sensitive.
- Loader passes resolved kind into metadata constructor: `0x803274fc..0x8032752c`.
- Header read: `0x80354390` swaps four halfwords, then twelve words. Halfword swap helper `0x802f7ec8` confirms header-kind little endian.
- Constructor fallback/override: `0x803293e0..0x803293f8`, metadata +0x150.
- Spatial kind dispatch: `0x8023a0e0..0x8023a140`.

The `mNumLeftBehindCollisionNodes` reflection entry identifies base offset +0x9c; getter `0x800a189c` reads it through object +0xd8. **No actual retained-node selection consumer was established.** Do not reinterpret it as “keep the first N nodes,” conclude it is globally unused, or reuse a vtable index from an unrelated class. Source Art dispatch can be analyzed without inventing that policy.

Private traces live under `local/analysis/destruction-node-kind-*`, `node-header-*-native.txt`, `destruction-retention-*`, and `destruction-art-model.json`. Original payloads remain ignored.

## Next startup work

Follow Art's actual spatial notification and explosion/light lifetime paths. Do not let an unproven general debris policy become a prerequisite for source nodes that this dispatcher ignores. Runtime verification and a complete transactional effect backend are still required before enabling Kill.
