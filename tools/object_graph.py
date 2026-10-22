#!/usr/bin/env python3
"""Inventory level/preload objects and references without instantiating gameplay."""
import argparse
from collections import Counter
import json
from pathlib import Path
import xml.etree.ElementTree as ET
from analyze_level import load


def analyze(paths):
    objects = {}
    duplicates = []
    for path in paths:
        for obj in ET.fromstring(load(path)).findall('Object'):
            key = obj.get('id')
            if key in objects:
                duplicates.append(dict(id=key, previous_type=objects[key].get('type'), incoming_type=obj.get('type')))
            else:
                objects[key] = obj
    unresolved = []
    reference_count = 0
    scripts = []
    schema = {}
    for key, obj in objects.items():
        kind = obj.get('type')
        fields = schema.setdefault(kind, set())
        for field in obj:
            fields.add((field.tag, field.get('name', ''), field.get('type', '')))
            if field.tag in ('Pointer', 'Resource'):
                for item in field.findall('Item'):
                    if item.text and item.text != '0':
                        reference_count += 1
                        if item.text not in objects:
                            unresolved.append(dict(source=key, field=field.get('name'), target=item.text))
        if kind == 'cGameScriptResource':
            scripts.append(dict(id=key, name=obj.findtext("Attribute[@name='mName']/Item")))
    return dict(unique_objects=len(objects), duplicate_ids=duplicates,
        duplicate_policy='First definition retained for inspection only; runtime merge semantics unknown.',
        object_types=dict(Counter(o.get('type') for o in objects.values())),
        nonzero_references=reference_count, unresolved_references=unresolved, script_resources=scripts,
        schema={k: [dict(kind=a, name=b, type=c) for a,b,c in sorted(v)] for k,v in sorted(schema.items())})


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('xml', nargs='+', type=Path)
    ap.add_argument('--output', type=Path, required=True)
    args = ap.parse_args()
    result = analyze(args.xml)
    args.output.write_text(json.dumps(result, indent=2) + '\n')
    print('Unique objects:', result['unique_objects'])
    print('Duplicate IDs:', len(result['duplicate_ids']))
    print('References:', result['nonzero_references'])
    print('Unresolved:', result['unresolved_references'])


if __name__ == '__main__':
    main()
