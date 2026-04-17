import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


class RustWorkspaceLayoutTests(unittest.TestCase):
    def test_workspace_manifest_declares_all_required_members(self):
        manifest = ROOT / "Cargo.toml"
        self.assertTrue(manifest.exists(), "缺少顶层 Cargo.toml workspace 清单")
        text = manifest.read_text(encoding="utf-8")
        for member in (
            'backend/crates/api-contracts',
            'backend/crates/build-tools',
            'backend/crates/desktop-app',
            'backend/crates/desktop-services',
            'backend/crates/domain-core',
            'backend/crates/license-service',
            'backend/crates/security-core',
            'apps/desktop',
            'backend/license-worker',
            'backend/xtask',
        ):
            self.assertIn(member, text)

    def test_required_crates_and_apps_exist(self):
        required_paths = [
            ROOT / "backend" / "crates" / "api-contracts" / "Cargo.toml",
            ROOT / "backend" / "crates" / "build-tools" / "Cargo.toml",
            ROOT / "backend" / "crates" / "desktop-app" / "Cargo.toml",
            ROOT / "backend" / "crates" / "desktop-services" / "Cargo.toml",
            ROOT / "backend" / "crates" / "domain-core" / "Cargo.toml",
            ROOT / "backend" / "crates" / "license-service" / "Cargo.toml",
            ROOT / "backend" / "crates" / "security-core" / "Cargo.toml",
            ROOT / "apps" / "desktop" / "Cargo.toml",
            ROOT / "backend" / "license-worker" / "README.md",
            ROOT / "backend" / "xtask" / "Cargo.toml",
        ]
        for path in required_paths:
            self.assertTrue(path.exists(), f"缺少 workspace 路径: {path.relative_to(ROOT)}")

if __name__ == "__main__":
    unittest.main()
