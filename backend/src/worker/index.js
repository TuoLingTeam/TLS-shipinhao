/**
 * TLS-shipinhao 卡密验证后端
 * Cloudflare Workers + D1
 */

import ADMIN_HTML from "../admin/admin.html";

// ---------------------------------------------------------------------------
// 工具函数
// ---------------------------------------------------------------------------

const B32_ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
const KEY_PREFIX = "TLS-";
const PAYLOAD_LEN = 10; // 2 (days) + 2 (salt) + 6 (hmac truncated)
const DEFAULT_PLAN_DAYS = 30;
const ADMIN_ACTOR = "admin";

let schemaReadyPromise = null;

/** Base32 解码（RFC 4648，兼容无填充输入） */
function base32Decode(input) {
  const raw = input.toUpperCase().replace(/=+$/, "");
  let bits = 0;
  let value = 0;
  const output = [];
  for (const ch of raw) {
    const idx = B32_ALPHABET.indexOf(ch);
    if (idx === -1) throw new Error("invalid base32 character");
    value = (value << 5) | idx;
    bits += 5;
    if (bits >= 8) {
      bits -= 8;
      output.push((value >>> bits) & 0xff);
    }
  }
  return new Uint8Array(output);
}

/** Base32 编码（RFC 4648，不带填充） */
function base32Encode(data) {
  let bits = 0;
  let value = 0;
  let result = "";
  for (const byte of data) {
    value = (value << 8) | byte;
    bits += 8;
    while (bits >= 5) {
      bits -= 5;
      result += B32_ALPHABET[(value >>> bits) & 0x1f];
    }
  }
  if (bits > 0) result += B32_ALPHABET[(value << (5 - bits)) & 0x1f];
  return result;
}

/** HMAC-SHA256 签名（使用 Web Crypto API） */
async function hmacSha256(secret, data) {
  const key = await crypto.subtle.importKey(
    "raw",
    secret,
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"],
  );
  return new Uint8Array(await crypto.subtle.sign("HMAC", key, data));
}

/** 恒定时间比较 */
function constantTimeEqual(a, b) {
  if (a.length !== b.length) return false;
  let result = 0;
  for (let i = 0; i < a.length; i += 1) result |= a[i] ^ b[i];
  return result === 0;
}

/** 当前时间 ISO 格式 */
function nowISO() {
  return new Date().toISOString().replace(/\.\d{3}Z$/, "+00:00");
}

/** 计算过期时间 ISO 格式 */
function expiresISO(days) {
  return new Date(Date.now() + days * 86400000).toISOString().replace(/\.\d{3}Z$/, "+00:00");
}

function startOfTodayISO() {
  const date = new Date();
  date.setUTCHours(0, 0, 0, 0);
  return date.toISOString().replace(/\.\d{3}Z$/, "+00:00");
}

function normalizeKey(key) {
  return String(key || "").trim().toUpperCase();
}

function normalizeReason(reason, fallback) {
  return String(reason || "").trim().slice(0, 200) || fallback;
}

function detailJson(detail) {
  return JSON.stringify(detail || {});
}

// ---------------------------------------------------------------------------
// 响应构造
// ---------------------------------------------------------------------------

function jsonResponse(data, status = 200) {
  return new Response(JSON.stringify(data), {
    status,
    headers: {
      "Content-Type": "application/json; charset=utf-8",
      "Access-Control-Allow-Origin": "*",
    },
  });
}

function errorResponse(message, status = 400, extra = {}) {
  return jsonResponse({ success: false, message, ...extra }, status);
}

function htmlResponse(html) {
  return new Response(html, {
    headers: { "Content-Type": "text/html; charset=utf-8" },
  });
}

// ---------------------------------------------------------------------------
// 管理员鉴权 / schema
// ---------------------------------------------------------------------------

function checkAdmin(request, env) {
  const enc = new TextEncoder();
  const auth = enc.encode(request.headers.get("X-Admin-Secret") || "");
  const secret = enc.encode(env.ADMIN_SECRET || "");
  if (auth.length !== secret.length) return false;
  return constantTimeEqual(auth, secret);
}

