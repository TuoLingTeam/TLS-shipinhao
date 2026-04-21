#!/usr/bin/env node
// ========================================
// 深度混淆镜像构建脚本
// 参考：TLS-douyin-CF/build-obfuscate.js（扁平 walk + JS 深混 + 逐文件日志）
// 适配：Tauri 项目（前端 + Rust workspace）
//
// 设计原则：
//   - 源码 readonly：0 改动 / 0 写入 apps/**、crates/**、backend/**、tools/**
//   - 镜像输出：TLS-shipinhao-release/（workspace 根级副本），每次全量重建
//   - 混淆范围：Vite 产物里的 .js 深度混淆，其它文件 1:1 复制
//   - Rust 侧：源码整体复制，依赖 release profile 现有硬化（lto/strip/panic=abort）
//
// 阶段：
//   1. clean            清空 TLS-shipinhao-release/
//   2. frontend         pnpm --filter tls-shipinhao-ui build → apps/ui/dist
//   3. mirror-rust      复制所有 workspace member 源码 + Cargo.toml / Cargo.lock / .cargo
//   4. mirror-ui-dist   apps/ui/dist → TLS-shipinhao-release/apps/ui/dist
//   5. obfuscate-js     扁平 walk TLS-shipinhao-release/apps/ui/dist 内所有 .js 深度混淆
//   6. rewrite-conf     去掉镜像里 tauri.conf.json 的 beforeBuildCommand，避免二次 vite build
//   7. build-tauri      在 TLS-shipinhao-release/apps/desktop 跑 cargo tauri build
//   8. collect          dmg / exe → dist/release/
//
// 用法：
//   node scripts/build-obfuscate.mjs                  完整流程
//   node scripts/build-obfuscate.mjs --skip-build     只跑混淆，不编译（验证镜像是否干净）
//   node scripts/build-obfuscate.mjs --skip-frontend  复用现有 apps/ui/dist
// ========================================

import { execSync, spawnSync } from "node:child_process";
import {
  mkdirSync,
  rmSync,
  cpSync,
  existsSync,
  readdirSync,
  statSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, relative, resolve, basename, extname } from "node:path";
import { fileURLToPath } from "node:url";
import { homedir } from "node:os";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const REPO_ROOT = resolve(__dirname, "..");

// 镜像输出根：与源码根同级的子目录（大小写差异命名，避免与项目同名误识别）
const OUT_DIR = join(REPO_ROOT, "TLS-shipinhao-release");

// Rust workspace 成员（保持与根 Cargo.toml 一致，漏一个 cargo 会报错）
const RUST_MEMBER_DIRS = [
  "apps/desktop",
  "backend/api-contracts",
  "backend/license-service",
  "backend/license-worker",
  "crates/domain-core",
  "crates/security-core",
  "crates/desktop-services",
  "tools/build-tools",
  "tools/xtask",
];

// workspace 根必需的 manifest（不复制就没法构建）
const ROOT_FILES = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"];
const ROOT_DIRS = [".cargo"];

const OBFUSCATOR_CONFIG = join(REPO_ROOT, "scripts", "obfuscator.config.json");
const UI_DIST_SRC = join(REPO_ROOT, "apps", "ui", "dist");
const RELEASE_OUT = join(REPO_ROOT, "dist", "release");

// 复制源码时的过滤清单（basename 粒度，避免把构建产物 / 运行时文件带进镜像）
// 注意：不要把 "dist" 加进来，否则 copyTree(apps/ui/dist, ...) 会因源路径 basename=dist 被整体排除
const EXCLUDE_NAMES = new Set([
  "target",
  "node_modules",
  "build", // backend/license-worker/build 等运行时产物
  ".DS_Store",
  "Thumbs.db",
  "__pycache__",
  ".pytest_cache",
  ".venv",
  ".playwright-mcp",
  ".worktrees",
  "cookie.txt",
  ".env",
]);

const EXCLUDE_EXTS = new Set([".log"]);

// 点号开头的文件/目录白名单（默认排除所有 dotfiles，但这些是构建必需）
const KEEP_DOTFILES = new Set([".cargo", ".env.example", ".gitkeep"]);

