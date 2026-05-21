#!/usr/bin/env python3
"""Convert dumped RT_BITMAP/DIB resource blobs into viewable BMP files."""

from __future__ import annotations

import argparse
import struct
from pathlib import Path


def u16(data: bytes, off: int) -> int:
    return struct.unpack_from("<H", data, off)[0]


def u32(data: bytes, off: int) -> int:
    return struct.unpack_from("<I", data, off)[0]


def dib_to_bmp(data: bytes) -> bytes | None:
    if len(data) < 40:
        return None
    header_size = u32(data, 0)
    if header_size not in {12, 40, 52, 56, 108, 124}:
        return None
    if header_size == 12:
        width = u16(data, 4)
        height = u16(data, 6)
        bit_count = u16(data, 10)
        compression = 0
        colors_used = 0
    else:
        width = u32(data, 4)
        height = u32(data, 8)
        bit_count = u16(data, 14)
        compression = u32(data, 16)
        colors_used = u32(data, 32) if len(data) >= 36 else 0
    if width == 0 or height == 0 or bit_count not in {1, 4, 8, 16, 24, 32}:
        return None
    if compression not in {0, 3}:
        return None

    palette_entries = 0
    if bit_count <= 8:
        palette_entries = colors_used or (1 << bit_count)
    palette_entry_size = 3 if header_size == 12 else 4
    pixel_offset = 14 + header_size + palette_entries * palette_entry_size
    file_size = 14 + len(data)
    file_header = (
        b"BM"
        + struct.pack("<I", file_size)
        + b"\0\0\0\0"
        + struct.pack("<I", pixel_offset)
    )
    return file_header + data


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("resources", type=Path)
    parser.add_argument("out", type=Path)
    args = parser.parse_args()

    args.out.mkdir(parents=True, exist_ok=True)
    count = 0
    for path in sorted(args.resources.glob("*.bin")):
        if not path.name.startswith("2_"):
            continue
        bmp = dib_to_bmp(path.read_bytes())
        if bmp is None:
            continue
        target = args.out / f"{path.stem}.bmp"
        target.write_bytes(bmp)
        count += 1
    print(f"exported {count} BMP files to {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
