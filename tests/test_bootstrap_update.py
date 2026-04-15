import sys
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
APP_ROOT = ROOT / 'app'
if str(APP_ROOT) not in sys.path:
    sys.path.insert(0, str(APP_ROOT))

import bootstrap


class BootstrapUpdateTests(unittest.TestCase):
    def test_main_schedules_background_update_check_after_window_show(self):
        fake_app = mock.Mock()
        fake_window = mock.Mock()

        with mock.patch.object(bootstrap, 'QApplication', return_value=fake_app), mock.patch.object(
            bootstrap,
            '_load_runtime_objects',
            return_value=(lambda **kwargs: fake_window, lambda: ({}, 'ok')),
        ), mock.patch.object(
            bootstrap,
            '_apply_app_icon',
        ), mock.patch.object(bootstrap, 'QTimer') as qtimer_mock, mock.patch.object(
            bootstrap.sys,
            'argv',
            ['app/main.py'],
        ), mock.patch.object(bootstrap.sys, 'exit', side_effect=SystemExit):
            fake_app.exec.return_value = 0
            with self.assertRaises(SystemExit):
                bootstrap.main()

        fake_window.show.assert_called_once()
        qtimer_mock.singleShot.assert_called_once()
        delay, callback = qtimer_mock.singleShot.call_args.args
        self.assertGreaterEqual(delay, 500)
        self.assertTrue(callable(callback))


if __name__ == '__main__':
    unittest.main()
