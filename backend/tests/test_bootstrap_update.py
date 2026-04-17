import sys
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
APP_ROOT = ROOT / 'backup' / 'legacy-src' / 'app'
if str(APP_ROOT) not in sys.path:
    sys.path.insert(0, str(APP_ROOT))

import bootstrap


class BootstrapUpdateTests(unittest.TestCase):
    def test_main_prefers_built_rust_binary(self):
        binary = bootstrap.TARGET_DIR / 'debug' / bootstrap.DESKTOP_BINARY_NAME
        with mock.patch.object(bootstrap.Path, 'exists', autospec=True) as exists_mock, \
             mock.patch.object(bootstrap, 'subprocess') as subprocess_mock:
            exists_mock.side_effect = lambda path_obj: Path(path_obj) == binary
            subprocess_mock.run.return_value.returncode = 0
            with self.assertRaises(SystemExit) as ctx:
                with mock.patch.object(bootstrap.sys, 'argv', ['backup/legacy-src/app/main.py']):
                    bootstrap.main()

        self.assertEqual(ctx.exception.code, 0)
        subprocess_mock.run.assert_called_once_with([str(binary)], check=False)

    def test_main_falls_back_to_cargo_run_when_binary_missing(self):
        with mock.patch.object(bootstrap.Path, 'exists', return_value=False), \
             mock.patch.object(bootstrap.shutil, 'which', return_value='/usr/bin/cargo'), \
             mock.patch.object(bootstrap, 'subprocess') as subprocess_mock:
            subprocess_mock.run.return_value.returncode = 0
            with self.assertRaises(SystemExit):
                with mock.patch.object(bootstrap.sys, 'argv', ['backup/legacy-src/app/main.py', '--demo']):
                    bootstrap.main()

        subprocess_mock.run.assert_called_once_with(
            ['/usr/bin/cargo', 'run', '-p', 'desktop-app', '--', '--demo'],
            check=False,
        )


if __name__ == '__main__':
    unittest.main()
