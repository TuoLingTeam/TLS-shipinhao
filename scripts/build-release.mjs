#!/usr/bin/env node
// 发版编排：前端构建 → JS 混淆 → Tauri release 打包。

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
const releaseOutDir = join(repoRoot, "dist");
const obfuscatorConfigPath = join(repoRoot, "scripts", "obfuscator.config.json");

const desktopDir = join(repoRoot, "apps", "desktop");
const webview2RuntimeSource = process.env.TLS_WEBVIEW2_FIXED_RUNTIME_DIR
  ? resolve(process.env.TLS_WEBVIEW2_FIXED_RUNTIME_DIR)
  : join(repoRoot, "vendor", "webview2-runtime");

/** 合并 release RUSTFLAGS，隐藏源码与依赖缓存绝对路径。 */
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
  // Tauri CLI 只接受 CI=true/false。
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

function cleanBuildDir() {
  if (!existsSync(buildDir)) return;
  try {
    rmSync(buildDir, { recursive: true, force: true });
    log("clean", `✓ 已清理临时构建目录：${relative(repoRoot, buildDir)}/`);
  } catch (err) {
    log("clean", `⚠️ 临时构建目录清理失败：${err?.message || err}`);
  }
}

function copyTree(from, to) {
  cpSync(from, to, { recursive: true });
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

function collectPortableBundle(exePath) {
  if (!existsSync(exePath)) return;
  const portableDir = join(releaseOutDir, "TLS-shipinhao-portable");
  ensureDir(portableDir);
  cpSync(exePath, join(portableDir, "TLS-shipinhao.exe"));

  const runtimeDir = findFixedWebview2Runtime(webview2RuntimeSource);
  if (!runtimeDir) {
    log(
      "collect",
      `未找到 Fixed WebView2 Runtime，跳过便携版内置运行时：${relative(repoRoot, webview2RuntimeSource)}`,
    );
    return;
  }

  cpSync(runtimeDir, join(portableDir, "WebView2Runtime"), { recursive: true });
  log("collect", `✓ ${relative(repoRoot, portableDir)}（含 Fixed WebView2 Runtime）`);
}

function buildFrontend() {
  log("frontend", "pnpm --filter tls-shipinhao-ui build");
  run("pnpm --filter tls-shipinhao-ui build", { cwd: repoRoot });
  if (!existsSync(uiDistDir)) {
    throw new Error(`前端构建失败：${uiDistDir} 不存在`);
  }
}

function obfuscateFrontend() {
  cleanDir(obfUiDir);
  copyTree(uiDistDir, obfUiDir);
  log("obfuscate", `复制 ${relative(repoRoot, uiDistDir)} → ${relative(repoRoot, obfUiDir)}`);

  if (!existsSync(obfuscatorConfigPath)) {
    log("obfuscate", `警告：未找到 ${relative(repoRoot, obfuscatorConfigPath)}，跳过 JS 混淆`);
    return;
  }

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

function buildTauri() {
  const frontendDist = skipObfuscate
    ? "../ui/dist"
    : relative(desktopDir, obfUiDir).split("\\").join("/");

  const configOverride = JSON.stringify({ build: { frontendDist } });
  log("tauri", `cargo tauri build --config '${configOverride}'`);

  const rustflags = mergeReleaseRustflags();
  log("tauri", `RUSTFLAGS 已合并 path remap（长度=${rustflags.length}）`);
  run(`cargo tauri build --config '${configOverride}'`, {
    cwd: desktopDir,
    env: { ...process.env, RUSTFLAGS: rustflags },
  });
}

function collectArtifacts() {
  cleanDir(releaseOutDir);
  collectPortableBundle(join(repoRoot, "target", "release", "desktop.exe"));
  const candidates = [
    { fromGlob: join(repoRoot, "target", "release", "bundle", "nsis"), keep: /\.exe$/, toName: "TLS-shipinhao-windows-setup.exe" },
    { from: join(repoRoot, "target", "release", "desktop.exe"), toName: "TLS-shipinhao-portable.exe" },
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
        const dest = join(releaseOutDir, c.toName || entry);
        cpSync(src, dest);
        log("collect", `✓ ${relative(repoRoot, dest)}`);
      }
    }
  }
}

(async () => {
  try {
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
  } finally {
    cleanBuildDir();
  }
})().catch((err) => {
  console.error("\n[build-release] FATAL:", err?.stack || err);
  process.exit(1);
});
