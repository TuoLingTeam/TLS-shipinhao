#!/usr/bin/env node
// 发版编排脚本：前端构建 → JS 混淆 → Tauri 打包（Rust release）
//
// 默认产物目录：
//   - 前端混淆产物：build/obfuscated-ui/
//   - 最终安装/便携产物：dist/release/（拷贝自 Tauri target/release）
//
// 用法：
//   node scripts/build-release.mjs           # 完整流程（默认启用混淆）
//   node scripts/build-release.mjs --no-obfuscate   # 跳过 JS 混淆（走 apps/ui/dist 原始产物）
//   node scripts/build-release.mjs --skip-rust-build # 只跑前端 + 混淆，不 cargo tauri build
//
// 设计原则：
//   1. 不动源文件（apps/ui/src、apps/desktop/src 都是只读输入）
//   2. 产物统一落在 build/ 和 dist/release/，便于 .gitignore
//   3. 单一入口：CI 和本地都走这个脚本，保证一致性
//   4. 各阶段独立可关闭，便于排障

import { execSync, spawnSync } from "node:child_process";
import { mkdirSync, rmSync, cpSync, existsSync, readdirSync, statSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve, relative } from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const repoRoot = resolve(__dirname, "..");

const args = new Set(process.argv.slice(2));
const skipObfuscate = args.has("--no-obfuscate");
const skipRustBuild = args.has("--skip-rust-build");

const uiSrcDir = join(repoRoot, "apps", "ui");
const uiDistDir = join(uiSrcDir, "dist");
const buildDir = join(repoRoot, "build");
const obfUiDir = join(buildDir, "obfuscated-ui");
const releaseOutDir = join(repoRoot, "dist", "release");
const obfuscatorConfigPath = join(repoRoot, "scripts", "obfuscator.config.json");

const desktopDir = join(repoRoot, "apps", "desktop");

/**
 * 发版专用 RUSTFLAGS：在稳定工具链上把源码/依赖缓存的绝对路径映射成短前缀，
 * 降低 `strings desktop` 命中 `.../apps/desktop/src/...`、registry 路径等可读面。
 * nightly 可选追加见 `TLS_RELEASE_RUSTFLAGS`（例如 `-Zlocation-detail=none`）。
 */
function mergeReleaseRustflags() {
  const chunks = [];
  const base = (process.env.RUSTFLAGS || "").trim();
  if (base) {
    chunks.push(base);
  }

  const normRoot = repoRoot.replace(/\\/g, "/");
  chunks.push(`-Cremap-path-prefix=${normRoot}=.`);

  const cargoHome = process.env.CARGO_HOME;
  if (cargoHome) {
    chunks.push(`-Cremap-path-prefix=${cargoHome.replace(/\\/g, "/")}=/.cargo`);
  }

  const rustupHome = process.env.RUSTUP_HOME;
  if (rustupHome) {
    chunks.push(`-Cremap-path-prefix=${rustupHome.replace(/\\/g, "/")}=/.rustup`);
  }

  const extra = (process.env.TLS_RELEASE_RUSTFLAGS || "").trim();
  if (extra) {
    chunks.push(extra);
  }

  return chunks.join(" ");
}

function log(section, msg) {
  const stamp = new Date().toISOString().split("T")[1].slice(0, 8);
  console.log(`\n[${stamp}] [${section}] ${msg}`);
}

function run(cmd, opts = {}) {
  log("run", `${cmd}${opts.cwd ? ` (cwd=${relative(repoRoot, opts.cwd)})` : ""}`);
  // Tauri CLI 的 --ci 只接受 true/false；若宿主 shell 已有 CI=1（部分 zsh 默认），
  // 透传给子进程会让 cargo tauri build 直接报错。这里把 env 统一规整一遍。
  const env = { ...process.env, ...(opts.env || {}) };
  const ciRaw = (env.CI ?? "").toString().trim();
  if (ciRaw && !/^(true|false)$/i.test(ciRaw)) {
    env.CI = ciRaw === "0" ? "false" : "true";
  }
  execSync(cmd, { stdio: "inherit", ...opts, env });
}

function ensureDir(dir) {
  mkdirSync(dir, { recursive: true });
}

function cleanDir(dir) {
  if (existsSync(dir)) rmSync(dir, { recursive: true, force: true });
  ensureDir(dir);
}

function copyTree(from, to) {
  cpSync(from, to, { recursive: true });
}

// ---------- 阶段 1：前端 Vite 构建 ----------
function buildFrontend() {
  log("frontend", "pnpm --filter tls-shipinhao-ui build");
  run("pnpm --filter tls-shipinhao-ui build", { cwd: repoRoot });
  if (!existsSync(uiDistDir)) {
    throw new Error(`前端构建失败：${uiDistDir} 不存在`);
  }
}

