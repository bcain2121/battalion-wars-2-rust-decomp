# Disc formats and reengineering references

Research date: 2026-10-02. These are project notes, not copied source code. External documentation is evidence to test, not authority over project instructions.

## Read reference

### WiiBrew — Wii disc

URL: https://wiibrew.org/wiki/Wii_disc
Revision link returned by page: https://wiibrew.org/w/index.php?title=Wii_disc&oldid=122714

Relevant documented facts:
- Disc partition groups begin at logical offset 0x40000.
- Partition types distinguish game data (0), update (1), and channel installer (2).
- Partition headers include ticket, TMD, certificates, and offsets for hash/data regions.
- Normal encrypted partition clusters are 0x8000 bytes: 0x400 bytes hash metadata plus 0x7c00 bytes user data.
- User data uses AES-128-CBC; the IV is taken from encrypted hash-block bytes 0x3d0 through 0x3df.
- Decrypted partition structure resembles a GameCube disc; Wii executable offsets require attention to word-versus-byte addressing.

Use: implement a bounded random-access reader and validate the filesystem before interpreting any game structures. The page contains additional physical-disc claims not needed here and not independently validated.

## Additional consulted references

- https://www.lua.org/source/5.0/lundump.c.html — official Lua 5.0.3 undumper; consulted LoadHeader and prototype layout. Sample mission headers match its 5.0 layout; no claim of full bytecode compatibility yet.
- https://raw.githubusercontent.com/dolphin-emu/dolphin/2609/Source/Core/Core/IOS/IOSC.cpp — official tagged platform key definitions; local research copy only, no source copied into the runtime or committed key material.
- Official Dolphin 2609 macOS distribution: https://dl.dolphin-emu.org/releases/2609/dolphin-2609-universal.dmg . SHA-256 verified: b8910a6f8710cbe93b916f5b3867a46e67e49aa8272657d282d4accae359cd6b. Distribution lacks DolphinTool.

## Discovered references — not yet read/validated

- https://delroth.net/posts/reading-wii-discs-python — educational disc reader walkthrough.
- https://wiibrew.org/wiki/DOL — executable container specification.
- https://github.com/NationalSecurityAgency/ghidra — candidate static analysis tool; verify PowerPC import configuration and loader support before use.
- https://github.com/dolphin-emu/dolphin — candidate behavioral reference/debugging environment, NOT the replacement runtime.

## Proposed reverse-engineering method

1. Preserve media and calculate hashes; record image revision and tool versions.
2. Separate container formats, assets, script semantics, and native engine behavior.
3. Follow high-information structures first: filenames, strings, object type names, script bindings, executable entry points.
4. Map native functions with address, evidence, confidence, inputs/outputs, callers, and unresolved behavior. Do not mistake decompiler guesses for recovered original source.
5. Compare controlled scenarios against the original: fixed inputs, frame counts, positions, damage, target selection, mission transitions, and save behavior.
6. Convert findings into small behavioral fixtures for the Rust implementation.
7. Keep compatibility simulation independent of rendering and platform input. Use stable original object IDs rather than exposing transient Bevy entity IDs to scripts.
8. Introduce architectural abstractions only when local evidence supports them.

We do not need to reproduce the original memory layout or Wii graphics API to reproduce observable game behavior. Conversely, merely importing models is not a functioning port.
