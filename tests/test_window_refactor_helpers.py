import os
import sys
import unittest
from pathlib import Path

os.environ.setdefault('QT_QPA_PLATFORM', 'offscreen')

ROOT = Path(__file__).resolve().parents[1]
APP_ROOT = ROOT / 'app'
if str(APP_ROOT) not in sys.path:
    sys.path.insert(0, str(APP_ROOT))

from PySide6.QtWidgets import QApplication


class WindowRefactorHelperTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QApplication.instance() or QApplication([])

    def test_refactor_helper_modules_should_be_converged(self):
        from ui import window_view
        from ui.window_dialogs import MessagePresenter

        self.assertTrue(callable(window_view.build_main_window_stylesheet))
        self.assertTrue(callable(window_view.resolve_layout_mode))
        self.assertTrue(callable(window_view.build_header_section))
        self.assertTrue(callable(window_view.create_card))
        self.assertTrue(callable(MessagePresenter))


if __name__ == '__main__':
    unittest.main()
