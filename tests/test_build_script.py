import importlib.util
import tempfile
import unittest
from pathlib import Path
from unittest import mock


BUILD_SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "build.py"
SPEC = importlib.util.spec_from_file_location("tls_build_script", BUILD_SCRIPT)
build_script = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(build_script)


class BuildWindowsOnefileTests(unittest.TestCase):
    def test_build_windows_uses_onefile_and_packages_single_exe(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            dist_dir = tmp_path / "dist"
            build_dir = tmp_path / "build"
            dist_dir.mkdir()
            build_dir.mkdir()
            output_exe = dist_dir / "TLS-shipinhao.exe"

            def fake_run(cmd):
                output_exe.write_bytes(b"exe")

            with mock.patch.object(build_script, "DIST_DIR", dist_dir), mock.patch.object(
                build_script,
                "BUILD_DIR",
                build_dir,
            ), mock.patch.object(
                build_script,
                "build_pyinstaller_base_cmd",
                return_value=["pyinstaller"],
            ), mock.patch.object(
                build_script,
                "prepare_windows_version_file",
                return_value=build_dir / "version.txt",
            ), mock.patch.object(
                build_script,
                "get_app_version",
                return_value="4.2",
            ), mock.patch.object(
                build_script,
                "cleanup_temp_files",
            ), mock.patch.object(
                build_script,
                "copy_runtime_files",
            ) as copy_runtime_files_mock, mock.patch.object(
                build_script.shutil,
                "make_archive",
            ) as archive_mock, mock.patch.object(
                build_script,
                "run",
                side_effect=fake_run,
            ) as run_mock:
                artifact = build_script.build_windows(
                    python_bin="python",
                    app_name="TLS-shipinhao",
                    entry_file=Path("app/main.py"),
                    profile=build_script.PROFILE_MAIN,
                    use_dist=True,
                )

        self.assertEqual(artifact, output_exe)
        self.assertIn("--onefile", run_mock.call_args.args[0])
        copy_runtime_files_mock.assert_called_once_with(dist_dir / "TLS-shipinhao")
        archive_mock.assert_called_once_with(str(dist_dir / "TLS-shipinhao-win"), "zip", dist_dir, "TLS-shipinhao")


if __name__ == "__main__":
    unittest.main()