async function ensureSchema(env) {
  if (!schemaReadyPromise) {
    schemaReadyPromise = (async () => {
      await env.DB.exec(`
        CREATE TABLE IF NOT EXISTS admin_audit_logs (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          action TEXT NOT NULL,
          license_key TEXT NOT NULL DEFAULT '',
          admin_actor TEXT NOT NULL DEFAULT 'admin',
          reason TEXT NOT NULL DEFAULT '',
          detail_json TEXT NOT NULL DEFAULT '{}',
          created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_admin_audit_logs_created_at ON admin_audit_logs (created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_admin_audit_logs_license_key ON admin_audit_logs (license_key);
        CREATE TABLE IF NOT EXISTS verify_logs (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          license_key TEXT NOT NULL DEFAULT '',
          device_id TEXT NOT NULL DEFAULT '',
          success INTEGER NOT NULL DEFAULT 0,
          message TEXT NOT NULL DEFAULT '',
          created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_verify_logs_created_at ON verify_logs (created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_verify_logs_license_key ON verify_logs (license_key);
      `);
    })();
  }
  await schemaReadyPromise;
}

async function recordAdminAudit(env, { action, licenseKey = "", reason = "", detail = {} }) {
  const now = nowISO();
  await env.DB.prepare(
    "INSERT INTO admin_audit_logs (action, license_key, admin_actor, reason, detail_json, created_at) VALUES (?, ?, ?, ?, ?, ?)"
  ).bind(action, licenseKey, ADMIN_ACTOR, reason, detailJson(detail), now).run();
}

async function recordVerifyLog(env, { licenseKey = "", deviceId = "", success = false, message = "" }) {
  const now = nowISO();
  await env.DB.prepare(
    "INSERT INTO verify_logs (license_key, device_id, success, message, created_at) VALUES (?, ?, ?, ?, ?)"
  ).bind(licenseKey, deviceId, success ? 1 : 0, String(message || "").slice(0, 200), now).run();
}

// ---------------------------------------------------------------------------
// 卡密生成与校验
// ---------------------------------------------------------------------------

/** 生成一个卡密 */
async function generateKey(planDays, secretBytes) {
  const daysBytes = new Uint8Array(2);
  daysBytes[0] = (planDays >> 8) & 0xff;
  daysBytes[1] = planDays & 0xff;
  const salt = crypto.getRandomValues(new Uint8Array(2));

  const sigInput = new Uint8Array(4);
  sigInput.set(daysBytes, 0);
  sigInput.set(salt, 2);
  const sig = (await hmacSha256(secretBytes, sigInput)).slice(0, 6);

  const payload = new Uint8Array(10);
  payload.set(daysBytes, 0);
  payload.set(salt, 2);
  payload.set(sig, 4);

  const encoded = base32Encode(payload);
  const parts = [];
  for (let i = 0; i < encoded.length; i += 4) parts.push(encoded.slice(i, i + 4));
  return KEY_PREFIX + parts.join("-");
}

/** 校验卡密并返回 { valid, planDays } */
async function validateKey(key, secretBytes) {
  try {
    let body = key.trim().toUpperCase();
    if (body.startsWith(KEY_PREFIX)) body = body.slice(KEY_PREFIX.length);
    const raw = body.replace(/-/g, "");
    const padding = (8 - (raw.length % 8)) % 8;
    const decoded = base32Decode(raw + "=".repeat(padding));
    if (decoded.length !== PAYLOAD_LEN) return { valid: false, planDays: 0 };

    const daysBytes = decoded.slice(0, 2);
    const salt = decoded.slice(2, 4);
    const sigStored = decoded.slice(4, 10);
    const sigInput = new Uint8Array(4);
    sigInput.set(daysBytes, 0);
    sigInput.set(salt, 2);
    const sigExpected = (await hmacSha256(secretBytes, sigInput)).slice(0, 6);

    if (!constantTimeEqual(sigStored, sigExpected)) return { valid: false, planDays: 0 };
    return { valid: true, planDays: (daysBytes[0] << 8) | daysBytes[1] };
  } catch {
    return { valid: false, planDays: 0 };
  }
}

// ---------------------------------------------------------------------------
// 查询 / 统计
// ---------------------------------------------------------------------------

