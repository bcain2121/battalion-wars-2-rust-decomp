#!/usr/bin/env python3
"""Read a Wii data partition from WBFS; inventory files and optionally extract sys data.
Requires cryptography. Keys and extracted outputs must remain in ignored local/.
"""
import argparse
from collections import Counter
from functools import lru_cache
import hashlib
import json
from pathlib import Path
import struct
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
from inspect_disc import WBFS


def decrypt(key, iv, data):
    ctx = Cipher(algorithms.AES(key), modes.CBC(iv)).decryptor()
    return ctx.update(data) + ctx.finalize()


class Partition:
    def __init__(self, disc, key):
        self.disc = disc
        self.meta = next(p for p in disc.inventory()['partitions'] if p['type'] == 0)
        ticket = disc.read(self.meta['offset'], 0x2a4)
        if ticket[0x1f1] != 0:
            raise ValueError('This reader currently supports common-key index zero only')
        self.key = decrypt(key, ticket[0x1dc:0x1e4] + bytes(8), ticket[0x1bf:0x1cf])
        self.base = self.meta['offset'] + self.meta['data_offset']
        self.size = self.meta['data_size'] // 0x8000 * 0x7c00
        self.checked_clusters = set()

    @lru_cache(maxsize=64)
    def cluster(self, index):
        raw = self.disc.read(self.base + index * 0x8000, 0x8000)
        data = decrypt(self.key, raw[0x3d0:0x3e0], raw[0x400:])
        hashes = decrypt(self.key, bytes(16), raw[:0x400])
        for i in range(31):
            if hashlib.sha1(data[i * 1024:(i + 1) * 1024]).digest() != hashes[i * 20:(i + 1) * 20]:
                raise ValueError(f'Cluster {index}: H0 integrity failure at block {i}')
        self.checked_clusters.add(index)
        return data

    def read(self, offset, size):
        if offset < 0 or size < 0 or offset + size > self.size:
            raise ValueError('Partition read out of range')
        out = bytearray()
        while size:
            index, inside = divmod(offset, 0x7c00)
            n = min(size, 0x7c00 - inside)
            out.extend(self.cluster(index)[inside:inside + n])
            offset += n
            size -= n
        return bytes(out)

    def filesystem(self):
        boot = self.read(0, 0x440)
        if boot[0x18:0x1c] != bytes.fromhex('5d1c9ea3'):
            raise ValueError('Decrypted Wii magic mismatch')
        dol, fst, size = (v * 4 for v in struct.unpack_from('>III', boot, 0x420))
        if size > 64 * 1024 * 1024:
            raise ValueError('Unreasonable filesystem table size')
        raw = self.read(fst, size)
        entries = parse_fst(raw, self.size)
        return dol, raw, entries


def parse_fst(raw, partition_size):
    if len(raw) < 12:
        raise ValueError('Truncated FST')
    root, parent, count = struct.unpack_from('>III', raw)
    if root >> 24 != 1 or parent != 0 or count < 1 or count * 12 > len(raw):
        raise ValueError('Invalid FST root/count')
    strings = raw[count * 12:]
    stack = [(count, '')]
    entries = []
    for i in range(1, count):
        while stack and i >= stack[-1][0]:
            stack.pop()
        if not stack:
            raise ValueError('Entry outside directory tree')
        word, offset, size = struct.unpack_from('>III', raw, i * 12)
        kind, name_index = word >> 24, word & 0xffffff
        if kind not in (0, 1) or name_index >= len(strings):
            raise ValueError('Invalid FST entry')
        end = strings.find(b'\0', name_index)
        if end < 0:
            raise ValueError('Unterminated filename')
        name = strings[name_index:end].decode('utf-8', errors='strict')
        if name in ('', '.', '..') or '/' in name or '\\' in name:
            raise ValueError('Unsafe filename')
        path = stack[-1][1] + name
        if kind:
            if not i < size <= stack[-1][0]:
                raise ValueError('Invalid directory extent')
            stack.append((size, path + '/'))
        else:
            offset *= 4
            if offset + size > partition_size:
                raise ValueError('File exceeds partition')
            entries.append(dict(path=path, offset=offset, size=size))
    return entries


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('image', type=Path)
    ap.add_argument('--key-file', required=True, type=Path)
    ap.add_argument('--output', required=True, type=Path)
    ap.add_argument('--extract-system', type=Path)
    ap.add_argument('--extract-file', action='append', default=[])
    ap.add_argument('--extract-root', type=Path)
    args = ap.parse_args()
    key = args.key_file.read_bytes()
    if len(key) != 16:
        raise ValueError('Expected 16-byte key')
    disc = WBFS(args.image)
    try:
        part = Partition(disc, key)
        dol_offset, fst, files = part.filesystem()
        dh = part.read(dol_offset, 256)
        offsets = struct.unpack_from('>18I', dh, 0)
        addresses = struct.unpack_from('>18I', dh, 0x48)
        sizes = struct.unpack_from('>18I', dh, 0x90)
        sections = [dict(index=i, kind='text' if i < 7 else 'data', offset=o, address=a, size=s)
                    for i, (o, a, s) in enumerate(zip(offsets, addresses, sizes)) if s]
        dol_size = max(s['offset'] + s['size'] for s in sections)
        if dol_size > 64 * 1024 * 1024:
            raise ValueError('Unreasonable executable size')
        binary = part.read(dol_offset, dol_size)
        if args.extract_system:
            args.extract_system.mkdir(parents=True, exist_ok=True)
            for name, data in [('main.dol', binary), ('fst.bin', fst)]:
                with (args.extract_system / name).open('xb') as f:
                    f.write(data)
        for name in args.extract_file:
            if not args.extract_root:
                raise ValueError('--extract-root required')
            entry = next(e for e in files if e['path'] == name)
            dest = args.extract_root / entry['path']
            dest.parent.mkdir(parents=True, exist_ok=True)
            with dest.open('xb') as f:
                f.write(part.read(entry['offset'], entry['size']))
        output = dict(disc_id=disc.header[:6].decode(), file_count=len(files),
            total_file_bytes=sum(e['size'] for e in files),
            extensions=dict(Counter(Path(e['path']).suffix.lower() for e in files)),
            dol=dict(partition_offset=dol_offset, size=dol_size,
                sha256=hashlib.sha256(binary).hexdigest(), sections=sections,
                bss_address=struct.unpack_from('>I', dh, 0xd8)[0],
                bss_size=struct.unpack_from('>I', dh, 0xdc)[0],
                entry_point=struct.unpack_from('>I', dh, 0xe0)[0]),
            h0_checked_clusters=len(part.checked_clusters),
            integrity_scope='H0 for accessed clusters only; not full-disc or H1/H2/H3 verification', files=files)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(output, indent=2) + '\n')
        print(json.dumps({k: v for k, v in output.items() if k != 'files'}, indent=2))
    finally:
        disc.file.close()


if __name__ == '__main__':
    main()
