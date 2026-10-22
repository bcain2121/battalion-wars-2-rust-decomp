#!/usr/bin/env python3
"""Find candidate (name pointer, text pointer) Lua registration pairs in a DOL.
Heuristic evidence only: no function signatures or behavior inferred.
"""
import argparse
import json
from pathlib import Path
import struct


def scan(binary, sections, names):
    def address(offset):
        for section in sections:
            if section['offset'] <= offset < section['offset'] + section['size']:
                return section['address'] + offset - section['offset']
        return None

    text = [s for s in sections if s['kind'] == 'text']
    pointers = {}
    for name in names:
        needle = name.encode() + b'\0'
        start = 0
        while True:
            offset = binary.find(needle, start)
            if offset < 0:
                break
            if offset == 0 or binary[offset - 1] == 0:
                va = address(offset)
                if va is not None:
                    pointers[va] = name
            start = offset + 1
    pairs = []
    for section in sections:
        if section['kind'] != 'data':
            continue
        start, end = section['offset'], section['offset'] + section['size']
        for offset in range(start, end - 7, 4):
            name_ptr, function = struct.unpack_from('>II', binary, offset)
            if name_ptr not in pointers or function % 4:
                continue
            if any(s['address'] <= function < s['address'] + s['size'] for s in text):
                pairs.append(dict(name=pointers[name_ptr], table_address=address(offset),
                                  name_address=name_ptr, function_address=function))
    return pairs


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('dol', type=Path)
    ap.add_argument('filesystem_inventory', type=Path)
    ap.add_argument('lua_inventory', type=Path)
    ap.add_argument('--output', required=True, type=Path)
    args = ap.parse_args()
    sections = json.loads(args.filesystem_inventory.read_text())['dol']['sections']
    names = json.loads(args.lua_inventory.read_text())['external_global_candidates']
    pairs = scan(args.dol.read_bytes(), sections, names)
    result = dict(candidates=pairs, matched_names=len({p['name'] for p in pairs}),
                  unmatched_names=sorted(set(names) - {p['name'] for p in pairs}),
                  caveat='Aligned adjacent data pointers into strings and executable sections; registration semantics require disassembly confirmation.')
    args.output.write_text(json.dumps(result, indent=2) + '\n')
    print('Matched names:', result['matched_names'], 'of', len(names))
    for p in pairs[:12]:
        print(p['name'], hex(p['table_address']), '->', hex(p['function_address']))


if __name__ == '__main__':
    main()
