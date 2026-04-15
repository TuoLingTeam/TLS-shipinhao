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
        for symbol in ("fn main()", "\"release\"", "\"manifest\"", "\"desktop-build\""):
            self.assertIn(symbol, text)


if __name__ == "__main__":
    unittest.main()
