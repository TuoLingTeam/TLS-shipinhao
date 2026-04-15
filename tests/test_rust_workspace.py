import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


class RustWorkspaceLayoutTests(unittest.TestCase):
    def test_workspace_manifest_declares_all_required_members(self):
        manifest = ROOT / "Cargo.toml"
        self.assertTrue(manifest.exists(), "缺少顶层 Cargo.toml workspace 清单")
        text = manifest.read_text(encoding="utf-8")
        for member in (
            'crates/api-contracts',
            'crates/build-tools',
            'crates/desktop-app',
            'crates/desktop-services',
            'crates/domain-core',
            'crates/license-service',
            'crates/security-core',
            'apps/desktop',
            'apps/license-worker',
        ):
            self.assertIn(member, text)

    def test_required_crates_and_apps_exist(self):
        required_paths = [
            ROOT / "crates" / "api-contracts" / "Cargo.toml",
            ROOT / "crates" / "build-tools" / "Cargo.toml",
            ROOT / "crates" / "desktop-app" / "Cargo.toml",
            ROOT / "crates" / "desktop-services" / "Cargo.toml",
            ROOT / "crates" / "domain-core" / "Cargo.toml",
            ROOT / "crates" / "license-service" / "Cargo.toml",
            ROOT / "crates" / "security-core" / "Cargo.toml",
            ROOT / "apps" / "desktop" / "README.md",
            ROOT / "apps" / "license-worker" / "README.md",
        ]
        for path in required_paths:
            self.assertTrue(path.exists(), f"缺少 workspace 路径: {path.relative_to(ROOT)}")

if __name__ == "__main__":
    unittest.main()
