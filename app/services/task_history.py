# -*- coding: utf-8 -*-
"""TLS-shipinhao 任务历史记录与导出。"""

from __future__ import annotations

import csv
import json
from pathlib import Path
from typing import Iterable

from settings import (
    TASK_HISTORY_FILE_NAME,
    TASK_HISTORY_MAX_ENTRIES,
    get_user_data_dir,
)


class TaskHistoryStore:
    """任务历史存储（JSON）与导出（CSV）。"""

    def __init__(self, path: str | Path | None = None, *, max_entries: int = TASK_HISTORY_MAX_ENTRIES):
        self.path = Path(path) if path else (get_user_data_dir() / TASK_HISTORY_FILE_NAME)
        self.max_entries = max(1, int(max_entries))

    def load(self) -> list[dict]:
        """读取历史任务。"""
        try:
            payload = json.loads(self.path.read_text(encoding="utf-8"))
        except FileNotFoundError:
            return []
        except Exception:  # noqa: BLE001
            return []

        if not isinstance(payload, list):
            return []
        return [item for item in payload if isinstance(item, dict)]

    def append(self, entry: dict) -> list[dict]:
        """追加一条历史记录并返回最新列表。"""
        rows = self.load()
        rows.insert(0, self._normalize_entry(entry))
        rows = rows[: self.max_entries]
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self.path.write_text(
            json.dumps(rows, ensure_ascii=False, indent=2),
            encoding="utf-8",
        )
        return rows

    def export_csv(self, destination: str | Path, rows: Iterable[dict], *, fieldnames: list[str] | None = None) -> Path:
        """将指定行导出为 CSV。"""
        destination_path = Path(destination)
        destination_path.parent.mkdir(parents=True, exist_ok=True)
        normalized_rows = [self._normalize_entry(row) for row in rows if isinstance(row, dict)]
        resolved_fieldnames = list(fieldnames or self._collect_fieldnames(normalized_rows))

        with destination_path.open("w", encoding="utf-8-sig", newline="") as fp:
            writer = csv.DictWriter(fp, fieldnames=resolved_fieldnames)
            writer.writeheader()
            for row in normalized_rows:
                writer.writerow({name: row.get(name, "") for name in resolved_fieldnames})
        return destination_path

    @staticmethod
    def _collect_fieldnames(rows: list[dict]) -> list[str]:
        keys: list[str] = []
        for row in rows:
            for key in row.keys():
                if key not in keys:
                    keys.append(key)
        return keys

    @staticmethod
    def _normalize_entry(entry: dict) -> dict:
        normalized = {}
        for key, value in (entry or {}).items():
            if isinstance(value, bool):
                normalized[str(key)] = "是" if value else "否"
            elif isinstance(value, (str, int, float)) or value is None:
                normalized[str(key)] = value
            elif isinstance(value, (list, tuple)):
                normalized[str(key)] = " | ".join(str(item) for item in value)
            elif isinstance(value, dict):
                normalized[str(key)] = json.dumps(value, ensure_ascii=False, sort_keys=True)
            else:
                normalized[str(key)] = str(value)
        return normalized
