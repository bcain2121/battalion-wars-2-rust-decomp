# First native Rust loader

2026-10-02

## Implemented

- Rust 1.95.0, edition 2024; toolchain pinned.
- roxmltree 0.21.1, Serde and serde_json locked in Cargo.lock.
- Rendering-independent library with ObjectId, Object, Field, ObjectGraph and summary types.
- Reads original uncompressed Instances XML, including level and preload.
- Preserves original numeric identity, arbitrary object/field attributes, unknown field kinds, empty items, and raw field XML for future interpretation.
- Rejects invalid IDs, ID zero definitions, duplicate IDs, DTDs, malformed XML, wrong roots, excessive input/node counts, and malformed references.
- Atomic merge rejects duplicates instead of guessing override order.
- CLI emits deterministic JSON and fails if references remain unresolved.

## Validation

- Five synthetic Rust tests pass.
- cargo clippy --all-targets -- -D warnings passes.
- Rust summary exactly matches Python on unique-object count, all type counts, reference count and unresolved-reference list for SP_0.1 level + preload.
- Real-data result: 6,635 objects; 8,149 nonzero references; zero unresolved.
- Evidence: docs/evidence/first-mission-rust-loader.json.

```sh
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo run --locked --quiet -- local/extracted/files/Data/CompoundFiles/SP_0.1_Level.xml local/extracted/files/Data/CompoundFiles/SP_0.1_Level_preload.xml
```

## Deliberate limits

This is a data loader, not a game runtime or Bevy viewer yet. No original scripts are executed. Gzip input, archive/model decoding, transforms, resource loading and gameplay are not implemented in Rust yet. Reference resolution tests ID existence, not target type compatibility. Parsed fields retain raw data; they are not interpreted as physics or gameplay components. Hints are intentionally not authoritative. Unknown top-level Instances children fail rather than silently disappearing.

Only native macOS execution was tested. Linux and Windows validation remains outstanding. No dependence on macOS APIs is intended, but that is not equivalent to tested portability.

## Dependency research

Context7 consulted /tafia/quick-xml and /razrfalcon/roxmltree. Chose roxmltree for the first bounded document-tree importer rather than a streaming state machine; verified DTD disabling and node-limit API. Bevy is deferred until a displayable asset path is ready, keeping this compatibility data model independent of ECS lifecycle.
