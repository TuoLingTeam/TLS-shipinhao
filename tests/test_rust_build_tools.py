import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


class RustBuildToolsTests(unittest.TestCase):
    def test_build_tools_define_manifest_and_release_helpers(self):
        lib_rs = ROOT / "crates" / "build-tools" / "src" / "lib.rs"
        self.assertTrue(lib_rs.exists(), "缺少 crates/build-tools/src/lib.rs")
        text = lib_rs.read_text(encoding="utf-8")
        for symbol in (
            "pub struct ReleaseArtifact",
            "pub struct ManifestSignature",
            "pub fn generate_integrity_manifest",
            "pub fn sign_manifest",
            "pub fn attach_signature",
            "pub fn verify_manifest_signature",
            "pub fn inject_version",
        ):
            self.assertIn(symbol, text)

    def test_xtask_release_entry_exists(self):
        cargo_toml = ROOT / "xtask" / "Cargo.toml"
        main_rs = ROOT / "xtask" / "src" / "main.rs"
        self.assertTrue(cargo_toml.exists(), "缺少 xtask/Cargo.toml")
        self.assertTrue(main_rs.exists(), "缺少 xtask/src/main.rs")
        text = main_rs.read_text(encoding="utf-8")
        for symbol in (
            'fn main()',
            '"release"',
            '"manifest"',
            '"desktop-build"',
            'run_desktop_build_command(&[OsString::from("--release")])',
        ):
            self.assertIn(symbol, text)

    def test_workflow_uses_rust_only_release_pipeline(self):
        workflow = ROOT / ".github" / "workflows" / "build.yml"
        self.assertTrue(workflow.exists(), "缺少 Rust 构建工作流")
        text = workflow.read_text(encoding="utf-8")
        for expected in (
            "cargo test --workspace",
            "cargo run -p xtask -- desktop-build --release",
            "cargo run -p xtask -- manifest",
            "Swatinem/rust-cache@v2",
        ):
            self.assertIn(expected, text)
        for forbidden in (
            "actions/setup-python",
            "app/requirements.txt",
            "scripts/build.py",
            "scripts/obfuscate.py",
            "PyInstaller",
            "Cython",
        ):
            self.assertNotIn(forbidden, text)

    def test_legacy_python_build_scripts_are_removed(self):
        self.assertFalse((ROOT / "scripts" / "build.py").exists())
        self.assertFalse((ROOT / "scripts" / "obfuscate.py").exists())


if __name__ == "__main__":
    unittest.main()
