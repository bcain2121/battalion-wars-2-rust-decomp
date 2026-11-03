#!/usr/bin/env python3
"""Read a Wii data partition from WBFS; inventory files and optionally extract sys data.
Requires cryptography. Keys and extracted outputs must remain in ignored local/.
"""
import argparse
from collections import Counter
from functools import lru_cache
import gzip
import hashlib
import json
import os
from pathlib import Path
import struct
import sys
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
from inspect_disc import WBFS

EXPECTED_KEY_SHA256 = 'de38aeab4fe0c36d828a47e6fd315100e7ce234d3b00aa25e6ad6f5ff2824af8'
DOLPHIN_IOSC_ANCHOR = bytes.fromhex('f8246c58bae75003')


def resolve_common_key(key_file: Path | None = None) -> bytes:
    candidates = []
    if key_file is not None:
        candidates.append(key_file)
    repo_root = Path(__file__).resolve().parents[1]
    default_local_key = repo_root / 'local' / 'tools' / 'common-key.bin'
    candidates.append(default_local_key)
    for path in candidates:
        if path.is_file():
            raw = path.read_bytes().strip()
            if len(raw) == 32:
                try:
                    raw = bytes.fromhex(raw.decode('ascii'))
                except Exception:
                    pass
            if len(raw) == 16:
                return raw

    env_hex = os.environ.get('BW_WII_COMMON_KEY', '').strip()
    if len(env_hex) == 32:
        raw = bytes.fromhex(env_hex)
        if len(raw) == 16:
            return raw

    for keys_bin in [
        Path.home() / 'Library' / 'Application Support' / 'Dolphin' / 'Wii' / 'keys.bin',
        Path.home() / '.dolphin-emu' / 'Wii' / 'keys.bin',
        repo_root / 'keys.bin',
    ]:
        if keys_bin.is_file() and keys_bin.stat().st_size >= 0x124:
            raw = keys_bin.read_bytes()[0x114:0x124]
            if hashlib.sha256(raw).hexdigest() == EXPECTED_KEY_SHA256:
                default_local_key.parent.mkdir(parents=True, exist_ok=True)
                default_local_key.write_bytes(raw)
                return raw

    for dolphin_bin in [
        Path('/Applications/Dolphin.app/Contents/MacOS/Dolphin'),
        Path.home() / 'Applications' / 'Dolphin.app' / 'Contents' / 'MacOS' / 'Dolphin',
        repo_root / 'local' / 'tools' / 'Dolphin.app' / 'Contents' / 'MacOS' / 'Dolphin',
    ]:
        if dolphin_bin.is_file():
            blob = dolphin_bin.read_bytes()
            pos = blob.find(DOLPHIN_IOSC_ANCHOR)
            while pos != -1:
                start = max(0, pos - 512)
                end = min(len(blob) - 16, pos + 512)
                for i in range(start, end):
                    cand = blob[i:i + 16]
                    if hashlib.sha256(cand).hexdigest() == EXPECTED_KEY_SHA256:
                        default_local_key.parent.mkdir(parents=True, exist_ok=True)
                        default_local_key.write_bytes(cand)
                        return cand
                pos = blob.find(DOLPHIN_IOSC_ANCHOR, pos + 1)

    raise ValueError(
        'Could not locate 16-byte Wii common key. Supply --key-file local/tools/common-key.bin, '
        'install Dolphin.app in /Applications, or set BW_WII_COMMON_KEY.'
    )


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


def decompress_gz_tree(extract_root: Path):
    data_root = extract_root / 'Data'
    target_root = data_root if data_root.is_dir() else extract_root
    count = 0
    for gz_path in sorted(target_root.rglob('*.gz')):
        stem_path = gz_path.with_suffix('')
        if not stem_path.exists():
            with gzip.open(gz_path, 'rb') as src:
                payload = src.read()
            with stem_path.open('xb') as dst:
                dst.write(payload)
            count += 1
    return count


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('image', nargs='?', type=Path)
    ap.add_argument('--key-file', type=Path)
    ap.add_argument('--output', type=Path)
    ap.add_argument('--extract-system', type=Path)
    ap.add_argument('--extract-file', action='append', default=[])
    ap.add_argument('--extract-all', action='store_true')
    ap.add_argument('--decompress-only', action='store_true')
    ap.add_argument('--extract-root', type=Path)
    args = ap.parse_args()
    if args.decompress_only:
        if not args.extract_root:
            raise ValueError('--extract-root required with --decompress-only')
        decompressed = decompress_gz_tree(args.extract_root)
        print(json.dumps({'decompressed_gz_files': decompressed}, indent=2))
        return
    if not args.image:
        raise ValueError('image is required unless --decompress-only is set')
    key = resolve_common_key(args.key_file)
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
                dest = args.extract_system / name
                if not dest.exists():
                    with dest.open('xb') as f:
                        f.write(data)
        if args.extract_all:
            if not args.extract_root:
                raise ValueError('--extract-root required with --extract-all')
            total = len(files)
            for idx, entry in enumerate(files, 1):
                dest = args.extract_root / entry['path']
                if not dest.exists():
                    dest.parent.mkdir(parents=True, exist_ok=True)
                    with dest.open('xb') as f:
                        f.write(part.read(entry['offset'], entry['size']))
                if idx == 1 or idx % 500 == 0 or idx == total:
                    print(f'Extracting disc files: {idx}/{total} ({idx * 100 // total}%)', file=sys.stderr, flush=True)
            print('Decompressing .gz compound and startup archives...', file=sys.stderr, flush=True)
            decompress_gz_tree(args.extract_root)
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
        if args.output:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(json.dumps(output, indent=2) + '\n')
        print(json.dumps({k: v for k, v in output.items() if k != 'files'}, indent=2))
    finally:
        disc.file.close()


if __name__ == '__main__':
    main()