// ========================================
// 工具
// ========================================
function log(stage, msg) {
  const t = new Date().toISOString().split("T")[1].slice(0, 8);
  console.log(`[${t}] [${stage}] ${msg}`);
}

function run(cmd, opts = {}) {
  log("run", `${cmd}${opts.cwd ? ` (cwd=${relative(REPO_ROOT, opts.cwd)})` : ""}`);
  const env = { ...process.env, ...(opts.env || {}) };
  // Tauri CLI 对 CI=0/1 会当成非 bool 字面量报错，规整一下
  const ciRaw = (env.CI ?? "").toString().trim();
  if (ciRaw && !/^(true|false)$/i.test(ciRaw)) {
    env.CI = ciRaw === "0" ? "false" : "true";
  }
  execSync(cmd, { stdio: "inherit", ...opts, env });
}

function ensureDir(d) {
  mkdirSync(d, { recursive: true });
}

function cleanDir(d) {
  if (existsSync(d)) rmSync(d, { recursive: true, force: true });
  ensureDir(d);
}

function shouldExclude(srcPath) {
  const name = basename(srcPath);
  if (EXCLUDE_NAMES.has(name)) return true;
  if (EXCLUDE_EXTS.has(extname(name))) return true;
  // 点号开头默认排除（.git .cursor .codex .windsurf .env 等），白名单豁免
  if (name.startsWith(".") && !KEEP_DOTFILES.has(name)) return true;
  return false;
}

function copyTree(src, dst) {
  cpSync(src, dst, {
    recursive: true,
    filter: (s) => !shouldExclude(s),
  });
}

function formatKB(bytes) {
  return (bytes / 1024).toFixed(1) + "KB";
}

// ========================================
// 阶段实现
// ========================================

// 阶段 1：清空输出
function stage_clean() {
  log("clean", `清空 ${relative(REPO_ROOT, OUT_DIR)}/`);
  cleanDir(OUT_DIR);
}

// 阶段 2：前端 Vite 构建
function stage_buildFrontend() {
  log("frontend", "pnpm --filter tls-shipinhao-ui build");
  run("pnpm --filter tls-shipinhao-ui build", { cwd: REPO_ROOT });
  if (!existsSync(UI_DIST_SRC)) {
    throw new Error(`前端构建失败：${UI_DIST_SRC} 不存在`);
  }
}

// 阶段 3：Rust workspace 源码镜像
function stage_mirrorRust() {
  for (const member of RUST_MEMBER_DIRS) {
    const src = join(REPO_ROOT, member);
    const dst = join(OUT_DIR, member);
    if (!existsSync(src)) {
      log("mirror", `⚠️  缺失 member 目录，跳过：${member}`);
      continue;
    }
    ensureDir(dirname(dst));
    copyTree(src, dst);
    log("mirror", `✓ ${member}`);
  }

  for (const f of ROOT_FILES) {
    const src = join(REPO_ROOT, f);
    if (!existsSync(src)) continue;
    cpSync(src, join(OUT_DIR, f));
    log("mirror", `✓ ${f}`);
  }

  for (const d of ROOT_DIRS) {
    const src = join(REPO_ROOT, d);
    if (!existsSync(src)) continue;
    copyTree(src, join(OUT_DIR, d));
    log("mirror", `✓ ${d}/`);
  }
}

// 阶段 4：前端构建产物镜像
function stage_mirrorUiDist() {
  const dst = join(OUT_DIR, "apps", "ui", "dist");
  ensureDir(dirname(dst));
  copyTree(UI_DIST_SRC, dst);
  log("mirror", `✓ apps/ui/dist → ${relative(REPO_ROOT, dst)}/`);
}

