# -*- coding: utf-8 -*-
"""版本号比较工具。"""

from __future__ import annotations


def parse_version(version: str) -> tuple[int, int, int]:
    parts = [int(part) for part in str(version).strip().split('.') if part != '']
    normalized = (parts + [0, 0, 0])[:3]
    return tuple(normalized)


def is_newer_version(current_version: str, latest_version: str) -> bool:
    return parse_version(latest_version) > parse_version(current_version)