async function fetchListRows(env, { query = "", status = "all", limit = 200 } = {}) {
  const trimmed = String(query || "").trim();
  const normalizedStatus = String(status || "all").trim();
  const normalizedLimit = Math.min(Math.max(parseInt(limit, 10) || 200, 1), 500);
  const baseSql = `
    SELECT
      g.*,
      a.device_id,
      a.device_fingerprint,
      a.activated_at,
      a.expires_at
    FROM generated_keys g
    LEFT JOIN activations a ON g.license_key = a.license_key
  `;
  const conditions = [];
  const bindings = [];
  if (trimmed) {
    const likeQuery = `%${trimmed}%`;
    conditions.push("(g.license_key LIKE ? OR g.note LIKE ? OR IFNULL(a.device_id, '') LIKE ?)");
    bindings.push(likeQuery, likeQuery, likeQuery);
  }
  if (normalizedStatus === "unused" || normalizedStatus === "activated" || normalizedStatus === "revoked") {
    conditions.push("g.status = ?");
    bindings.push(normalizedStatus);
  } else if (normalizedStatus === "expired") {
    conditions.push("g.status != 'revoked' AND a.expires_at IS NOT NULL AND a.expires_at < ?");
    bindings.push(nowISO());
  }
  const whereSql = conditions.length ? ` WHERE ${conditions.join(" AND ")}` : "";
  if (!trimmed && normalizedStatus === "all") {
    const result = await env.DB.prepare(`${baseSql} ORDER BY g.id DESC LIMIT ?`).bind(normalizedLimit).all();
    return result.results || [];
  }
  const result = await env.DB.prepare(
    `${baseSql}${whereSql} ORDER BY g.id DESC LIMIT ?`
  ).bind(...bindings, normalizedLimit).all();
  return result.results || [];
}

async function buildStatsPayload(env) {
  const statusRows = await env.DB.prepare(
    "SELECT status, COUNT(*) AS cnt FROM generated_keys GROUP BY status"
  ).all();
  const expiredRow = await env.DB.prepare(
    "SELECT COUNT(*) AS cnt FROM activations WHERE expires_at < ?"
  ).bind(nowISO()).first();
  const activeRow = await env.DB.prepare(
    "SELECT COUNT(*) AS cnt FROM activations WHERE expires_at >= ?"
  ).bind(nowISO()).first();
  const verifyTodayRows = await env.DB.prepare(
    "SELECT success, COUNT(*) AS cnt FROM verify_logs WHERE created_at >= ? GROUP BY success"
  ).bind(startOfTodayISO()).all();

  const statusMap = Object.fromEntries((statusRows.results || []).map((row) => [row.status, Number(row.cnt || 0)]));
  const verifyMap = Object.fromEntries((verifyTodayRows.results || []).map((row) => [String(row.success), Number(row.cnt || 0)]));

  return {
    total: Object.values(statusMap).reduce((sum, count) => sum + count, 0),
    unused: statusMap.unused || 0,
    activated: statusMap.activated || 0,
    revoked: statusMap.revoked || 0,
    expired: Number(expiredRow?.cnt || 0),
    active: Number(activeRow?.cnt || 0),
    verify_today: (verifyMap["1"] || 0) + (verifyMap["0"] || 0),
    verify_fail_today: verifyMap["0"] || 0,
  };
}

async function fetchAuditRows(env, limit = 100) {
  const normalizedLimit = Math.min(Math.max(parseInt(limit, 10) || 100, 1), 300);
  const result = await env.DB.prepare(
    "SELECT * FROM admin_audit_logs ORDER BY id DESC LIMIT ?"
  ).bind(normalizedLimit).all();
  return (result.results || []).map((row) => ({
    ...row,
    detail: (() => {
      try {
        return JSON.parse(row.detail_json || "{}");
      } catch {
        return {};
      }
    })(),
  }));
}

// ---------------------------------------------------------------------------
// 客户端 API
// ---------------------------------------------------------------------------

