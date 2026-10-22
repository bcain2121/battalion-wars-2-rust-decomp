#!/usr/bin/env python3
"""Summarize original mission XML and resource chunks without exporting game content."""
import argparse
from collections import Counter
import gzip
import json
from pathlib import Path
import struct
import xml.etree.ElementTree as ET


def load(path):
    data = path.read_bytes()
    return gzip.decompress(data) if path.suffix == '.gz' else data


def chunks(data):
    offset = 0
    result = []
    while offset < len(data):
        if offset + 8 > len(data):
            raise ValueError('Truncated resource header')
        tag = data[offset:offset + 4].decode('ascii')
        size = struct.unpack_from('<I', data, offset + 4)[0]
        if offset + 8 + size > len(data):
            raise ValueError('Resource chunk exceeds archive')
        result.append(dict(tag=tag, offset=offset, size=size))
        offset += 8 + size
    return result


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('xml', type=Path)
    ap.add_argument('resource', type=Path)
    ap.add_argument('--output', type=Path, required=True)
    args = ap.parse_args()
    objects = ET.fromstring(load(args.xml)).findall('Object')
    ids = [o.get('id') for o in objects]
    counts = Counter(o.get('type') for o in objects)
    refs = []
    known = set(ids)
    for o in objects:
        for field in o:
            if field.tag in ('Pointer', 'Resource'):
                for item in field.findall('Item'):
                    if item.text and item.text != '0' and item.text not in known:
                        refs.append(dict(source=o.get('id'), field=field.get('name'), target=item.text))
    data = load(args.resource)
    archive = chunks(data)
    scripts = []
    for chunk in archive:
        payload = data[chunk['offset'] + 8:chunk['offset'] + 8 + chunk['size']]
        pos = payload.find(b'\x1bLua')
        if pos >= 0:
            scripts.append(dict(chunk_offset=chunk['offset'], tag=chunk['tag'],
                signature_offset_in_payload=pos, header_hex=payload[pos:pos + 16].hex()))
    result = dict(object_count=len(objects), unique_id_count=len(known), types=dict(counts),
        unresolved_references_within_level_only=refs,
        note='References may resolve in preload/global data. Lua signatures alone are not validated bytecode.',
        resource_size=len(data), resource_chunks=archive,
        chunk_types=dict(Counter(c['tag'] for c in archive)), lua_chunks=scripts)
    args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({k: v for k, v in result.items() if k not in ('resource_chunks', 'unresolved_references_within_level_only', 'lua_chunks', 'types')}, indent=2))
    print('Unresolved references (level only):', len(refs))
    print('Chunks containing Lua signature:', len(scripts))
    print('Lua headers:', dict(Counter(s['header_hex'] for s in scripts)))


if __name__ == '__main__':
    main()
