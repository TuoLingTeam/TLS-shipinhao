#!/usr/bin/env node
/** Tauri 真壳 E2E launcher；macOS 无 tauri-driver 时提示并跳过。 */
import { existsSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import process from "node:process";

const __filename = fileURLToPath(import.meta.url);
const repoRoot = resolve(dirname(__filename), "..");
const e2eDir = resolve(repoRoot, "apps", "desktop", "e2e-tauri");

if (process.platform === "darwin") {
  console.log("[tauri-e2e] macOS 上 tauri-driver 不可用（Apple WKWebView 没有官方 WebDriver）。");
  console.log("[tauri-e2e] 本机请改跑：pnpm test:e2e:web");
  console.log("[tauri-e2e] CI 真壳验证：.github/workflows/tauri-e2e.yml（Linux runner）");
  process.exit(0);
}

if (!existsSync(e2eDir)) {
  console.error(`[tauri-e2e] 缺少子项目目录：${e2eDir}`);
  process.exit(1);
}

const npmCmd = process.platform === "win32" ? "npm.cmd" : "npm";
const nodeModules = resolve(e2eDir, "node_modules");
if (!existsSync(nodeModules)) {
  console.log("[tauri-e2e] node_modules 不存在，先执行 npm install");
  const install = spawnSync(npmCmd, ["install"], { cwd: e2eDir, stdio: "inherit" });
  if (install.status !== 0) {
    process.exit(install.status ?? 1);
  }
}

const testScript = process.env.TAURI_E2E_HEADLESS === "1" ? "test:ci" : "test";
console.log(`[tauri-e2e] 运行：cd apps/desktop/e2e-tauri && npm run ${testScript}`);
const run = spawnSync(npmCmd, ["run", testScript], { cwd: e2eDir, stdio: "inherit" });
process.exit(run.status ?? 1);