// 阶段 5：JS 深度混淆（扁平 walk + 逐文件）
function stage_obfuscateJs() {
  if (!existsSync(OBFUSCATOR_CONFIG)) {
    throw new Error(`缺少混淆配置：${OBFUSCATOR_CONFIG}`);
  }
  const distDir = join(OUT_DIR, "apps", "ui", "dist");

  const jsFiles = [];
  const walk = (d) => {
    for (const entry of readdirSync(d)) {
      const p = join(d, entry);
      const st = statSync(p);
      if (st.isDirectory()) walk(p);
      else if (p.endsWith(".js") && !p.endsWith(".min.js")) jsFiles.push(p);
    }
  };
  walk(distDir);
  log("obfuscate", `发现 ${jsFiles.length} 个 .js 文件待混淆`);

  let okCount = 0;
  let failCount = 0;
  let srcTotal = 0;
  let dstTotal = 0;

  for (const f of jsFiles) {
    const rel = relative(OUT_DIR, f);
    const srcSize = statSync(f).size;
    const start = Date.now();

    const result = spawnSync(
      "pnpm",
      [
        "--filter",
        "tls-shipinhao-ui",
        "exec",
        "javascript-obfuscator",
        f,
        "--output",
        f,
        "--config",
        OBFUSCATOR_CONFIG,
      ],
      {
        stdio: ["ignore", "pipe", "pipe"],
        cwd: REPO_ROOT,
        encoding: "utf8",
        timeout: 180_000,
      },
    );
    const elapsed = ((Date.now() - start) / 1000).toFixed(1);

    if (result.status !== 0) {
      const errHead = (result.stderr || "")
        .split("\n")
        .slice(0, 3)
        .map((l) => `   ${l}`)
        .join("\n");
      log("obfuscate", `⚠️  失败(${elapsed}s)：${rel}`);
      console.error(errHead);
      failCount++;
    } else {
      const dstSize = statSync(f).size;
      srcTotal += srcSize;
      dstTotal += dstSize;
      const ratio =
        dstSize >= srcSize
          ? `+${(((dstSize - srcSize) / srcSize) * 100).toFixed(0)}%`
          : `-${(((srcSize - dstSize) / srcSize) * 100).toFixed(0)}%`;
      log(
        "obfuscate",
        `🔒 ${rel} ${formatKB(srcSize)} → ${formatKB(dstSize)} (${ratio}, ${elapsed}s)`,
      );
      okCount++;
    }
  }

  log(
    "obfuscate",
    `完成：成功 ${okCount} / 失败 ${failCount} / 总 ${formatKB(srcTotal)} → ${formatKB(dstTotal)}`,
  );
  if (failCount > 0) {
    throw new Error("有 JS 混淆失败，中止构建（详见上方日志）");
  }
}

// 阶段 6：改写 tauri.conf.json
// 镜像里没有 apps/ui/src，不能再让 beforeBuildCommand 跑 vite build（会覆盖混淆产物）
function stage_rewriteTauriConf() {
  const confPath = join(OUT_DIR, "apps", "desktop", "tauri.conf.json");
  if (!existsSync(confPath)) {
    throw new Error(`镜像里找不到 tauri.conf.json：${confPath}`);
  }
  const conf = JSON.parse(readFileSync(confPath, "utf8"));
  conf.build = conf.build || {};
  delete conf.build.beforeBuildCommand;
  delete conf.build.beforeDevCommand;
  conf.build.frontendDist = "../ui/dist";
  writeFileSync(confPath, JSON.stringify(conf, null, 2) + "\n");
  log("conf", "✓ 改写 apps/desktop/tauri.conf.json（去 beforeBuildCommand/beforeDevCommand）");
}

// 阶段 6b：注入 Rust 二进制加固 rustflags
// 作用：
//   1. --remap-path-prefix：重写 file!()/panic location/debuginfo 中的绝对路径
//      （镜像内源码在 TLS-shipinhao-release/ 下，按子目录压成单字母前缀，strings 不易还原树形）
//   2. -Z location-detail=none：panic / track_caller 等不再嵌入 file/line/column（需 RUSTC_BOOTSTRAP=1）
// 说明：tracing 宏里的路径也走 file!()，额外依赖 workspace 里 tracing 的 release_max_level_off
//       先把 callsite 里「带行号的 event ...」打薄；remap 再把裸路径变短。
function stage_hardenRustflags() {
  const cargoConfigPath = join(OUT_DIR, ".cargo", "config.toml");
  ensureDir(dirname(cargoConfigPath));
  const cargoHome = (process.env.CARGO_HOME || join(homedir(), ".cargo")).replace(/\\/g, "/");
  const mirror = OUT_DIR.replace(/\\/g, "/");
  const config = `# 此文件由 scripts/build-obfuscate.mjs 在镜像目录内覆盖写入，源码目录不动。
[build]
target-dir = "target"
rustflags = [
  "--remap-path-prefix", "${mirror}/apps/desktop/src=s",
  "--remap-path-prefix", "${mirror}/crates/=c/",
  "--remap-path-prefix", "${mirror}/backend/=b/",
  "--remap-path-prefix", "${mirror}/tools/=t/",
  "--remap-path-prefix", "${cargoHome}=r",
  "-Z", "location-detail=none",
]
`;
  writeFileSync(cargoConfigPath, config);
  log("harden", `✓ 注入 rustflags（mirror remap + cargo registry + location-detail=none）`);
}

