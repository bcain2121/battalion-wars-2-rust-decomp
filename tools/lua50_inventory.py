#!/usr/bin/env python3
"""Bounded static Lua 5.0 chunk parser. Never executes game scripts.
Layout references: lua.org/source/5.0/{lundump.c,lopcodes.h}.html.
This is structural analysis, not a VM verifier or decompiler.
"""
import argparse
from collections import Counter
import json
from pathlib import Path
import struct
from analyze_level import chunks, load

OPS = ('MOVE LOADK LOADBOOL LOADNIL GETUPVAL GETGLOBAL GETTABLE SETGLOBAL '
       'SETUPVAL SETTABLE NEWTABLE SELF ADD SUB MUL DIV POW UNM NOT CONCAT '
       'JMP EQ LT LE TEST CALL TAILCALL RETURN FORLOOP TFORLOOP TFORPREP '
       'SETLIST SETLISTO CLOSE CLOSURE').split()
HEADER = bytes.fromhex('1b4c756150010404040608090908')


class Reader:
    def __init__(self, data):
        self.data = data
        self.pos = 0
        self.prototypes = 0

    def take(self, n):
        if n < 0 or n > len(self.data) - self.pos:
            raise ValueError(f'Truncated chunk at {self.pos}, requested {n}')
        value = self.data[self.pos:self.pos + n]
        self.pos += n
        return value

    def uint(self):
        return struct.unpack('<I', self.take(4))[0]

    def count(self, width=1):
        n = self.uint()
        if n > 1_000_000 or n > (len(self.data) - self.pos) // width:
            raise ValueError('Invalid/excessive vector count')
        return n

    def string(self):
        n = self.uint()
        if not n:
            return None
        raw = self.take(n)
        if raw[-1] != 0:
            raise ValueError('Unterminated Lua string')
        return raw[:-1].decode('utf-8', errors='backslashreplace')

    def proto(self, source=None, depth=0):
        self.prototypes += 1
        if depth > 100 or self.prototypes > 10000:
            raise ValueError('Prototype budget exceeded')
        source = self.string() or source
        line = self.uint()
        nups, params, vararg, stack = self.take(4)
        lines = [self.uint() for _ in range(self.count(4))]
        locals_ = []
        for _ in range(self.count(12)):
            locals_.append(dict(name=self.string(), start=self.uint(), end=self.uint()))
        upvalues = [self.string() for _ in range(self.count(4))]
        if len(upvalues) not in (0, nups):
            raise ValueError('Upvalue debug count mismatch')
        constants = []
        for _ in range(self.count()):
            kind = self.take(1)[0]
            if kind == 0:
                constants.append(None)
            elif kind == 3:
                constants.append(struct.unpack('<d', self.take(8))[0])
            elif kind == 4:
                constants.append(self.string())
            else:
                raise ValueError(f'Unsupported constant kind {kind}')
        children = [self.proto(source, depth + 1) for _ in range(self.count())]
        code = []
        for pc in range(self.count(4)):
            word = self.uint()
            op, a, b, c, bx = word & 63, word >> 24, (word >> 15) & 511, (word >> 6) & 511, (word >> 6) & 262143
            if op >= len(OPS):
                raise ValueError(f'Unknown opcode {op}')
            instruction = dict(pc=pc, op=OPS[op], a=a, b=b, c=c, bx=bx)
            if OPS[op] in ('LOADK', 'GETGLOBAL', 'SETGLOBAL'):
                if bx >= len(constants):
                    raise ValueError('Constant index out of bounds')
                instruction['constant'] = constants[bx]
                if OPS[op] != 'LOADK' and not isinstance(constants[bx], str):
                    raise ValueError('Global key must be a string')
            if OPS[op] == 'CLOSURE' and bx >= len(children):
                raise ValueError('Child prototype index out of bounds')
            code.append(instruction)
        if len(lines) not in (0, len(code)):
            raise ValueError('Line table/code length mismatch')
        return dict(source=source, line=line, parameters=params, vararg=vararg,
                    upvalue_count=nups, stack=stack, lines=lines, locals=locals_,
                    upvalues=upvalues, constants=constants, children=children, code=code)


def parse(data):
    reader = Reader(data)
    if reader.take(len(HEADER)) != HEADER:
        raise ValueError('Unsupported Lua header (expected observed little-endian 5.0 layout)')
    number = struct.unpack('<d', reader.take(8))[0]
    if abs(number - 31415926.535897932) > 0.001:
        raise ValueError('Lua numeric-format sentinel mismatch')
    root = reader.proto()
    if reader.pos != len(data):
        raise ValueError(f'Trailing bytes after prototype: {len(data) - reader.pos}')
    return root


def walk(root):
    yield root
    for child in root['children']:
        yield from walk(child)


def analyze(resource):
    summaries, decoded = [], {}
    for chunk in chunks(resource):
        if chunk['tag'] != 'PRCS':
            continue
        payload = resource[chunk['offset'] + 8:chunk['offset'] + 8 + chunk['size']]
        if len(payload) < 4:
            raise ValueError('Missing script name length')
        size = struct.unpack_from('<I', payload)[0]
        if size > len(payload) - 4:
            raise ValueError('Script name exceeds resource')
        name = payload[4:4 + size].decode('utf-8')
        if name in decoded:
            raise ValueError('Duplicate script resource name')
        root = parse(payload[4 + size:])
        decoded[name] = root
        functions = list(walk(root))
        code = [i for p in functions for i in p['code']]
        summaries.append(dict(name=name, prototype_count=len(functions),
            instruction_count=len(code),
            global_reads=sorted({i['constant'] for i in code if i['op'] == 'GETGLOBAL'}),
            global_writes=sorted({i['constant'] for i in code if i['op'] == 'SETGLOBAL'}),
            opcode_counts=dict(Counter(i['op'] for i in code))))
    reads = sorted({s for x in summaries for s in x['global_reads']})
    writes = sorted({s for x in summaries for s in x['global_writes']})
    return dict(script_count=len(summaries), scripts=summaries,
        total_prototypes=sum(s['prototype_count'] for s in summaries),
        total_instructions=sum(s['instruction_count'] for s in summaries),
        global_reads=reads, global_writes=writes,
        external_global_candidates=sorted(set(reads) - set(writes)),
        caveat='Reads are not necessarily calls. External candidates include builtins, injected data, and dynamically assigned globals. Static parse is not behavioral validation.'), decoded


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('resource', type=Path)
    ap.add_argument('--output', type=Path, required=True)
    ap.add_argument('--local-decoded', type=Path)
    args = ap.parse_args()
    summary, decoded = analyze(load(args.resource))
    args.output.write_text(json.dumps(summary, indent=2) + '\n')
    if args.local_decoded:
        args.local_decoded.parent.mkdir(parents=True, exist_ok=True)
        args.local_decoded.write_text(json.dumps(decoded, indent=2) + '\n')
    print(json.dumps({k: v for k, v in summary.items() if k != 'scripts'}, indent=2))


if __name__ == '__main__':
    main()
