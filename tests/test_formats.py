import sys
from pathlib import Path
import struct
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from inventory_filesystem import parse_fst
from analyze_level import chunks


class FormatTests(unittest.TestCase):
    def fst(self, name=b'file\0', offset=10, size=4):
        return struct.pack('>6I', 0x01000000, 0, 2, 0, offset, size) + name

    def test_fst_word_offsets(self):
        self.assertEqual(parse_fst(self.fst(), 100), [{'path': 'file', 'offset': 40, 'size': 4}])

    def test_fst_rejects_path_traversal(self):
        with self.assertRaises(ValueError):
            parse_fst(self.fst(b'../escape\0'), 100)

    def test_fst_rejects_out_of_bounds(self):
        with self.assertRaises(ValueError):
            parse_fst(self.fst(offset=25), 100)

    def test_fst_rejects_missing_terminator(self):
        with self.assertRaises(ValueError):
            parse_fst(self.fst(b'file'), 100)

    def test_chunks(self):
        self.assertEqual(chunks(b'PRCS' + struct.pack('<I', 3) + b'abc'),
                         [{'tag': 'PRCS', 'offset': 0, 'size': 3}])

    def test_truncated_chunks(self):
        for data in [b'PRC', b'PRCS' + struct.pack('<I', 4) + b'a']:
            with self.assertRaises(ValueError):
                chunks(data)
