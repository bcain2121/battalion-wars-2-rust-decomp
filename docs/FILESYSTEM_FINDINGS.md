# Filesystem and first-mission findings

Date: 2026-10-02. Source: local RBWE01 image; reproducible tools in `tools/`.

## Verified

- 7,257 files totaling 2,567,686,184 bytes of file payload.
- Main DOL: 5,240,032 bytes, two populated text sections, eight populated data sections. Section map and executable SHA-256 are in `evidence/filesystem-inventory.json`.
- 6,272 `.adp`, 497 `.str`, 199 `.gz`, 47 `.xml`, 36 `.pf2`, 15 `.thp`, and 10 `.rso` entries, among other formats. Extensions do not by themselves establish decoding formats.
- Main executable contains `Version 744`, Lua diagnostic strings, reflected type/field names, and physics-related source filenames. These are clues, not a complete engine identification.
- `SP_0.1.xml` explicitly links terrain, preload objects, localized strings, resource archive, and level objects.
- Important portability trap: descriptor references `SP_0.1_Level_Preload.xml.gz`, while the filesystem spells it `SP_0.1_Level_preload.xml.gz`. A native loader must resolve original asset references consistently on case-sensitive Linux filesystems and detect ambiguous folded names.
- First sampled level XML: 6,603 unique object IDs, 87 actual object types. Do not use the XML Hints list as the authoritative count.
- 183 cTroop objects, 30 cGroundVehicle objects and 18 cAirVehicle objects are declared in the level; this does not prove all are simultaneously active.
- Objects expose field names, scalar types, enum labels, resource references, matrices, and stable numeric IDs. Much configuration can be imported, not guessed.
- Only one nonzero Pointer/Resource reference was unresolved within this level file alone. Check preload data before treating it as missing.
- Resource archive expands to 36,674,604 bytes. A little-endian length-prefixed chunk walk consumes it exactly: 1 RXET, 1 DNOS, 116 LDOM, 184 MINA, 54 PRCS, 247 FEQT.
- All 54 PRCS chunks contain a Lua signature with identical first 16 header bytes: `1b4c756150010404040608090908b609`.

## Lua compatibility implication

Compared against the official Lua 5.0 undumper header layout, these bytes identify version 0x50 (5.0), little-endian serialization, 4-byte int/size_t/instruction, 6/8/9/9 instruction field widths, and 8-byte numbers. Full prototypes/opcodes have NOT yet been validated.

Do not assume Lua 5.4 or LuaJIT can load these chunks directly. Candidate approaches are a compatible 5.0 interpreter, a deliberately tested translation step, or a compatibility VM. No choice made yet. The serialized scripts are little-endian despite the Wii CPU's big-endian execution environment.

## Integrity and extraction boundaries

`inventory_filesystem.py` verifies each accessed decrypted cluster's 31 H0 data hashes. This is not full-disc verification and does not verify H1/H2/H3 or signatures. Original WBFS remains read-only. Extracted executable/resources, external tool binaries, and local key material are ignored by Git.

Dolphin 2609 official macOS DMG was downloaded and SHA-256 verified against Homebrew metadata, but the app does not contain DolphinTool. No system-wide installation was performed. Inspection therefore uses our local Python reader and project-scoped cryptography dependency instead.

## Next steps

1. Add a bounded Lua 5.0 prototype parser; enumerate global reads/calls and constants without executing scripts.
2. Match PRCS resource names against cGameScriptResource IDs and script-entity references.
3. Import level/preload into a typed object graph; preserve unknown fields and report unresolved references.
4. Decode terrain plus a single model/texture/animation path; begin Rust parsing and a Bevy viewer once interfaces are established.
5. Map engine script registrations in the DOL and build a behavior compatibility matrix.
