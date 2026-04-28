#!/usr/bin/env node
/**
 * Tauri 真壳 E2E 平台 aware launcher（L4-4）。
 *
 * - macOS：tauri-driver 不支持（Apple WKWebView 没有官方 WebDriver 实现），
 *   打印降级提示并以 exit 0 结束。让根 `pnpm test:e2e:tauri` 在 macOS 开发机上
 *   既不静默失败、也不阻塞 husky / 本地脚本链。
 * - Linux / Windows：进入 `e2e-tauri/` 子项目，按需 npm install 后 npm test。
 *   该子目录故意脱离根 pnpm-workspace.yaml，这里用 npm 而非 pnpm。
 *
 * 通过 `TAURI_E2E_BIN` 环境变量可覆盖被测二进制路径（与 wdio.conf.ts 对齐）。
 */
import { existsSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import process from "node:process";

const __filename = fileURLToPath(import.meta.url);
const repoRoot = resolve(dirname(__filename), "..");
const e2eDir = resolve(repoRoot, "e2e-tauri");

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
console.log(`[tauri-e2e] 运行：cd e2e-tauri && npm run ${testScript}`);
const run = spawnSync(npmCmd, ["run", testScript], { cwd: e2eDir, stdio: "inherit" });
process.exit(run.status ?? 1);
