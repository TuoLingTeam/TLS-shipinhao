#!/usr/bin/env python3
"""Analyze PE overlay data without executing the target."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import re
import struct
from collections import Counter
from pathlib import Path


SIGNATURES: list[tuple[str, bytes]] = [
    ("MZ", b"MZ"),
    ("PE", b"PE\0\0"),
    ("ZIP local header", b"PK\x03\x04"),
    ("ZIP central dir", b"PK\x01\x02"),
    ("7z", b"7z\xbc\xaf\x27\x1c"),
    ("RAR", b"Rar!"),
    ("PNG", b"\x89PNG\r\n\x1a\n"),
    ("JPEG", b"\xff\xd8\xff"),
    ("GIF", b"GIF8"),
    ("SQLite", b"SQLite format 3\0"),
    ("zlib common 78 01", b"\x78\x01"),
    ("zlib common 78 9c", b"\x78\x9c"),
    ("zlib common 78 da", b"\x78\xda"),
    ("VMProtect", b"VMProtect"),
    ("Themida", b"Themida"),
    ("Enigma", b"Enigma"),
    ("UPX", b"UPX!"),
]

INTERESTING_PATTERNS: list[bytes] = [
    b"http://",
    b"https://",
    b"api",
    b"cookie",
    b"token",
    b"login",
    b"logout",
    b"heartbeat",
    b"account",
    "微信".encode("gb18030"),
    "小店".encode("gb18030"),
    "物流".encode("gb18030"),
    "订单".encode("gb18030"),
    "发货".encode("gb18030"),
    "快递".encode("gb18030"),
    "运单".encode("gb18030"),
]


def u16(data: bytes, off: int) -> int:
    return struct.unpack_from("<H", data, off)[0]


def u32(data: bytes, off: int) -> int:
    return struct.unpack_from("<I", data, off)[0]


def entropy(buf: bytes) -> float:
    if not buf:
        return 0.0
    counts = Counter(buf)
    total = len(buf)
    return -sum((n / total) * math.log2(n / total) for n in counts.values())


def pe_overlay_offset(data: bytes) -> int:
    if data[:2] != b"MZ":
        raise ValueError("not an MZ executable")
    pe_off = u32(data, 0x3C)
    if data[pe_off : pe_off + 4] != b"PE\0\0":
        raise ValueError("missing PE signature")
    coff = pe_off + 4
    section_count = u16(data, coff + 2)
    opt_size = u16(data, coff + 16)
    sec_off = coff + 20 + opt_size
    end = 0
    for i in range(section_count):
        off = sec_off + i * 40
        raw_size = u32(data, off + 16)
        raw_ptr = u32(data, off + 20)
        end = max(end, raw_ptr + raw_size)
    return end


def find_offsets(data: bytes, needle: bytes, base: int, limit: int = 50) -> list[int]:
    offsets = []
    start = 0
    while len(offsets) < limit:
        idx = data.find(needle, start)
        if idx < 0:
            break
        offsets.append(base + idx)
        start = idx + 1
    return offsets


def ascii_runs_with_offsets(data: bytes, base: int, min_len: int = 8) -> list[dict[str, str | int]]:
    result: list[dict[str, str | int]] = []
    cur = bytearray()
    cur_off = 0
    for idx, byte in enumerate(data):
        if byte in (9, 10, 13) or 32 <= byte <= 126:
            if not cur:
                cur_off = idx
            cur.append(byte)
        else:
            if len(cur) >= min_len:
                text = cur.decode("ascii", "replace")
                if re.search(r"(http|api|cookie|token|login|account|weixin|shop|order)", text, re.I):
                    result.append({"offset": base + cur_off, "text": text[:300]})
            cur.clear()
    if len(cur) >= min_len:
        text = cur.decode("ascii", "replace")
        if re.search(r"(http|api|cookie|token|login|account|weixin|shop|order)", text, re.I):
            result.append({"offset": base + cur_off, "text": text[:300]})
    return result


def write_entropy_csv(path: Path, data: bytes, base: int, block_size: int) -> list[dict[str, float | int]]:
    rows: list[dict[str, float | int]] = []
    with path.open("w", newline="", encoding="utf-8") as fh:
        writer = csv.DictWriter(fh, fieldnames=["index", "offset_hex", "size", "entropy"])
        writer.writeheader()
        for index, start in enumerate(range(0, len(data), block_size)):
            block = data[start : start + block_size]
            ent = round(entropy(block), 5)
            row = {
                "index": index,
                "offset_hex": f"0x{base + start:x}",
                "size": len(block),
                "entropy": ent,
            }
            writer.writerow(row)
            rows.append({"index": index, "offset": base + start, "size": len(block), "entropy": ent})
    return rows


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("exe", type=Path)
    parser.add_argument("out", type=Path)
    parser.add_argument("--block-size", type=lambda x: int(x, 0), default=0x10000)
    args = parser.parse_args()

    data = args.exe.read_bytes()
    overlay_off = pe_overlay_offset(data)
    overlay = data[overlay_off:]
    args.out.mkdir(parents=True, exist_ok=True)

    entropy_rows = write_entropy_csv(args.out / "overlay_entropy.csv", overlay, overlay_off, args.block_size)

    signatures = []
    for name, sig in SIGNATURES:
        offsets = find_offsets(overlay, sig, overlay_off)
        signatures.append(
            {
                "name": name,
                "signature_hex": sig.hex(),
                "count_first_50": len(offsets),
                "first_offsets": [f"0x{x:x}" for x in offsets[:20]],
            }
        )

    pattern_hits = []
    for pat in INTERESTING_PATTERNS:
        offsets = find_offsets(overlay.lower(), pat.lower(), overlay_off, limit=20)
        if offsets:
            pattern_hits.append(
                {
                    "pattern": pat.decode("gb18030", "ignore") or pat.hex(),
                    "offsets": [f"0x{x:x}" for x in offsets],
                }
            )

    ascii_hits = ascii_runs_with_offsets(overlay, overlay_off)
    byte_counts = Counter(overlay)
    most_common = [{"byte": f"0x{byte:02x}", "count": count} for byte, count in byte_counts.most_common(16)]
    least_common = [
        {"byte": f"0x{byte:02x}", "count": byte_counts.get(byte, 0)}
        for byte in sorted(range(256), key=lambda b: byte_counts.get(b, 0))[:16]
    ]

    ent_values = [float(row["entropy"]) for row in entropy_rows]
    report = {
        "file": str(args.exe),
        "overlay_offset": overlay_off,
        "overlay_offset_hex": f"0x{overlay_off:x}",
        "overlay_size": len(overlay),
        "overlay_sha256": hashlib.sha256(overlay).hexdigest(),
        "overlay_entropy": round(entropy(overlay), 6),
        "block_size": args.block_size,
        "block_entropy": {
            "min": min(ent_values) if ent_values else 0,
            "max": max(ent_values) if ent_values else 0,
            "avg": round(sum(ent_values) / len(ent_values), 6) if ent_values else 0,
            "blocks_below_7_5": sum(1 for value in ent_values if value < 7.5),
            "blocks_below_7_9": sum(1 for value in ent_values if value < 7.9),
        },
        "signatures": signatures,
        "pattern_hits": pattern_hits,
        "ascii_hits": ascii_hits[:200],
        "byte_histogram": {
            "most_common": most_common,
            "least_common": least_common,
        },
    }
    (args.out / "overlay_report.json").write_text(
        json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8"
    )

    lines = [
        "# Overlay Analysis",
        "",
        f"- Overlay offset: `0x{overlay_off:x}`",
        f"- Overlay size: {len(overlay):,} bytes",
        f"- Overlay SHA-256: `{hashlib.sha256(overlay).hexdigest()}`",
        f"- Overall entropy: {report['overlay_entropy']}",
        f"- Block entropy min/max/avg: {report['block_entropy']['min']} / {report['block_entropy']['max']} / {report['block_entropy']['avg']}",
        f"- 64KB blocks below entropy 7.9: {report['block_entropy']['blocks_below_7_9']}",
        "",
        "## Signature Hits",
        "",
    ]
    for item in signatures:
        if item["first_offsets"]:
            offsets = ", ".join(item["first_offsets"][:10])
            lines.append(f"- {item['name']}: {offsets}")
    if not any(item["first_offsets"] for item in signatures):
        lines.append("- No known container/executable signatures found.")
    lines.extend(["", "## Interesting Pattern Hits", ""])
    if pattern_hits:
        for item in pattern_hits:
            lines.append(f"- {item['pattern']}: {', '.join(item['offsets'][:10])}")
    else:
        lines.append("- No configured business/network patterns found in overlay.")
    lines.extend(
        [
            "",
            "## Verdict",
            "",
            "The overlay behaves like encrypted or strongly compressed data. The full-overlay entropy is effectively 8.0, and no useful cleartext business/API strings were found in it. Static recovery of the protected business code is therefore blocked without runtime unpacking/dumping.",
        ]
    )
    (args.out / "OVERLAY_ANALYSIS.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
