#!/usr/bin/env python3
"""Small PE recovery helper for local, owned Windows binaries.

This intentionally avoids executing the target. It parses headers, imports,
resources, embedded PE markers, and several string encodings using only the
standard library.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import struct
from collections import Counter
from pathlib import Path


RESOURCE_TYPES = {
    1: "cursor",
    2: "bitmap",
    3: "icon",
    4: "menu",
    5: "dialog",
    6: "string",
    7: "fontdir",
    8: "font",
    9: "accelerator",
    10: "rcdata",
    11: "messagetable",
    12: "group_cursor",
    14: "group_icon",
    16: "version",
    24: "manifest",
}

INTERESTING_RE = re.compile(
    r"(微信|小店|物流|订单|快递|发货|签收|cookie|token|login|logout|account|api|"
    r"http|https|weixin|wechat|shop|order|express|delivery|shipping|curl|"
    r"version|update|result|client|硬件|机器码|绑定|解绑)",
    re.I,
)


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


class PE:
    def __init__(self, data: bytes) -> None:
        self.data = data
        if data[:2] != b"MZ":
            raise ValueError("not an MZ executable")
        self.pe_off = u32(data, 0x3C)
        if data[self.pe_off : self.pe_off + 4] != b"PE\0\0":
            raise ValueError("missing PE signature")
        coff = self.pe_off + 4
        (
            self.machine,
            self.section_count,
            self.timestamp,
            _ptrsym,
            _numsym,
            self.opt_size,
            self.characteristics,
        ) = struct.unpack_from("<HHIIIHH", data, coff)
        self.opt_off = coff + 20
        self.magic = u16(data, self.opt_off)
        if self.magic != 0x10B:
            raise ValueError(f"only PE32 is supported, got magic=0x{self.magic:x}")
        self.entry_rva = u32(data, self.opt_off + 16)
        self.image_base = u32(data, self.opt_off + 28)
        self.section_alignment = u32(data, self.opt_off + 32)
        self.file_alignment = u32(data, self.opt_off + 36)
        self.subsystem = u16(data, self.opt_off + 68)
        self.size_of_image = u32(data, self.opt_off + 56)
        self.data_dir_off = self.opt_off + 96
        self.data_dirs = []
        for i in range(16):
            off = self.data_dir_off + i * 8
            self.data_dirs.append((u32(data, off), u32(data, off + 4)))
        self.sections = []
        sec_off = self.opt_off + self.opt_size
        for i in range(self.section_count):
            off = sec_off + i * 40
            raw_name = data[off : off + 8].split(b"\0", 1)[0]
            name = raw_name.decode("ascii", "replace")
            virtual_size, virtual_address, raw_size, raw_ptr = struct.unpack_from(
                "<IIII", data, off + 8
            )
            chars = u32(data, off + 36)
            raw = data[raw_ptr : raw_ptr + raw_size]
            self.sections.append(
                {
                    "index": i,
                    "name": name,
                    "virtual_size": virtual_size,
                    "virtual_address": virtual_address,
                    "raw_size": raw_size,
                    "raw_ptr": raw_ptr,
                    "characteristics": chars,
                    "entropy": round(entropy(raw), 3),
                }
            )

    def rva_to_offset(self, rva: int) -> int | None:
        for sec in self.sections:
            va = sec["virtual_address"]
            size = max(sec["virtual_size"], sec["raw_size"])
            if va <= rva < va + size:
                return sec["raw_ptr"] + (rva - va)
        if rva < self.sections[0]["raw_ptr"]:
            return rva
        return None

    def cstr(self, off: int, limit: int = 4096) -> str:
        end = self.data.find(b"\0", off, min(len(self.data), off + limit))
        if end < 0:
            end = min(len(self.data), off + limit)
        return self.data[off:end].decode("ascii", "replace")

    def imports(self) -> list[dict[str, object]]:
        imports_rva, _ = self.data_dirs[1]
        off = self.rva_to_offset(imports_rva)
        if not off:
            return []
        result = []
        while off + 20 <= len(self.data):
            original_first_thunk, _time, _fwd, name_rva, first_thunk = struct.unpack_from(
                "<IIIII", self.data, off
            )
            if not any((original_first_thunk, name_rva, first_thunk)):
                break
            name_off = self.rva_to_offset(name_rva)
            thunk_rva = original_first_thunk or first_thunk
            thunk_off = self.rva_to_offset(thunk_rva)
            funcs = []
            if thunk_off is not None:
                cur = thunk_off
                while cur + 4 <= len(self.data):
                    val = u32(self.data, cur)
                    if val == 0:
                        break
                    if val & 0x80000000:
                        funcs.append(f"ordinal_{val & 0xffff}")
                    else:
                        hint_name = self.rva_to_offset(val)
                        if hint_name is not None:
                            funcs.append(self.cstr(hint_name + 2))
                    cur += 4
            result.append(
                {
                    "dll": self.cstr(name_off) if name_off is not None else f"rva_{name_rva:x}",
                    "functions": funcs,
                }
            )
            off += 20
        return result

    def resources(self, out_dir: Path) -> list[dict[str, object]]:
        res_rva, res_size = self.data_dirs[2]
        res_off = self.rva_to_offset(res_rva)
        if res_off is None or res_size == 0:
            return []
        resources_dir = out_dir / "resources"
        resources_dir.mkdir(parents=True, exist_ok=True)
        entries: list[dict[str, object]] = []

        def entry_name(raw: int) -> str | int:
            if raw & 0x80000000:
                noff = res_off + (raw & 0x7FFFFFFF)
                ln = u16(self.data, noff)
                raw_s = self.data[noff + 2 : noff + 2 + ln * 2]
                return raw_s.decode("utf-16le", "replace")
            return raw

        def walk(dir_rel: int, path: list[str | int]) -> None:
            cur = res_off + dir_rel
            if cur + 16 > len(self.data):
                return
            named = u16(self.data, cur + 12)
            ids = u16(self.data, cur + 14)
            ent = cur + 16
            for i in range(named + ids):
                name_raw = u32(self.data, ent + i * 8)
                data_raw = u32(self.data, ent + i * 8 + 4)
                name = entry_name(name_raw)
                next_path = path + [name]
                if data_raw & 0x80000000:
                    walk(data_raw & 0x7FFFFFFF, next_path)
                else:
                    data_entry = res_off + data_raw
                    if data_entry + 16 > len(self.data):
                        continue
                    data_rva, size, codepage, _reserved = struct.unpack_from(
                        "<IIII", self.data, data_entry
                    )
                    data_off = self.rva_to_offset(data_rva)
                    if data_off is None:
                        continue
                    blob = self.data[data_off : data_off + size]
                    type_id = next_path[0] if next_path else "unknown"
                    type_name = RESOURCE_TYPES.get(type_id, str(type_id)) if isinstance(type_id, int) else str(type_id)
                    safe_path = "_".join(str(x).replace("/", "_") for x in next_path)
                    suffix = guess_suffix(blob, type_name)
                    filename = f"{safe_path}_{data_rva:08x}{suffix}"
                    (resources_dir / filename).write_bytes(blob)
                    entries.append(
                        {
                            "path": next_path,
                            "type": type_name,
                            "rva": data_rva,
                            "offset": data_off,
                            "size": size,
                            "codepage": codepage,
                            "file": str(resources_dir / filename),
                            "sha256": hashlib.sha256(blob).hexdigest(),
                        }
                    )

        walk(0, [])
        return entries


def guess_suffix(blob: bytes, type_name: str) -> str:
    if blob.startswith(b"\x89PNG\r\n\x1a\n"):
        return ".png"
    if blob.startswith(b"\xff\xd8\xff"):
        return ".jpg"
    if blob.startswith(b"GIF8"):
        return ".gif"
    if blob.startswith(b"BM"):
        return ".bmp"
    if blob.startswith(b"MZ"):
        return ".exe"
    if blob.startswith(b"PK\x03\x04"):
        return ".zip"
    if type_name == "manifest":
        return ".xml"
    if type_name in {"icon", "group_icon"}:
        return ".ico.bin"
    if type_name == "version":
        return ".version.bin"
    return ".bin"


def printable_ascii_strings(data: bytes, min_len: int = 5) -> list[str]:
    out = []
    cur = bytearray()
    for b in data:
        if b in (9, 10, 13) or 32 <= b <= 126:
            cur.append(b)
        else:
            if len(cur) >= min_len:
                out.append(cur.decode("ascii", "replace"))
            cur.clear()
    if len(cur) >= min_len:
        out.append(cur.decode("ascii", "replace"))
    return out


def utf16le_strings(data: bytes, min_len: int = 3) -> list[str]:
    out = []
    cur = []
    for i in range(0, len(data) - 1, 2):
        code = data[i] | (data[i + 1] << 8)
        ch = chr(code)
        if ch in "\r\n\t" or (not ch.isspace() and ch.isprintable()):
            cur.append(ch)
        else:
            if len(cur) >= min_len:
                out.append("".join(cur))
            cur = []
    if len(cur) >= min_len:
        out.append("".join(cur))
    return out


def gb18030_strings(data: bytes, min_len: int = 4) -> list[str]:
    out = []
    cur = bytearray()
    for b in data:
        if b in (9, 10, 13) or 32 <= b <= 126 or 0x80 <= b <= 0xFE:
            cur.append(b)
        else:
            flush_gb(cur, out, min_len)
            cur.clear()
    flush_gb(cur, out, min_len)
    return out


def flush_gb(buf: bytearray, out: list[str], min_len: int) -> None:
    if len(buf) < min_len:
        return
    text = bytes(buf).decode("gb18030", "ignore").strip()
    cjk = sum(1 for ch in text if "\u4e00" <= ch <= "\u9fff")
    if len(text) >= min_len and (cjk >= 2 or INTERESTING_RE.search(text)):
        out.append(text)


def unique_keep_order(items: list[str]) -> list[str]:
    seen = set()
    out = []
    for item in items:
        item = item.replace("\r", "\\r").replace("\n", "\\n")
        if item not in seen:
            seen.add(item)
            out.append(item)
    return out


def find_embedded_pe(data: bytes, out_dir: Path) -> list[dict[str, object]]:
    embedded_dir = out_dir / "embedded_pe"
    embedded_dir.mkdir(parents=True, exist_ok=True)
    result = []
    for m in re.finditer(b"MZ", data):
        off = m.start()
        if off == 0 or off + 0x40 >= len(data):
            continue
        pe_rel = u32(data, off + 0x3C)
        pe_sig = off + pe_rel
        if pe_rel <= 0 or pe_sig + 0x18 >= len(data) or data[pe_sig : pe_sig + 4] != b"PE\0\0":
            continue
        try:
            section_count = u16(data, pe_sig + 6)
            opt_size = u16(data, pe_sig + 20)
            sec_off = pe_sig + 24 + opt_size
            end = off
            for i in range(section_count):
                sh = sec_off + i * 40
                if sh + 40 > len(data):
                    break
                raw_size = u32(data, sh + 16)
                raw_ptr = u32(data, sh + 20)
                end = max(end, off + raw_ptr + raw_size)
            if end <= off or end > len(data):
                continue
            blob = data[off:end]
            name = embedded_dir / f"embedded_{off:08x}_{len(blob)}.exe"
            name.write_bytes(blob)
            result.append(
                {
                    "offset": off,
                    "size": len(blob),
                    "file": str(name),
                    "sha256": hashlib.sha256(blob).hexdigest(),
                }
            )
        except struct.error:
            continue
    return result


def write_lines(path: Path, lines: list[str]) -> None:
    path.write_text("\n".join(lines) + ("\n" if lines else ""), encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("exe", type=Path)
    parser.add_argument("out", type=Path)
    args = parser.parse_args()

    data = args.exe.read_bytes()
    pe = PE(data)
    args.out.mkdir(parents=True, exist_ok=True)

    imports = pe.imports()
    resources = pe.resources(args.out)
    embedded = find_embedded_pe(data, args.out)

    ascii_strings = unique_keep_order(printable_ascii_strings(data, 6))
    utf16_strings = unique_keep_order(utf16le_strings(data, 4))
    gb_strings = unique_keep_order(gb18030_strings(data, 4))
    interesting = unique_keep_order(
        [s for s in ascii_strings + utf16_strings + gb_strings if INTERESTING_RE.search(s)]
    )

    write_lines(args.out / "strings_ascii.txt", ascii_strings)
    write_lines(args.out / "strings_utf16le.txt", utf16_strings)
    write_lines(args.out / "strings_gb18030_interesting.txt", gb_strings)
    write_lines(args.out / "strings_interesting.txt", interesting)

    imports_txt = []
    for item in imports:
        imports_txt.append(f"[{item['dll']}]")
        imports_txt.extend(f"  {fn}" for fn in item["functions"])
        imports_txt.append("")
    write_lines(args.out / "imports.txt", imports_txt)

    report = {
        "file": str(args.exe),
        "size": len(data),
        "sha256": hashlib.sha256(data).hexdigest(),
        "pe_offset": pe.pe_off,
        "machine": hex(pe.machine),
        "entry_rva": hex(pe.entry_rva),
        "image_base": hex(pe.image_base),
        "subsystem": pe.subsystem,
        "characteristics": hex(pe.characteristics),
        "sections": pe.sections,
        "import_dlls": [item["dll"] for item in imports],
        "resource_count": len(resources),
        "embedded_pe_count": len(embedded),
        "string_counts": {
            "ascii": len(ascii_strings),
            "utf16le": len(utf16_strings),
            "gb18030_interesting": len(gb_strings),
            "interesting": len(interesting),
        },
        "resources": resources,
        "embedded_pe": embedded,
    }
    (args.out / "pe_report.json").write_text(
        json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8"
    )

    summary = [
        f"# PE Recovery Summary",
        "",
        f"- File: `{args.exe}`",
        f"- Size: {len(data):,} bytes",
        f"- SHA-256: `{hashlib.sha256(data).hexdigest()}`",
        f"- Entry RVA: `0x{pe.entry_rva:x}`",
        f"- Image base: `0x{pe.image_base:x}`",
        f"- Sections: {len(pe.sections)}",
        f"- Import DLLs: {len(imports)}",
        f"- Resources dumped: {len(resources)}",
        f"- Embedded PE files carved: {len(embedded)}",
        f"- Interesting strings: {len(interesting)}",
        "",
        "## Sections",
        "",
        "| Name | RVA | Raw offset | Raw size | Entropy |",
        "| --- | ---: | ---: | ---: | ---: |",
    ]
    for sec in pe.sections:
        summary.append(
            f"| {sec['name']} | 0x{sec['virtual_address']:x} | 0x{sec['raw_ptr']:x} | "
            f"{sec['raw_size']:,} | {sec['entropy']} |"
        )
    summary.extend(
        [
            "",
            "## Notes",
            "",
            "- High entropy or explicit VMProtect strings usually means native decompilation will be incomplete without unpacking/runtime tracing.",
            "- See `strings_interesting.txt` for business/API clues and `resources/` for dumped dialogs/icons/RCData.",
        ]
    )
    (args.out / "SUMMARY.md").write_text("\n".join(summary) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