// ---------- 阶段 2：JS 混淆（javascript-obfuscator）----------
function obfuscateFrontend() {
  cleanDir(obfUiDir);
  copyTree(uiDistDir, obfUiDir);
  log("obfuscate", `复制 ${relative(repoRoot, uiDistDir)} → ${relative(repoRoot, obfUiDir)}`);

  if (!existsSync(obfuscatorConfigPath)) {
    log("obfuscate", `警告：未找到 ${relative(repoRoot, obfuscatorConfigPath)}，跳过 JS 混淆`);
    return;
  }

  // 收集所有 .js 文件（排除 .map、html/css 等）
  const jsFiles = [];
  const walk = (d) => {
    for (const entry of readdirSync(d)) {
      const p = join(d, entry);
      const st = statSync(p);
      if (st.isDirectory()) walk(p);
      else if (entry.endsWith(".js") && !entry.endsWith(".min.js.map")) jsFiles.push(p);
    }
  };
  walk(obfUiDir);
  log("obfuscate", `发现 ${jsFiles.length} 个 .js 文件待混淆`);

  // 逐文件跑 javascript-obfuscator CLI，保持相对路径
  for (const jsFile of jsFiles) {
    const rel = relative(obfUiDir, jsFile);
    log("obfuscate", `→ ${rel}`);
    const result = spawnSync(
      "pnpm",
      [
        "--filter",
        "tls-shipinhao-ui",
        "exec",
        "javascript-obfuscator",
        jsFile,
        "--output",
        jsFile,
        "--config",
        obfuscatorConfigPath,
      ],
      { stdio: "inherit", cwd: repoRoot },
    );
    if (result.status !== 0) {
      throw new Error(`javascript-obfuscator 处理失败：${rel}`);
    }
  }
}

// ---------- 阶段 3：Tauri Rust release 构建 ----------
function buildTauri() {
  const frontendDist = skipObfuscate
    ? "../ui/dist" // 相对 apps/desktop 的位置
    : relative(desktopDir, obfUiDir).split("\\").join("/");

  const configOverride = JSON.stringify({ build: { frontendDist } });
  log("tauri", `cargo tauri build --config '${configOverride}'`);

  // 注意：beforeBuildCommand 是 `pnpm --filter tls-shipinhao-ui build`，会再跑一次前端
  // 这里用 --no-bundle 的逻辑也可考虑，但保守：仍让 Tauri 自行跑一次（幂等，不影响混淆产物的 frontendDist）
  // 如果要绕开 beforeBuildCommand，可加环境变量或未来迁移 Tauri 配置
  const rustflags = mergeReleaseRustflags();
  log("tauri", `RUSTFLAGS 已合并 path remap（长度=${rustflags.length}）`);
  run(`cargo tauri build --config '${configOverride}'`, {
    cwd: desktopDir,
    env: { ...process.env, RUSTFLAGS: rustflags },
  });
}

// ---------- 阶段 4：收集产物到 dist/release/ ----------
function collectArtifacts() {
  cleanDir(releaseOutDir);
  const candidates = [
    // 便携版 exe（Windows）
    { from: join(repoRoot, "target", "release", "desktop.exe"), toName: "TLS-shipinhao-portable.exe" },
    // macOS dmg
    { fromGlob: join(repoRoot, "target", "release", "bundle", "dmg"), keep: /\.dmg$/ },
  ];
  for (const c of candidates) {
    if (c.from && existsSync(c.from)) {
      const dest = join(releaseOutDir, c.toName);
      cpSync(c.from, dest);
      log("collect", `✓ ${relative(repoRoot, dest)}`);
    }
    if (c.fromGlob && existsSync(c.fromGlob)) {
      for (const entry of readdirSync(c.fromGlob)) {
        if (c.keep && !c.keep.test(entry)) continue;
        const src = join(c.fromGlob, entry);
        const dest = join(releaseOutDir, entry);
        cpSync(src, dest);
        log("collect", `✓ ${relative(repoRoot, dest)}`);
      }
    }
  }
}

// ---------- 主流程 ----------
(async () => {
  log("boot", `混淆模式=${skipObfuscate ? "关闭" : "开启"} / skipRustBuild=${skipRustBuild}`);
  buildFrontend();
  if (!skipObfuscate) {
    obfuscateFrontend();
  }
  if (skipRustBuild) {
    log("done", "仅前端阶段完成，跳过 Tauri 构建");
    return;
  }
  buildTauri();
  collectArtifacts();
  log("done", `完整发版产物已就绪：${relative(repoRoot, releaseOutDir)}`);
})().catch((err) => {
  console.error("\n[build-release] FATAL:", err?.stack || err);
  process.exit(1);
});