// 阶段 7：Tauri 构建
function stage_buildTauri() {
  const desktopDir = join(OUT_DIR, "apps", "desktop");
  const mirrorTarget = join(OUT_DIR, "target");
  log("tauri", `cargo tauri build (cwd=${relative(REPO_ROOT, desktopDir)})`);
  // - CARGO_TARGET_DIR：强制 target 落在镜像内，不污染源码根
  // - RUSTC_BOOTSTRAP=1：允许 stable rustc 使用 -Z unstable flag（本项目用到 -Z location-detail=none）
  run("cargo tauri build", {
    cwd: desktopDir,
    env: {
      CARGO_TARGET_DIR: mirrorTarget,
      RUSTC_BOOTSTRAP: "1",
    },
  });
}

// 阶段 8：收集产物
function stage_collectArtifacts() {
  cleanDir(RELEASE_OUT);
  const sources = [
    // Windows 便携 exe
    {
      from: join(OUT_DIR, "target", "release", "desktop.exe"),
      name: "TLS-shipinhao-portable.exe",
    },
    // macOS dmg
    {
      fromDir: join(OUT_DIR, "target", "release", "bundle", "dmg"),
      match: /\.dmg$/,
    },
    // macOS .app（可选，便于本地测试）
    {
      fromDir: join(OUT_DIR, "target", "release", "bundle", "macos"),
      match: /\.app$/,
    },
  ];

  for (const s of sources) {
    if (s.from && existsSync(s.from)) {
      const dst = join(RELEASE_OUT, s.name);
      cpSync(s.from, dst);
      log("collect", `✓ ${relative(REPO_ROOT, dst)}`);
    }
    if (s.fromDir && existsSync(s.fromDir)) {
      for (const entry of readdirSync(s.fromDir)) {
        if (s.match && !s.match.test(entry)) continue;
        const src = join(s.fromDir, entry);
        const dst = join(RELEASE_OUT, entry);
        cpSync(src, dst, { recursive: true });
        log("collect", `✓ ${relative(REPO_ROOT, dst)}`);
      }
    }
  }
}

// ========================================
// 主流程
// ========================================
const args = new Set(process.argv.slice(2));
const skipFrontend = args.has("--skip-frontend");
const skipBuild = args.has("--skip-build");

(async () => {
  const t0 = Date.now();
  log(
    "boot",
    `OUT_DIR=${relative(REPO_ROOT, OUT_DIR)} skipFrontend=${skipFrontend} skipBuild=${skipBuild}`,
  );

  stage_clean();
  if (!skipFrontend) {
    stage_buildFrontend();
  } else if (!existsSync(UI_DIST_SRC)) {
    throw new Error(`--skip-frontend 要求 ${UI_DIST_SRC} 已存在`);
  }
  stage_mirrorRust();
  stage_mirrorUiDist();
  stage_obfuscateJs();
  stage_rewriteTauriConf();
  stage_hardenRustflags();

  if (skipBuild) {
    const t = ((Date.now() - t0) / 1000).toFixed(1);
    log("done", `✅ 仅镜像阶段完成，未执行 cargo tauri build（${t}s）`);
    log(
      "hint",
      `人工验证：cd ${relative(REPO_ROOT, OUT_DIR)}/apps/desktop && cargo tauri build`,
    );
    return;
  }

  stage_buildTauri();
  stage_collectArtifacts();

  const t = ((Date.now() - t0) / 1000).toFixed(1);
  log("done", `✅ 深度混淆构建完成，耗时 ${t}s`);
})().catch((err) => {
  console.error("\n[FATAL]", err?.stack || err);
  process.exit(1);
});
