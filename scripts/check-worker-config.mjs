#!/usr/bin/env node
// 守住 Cloudflare Worker 发版前的关键配置：
//
// - `backend/wrangler.toml` 静态结构：name / main / routes / [[d1_databases]]
//   任一字段缺失或为空都直接报错。
// - 远端 secret：必备 `ADMIN_SECRET` 与 `LICENSE_SIGNING_PRIVATE_KEY_B64`
//   缺失会让 `/api/admin/*` 与签名 Lease 全链路在线上炸掉，必须在 deploy 前
//   挡住。需 wrangler 处于已登录态；用 `npx wrangler login` 完成 OAuth。
//
// 用法：
//   pnpm worker:check                 # 默认走完静态 + 远端两段
//   node scripts/check-worker-config.mjs --skip-remote   # 仅静态结构（CI 用）
//   node scripts/check-worker-config.mjs --no-color      # 关 ANSI 颜色

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(__dirname, "..");
const WRANGLER_TOML = join(REPO_ROOT, "backend/wrangler.toml");

const args = new Set(process.argv.slice(2));
const SKIP_REMOTE = args.has("--skip-remote");
const NO_COLOR = args.has("--no-color") || !process.stdout.isTTY;

const REQUIRED_SECRETS = ["ADMIN_SECRET", "LICENSE_SIGNING_PRIVATE_KEY_B64"];

function paint(color, text) {
  if (NO_COLOR) return text;
  const codes = { red: 31, green: 32, yellow: 33, cyan: 36 };
  return `\u001b[${codes[color] ?? 0}m${text}\u001b[0m`;
}

function fail(message) {
  console.error(paint("red", "[worker:check] ✗ ") + message);
  process.exit(1);
}

function info(message) {
  console.log(paint("cyan", "[worker:check] ") + message);
}

function ok(message) {
  console.log(paint("green", "[worker:check] ✓ ") + message);
}

function warn(message) {
  console.warn(paint("yellow", "[worker:check] ⚠ ") + message);
}

function readWranglerToml() {
  try {
    return readFileSync(WRANGLER_TOML, "utf-8");
  } catch (error) {
    fail(`无法读取 ${WRANGLER_TOML}：${error.message}`);
  }
}

function checkStaticField(source, field) {
  const match = source.match(new RegExp(`^${field}\\s*=\\s*"(.+?)"`, "m"));
  if (!match || match[1].trim().length === 0) {
    fail(`backend/wrangler.toml 缺少非空 \`${field} = "..."\``);
  }
  return match[1];
}

function checkRoutes(source) {
  const match = source.match(/^routes\s*=\s*\[([\s\S]*?)\]/m);
  if (!match) {
    fail("backend/wrangler.toml 缺少 `routes = [...]` 数组");
  }
  const entries = [...match[1].matchAll(/\{[^}]*pattern\s*=\s*"([^"]+)"/g)];
  if (entries.length === 0) {
    fail("backend/wrangler.toml 中 routes 数组为空，至少需要 1 条 pattern");
  }
  return entries.map((m) => m[1]);
}

function checkD1Bindings(source) {
  const blocks = [...source.matchAll(/\[\[d1_databases\]\]([\s\S]*?)(?=\n\[|\n*$)/g)];
  if (blocks.length === 0) {
    fail("backend/wrangler.toml 缺少 `[[d1_databases]]` 配置");
  }
  const bindings = blocks.map((block, index) => {
    const body = block[1];
    const required = ["binding", "database_name", "database_id"];
    const fields = {};
    for (const field of required) {
      const m = body.match(new RegExp(`^\\s*${field}\\s*=\\s*"(.+?)"`, "m"));
      if (!m || m[1].trim().length === 0) {
        fail(`第 ${index + 1} 个 [[d1_databases]] 缺少非空 \`${field}\``);
      }
      fields[field] = m[1];
    }
    return fields;
  });
  return bindings;
}

function runStaticChecks() {
  info("校验 backend/wrangler.toml 静态结构");
  const source = readWranglerToml();
  const name = checkStaticField(source, "name");
  const main = checkStaticField(source, "main");
  const routes = checkRoutes(source);
  const dbs = checkD1Bindings(source);
  ok(
    `静态结构通过：name=${name} · main=${main} · routes=${routes.length} 条 · D1=${dbs.length} 个`,
  );
  return { name, main, routes, dbs };
}

function runWranglerJson(args) {
  try {
    const out = execFileSync("npx", ["--yes", "wrangler", ...args], {
      cwd: join(REPO_ROOT, "backend"),
      stdio: ["ignore", "pipe", "pipe"],
      encoding: "utf-8",
    });
    return out;
  } catch (error) {
    const stderr = error.stderr?.toString() ?? "";
    const stdout = error.stdout?.toString() ?? "";
    return { error: error.message, stderr, stdout };
  }
}

function runRemoteChecks(workerName) {
  info("校验 Cloudflare 远端登录态（npx wrangler whoami）");
  const whoami = runWranglerJson(["whoami"]);
  if (typeof whoami === "object" && whoami.error) {
    fail(
      `wrangler whoami 失败，请先 \`npx wrangler login\`：\n${whoami.stderr || whoami.error}`,
    );
  }
  ok("wrangler 已登录");

  info(`校验远端 secret（npx wrangler secret list --name ${workerName}）`);
  const result = runWranglerJson([
    "secret",
    "list",
    "--name",
    workerName,
  ]);
  if (typeof result === "object" && result.error) {
    fail(
      `wrangler secret list 失败：\n${result.stderr || result.error}\n` +
        "请确认 Worker 已部署过、当前账号有权限、网络可达。",
    );
  }
  // wrangler secret list 输出形如：[{ "name": "X", "type": "secret_text" }]
  let secrets;
  try {
    const jsonStart = result.indexOf("[");
    const jsonEnd = result.lastIndexOf("]");
    if (jsonStart < 0 || jsonEnd < 0) {
      throw new Error("输出中未找到 JSON 数组");
    }
    secrets = JSON.parse(result.slice(jsonStart, jsonEnd + 1));
  } catch (error) {
    fail(`wrangler secret list 输出解析失败：${error.message}\n原始输出：\n${result}`);
  }
  const names = new Set(secrets.map((s) => s.name));
  const missing = REQUIRED_SECRETS.filter((name) => !names.has(name));
  if (missing.length > 0) {
    fail(
      `远端缺少必备 secret：${missing.join(", ")}\n` +
        "请用 `cd backend && npx wrangler secret put <NAME>` 设置后重试。",
    );
  }
  ok(`远端 secret 通过：${REQUIRED_SECRETS.join(", ")} 均已配置（共 ${secrets.length} 个）`);
}

function main() {
  const { name } = runStaticChecks();
  if (SKIP_REMOTE) {
    warn("已传 --skip-remote，跳过 wrangler 登录与 secret 检查（仅适合 CI / 离线场景）");
    return;
  }
  runRemoteChecks(name);
  console.log(paint("green", "[worker:check] 全部通过，可以 deploy"));
}

main();
