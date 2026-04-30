#!/usr/bin/env node
// 校验 LicenseState 前后端 SSoT 一致性。

import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(__dirname, "..");

const RUST_PATH = join(REPO_ROOT, "backend/src/contracts.rs");
const TS_PATH = join(REPO_ROOT, "apps/ui/src/license/types.ts");

/** 从 Rust 源中提取 LICENSE_STATE_SERDE_LABELS 数组（snake_case 字符串）。 */
function extractRustLabels(source) {
  const match = source.match(
    /pub const LICENSE_STATE_SERDE_LABELS:\s*&\[&str\]\s*=\s*&\[([\s\S]*?)\];/,
  );
  if (!match) {
    throw new Error(
      `未在 ${RUST_PATH} 找到 LICENSE_STATE_SERDE_LABELS 常量；新增/重命名时是否漏改了 SSoT？`,
    );
  }
  return [...match[1].matchAll(/"([a-z_]+)"/g)].map((m) => m[1]);
}

/** 从 TS 源中提取 LICENSE_STATE_LABELS 的所有 key。 */
function extractTsKeys(source) {
  const match = source.match(
    /export const LICENSE_STATE_LABELS:\s*Record<LicenseState,\s*string>\s*=\s*\{([\s\S]*?)\};/,
  );
  if (!match) {
    throw new Error(
      `未在 ${TS_PATH} 找到 LICENSE_STATE_LABELS；前端 SSoT 缺失？`,
    );
  }
  return [...match[1].matchAll(/^\s*([a-z_]+)\s*:/gm)].map((m) => m[1]);
}

function diffSets(left, right) {
  const a = new Set(left);
  const b = new Set(right);
  const onlyLeft = [...a].filter((x) => !b.has(x));
  const onlyRight = [...b].filter((x) => !a.has(x));
  return { onlyLeft, onlyRight };
}

function main() {
  const rustSource = readFileSync(RUST_PATH, "utf-8");
  const tsSource = readFileSync(TS_PATH, "utf-8");

  const rustLabels = extractRustLabels(rustSource);
  const tsKeys = extractTsKeys(tsSource);

  if (rustLabels.length === 0) {
    console.error("[check] Rust LICENSE_STATE_SERDE_LABELS 为空，停止");
    process.exit(1);
  }

  const { onlyLeft: missingInTs, onlyRight: missingInRust } = diffSets(
    rustLabels,
    tsKeys,
  );

  if (missingInTs.length === 0 && missingInRust.length === 0) {
    console.log(
      `[check] ✓ LicenseState 前后端 ${rustLabels.length} 项一致：${rustLabels.join(", ")}`,
    );
    return;
  }

  console.error("[check] ✗ LicenseState 前后端漂移：");
  if (missingInTs.length > 0) {
    console.error(`  TS 缺少：${missingInTs.join(", ")}`);
  }
  if (missingInRust.length > 0) {
    console.error(`  Rust 缺少：${missingInRust.join(", ")}`);
  }
  console.error(
    "请同步以下三处后再提交：\n" +
      "  1. backend/src/contracts.rs::LicenseState 变体\n" +
      "  2. backend/src/contracts.rs::LICENSE_STATE_SERDE_LABELS 常量\n" +
      "  3. apps/ui/src/license/types.ts::LICENSE_STATE / LICENSE_STATE_LABELS",
  );
  process.exit(1);
}

main();
