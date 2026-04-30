#!/usr/bin/env node
// 深度混淆镜像构建：复制 workspace、混淆前端产物，再在镜像内打包。

import { execSync } from "node:child_process";
import { createRequire } from "node:module";
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

// 镜像输出根，避免与源码目录同名。
const OUT_DIR = join(REPO_ROOT, "TLS-shipinhao-release");

// 启动期会与根 Cargo.toml 校验一致性。
const RUST_MEMBER_DIRS = [
  "apps/desktop",
  "backend/contracts",
  "backend/license",
  "backend/worker",
  "apps/desktop/security",
  "scripts/build-tools",
  "scripts/xtask",
];

/** 解析根 Cargo.toml 的 [workspace].members 列表。 */
function parseCargoMembers(cargoTomlPath) {
  const text = readFileSync(cargoTomlPath, "utf8");
  const match = text.match(/\[workspace\][\s\S]*?members\s*=\s*\[([\s\S]*?)\]/);
  if (!match) {
    throw new Error(`未在 ${cargoTomlPath} 中识别到 [workspace].members 段`);
  }
  return match[1]
    .split(/[\n,]/)
    .map((s) => s.trim())
    .filter((s) => s.length > 0 && !s.startsWith("#"))
    .map((s) => s.replace(/^["']|["']$/g, ""));
}

/** 校验脚本成员列表与 Cargo workspace 完全一致。 */
function verifyWorkspaceMembers() {
  const cargoMembers = parseCargoMembers(join(REPO_ROOT, "Cargo.toml"));
  const cargoSet = new Set(cargoMembers);
  const scriptSet = new Set(RUST_MEMBER_DIRS);
  const missingInScript = [...cargoSet].filter((m) => !scriptSet.has(m));
  const missingInCargo = [...scriptSet].filter((m) => !cargoSet.has(m));
  if (missingInScript.length > 0 || missingInCargo.length > 0) {
    const lines = [
      "RUST_MEMBER_DIRS 与根 Cargo.toml [workspace].members 不一致：",
      missingInScript.length > 0 ? `  脚本里缺少：${missingInScript.join(", ")}` : null,
      missingInCargo.length > 0 ? `  Cargo.toml 里缺少：${missingInCargo.join(", ")}` : null,
      "请同步两边后再运行深度混淆构建。",
    ].filter(Boolean);
    throw new Error(lines.join("\n"));
  }
  log("verify", `✓ workspace members 一致（${cargoMembers.length} 个）`);
}

const ROOT_FILES = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"];
const ROOT_DIRS = [".cargo"];

const OBFUSCATOR_CONFIG = join(REPO_ROOT, "scripts", "obfuscator.config.json");
const UI_DIST_SRC = join(REPO_ROOT, "apps", "ui", "dist");
const RELEASE_OUT = join(REPO_ROOT, "dist");
const WEBVIEW2_RUNTIME_SOURCE = process.env.TLS_WEBVIEW2_FIXED_RUNTIME_DIR
  ? resolve(process.env.TLS_WEBVIEW2_FIXED_RUNTIME_DIR)
  : join(REPO_ROOT, "vendor", "webview2-runtime");

// basename 粒度过滤；不要加入 "dist"，否则 apps/ui/dist 会被整体排除。
const EXCLUDE_NAMES = new Set([
  "target",
  "node_modules",
  "build", // backend/worker/build 等运行时产物
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

const KEEP_DOTFILES = new Set([".cargo", ".env.example", ".gitkeep"]);

function log(stage, msg) {
  const t = new Date().toISOString().split("T")[1].slice(0, 8);
  console.log(`[${t}] [${stage}] ${msg}`);
}

function run(cmd, opts = {}) {
  log("run", `${cmd}${opts.cwd ? ` (cwd=${relative(REPO_ROOT, opts.cwd)})` : ""}`);
  const env = { ...process.env, ...(opts.env || {}) };
  // Tauri CLI 只接受 CI=true/false。
  const ciRaw = (env.CI ?? "").toString().trim();
  if (ciRaw && !/^(true|false)$/i.test(ciRaw)) {
    env.CI = ciRaw === "0" ? "false" : "true";
  }
  execSync(cmd, { stdio: "inherit", ...opts, env });
}

function ensureDir(d) {
  mkdirSync(d, { recursive: true });
}

/** 读取混淆配置，去掉 JSON 里以下划线开头的说明性字段（避免传入 obfuscate 报错） */
function loadObfuscatorOptions() {
  const raw = JSON.parse(readFileSync(OBFUSCATOR_CONFIG, "utf8"));
  return Object.fromEntries(Object.entries(raw).filter(([k]) => !k.startsWith("_")));
}

function cleanDir(d) {
  if (existsSync(d)) rmSync(d, { recursive: true, force: true });
  ensureDir(d);
}

function shouldExclude(srcPath) {
  const name = basename(srcPath);
  if (EXCLUDE_NAMES.has(name)) return true;
  if (EXCLUDE_EXTS.has(extname(name))) return true;
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

function findFixedWebview2Runtime(dir) {
  if (!existsSync(dir)) return null;
  if (existsSync(join(dir, "msedgewebview2.exe"))) return dir;
  for (const entry of readdirSync(dir)) {
    const candidate = join(dir, entry);
    if (statSync(candidate).isDirectory() && existsSync(join(candidate, "msedgewebview2.exe"))) {
      return dir;
    }
  }
  return null;
}

function stage_clean() {
  log("clean", `清空 ${relative(REPO_ROOT, OUT_DIR)}/`);
  cleanDir(OUT_DIR);
}

function stage_buildFrontend() {
  log("frontend", "pnpm --filter tls-shipinhao-ui build");
  run("pnpm --filter tls-shipinhao-ui build", { cwd: REPO_ROOT });
  if (!existsSync(UI_DIST_SRC)) {
    throw new Error(`前端构建失败：${UI_DIST_SRC} 不存在`);
  }
}

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

function stage_mirrorUiDist() {
  const dst = join(OUT_DIR, "apps", "ui", "dist");
  ensureDir(dirname(dst));
  copyTree(UI_DIST_SRC, dst);
  log("mirror", `✓ apps/ui/dist → ${relative(REPO_ROOT, dst)}/`);
}

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

  // 直接调用 obfuscate API，避开 Windows pnpm.cmd 解析差异。
  const requireUi = createRequire(join(REPO_ROOT, "apps", "ui", "package.json"));
  const { obfuscate } = requireUi("javascript-obfuscator");
  const obfuscatorOptions = loadObfuscatorOptions();

  let okCount = 0;
  let failCount = 0;
  let srcTotal = 0;
  let dstTotal = 0;

  for (const f of jsFiles) {
    const rel = relative(OUT_DIR, f);
    const srcSize = statSync(f).size;
    const start = Date.now();

    try {
      const code = readFileSync(f, "utf8");
      const obfuscated = obfuscate(code, obfuscatorOptions);
      writeFileSync(f, obfuscated.getObfuscatedCode(), "utf8");
      const elapsed = ((Date.now() - start) / 1000).toFixed(1);
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
    } catch (err) {
      const elapsed = ((Date.now() - start) / 1000).toFixed(1);
      log("obfuscate", `⚠️  失败(${elapsed}s)：${rel}`);
      console.error(err?.stack || String(err));
      failCount++;
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

function stage_hardenRustflags() {
  const cargoConfigPath = join(OUT_DIR, ".cargo", "config.toml");
  ensureDir(dirname(cargoConfigPath));
  const cargoHome = (process.env.CARGO_HOME || join(homedir(), ".cargo")).replace(/\\/g, "/");
  const mirror = OUT_DIR.replace(/\\/g, "/");
  const config = `# 此文件由 scripts/build-obfuscate.mjs 在镜像目录内覆盖写入，源码目录不动。
[build]
target-dir = "target"
rustflags = [
  "--remap-path-prefix", "${mirror}/apps/desktop/src/services/=svc/",
  "--remap-path-prefix", "${mirror}/apps/desktop/src/domain/=dom/",
  "--remap-path-prefix", "${mirror}/apps/desktop/src=s",
  "--remap-path-prefix", "${mirror}/apps/desktop/security/=sec/",
  "--remap-path-prefix", "${mirror}/backend/=b/",
  "--remap-path-prefix", "${mirror}/scripts/=x/",
  "--remap-path-prefix", "${cargoHome}=r",
  "-Z", "location-detail=none",
]
`;
  writeFileSync(cargoConfigPath, config);
  log("harden", `✓ 注入 rustflags（mirror remap + cargo registry + location-detail=none）`);
}

function stage_buildTauri() {
  const desktopDir = join(OUT_DIR, "apps", "desktop");
  const mirrorTarget = join(OUT_DIR, "target");
  const noBundle =
    process.env.TLS_TAURI_NO_BUNDLE === "1" || process.env.TLS_WINDOWS_PORTABLE_ONLY === "1";
  const tauriCmd = noBundle ? "cargo tauri build --no-bundle" : "cargo tauri build";
  log("tauri", `${tauriCmd} (cwd=${relative(REPO_ROOT, desktopDir)})`);
  run(tauriCmd, {
    cwd: desktopDir,
    env: {
      CARGO_TARGET_DIR: mirrorTarget,
      RUSTC_BOOTSTRAP: "1",
    },
  });
}

function stage_collectArtifacts() {
  cleanDir(RELEASE_OUT);
  const portableExe = join(OUT_DIR, "target", "release", "desktop.exe");
  if (existsSync(portableExe)) {
    const portableDir = join(RELEASE_OUT, "TLS-shipinhao-portable");
    ensureDir(portableDir);
    cpSync(portableExe, join(portableDir, "TLS-shipinhao.exe"));

    const runtimeDir = findFixedWebview2Runtime(WEBVIEW2_RUNTIME_SOURCE);
    if (runtimeDir) {
      cpSync(runtimeDir, join(portableDir, "WebView2Runtime"), { recursive: true });
      log("collect", `✓ ${relative(REPO_ROOT, portableDir)}（含 Fixed WebView2 Runtime）`);
    } else {
      log(
        "collect",
        `未找到 Fixed WebView2 Runtime，跳过便携版内置运行时：${relative(REPO_ROOT, WEBVIEW2_RUNTIME_SOURCE)}`,
      );
    }
  }

  const sources = [
    {
      fromDir: join(OUT_DIR, "target", "release", "bundle", "nsis"),
      match: /\.exe$/,
      name: "TLS-shipinhao-windows-setup.exe",
    },
    {
      from: join(OUT_DIR, "target", "release", "desktop.exe"),
      name: "TLS-shipinhao-portable.exe",
    },
    {
      fromDir: join(OUT_DIR, "target", "release", "bundle", "dmg"),
      match: /\.dmg$/,
    },
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
        const dst = join(RELEASE_OUT, s.name || entry);
        cpSync(src, dst, { recursive: true });
        log("collect", `✓ ${relative(REPO_ROOT, dst)}`);
      }
    }
  }
}

const args = new Set(process.argv.slice(2));
const skipFrontend = args.has("--skip-frontend");
const skipBuild = args.has("--skip-build");

(async () => {
  const t0 = Date.now();
  log(
    "boot",
    `OUT_DIR=${relative(REPO_ROOT, OUT_DIR)} skipFrontend=${skipFrontend} skipBuild=${skipBuild}`,
  );

  verifyWorkspaceMembers();
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
