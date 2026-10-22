#!/usr/bin/env python3
"""Read-only WBFS disc/partition inventory. No decryption or extraction."""
import argparse
import json
import struct
from pathlib import Path


class WBFS:
    def __init__(self, path):
        self.path = Path(path)
        self.file = self.path.open('rb')
        h = self.file.read(12)
        if len(h) != 12 or h[:4] != b'WBFS':
            raise ValueError('Not a WBFS image')
        self.host_shift, self.block_shift = h[8:10]
        if not 9 <= self.host_shift <= self.block_shift <= 26:
            raise ValueError('Unsupported sector geometry')
        self.block_size = 1 << self.block_shift
        table = self.file.read((1 << self.host_shift) - 12)
        slots = [i for i, value in enumerate(table) if value]
        if slots != [0]:
            raise ValueError(f'Expected one disc in slot zero, found {slots}')
        self.file.seek(1 << self.host_shift)
        self.header = self.file.read(256)
        # libwbfs addresses a maximum dual-layer Wii disc in 0x8000-byte sectors.
        count = (143432 * 2) >> (self.block_shift - 15)
        raw = self.file.read(count * 2)
        self.mapping = struct.unpack(f'>{count}H', raw)

    def read(self, offset, size):
        if offset < 0 or size < 0 or offset + size > len(self.mapping) * self.block_size:
            raise ValueError('Disc read out of range')
        out = bytearray()
        while size:
            block, within = divmod(offset, self.block_size)
            if block >= len(self.mapping):
                raise ValueError('Disc address exceeds WBFS mapping')
            physical = self.mapping[block]
            amount = min(size, self.block_size - within)
            if physical:
                self.file.seek(physical * self.block_size + within)
                data = self.file.read(amount)
                if len(data) != amount:
                    raise ValueError('Truncated physical block')
                out.extend(data)
            else:
                out.extend(bytes(amount))
            offset += amount
            size -= amount
        return bytes(out)

    def inventory(self):
        partitions = []
        for group in range(4):
            count, address = struct.unpack('>II', self.read(0x40000 + group * 8, 8))
            if count > 128:
                raise ValueError('Invalid partition count')
            for index in range(count):
                offset, kind = struct.unpack('>II', self.read(address * 4 + index * 8, 8))
                offset *= 4
                meta = self.read(offset + 0x2a4, 28)
                tmd_size, tmd, cert_size, cert, h3, data, data_size = struct.unpack('>7I', meta)
                partitions.append(dict(group=group, index=index, type=kind,
                    offset=offset, tmd_size=tmd_size, tmd_offset=tmd * 4,
                    certificate_size=cert_size, certificate_offset=cert * 4,
                    hash_table_offset=h3 * 4, data_offset=data * 4,
                    data_size=data_size * 4))
        return dict(format='WBFS', file_size=self.path.stat().st_size,
            disc_id=self.header[:6].decode('ascii'),
            title=self.header[32:96].split(b'\0')[0].decode('ascii'),
            host_sector_size=1 << self.host_shift, wbfs_block_size=self.block_size,
            mapped_blocks=sum(bool(x) for x in self.mapping), partitions=partitions)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('image', type=Path)
    args = parser.parse_args()
    disc = WBFS(args.image)
    try:
        print(json.dumps(disc.inventory(), indent=2))
    finally:
        disc.file.close()


if __name__ == '__main__':
    main()
