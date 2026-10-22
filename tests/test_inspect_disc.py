import importlib.util
from pathlib import Path
import struct
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('inspect_disc', Path(__file__).resolve().parents[1] / 'tools/inspect_disc.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


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


if __name__ == '__main__':
    unittest.main()