async function handleActivate(request, env) {
  let body;
  try {
    body = await request.json();
  } catch {
    return errorResponse("请求体 JSON 格式错误", 400);
  }

  const { key, device_id, device_fingerprint } = body || {};
  if (!key || !device_id) return errorResponse("缺少必填参数：key、device_id", 400);

  const secretBytes = new TextEncoder().encode(env.HMAC_SECRET);
  const { valid, planDays } = await validateKey(key, secretBytes);
  if (!valid) return errorResponse("卡密无效：格式错误或签名不匹配", 403);
  if (planDays <= 0) return errorResponse("卡密无效：有效期异常", 403);

  const normalizedKey = normalizeKey(key);
  const genRecord = await env.DB.prepare("SELECT * FROM generated_keys WHERE license_key = ?").bind(normalizedKey).first();
  if (!genRecord) return errorResponse("该卡密不存在或已被吊销", 403);
  if (genRecord.status === "revoked") {
    return errorResponse("该卡密已被吊销，无法使用", 403, { revoked: true });
  }

  const existing = await env.DB.prepare("SELECT * FROM activations WHERE license_key = ?").bind(normalizedKey).first();
  const now = nowISO();

  if (existing) {
    if (existing.device_id !== device_id) {
      return errorResponse("该卡密已在其他设备激活，不允许更换设备。如需帮助请联系作者处理设备迁移。", 403);
    }
    await env.DB.prepare(
      "UPDATE activations SET updated_at=?, device_fingerprint=? WHERE license_key=?"
    ).bind(now, device_fingerprint || "", normalizedKey).run();
    return jsonResponse({
      success: true,
      message: "重新激活成功",
      activated_at: existing.activated_at,
      expires_at: existing.expires_at,
      plan_days: existing.plan_days,
    });
  }

  const expires = expiresISO(planDays);
  await env.DB.prepare(
    "INSERT INTO activations (license_key,device_id,device_fingerprint,plan_days,activated_at,expires_at,updated_at) VALUES (?,?,?,?,?,?,?)"
  ).bind(normalizedKey, device_id, device_fingerprint || "", planDays, now, expires, now).run();
  await env.DB.prepare("UPDATE generated_keys SET status='activated' WHERE license_key=?").bind(normalizedKey).run();

  return jsonResponse({
    success: true,
    message: "激活成功",
    activated_at: now,
    expires_at: expires,
    plan_days: planDays,
  });
}

async function handleVerify(request, env) {
  let body;
  try {
    body = await request.json();
  } catch {
    return errorResponse("请求体 JSON 格式错误", 400);
  }

  const { key, device_id } = body || {};
  if (!key || !device_id) return errorResponse("缺少必填参数：key、device_id", 400);

  const normalizedKey = normalizeKey(key);
  const genRecord = await env.DB.prepare("SELECT status FROM generated_keys WHERE license_key = ?").bind(normalizedKey).first();
  if (!genRecord) {
    await recordVerifyLog(env, { licenseKey: normalizedKey, deviceId: device_id, success: false, message: "卡密不存在" });
    return jsonResponse({ success: false, message: "该卡密不存在或已被吊销", expired: true });
  }
  if (genRecord.status === "revoked") {
    await recordVerifyLog(env, { licenseKey: normalizedKey, deviceId: device_id, success: false, message: "卡密已吊销" });
    return jsonResponse({ success: false, message: "该卡密已被吊销", revoked: true, expired: true });
  }

  const record = await env.DB.prepare("SELECT * FROM activations WHERE license_key = ?").bind(normalizedKey).first();
  if (!record) {
    await recordVerifyLog(env, { licenseKey: normalizedKey, deviceId: device_id, success: false, message: "卡密尚未激活" });
    return errorResponse("该卡密尚未激活", 404);
  }
  if (record.device_id !== device_id) {
    await recordVerifyLog(env, { licenseKey: normalizedKey, deviceId: device_id, success: false, message: "设备不匹配" });
    return errorResponse("设备不匹配：该卡密已绑定其他设备，请联系管理员处理设备迁移。", 403);
  }
  if (new Date() > new Date(record.expires_at)) {
    await recordVerifyLog(env, { licenseKey: normalizedKey, deviceId: device_id, success: false, message: "授权已过期" });
    return jsonResponse({ success: false, message: "授权已过期", expires_at: record.expires_at, expired: true });
  }

  await recordVerifyLog(env, { licenseKey: normalizedKey, deviceId: device_id, success: true, message: "授权有效" });
  return jsonResponse({
    success: true,
    message: "授权有效",
    expires_at: record.expires_at,
    plan_days: record.plan_days,
    activated_at: record.activated_at,
  });
}

