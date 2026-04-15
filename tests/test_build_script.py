import importlib.util
import json
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

            def fake_run(cmd, **kwargs):
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
                build_script,
                "_install_security_artifacts",
            ) as install_security_artifacts_mock, mock.patch.object(
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
        install_security_artifacts_mock.assert_called_once_with(dist_dir / "TLS-shipinhao", build_script.SYSTEM_WINDOWS)
        archive_mock.assert_called_once_with(str(dist_dir / "TLS-shipinhao-win"), "zip", dist_dir, "TLS-shipinhao")


if __name__ == "__main__":
    unittest.main()


class BuildSecurityArtifactsTests(unittest.TestCase):
    def test_ensure_security_core_build_invokes_cargo_with_expected_profile(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            crate_dir = tmp_path / "security-core"
            crate_dir.mkdir()
            artifact_path = crate_dir / "target" / "release" / "libsecurity_core.dylib"

            def fake_run(cmd, cwd=None):
                artifact_path.parent.mkdir(parents=True, exist_ok=True)
                artifact_path.write_bytes(b"native")

            with mock.patch.object(build_script, "SECURITY_CORE_DIR", crate_dir), \
                 mock.patch.object(build_script, "security_core_binary_name", return_value="libsecurity_core.dylib"), \
                 mock.patch.object(build_script, "run", side_effect=fake_run) as run_mock:
                built = build_script.ensure_security_core_built(build_script.SYSTEM_MACOS)

        self.assertEqual(built, artifact_path)
        self.assertEqual(run_mock.call_args.kwargs["cwd"], crate_dir)
        self.assertEqual(run_mock.call_args.args[0][:3], ["cargo", "build", "--release"])

    def test_generate_integrity_manifest_writes_signed_manifest(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            artifact = tmp_path / "TLS-shipinhao.exe"
            artifact.write_bytes(b"exe")
            key = build_script.generate_manifest_signing_keypair()

            manifest_path = build_script.generate_integrity_manifest(
                base_dir=tmp_path,
                files=[artifact],
                signing_private_key_b64=key["private_key_b64"],
            )

            data = json.loads(manifest_path.read_text(encoding="utf-8"))

        self.assertEqual(data["version"], 1)
        self.assertIn("signature", data)
        self.assertEqual(data["files"][0]["path"], "TLS-shipinhao.exe")
