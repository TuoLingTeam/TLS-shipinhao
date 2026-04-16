# -*- coding: utf-8 -*-
"""在线更新检查服务。"""

from __future__ import annotations

from dataclasses import dataclass
import os
import sys
from typing import Any

import requests

from settings import APP_VERSION, REQUEST_TIMEOUT, UPDATE_VERSION_URL
from services.versioning import is_newer_version


@dataclass(frozen=True)
class UpdateInfo:
    app: str
    version: str
    build: int
    mandatory: bool
    platform: str
    download_url: str
    tutorial_url: str
    notes: list[str]
    has_update: bool
    raw_payload: dict[str, Any]


def detect_platform() -> str:
    if sys.platform == 'darwin':
        return 'mac'
    if os.name == 'nt' or sys.platform.startswith('win'):
        return 'windows'
    return 'unknown'


def _normalize_notes(payload: dict[str, Any]) -> list[str]:
    notes = payload.get('notes') or []
    if isinstance(notes, list):
        return [str(item).strip() for item in notes if str(item).strip()]
    if isinstance(notes, str) and notes.strip():
        return [notes.strip()]
    return []



def fetch_latest_version_info(current_version: str | None = None) -> UpdateInfo:
    response = requests.get(UPDATE_VERSION_URL, timeout=REQUEST_TIMEOUT)
    response.raise_for_status()
    payload = response.json()
    platform = detect_platform()
    latest_version = str(payload.get('version') or '').strip()
    resolved_current_version = current_version or APP_VERSION
    has_update = bool(latest_version and is_newer_version(resolved_current_version, latest_version))
    return UpdateInfo(
        app=str(payload.get('app') or 'TLS-shipinhao'),
        version=latest_version,
        build=int(payload.get('build') or 0),
        mandatory=bool(payload.get('mandatory')),
        platform=platform,
        download_url=str(payload.get('download_url') or '').strip(),
        tutorial_url=str(payload.get('tutorial_url') or '').strip(),
        notes=_normalize_notes(payload),
        has_update=has_update,
        raw_payload=payload,
    )
