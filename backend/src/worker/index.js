/**
 * TLS-shipinhao 卡密验证后端（V2 授权票据）
 * Cloudflare Workers + D1
 */

import ADMIN_HTML from "../admin/admin.html";

const B32_ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
const KEY_PREFIX = "TLS-";
const PAYLOAD_LEN = 10;
const DEFAULT_PLAN_DAYS = 30;
const LICENSE_PROTOCOL_VERSION = 2;
const OFFLINE_GRANT_HOURS = 24;
const SESSION_TOKEN_MINUTES = 15;
const LICENSE_SIGNING_PUBLIC_KEY = "H0KTidHIXV0nvzkUNmssrx5t5IrUvEQi1WVelkuCJm8";
const ISSUER = "tls-license-backend";

function base32Decode(input) {
  const raw = input.toUpperCase().replace(/=+$/, "");
  let bits = 0, value = 0;
  const output = [];
  for (const ch of raw) {
    const idx = B32_ALPHABET.indexOf(ch);
    if (idx === -1) throw new Error("invalid base32 character");
    value = (value << 5) | idx;
    bits += 5;
    if (bits >= 8) { bits -= 8; output.push((value >>> bits) & 0xff); }
  }
  return new Uint8Array(output);
}

function base32Encode(data) {
  let bits = 0, value = 0, result = "";
  for (const byte of data) {
    value = (value << 8) | byte;
    bits += 8;
    while (bits >= 5) { bits -= 5; result += B32_ALPHABET[(value >>> bits) & 0x1f]; }
  }
  if (bits > 0) result += B32_ALPHABET[(value << (5 - bits)) & 0x1f];
  return result;
}

async function hmacSha256(secret, data) {
  const key = await crypto.subtle.importKey("raw", secret, { name: "HMAC", hash: "SHA-256" }, false, ["sign"]);
  return new Uint8Array(await crypto.subtle.sign("HMAC", key, data));
}

function constantTimeEqual(a, b) {
  if (a.length !== b.length) return false;
  let result = 0;
  for (let i = 0; i < a.length; i++) result |= a[i] ^ b[i];
  return result === 0;
}

function nowISO() {
  return new Date().toISOString().replace(/\.\d{3}Z$/, "+00:00");
}

function expiresISO(days) {
  return new Date(Date.now() + days * 86400000).toISOString().replace(/\.\d{3}Z$/, "+00:00");
}

function epochSeconds() {
  return Math.floor(Date.now() / 1000);
}

function epochToISO(seconds) {
  return new Date(seconds * 1000).toISOString().replace(/\.\d{3}Z$/, "+00:00");
}

function bytesToBase64Url(bytes) {
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
}

function base64UrlToBytes(value) {
  const padded = value + "=".repeat((4 - (value.length % 4)) % 4);
  const binary = atob(padded.replace(/-/g, "+").replace(/_/g, "/"));
  return Uint8Array.from(binary, ch => ch.charCodeAt(0));
}

