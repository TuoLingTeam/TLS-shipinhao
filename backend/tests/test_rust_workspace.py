import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


class RustWorkspaceLayoutTests(unittest.TestCase):
    def test_workspace_manifest_declares_all_required_members(self):
        manifest = ROOT / "Cargo.toml"
        self.assertTrue(manifest.exists(), "缺少顶层 Cargo.toml workspace 清单")
        text = manifest.read_text(encoding="utf-8")
        for member in (
            'backend/shared/api-contracts',
            'backend/infra/tooling/build-tools',
            'backend/modules/desktop-services',
            'backend/shared/domain-core',
            'backend/modules/license-service',
            'backend/shared/security-core',
            'apps/desktop',
            'backend/apps/license-worker',
            'backend/infra/tooling/xtask',
        ):
            self.assertIn(member, text)

    def test_required_crates_and_apps_exist(self):
        required_paths = [
            ROOT / "backend" / "crates" / "core" / "api-contracts" / "Cargo.toml",
            ROOT / "backend" / "crates" / "tooling" / "build-tools" / "Cargo.toml",
            ROOT / "backend" / "crates" / "desktop-services" / "Cargo.toml",
            ROOT / "backend" / "crates" / "core" / "domain-core" / "Cargo.toml",
            ROOT / "backend" / "crates" / "license-service" / "Cargo.toml",
            ROOT / "backend" / "crates" / "core" / "security-core" / "Cargo.toml",
            ROOT / "apps" / "desktop" / "Cargo.toml",
            ROOT / "backend" / "worker" / "license-worker" / "README.md",
            ROOT / "backend" / "crates" / "tooling" / "xtask" / "Cargo.toml",
        ]
        for path in required_paths:
            self.assertTrue(path.exists(), f"缺少 workspace 路径: {path.relative_to(ROOT)}")

if __name__ == "__main__":
    unittest.main()
