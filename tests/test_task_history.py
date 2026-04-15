import csv
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
APP_ROOT = ROOT / "app"
if str(APP_ROOT) not in sys.path:
    sys.path.insert(0, str(APP_ROOT))

from services.task_history import TaskHistoryStore


class TaskHistoryStoreTests(unittest.TestCase):
    def test_append_should_keep_latest_entries_only(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            store = TaskHistoryStore(Path(temp_dir) / "history.json", max_entries=2)
            store.append({"task_label": "任务1", "summary": "第一次"})
            store.append({"task_label": "任务2", "summary": "第二次"})
            rows = store.append({"task_label": "任务3", "summary": "第三次"})

        self.assertEqual(len(rows), 2)
        self.assertEqual(rows[0]["task_label"], "任务3")
        self.assertEqual(rows[1]["task_label"], "任务2")

    def test_export_csv_should_write_utf8_bom_file(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            export_path = Path(temp_dir) / "export.csv"
            store = TaskHistoryStore(Path(temp_dir) / "history.json")
            store.export_csv(
                export_path,
                [
                    {"task_label": "中差评查找", "status_text": "完成", "summary": "匹配到 3 个订单"},
                    {"task_label": "批量处理", "status_text": "部分失败", "summary": "失败 1 条"},
                ],
            )

            with export_path.open("r", encoding="utf-8-sig", newline="") as fp:
                reader = csv.DictReader(fp)
                rows = list(reader)

        self.assertEqual(len(rows), 2)
        self.assertEqual(rows[0]["task_label"], "中差评查找")
        self.assertIn("summary", rows[1])


if __name__ == "__main__":
    unittest.main()