async function handleHealth(env) {
  try {
    const probe = await env.DB.prepare("SELECT 1 AS ok").first();
    return jsonResponse({
      success: true,
      message: "服务运行正常",
      now: nowISO(),
      db_ok: probe?.ok === 1,
      secrets_ok: Boolean(env.HMAC_SECRET && env.ADMIN_SECRET),
    });
  } catch (error) {
    return errorResponse(`健康检查失败：${error}`, 500);
  }
}

// ---------------------------------------------------------------------------
// 管理员 API
// ---------------------------------------------------------------------------

async function handleAdminGenerate(request, env) {
  if (!checkAdmin(request, env)) return errorResponse("管理员密钥错误", 401);

  let body;
  try {
    body = await request.json();
  } catch {
    return errorResponse("请求体 JSON 格式错误", 400);
  }

  const count = Math.min(Math.max(parseInt(body.count, 10) || 1, 1), 50);
  const planDays = Math.max(parseInt(body.plan_days, 10) || DEFAULT_PLAN_DAYS, 1);
  const note = String(body.note || "").slice(0, 200);
  const secretBytes = new TextEncoder().encode(env.HMAC_SECRET);
  const now = nowISO();

  const keys = [];
  for (let i = 0; i < count; i += 1) {
    const generatedKey = await generateKey(planDays, secretBytes);
    keys.push(generatedKey);
    await env.DB.prepare(
      "INSERT INTO generated_keys (license_key,plan_days,status,created_at,note) VALUES (?,?,?,?,?)"
    ).bind(generatedKey, planDays, "unused", now, note).run();
    await recordAdminAudit(env, {
      action: "generate",
      licenseKey: generatedKey,
      reason: note || "批量生成卡密",
      detail: { plan_days: planDays },
    });
  }

  return jsonResponse({ success: true, keys, plan_days: planDays, count: keys.length });
}

async function handleAdminList(request, env) {
  if (!checkAdmin(request, env)) return errorResponse("管理员密钥错误", 401);
  const keys = await fetchListRows(env);
  const stats = await buildStatsPayload(env);
  const audit = await fetchAuditRows(env, 30);
  return jsonResponse({ success: true, keys, stats, audit });
}

async function handleAdminStats(request, env) {
  if (!checkAdmin(request, env)) return errorResponse("管理员密钥错误", 401);
  const stats = await buildStatsPayload(env);
  return jsonResponse({ success: true, stats });
}

async function handleAdminSearch(request, env) {
  if (!checkAdmin(request, env)) return errorResponse("管理员密钥错误", 401);
  let body;
  try {
    body = await request.json();
  } catch {
    return errorResponse("请求体 JSON 格式错误", 400);
  }
  const query = String(body.query || "").trim();
  const status = String(body.status || "all").trim();
  const keys = await fetchListRows(env, { query, status, limit: body.limit || 200 });
  const stats = await buildStatsPayload(env);
  return jsonResponse({ success: true, keys, stats, query, status });
}

async function handleAdminAuditList(request, env) {
  if (!checkAdmin(request, env)) return errorResponse("管理员密钥错误", 401);
  let body = {};
  try {
    body = await request.json();
  } catch {
    body = {};
  }
  const audit = await fetchAuditRows(env, body.limit || 100);
  return jsonResponse({ success: true, audit });
}

async function handleAdminRevoke(request, env) {
  if (!checkAdmin(request, env)) return errorResponse("管理员密钥错误", 401);

  let body;
  try {
    body = await request.json();
  } catch {
    return errorResponse("请求体 JSON 格式错误", 400);
  }

  const key = normalizeKey(body.key);
  if (!key) return errorResponse("缺少参数：key", 400);
  const reason = normalizeReason(body.reason, "手动吊销");

  const record = await env.DB.prepare("SELECT * FROM generated_keys WHERE license_key = ?").bind(key).first();
  if (!record) return errorResponse("卡密不存在", 404);
  if (record.status === "revoked") return errorResponse("该卡密已吊销", 409);

  const activation = await env.DB.prepare("SELECT * FROM activations WHERE license_key = ?").bind(key).first();
  await env.DB.prepare("UPDATE generated_keys SET status='revoked' WHERE license_key=?").bind(key).run();
  await recordAdminAudit(env, {
    action: "revoke",
    licenseKey: key,
    reason,
    detail: {
      activation,
      previous_status: record.status,
    },
  });
  return jsonResponse({ success: true, message: "卡密已吊销，历史记录已保留" });
}

