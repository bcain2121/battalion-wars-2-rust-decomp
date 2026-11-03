import gzip
import importlib.util
from pathlib import Path
import struct
import sys
import tempfile
import unittest

TOOLS_DIR = Path(__file__).resolve().parents[1] / 'tools'
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

spec = importlib.util.spec_from_file_location('inspect_disc', TOOLS_DIR / 'inspect_disc.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

inv_spec = importlib.util.spec_from_file_location('inventory_filesystem', TOOLS_DIR / 'inventory_filesystem.py')
inv_module = importlib.util.module_from_spec(inv_spec)
inv_spec.loader.exec_module(inv_module)


class DiscTests(unittest.TestCase):
    def test_mapping_sparse_and_boundary(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'fixture.wbfs'
            block = 1 << 21
            with path.open('wb') as f:
                f.write(b'WBFS' + struct.pack('>I', 8192) + bytes([9, 21, 0, 0, 1]))
                f.seek(512)
                f.write(b'TEST01' + bytes(250))
                f.write(struct.pack('>HH', 1, 0))
                f.seek(block)
                f.write(b'DATA')
                f.seek(2 * block - 2)
                f.write(b'AB')
            disc = module.WBFS(path)
            try:
                self.assertEqual(disc.read(0, 4), b'DATA')
                self.assertEqual(disc.read(block - 2, 4), b'AB\0\0')
                self.assertEqual(disc.read(block, 5), bytes(5))
                with self.assertRaises(ValueError):
                    disc.read(len(disc.mapping) * block, 1)
            finally:
                disc.file.close()

    def test_decompress_gz_tree(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            compound = root / 'Data' / 'CompoundFiles'
            compound.mkdir(parents=True)
            gz_file = compound / 'SP_0.1_Level.xml.gz'
            with gzip.open(gz_file, 'wb') as f:
                f.write(b'<Instances/>')
            count1 = inv_module.decompress_gz_tree(root)
            self.assertEqual(count1, 1)
            self.assertEqual((compound / 'SP_0.1_Level.xml').read_bytes(), b'<Instances/>')
            # Second call is idempotent and skips already-decompressed files
            count2 = inv_module.decompress_gz_tree(root)
            self.assertEqual(count2, 0)

    def test_raw_wii_iso_support(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'fixture.iso'
            with path.open('wb') as f:
                f.write(b'RBWE01' + bytes(0x18 - 6) + bytes.fromhex('5d1c9ea3') + bytes(256 - 0x1c))
                f.write(b'PAYLOAD_BYTES')
            disc = module.WBFS(path)
            try:
                self.assertTrue(disc.is_iso)
                self.assertEqual(disc.header[:6], b'RBWE01')
                self.assertEqual(disc.read(256, 13), b'PAYLOAD_BYTES')
            finally:
                disc.file.close()


if __name__ == '__main__':
    unittest.main()