function stableStringify(value) {
  if (value === null || typeof value !== "object") return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(stableStringify).join(",")}]`;
  const keys = Object.keys(value).sort();
  return `{${keys.map(key => `${JSON.stringify(key)}:${stableStringify(value[key])}`).join(",")}}`;
}

function jsonResponse(data, status = 200) {
  return new Response(JSON.stringify(data), {
    status,
    headers: { "Content-Type": "application/json; charset=utf-8", "Access-Control-Allow-Origin": "*" },
  });
}

function errorResponse(message, status = 400, extra = {}) {
  return jsonResponse({ success: false, message, ...extra }, status);
}

function htmlResponse(html) {
  return new Response(html, { headers: { "Content-Type": "text/html; charset=utf-8" } });
}

function checkAdmin(request, env) {
  const enc = new TextEncoder();
  const auth = enc.encode(request.headers.get("X-Admin-Secret") || "");
  const secret = enc.encode(env.ADMIN_SECRET || "");
  if (auth.length !== secret.length) return false;
  return constantTimeEqual(auth, secret);
}

function buildResponsePayload(record, overrides = {}) {
  const licenseExpiresAt = overrides.license_expires_at || record.expires_at || "";
  return {
    success: true,
    message: overrides.message || "授权有效",
    license_version: LICENSE_PROTOCOL_VERSION,
    key: record.license_key,
    license_key: record.license_key,
    activated_at: record.activated_at,
    expires_at: licenseExpiresAt,
    license_expires_at: licenseExpiresAt,
    plan_days: record.plan_days,
    issuer: ISSUER,
    issued_at: overrides.issued_at || nowISO(),
    device_claims: overrides.device_claims || "",
    device_claims_expires_at: overrides.device_claims_expires_at || "",
    offline_grant: overrides.offline_grant || "",
    offline_grant_expires_at: overrides.offline_grant_expires_at || "",
    session_token: overrides.session_token || "",
    session_token_expires_at: overrides.session_token_expires_at || "",
    session_id: overrides.session_id || "",
    license_state: overrides.license_state || "ok",
    refresh_required: !!overrides.refresh_required,
    server_time: nowISO(),
    grace_policy: `${OFFLINE_GRANT_HOURS}h offline state cache`,
  };
}

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

function sha256Hex(value) {
  const data = new TextEncoder().encode(value || "");
  return crypto.subtle.digest("SHA-256", data).then(buf => Array.from(new Uint8Array(buf)).map(b => b.toString(16).padStart(2, "0")).join(""));
}

function parsePemBase64ToDer(base64Pem) {
  const pemText = atob(base64Pem);
  const body = pemText
    .replace(/-----BEGIN [^-]+-----/g, "")
    .replace(/-----END [^-]+-----/g, "")
    .replace(/\s+/g, "");
  const binary = atob(body);
  return Uint8Array.from(binary, ch => ch.charCodeAt(0));
}

async function importSigningPrivateKey(env) {
  const raw = env.LICENSE_SIGNING_PRIVATE_KEY_B64 || "";
  if (!raw) throw new Error("服务器配置错误：缺少 LICENSE_SIGNING_PRIVATE_KEY_B64");
  return crypto.subtle.importKey("pkcs8", parsePemBase64ToDer(raw), { name: "Ed25519" }, false, ["sign"]);
}

async function importVerifyPublicKey() {
  return crypto.subtle.importKey("raw", base64UrlToBytes(LICENSE_SIGNING_PUBLIC_KEY), { name: "Ed25519" }, false, ["verify"]);
}

async function signClaims(payload, env) {
  const normalized = stableStringify(payload);
  const encodedPayload = bytesToBase64Url(new TextEncoder().encode(normalized));
  const key = await importSigningPrivateKey(env);
  const signature = new Uint8Array(await crypto.subtle.sign("Ed25519", key, new TextEncoder().encode(encodedPayload)));
  return `${encodedPayload}.${bytesToBase64Url(signature)}`;
}

async function verifyClaimsToken(token, expectedKind = null) {
  try {
    const [encodedPayload, encodedSig] = token.split(".", 2);
    if (!encodedPayload || !encodedSig) return null;
    const publicKey = await importVerifyPublicKey();
    const ok = await crypto.subtle.verify(
      "Ed25519",
      publicKey,
      base64UrlToBytes(encodedSig),
      new TextEncoder().encode(encodedPayload),
    );
    if (!ok) return null;
    const payload = JSON.parse(new TextDecoder().decode(base64UrlToBytes(encodedPayload)));
    if (expectedKind && payload.kind !== expectedKind) return null;
    if (!payload.exp || epochSeconds() >= Number(payload.exp)) return null;
    return payload;
  } catch {
    return null;
  }
}

async function appendAuditLog(env, { licenseKey = "", deviceId = "", action, actionReason = "", operator = "system", meta = null }) {
  try {
    await env.DB.prepare(
      "INSERT INTO license_audit_logs (license_key, device_id, action, action_reason, created_at, operator, meta_json) VALUES (?, ?, ?, ?, ?, ?, ?)"
    ).bind(licenseKey, deviceId, action, actionReason, nowISO(), operator, meta ? JSON.stringify(meta) : "").run();
  } catch {
    // 审计失败不阻断主流程
  }
}

async function upsertDeviceRegistration(env, licenseKey, deviceId, deviceFingerprint) {
  const hash = await sha256Hex(deviceFingerprint || "");
  const now = nowISO();
  const existing = await env.DB.prepare(
    "SELECT id FROM device_registrations WHERE license_key = ? AND device_id = ?"
  ).bind(licenseKey, deviceId).first();
  if (existing) {
    await env.DB.prepare(
      "UPDATE device_registrations SET device_fingerprint_hash = ?, last_seen_at = ?, registration_status = 'active' WHERE id = ?"
    ).bind(hash, now, existing.id).run();
    return;
  }
  await env.DB.prepare(
    "INSERT INTO device_registrations (license_key, device_id, device_fingerprint_hash, registered_at, last_seen_at, registration_status) VALUES (?, ?, ?, ?, ?, 'active')"
  ).bind(licenseKey, deviceId, hash, now, now).run();
}

async function revokeSessionsForLicense(env, licenseKey, reason = "revoked") {
  await env.DB.prepare(
    "UPDATE device_sessions SET revoked_at = ? WHERE license_key = ? AND revoked_at IS NULL"
  ).bind(nowISO(), licenseKey).run();
  await appendAuditLog(env, { licenseKey, action: "revoke_sessions", actionReason: reason });
}

async function loadActiveLicenseRecord(env, licenseKey, deviceId) {
  const record = await env.DB.prepare("SELECT * FROM activations WHERE license_key=?").bind(licenseKey).first();
  if (!record) return { record: null, reason: "not_found" };
  if (record.status === "revoked") return { record, reason: "revoked" };
  if (record.device_id !== deviceId) return { record, reason: "device_mismatch" };
  if (new Date() > new Date(record.expires_at)) {
    await env.DB.prepare("UPDATE activations SET status='expired' WHERE license_key=?").bind(licenseKey).run();
    return { record: { ...record, status: "expired" }, reason: "expired" };
  }
  return { record, reason: "ok" };
}

async function issueGrants(env, record, { deviceId, taskType = "bootstrap", sessionId = null }) {
  const now = epochSeconds();
  const deviceClaimsExp = Math.max(now + OFFLINE_GRANT_HOURS * 3600, Math.floor(new Date(record.expires_at).getTime() / 1000));
  const offlineGrantExp = now + OFFLINE_GRANT_HOURS * 3600;
  const sessionExp = now + SESSION_TOKEN_MINUTES * 60;
  const actualSessionId = sessionId || crypto.randomUUID();

  const deviceClaims = await signClaims({
    kind: "device_claims",
    issuer: ISSUER,
    license_key: record.license_key,
    device_id: deviceId,
    activated_at: record.activated_at,
    license_expires_at: record.expires_at,
    plan_days: record.plan_days,
    binding_version: LICENSE_PROTOCOL_VERSION,
    iat: now,
    exp: deviceClaimsExp,
  }, env);

  const offlineGrant = await signClaims({
    kind: "offline_grant",
    issuer: ISSUER,
    license_key: record.license_key,
    device_id: deviceId,
    license_expires_at: record.expires_at,
    binding_version: LICENSE_PROTOCOL_VERSION,
    iat: now,
    exp: offlineGrantExp,
  }, env);

  const sessionToken = await signClaims({
    kind: "session_token",
    issuer: ISSUER,
    license_key: record.license_key,
    device_id: deviceId,
    task_type: taskType,
    session_id: actualSessionId,
    binding_version: LICENSE_PROTOCOL_VERSION,
    iat: now,
    exp: sessionExp,
  }, env);

  return {
    device_claims: deviceClaims,
    device_claims_expires_at: epochToISO(deviceClaimsExp),
    offline_grant: offlineGrant,
    offline_grant_expires_at: epochToISO(offlineGrantExp),
    session_token: sessionToken,
    session_token_expires_at: epochToISO(sessionExp),
    session_id: actualSessionId,
    issued_at: nowISO(),
  };
}

async function persistSession(env, { licenseKey, deviceId, sessionId, expiresAt, clientVersion = "" }) {
  const existing = await env.DB.prepare(
    "SELECT id FROM device_sessions WHERE session_id = ?"
  ).bind(sessionId).first();
  if (existing) {
    await env.DB.prepare(
      "UPDATE device_sessions SET expires_at = ?, revoked_at = NULL, client_version = ? WHERE id = ?"
    ).bind(expiresAt, clientVersion, existing.id).run();
    return;
  }
  await env.DB.prepare(
    "INSERT INTO device_sessions (license_key, device_id, session_id, issued_at, expires_at, revoked_at, client_version, ip_hash) VALUES (?, ?, ?, ?, ?, NULL, ?, '')"
  ).bind(licenseKey, deviceId, sessionId, nowISO(), expiresAt, clientVersion).run();
}

async function handleActivate(request, env) {
  let body;
  try { body = await request.json(); } catch { return errorResponse("请求体 JSON 格式错误", 400); }
  const { key, device_id, device_fingerprint, client_version } = body || {};
  if (!key || !device_id) return errorResponse("缺少必填参数：key、device_id", 400);

  const secretBytes = new TextEncoder().encode(env.HMAC_SECRET);
  const { valid, planDays } = await validateKey(key, secretBytes);
  if (!valid) return errorResponse("卡密无效：格式错误或签名不匹配", 403, { license_state: "invalid" });
  if (planDays <= 0) return errorResponse("卡密无效：有效期异常", 403, { license_state: "invalid" });

  const normalizedKey = key.trim().toUpperCase();
  const genRecord = await env.DB.prepare("SELECT * FROM generated_keys WHERE license_key = ?").bind(normalizedKey).first();
  if (!genRecord) return errorResponse("该卡密不存在或已被吊销", 403, { license_state: "revoked" });
  if (genRecord.status === "revoked") return errorResponse("该卡密已被吊销，无法使用", 403, { license_state: "revoked" });

  let record = await env.DB.prepare("SELECT * FROM activations WHERE license_key = ?").bind(normalizedKey).first();
  const now = nowISO();
  if (record) {
    if (record.device_id !== device_id) {
      return errorResponse("该卡密已在其他设备激活，不允许更换设备。如需帮助请联系作者。", 403, { license_state: "device_mismatch" });
    }
    await env.DB.prepare(
      "UPDATE activations SET updated_at=?, device_fingerprint=?, binding_version=?, status='active', last_verify_at=? WHERE license_key=?"
    ).bind(now, device_fingerprint || "", LICENSE_PROTOCOL_VERSION, now, normalizedKey).run();
    record = await env.DB.prepare("SELECT * FROM activations WHERE license_key = ?").bind(normalizedKey).first();
  } else {
    const expires = expiresISO(planDays);
    await env.DB.prepare(
      "INSERT INTO activations (license_key,device_id,device_fingerprint,plan_days,activated_at,expires_at,updated_at,binding_version,status,last_verify_at,last_session_issued_at,last_offline_grant_issued_at) VALUES (?,?,?,?,?,?,?,?,?,?,?,?)"
    ).bind(normalizedKey, device_id, device_fingerprint || "", planDays, now, expires, now, LICENSE_PROTOCOL_VERSION, "active", now, now, now).run();
    await env.DB.prepare("UPDATE generated_keys SET status='activated' WHERE license_key=?").bind(normalizedKey).run();
    record = await env.DB.prepare("SELECT * FROM activations WHERE license_key = ?").bind(normalizedKey).first();
  }

  await upsertDeviceRegistration(env, normalizedKey, device_id, device_fingerprint || "");
  const grants = await issueGrants(env, record, { deviceId: device_id, taskType: "bootstrap" });
  await persistSession(env, {
    licenseKey: normalizedKey,
    deviceId: device_id,
    sessionId: grants.session_id,
    expiresAt: grants.session_token_expires_at,
    clientVersion: client_version || "",
  });
  await env.DB.prepare(
    "UPDATE activations SET last_verify_at=?, last_session_issued_at=?, last_offline_grant_issued_at=? WHERE license_key=?"
  ).bind(nowISO(), grants.session_token_expires_at, grants.offline_grant_expires_at, normalizedKey).run();
  await appendAuditLog(env, { licenseKey: normalizedKey, deviceId: device_id, action: "activate", actionReason: "client_activate", meta: { client_version } });

  return jsonResponse(buildResponsePayload(record, {
    message: record.activated_at === now ? "激活成功" : "重新激活成功",
    ...grants,
  }));
}

async function handleVerify(request, env) {
  let body;
  try { body = await request.json(); } catch { return errorResponse("请求体 JSON 格式错误", 400); }
  const { key, device_id, client_version } = body || {};
  if (!key || !device_id) return errorResponse("缺少必填参数：key、device_id", 400);
  const normalizedKey = key.trim().toUpperCase();

  const genRecord = await env.DB.prepare("SELECT status FROM generated_keys WHERE license_key = ?").bind(normalizedKey).first();
  if (!genRecord || genRecord.status === "revoked") {
    await revokeSessionsForLicense(env, normalizedKey, "verify_revoked");
    return errorResponse("该卡密已被吊销", 403, { license_state: "revoked", expired: true });
  }

  const { record, reason } = await loadActiveLicenseRecord(env, normalizedKey, device_id);
  if (!record) return errorResponse("该卡密尚未激活", 404, { license_state: "reactivation_required" });
  if (reason !== "ok") {
    return errorResponse(
      reason === "expired" ? "授权已过期" : reason === "device_mismatch" ? "设备不匹配：该卡密已绑定其他设备" : "授权不可用",
      reason === "device_mismatch" ? 403 : 200,
      { license_state: reason, expires_at: record.expires_at, expired: reason === "expired" }
    );
  }

  const grants = await issueGrants(env, record, { deviceId: device_id, taskType: "bootstrap" });
  await persistSession(env, {
    licenseKey: normalizedKey,
    deviceId: device_id,
    sessionId: grants.session_id,
    expiresAt: grants.session_token_expires_at,
    clientVersion: client_version || "",
  });
  await env.DB.prepare(
    "UPDATE activations SET last_verify_at=?, last_session_issued_at=?, last_offline_grant_issued_at=?, status='active' WHERE license_key=?"
  ).bind(nowISO(), grants.session_token_expires_at, grants.offline_grant_expires_at, normalizedKey).run();
  await appendAuditLog(env, { licenseKey: normalizedKey, deviceId: device_id, action: "verify", actionReason: "client_verify", meta: { client_version } });
  return jsonResponse(buildResponsePayload(record, grants));
}

async function handleSessionIssue(request, env) {
  let body;
  try { body = await request.json(); } catch { return errorResponse("请求体 JSON 格式错误", 400); }
  const { license_key, device_id, device_claims, task_type, client_version } = body || {};
  if (!license_key || !device_id || !device_claims || !task_type) {
    return errorResponse("缺少必填参数：license_key、device_id、device_claims、task_type", 400);
  }
  const normalizedKey = String(license_key).trim().toUpperCase();
  const claims = await verifyClaimsToken(device_claims, "device_claims");
  if (!claims || claims.license_key !== normalizedKey || claims.device_id !== device_id) {
    return errorResponse("设备声明无效，请重新联网激活。", 403, { license_state: "reactivation_required" });
  }
  const { record, reason } = await loadActiveLicenseRecord(env, normalizedKey, device_id);
  if (!record || reason !== "ok") {
    return errorResponse("当前授权不可用。", 403, { license_state: record ? reason : "reactivation_required" });
  }
  const grants = await issueGrants(env, record, { deviceId: device_id, taskType: task_type });
  await persistSession(env, {
    licenseKey: normalizedKey,
    deviceId: device_id,
    sessionId: grants.session_id,
    expiresAt: grants.session_token_expires_at,
    clientVersion: client_version || "",
  });
  await env.DB.prepare(
    "UPDATE activations SET last_session_issued_at=?, status='active' WHERE license_key=?"
  ).bind(grants.session_token_expires_at, normalizedKey).run();
  await appendAuditLog(env, { licenseKey: normalizedKey, deviceId: device_id, action: "issue_session", actionReason: task_type });
  return jsonResponse({
    success: true,
    session_token: grants.session_token,
    session_token_expires_at: grants.session_token_expires_at,
    task_type,
    server_time: nowISO(),
    license_state: "ok",
  });
}

async function handleSessionRefresh(request, env) {
  let body;
  try { body = await request.json(); } catch { return errorResponse("请求体 JSON 格式错误", 400); }
  const { license_key, device_id, session_token, task_type } = body || {};
  if (!license_key || !device_id || !session_token || !task_type) {
    return errorResponse("缺少必填参数：license_key、device_id、session_token、task_type", 400);
  }
  const normalizedKey = String(license_key).trim().toUpperCase();
  const tokenPayload = await verifyClaimsToken(session_token, "session_token");
  if (!tokenPayload || tokenPayload.license_key !== normalizedKey || tokenPayload.device_id !== device_id || tokenPayload.task_type !== task_type) {
    return errorResponse("任务令牌无效，请重新联网获取。", 403, { license_state: "online_refresh_required" });
  }
  const dbSession = await env.DB.prepare(
    "SELECT * FROM device_sessions WHERE session_id = ?"
  ).bind(tokenPayload.session_id).first();
  if (!dbSession || dbSession.revoked_at) {
    return errorResponse("任务令牌已失效，请重新联网获取。", 403, { license_state: "online_refresh_required" });
  }
  const { record, reason } = await loadActiveLicenseRecord(env, normalizedKey, device_id);
  if (!record || reason !== "ok") {
    return errorResponse("当前授权不可用。", 403, { license_state: record ? reason : "reactivation_required" });
  }
  const grants = await issueGrants(env, record, { deviceId: device_id, taskType: task_type, sessionId: tokenPayload.session_id });
  await persistSession(env, {
    licenseKey: normalizedKey,
    deviceId: device_id,
    sessionId: tokenPayload.session_id,
    expiresAt: grants.session_token_expires_at,
    clientVersion: dbSession.client_version || "",
  });
  await env.DB.prepare(
    "UPDATE activations SET last_session_issued_at=?, status='active' WHERE license_key=?"
  ).bind(grants.session_token_expires_at, normalizedKey).run();
  await appendAuditLog(env, { licenseKey: normalizedKey, deviceId: device_id, action: "refresh_session", actionReason: task_type });
  return jsonResponse({
    success: true,
    session_token: grants.session_token,
    session_token_expires_at: grants.session_token_expires_at,
    task_type,
    server_time: nowISO(),
    license_state: "ok",
  });
}

async function handleAdminGenerate(request, env) {
  if (!checkAdmin(request, env)) return errorResponse("管理员密钥错误", 401);
  let body;
  try { body = await request.json(); } catch { return errorResponse("请求体 JSON 格式错误", 400); }
  const count = Math.min(Math.max(parseInt(body.count) || 1, 1), 50);
  const planDays = Math.max(parseInt(body.plan_days) || DEFAULT_PLAN_DAYS, 1);
  const note = (body.note || "").slice(0, 200);
  const secretBytes = new TextEncoder().encode(env.HMAC_SECRET);
  const now = nowISO();
  const keys = [];
  for (let i = 0; i < count; i++) {
    const k = await generateKey(planDays, secretBytes);
    keys.push(k);
    await env.DB.prepare(
      "INSERT INTO generated_keys (license_key,plan_days,status,created_at,note) VALUES (?,?,?,?,?)"
    ).bind(k, planDays, "unused", now, note).run();
  }
  await appendAuditLog(env, { action: "generate_keys", actionReason: `count=${keys.length}`, operator: "admin", meta: { note } });
  return jsonResponse({ success: true, keys, plan_days: planDays, count: keys.length });
}

async function handleAdminList(request, env) {
  if (!checkAdmin(request, env)) return errorResponse("管理员密钥错误", 401);
  const generated = await env.DB.prepare(
    "SELECT g.*, a.device_id, a.device_fingerprint, a.activated_at, a.expires_at, a.status as activation_status FROM generated_keys g LEFT JOIN activations a ON g.license_key = a.license_key ORDER BY g.id DESC LIMIT 200"
  ).all();
  const stats = await env.DB.prepare(
    "SELECT status, COUNT(*) as cnt FROM generated_keys GROUP BY status"
  ).all();
  return jsonResponse({ success: true, keys: generated.results || [], stats: stats.results || [] });
}

async function handleAdminRevoke(request, env) {
  if (!checkAdmin(request, env)) return errorResponse("管理员密钥错误", 401);
  let body;
  try { body = await request.json(); } catch { return errorResponse("请求体 JSON 格式错误", 400); }
  const key = (body.key || "").trim().toUpperCase();
  if (!key) return errorResponse("缺少参数：key", 400);
  const record = await env.DB.prepare("SELECT * FROM generated_keys WHERE license_key = ?").bind(key).first();
  if (!record) return errorResponse("卡密不存在", 404);
  await env.DB.prepare("UPDATE generated_keys SET status='revoked' WHERE license_key = ?").bind(key).run();
  await env.DB.prepare("UPDATE activations SET status='revoked', updated_at=? WHERE license_key = ?").bind(nowISO(), key).run();
  await revokeSessionsForLicense(env, key, "admin_revoke");
  await appendAuditLog(env, { licenseKey: key, action: "revoke", actionReason: "admin_revoke", operator: "admin" });
  return jsonResponse({ success: true, message: "卡密已吊销，短期会话已全部失效" });
}

async function handleAdminDeviceRebind(request, env) {
  if (!checkAdmin(request, env)) return errorResponse("管理员密钥错误", 401);
  let body;
  try { body = await request.json(); } catch { return errorResponse("请求体 JSON 格式错误", 400); }
  const key = (body.key || "").trim().toUpperCase();
  if (!key) return errorResponse("缺少参数：key", 400);
  await env.DB.prepare("UPDATE activations SET device_id='', device_fingerprint='', status='migrated', updated_at=? WHERE license_key = ?").bind(nowISO(), key).run();
  await revokeSessionsForLicense(env, key, "device_rebind");
  await appendAuditLog(env, { licenseKey: key, action: "device_rebind", actionReason: "admin_rebind", operator: "admin" });
  return jsonResponse({ success: true, message: "设备绑定已重置，用户需重新激活。" });
}

async function handleAdminRevokeSessions(request, env) {
  if (!checkAdmin(request, env)) return errorResponse("管理员密钥错误", 401);
  let body;
  try { body = await request.json(); } catch { return errorResponse("请求体 JSON 格式错误", 400); }
  const key = (body.key || "").trim().toUpperCase();
  if (!key) return errorResponse("缺少参数：key", 400);
  await revokeSessionsForLicense(env, key, "admin_manual_revoke_sessions");
  return jsonResponse({ success: true, message: "该卡密下发的所有短期会话均已失效。" });
}

async function handleAdminAuditList(request, env) {
  if (!checkAdmin(request, env)) return errorResponse("管理员密钥错误", 401);
  const rows = await env.DB.prepare(
    "SELECT * FROM license_audit_logs ORDER BY id DESC LIMIT 200"
  ).all();
  return jsonResponse({ success: true, logs: rows.results || [] });
}

function serveAdminPage() {
  return htmlResponse(ADMIN_HTML);
}

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

    const url = new URL(request.url);
    if (request.method === "GET" && (url.pathname === "/admin" || url.pathname === "/admin/")) {
      return serveAdminPage();
    }
    if (request.method !== "POST") return errorResponse("仅支持 POST 请求", 405);
    if (!env.HMAC_SECRET) return errorResponse("服务器配置错误：缺少 HMAC_SECRET", 500);
    if (!env.LICENSE_SIGNING_PRIVATE_KEY_B64) return errorResponse("服务器配置错误：缺少 LICENSE_SIGNING_PRIVATE_KEY_B64", 500);

    switch (url.pathname) {
      case "/api/activate":
        return handleActivate(request, env);
      case "/api/verify":
        return handleVerify(request, env);
      case "/api/session/issue":
        return handleSessionIssue(request, env);
      case "/api/session/refresh":
        return handleSessionRefresh(request, env);
      case "/api/admin/generate":
      case "/api/admin/list":
      case "/api/admin/revoke":
      case "/api/admin/device/rebind":
      case "/api/admin/device/revoke_sessions":
      case "/api/admin/audit/list": {
        let resp;
        if (url.pathname === "/api/admin/generate") resp = await handleAdminGenerate(request, env);
        else if (url.pathname === "/api/admin/list") resp = await handleAdminList(request, env);
        else if (url.pathname === "/api/admin/revoke") resp = await handleAdminRevoke(request, env);
        else if (url.pathname === "/api/admin/device/rebind") resp = await handleAdminDeviceRebind(request, env);
        else if (url.pathname === "/api/admin/device/revoke_sessions") resp = await handleAdminRevokeSessions(request, env);
        else resp = await handleAdminAuditList(request, env);
        resp.headers.delete("Access-Control-Allow-Origin");
        return resp;
      }
      default:
        return errorResponse("未知路由", 404);
    }
  },
};