async function handleAdminDeviceReset(request, env) {
  if (!checkAdmin(request, env)) return errorResponse("管理员密钥错误", 401);

  let body;
  try {
    body = await request.json();
  } catch {
    return errorResponse("请求体 JSON 格式错误", 400);
  }

  const key = normalizeKey(body.key);
  if (!key) return errorResponse("缺少参数：key", 400);
  const reason = normalizeReason(body.reason, "人工重置设备绑定");

  const record = await env.DB.prepare("SELECT * FROM generated_keys WHERE license_key = ?").bind(key).first();
  if (!record) return errorResponse("卡密不存在", 404);
  if (record.status === "revoked") return errorResponse("已吊销卡密不支持设备重置", 409);

  const activation = await env.DB.prepare("SELECT * FROM activations WHERE license_key = ?").bind(key).first();
  if (!activation) return errorResponse("该卡密当前没有激活设备，无需重置", 409);

  await env.DB.prepare("DELETE FROM activations WHERE license_key = ?").bind(key).run();
  await env.DB.prepare("UPDATE generated_keys SET status='unused' WHERE license_key = ?").bind(key).run();
  await recordAdminAudit(env, {
    action: "device_reset",
    licenseKey: key,
    reason,
    detail: { activation },
  });

  return jsonResponse({ success: true, message: "设备绑定已重置，卡密可重新激活" });
}

function serveAdminPage() {
  return htmlResponse(ADMIN_HTML);
}

// ---------------------------------------------------------------------------
// 入口
// ---------------------------------------------------------------------------

export default {
  async fetch(request, env) {
    if (request.method === "OPTIONS") {
      return new Response(null, {
        headers: {
          "Access-Control-Allow-Origin": "*",
          "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
          "Access-Control-Allow-Headers": "Content-Type, X-Admin-Secret",
          "Access-Control-Max-Age": "86400",
        },
      });
    }

    if (!env.HMAC_SECRET) {
      return errorResponse("服务器配置错误：缺少 HMAC_SECRET", 500);
    }

    await ensureSchema(env);
    const url = new URL(request.url);

    if (request.method === "GET" && (url.pathname === "/admin" || url.pathname === "/admin/")) {
      return serveAdminPage();
    }
    if (request.method === "GET" && url.pathname === "/api/health") {
      return handleHealth(env);
    }

    if (request.method !== "POST") {
      return errorResponse("仅支持 POST 请求", 405);
    }

    switch (url.pathname) {
      case "/api/activate":
        return handleActivate(request, env);
      case "/api/verify":
        return handleVerify(request, env);
      case "/api/admin/generate":
      case "/api/admin/list":
      case "/api/admin/revoke":
      case "/api/admin/stats":
      case "/api/admin/search":
      case "/api/admin/device/reset":
      case "/api/admin/audit/list": {
        let response;
        if (url.pathname === "/api/admin/generate") response = await handleAdminGenerate(request, env);
        else if (url.pathname === "/api/admin/list") response = await handleAdminList(request, env);
        else if (url.pathname === "/api/admin/revoke") response = await handleAdminRevoke(request, env);
        else if (url.pathname === "/api/admin/stats") response = await handleAdminStats(request, env);
        else if (url.pathname === "/api/admin/search") response = await handleAdminSearch(request, env);
        else if (url.pathname === "/api/admin/device/reset") response = await handleAdminDeviceReset(request, env);
        else response = await handleAdminAuditList(request, env);
        response.headers.delete("Access-Control-Allow-Origin");
        return response;
      }
      default:
        return errorResponse("未知路由", 404);
    }
  },
};
