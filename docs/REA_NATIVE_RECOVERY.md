# REA-assisted native recovery workflow

The REA MCP server (Ghidra 12.1.4 provider) now decompiles the original
RBWE01 `main.dol`. Generated artifacts stay in ignored `local/`.

## One-time setup

```sh
python3 local/tools/dol2elf.py local/extracted/sys/main.dol local/analysis/main.elf
python3 local/tools/dump_lua_bindings.py local/extracted/sys/main.dol 0x804b1fe0 local/analysis/lua-bindings.json
```

- `dol2elf.py` wraps the DOL text/data segments in a big-endian PowerPC ELF32
  (`e_flags = 0`; REA rejects `EF_PPC_EMB`). The DOL `.bss` range overlaps
  `.sdata`/`.sdata2`, so it is split into non-overlapping NOBITS sections.
- Open with REA `open_binary` (`provider_id: "ghidra"`). First query waits for
  Ghidra auto-analysis (~2 minutes); later queries are fast.
- `dump_lua_bindings.py` walks the native Lua registration table
  (`0x804b1a60..0x804b22c0`, 268 `{name, function}` pairs) found from the
  `ForceUnitTransfer` string cross-reference.
- `r2`/`r13` small-data bases are not set in the Ghidra import, so SDA accesses
  appear as `unaff_r2 + off` / `unaff_r13 + off`. Some tail-call stubs have no
  Ghidra function; disassemble them with `.venv/bin/python local/analysis/disasm_range.py`.

## Native facts recovered this way

| Address | Fact | Rust |
|---|---|---|
| `0x800e8258` `GetPlayerUnit` | pushes `*(obj+0xc)` (object ID) via `lua_pushnumber`: script object handles are numbers | `mission_vm` ordered/equality comparisons treat `Value::Object(id)` as `id` |
| `0x800e9ca8` `Kill` | global-script entities clear byte `+0x24` (script stops); units go through `0x8009fb70` | already modelled (`killed_scripts`) |
| `0x800d4888` / `0x800e9ed8` `Spawn`/`Despawn` | pickups branch to `0x800c3c10` / `0x800c3c8c` before reading the optional retain flag | `mission_pickup` |
| `0x800c39cc` | pickup contact rules by `mType` (`+0x1c`), heal `mData` (`+0x20`) via damage vtable `+0x334` | `mission_pickup::effect` |
