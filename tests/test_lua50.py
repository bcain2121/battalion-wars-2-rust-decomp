import sys
from pathlib import Path
import struct
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from lua50_inventory import HEADER, parse
from map_script_bindings import scan


def uint(n):
    return struct.pack('<I', n)


def fixture(word=27, extra=b''):
    # Source=nil, line=0, no upvalues/parameters, stack=2; empty debug/constants/children.
    proto = uint(0) + uint(0) + bytes([0, 0, 0, 2]) + uint(0) * 5 + uint(1) + uint(word)
    return HEADER + struct.pack('<d', 31415926.535897932) + proto + extra


class LuaTests(unittest.TestCase):
    def test_minimal(self):
        self.assertEqual(parse(fixture())['code'][0]['op'], 'RETURN')

    def test_bad_header(self):
        with self.assertRaises(ValueError):
            parse(b'bad' + fixture())

    def test_truncation(self):
        for end in range(len(fixture())):
            with self.assertRaises(ValueError):
                parse(fixture()[:end])

    def test_trailing(self):
        with self.assertRaises(ValueError):
            parse(fixture(extra=b'x'))

    def test_unknown_opcode(self):
        with self.assertRaises(ValueError):
            parse(fixture(word=63))

    def test_constant_bounds(self):
        with self.assertRaises(ValueError):
            parse(fixture(word=5))

    def test_instruction_field_positions(self):
        instruction = parse(fixture(word=27 | 7 << 24 | 2 << 15 | 3 << 6))['code'][0]
        self.assertEqual((instruction['a'], instruction['b'], instruction['c']), (7, 2, 3))

    def test_binding_pair(self):
        binary = bytearray(96)
        binary[32:37] = b'Test\0'
        struct.pack_into('>II', binary, 48, 0x2000, 0x1000)
        sections = [dict(offset=0, address=0x1000, size=32, kind='text'),
                    dict(offset=32, address=0x2000, size=64, kind='data')]
        result = scan(bytes(binary), sections, ['Test'])
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0]['function_address'], 0x1000)
