/**
 * TLS-shipinhao 卡密验证后端
 * Cloudflare Workers + D1
 */

// ---------------------------------------------------------------------------
// 工具函数
// ---------------------------------------------------------------------------

const B32_ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/** Base32 解码（RFC 4648，兼容无填充输入） */
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

/** Base32 编码（RFC 4648，不带填充） */
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

/** HMAC-SHA256 签名（使用 Web Crypto API） */
async function hmacSha256(secret, data) {
  const key = await crypto.subtle.importKey(
    "raw", secret, { name: "HMAC", hash: "SHA-256" }, false, ["sign"]
  );
  return new Uint8Array(await crypto.subtle.sign("HMAC", key, data));
}

/** 恒定时间比较 */
function constantTimeEqual(a, b) {
  if (a.length !== b.length) return false;
  let result = 0;
  for (let i = 0; i < a.length; i++) result |= a[i] ^ b[i];
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

// ---------------------------------------------------------------------------
// 卡密生成与校验
// ---------------------------------------------------------------------------

const KEY_PREFIX = "TLS-";
const PAYLOAD_LEN = 10; // 2 (days) + 2 (salt) + 6 (hmac truncated)
const DEFAULT_PLAN_DAYS = 30;

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
// 响应构造
// ---------------------------------------------------------------------------

function jsonResponse(data, status = 200) {
  return new Response(JSON.stringify(data), {
    status,
    headers: { "Content-Type": "application/json; charset=utf-8", "Access-Control-Allow-Origin": "*" },
  });
}

function errorResponse(message, status = 400) {
  return jsonResponse({ success: false, message }, status);
}

function htmlResponse(html) {
  return new Response(html, {
    headers: { "Content-Type": "text/html; charset=utf-8" },
  });
}

// ---------------------------------------------------------------------------
// 管理员鉴权
// ---------------------------------------------------------------------------

function checkAdmin(request, env) {
  const enc = new TextEncoder();
  const auth = enc.encode(request.headers.get("X-Admin-Secret") || "");
  const secret = enc.encode(env.ADMIN_SECRET || "");
  if (auth.length !== secret.length) return false;
  return constantTimeEqual(auth, secret);
}

async function deleteLicenseKeyRecords(env, licenseKey) {
  await env.DB.prepare("DELETE FROM activations WHERE license_key = ?").bind(licenseKey).run();
  await env.DB.prepare("DELETE FROM generated_keys WHERE license_key = ?").bind(licenseKey).run();
}

async function purgeRevokedKeys(env) {
  await env.DB.prepare(
    "DELETE FROM activations WHERE license_key IN (SELECT license_key FROM generated_keys WHERE status = 'revoked')"
  ).run();
  await env.DB.prepare("DELETE FROM generated_keys WHERE status = 'revoked'").run();
}

// ---------------------------------------------------------------------------
// 客户端 API
// ---------------------------------------------------------------------------

async function handleActivate(request, env) {
  let body;
  try { body = await request.json(); } catch { return errorResponse("请求体 JSON 格式错误", 400); }

  const { key, device_id, device_fingerprint } = body || {};
  if (!key || !device_id) return errorResponse("缺少必填参数：key、device_id", 400);

  const secretBytes = new TextEncoder().encode(env.HMAC_SECRET);
  const { valid, planDays } = await validateKey(key, secretBytes);
  if (!valid) return errorResponse("卡密无效：格式错误或签名不匹配", 403);
  if (planDays <= 0) return errorResponse("卡密无效：有效期异常", 403);

  const normalizedKey = key.trim().toUpperCase();

  // 检查卡密是否在 generated_keys 表中注册且未被吊销
  const genRecord = await env.DB.prepare("SELECT * FROM generated_keys WHERE license_key = ?").bind(normalizedKey).first();
  if (!genRecord) return errorResponse("该卡密不存在或已被吊销", 403);
  if (genRecord.status === "revoked") {
    await deleteLicenseKeyRecords(env, normalizedKey);
    return errorResponse("该卡密已被吊销，无法使用", 403);
  }

  const existing = await env.DB.prepare("SELECT * FROM activations WHERE license_key = ?").bind(normalizedKey).first();
  const now = nowISO();

  if (existing) {
    if (existing.device_id !== device_id) {
      return errorResponse("该卡密已在其他设备激活，不允许更换设备。如需帮助请联系作者。", 403);
    }
    // 同设备重复激活：保留原过期时间，仅更新指纹和时间戳
    await env.DB.prepare(
      "UPDATE activations SET updated_at=?, device_fingerprint=? WHERE license_key=?"
    ).bind(now, device_fingerprint || "", normalizedKey).run();
    return jsonResponse({
      success: true, message: "重新激活成功",
      activated_at: existing.activated_at, expires_at: existing.expires_at, plan_days: existing.plan_days,
    });
  }

  const expires = expiresISO(planDays);
  await env.DB.prepare(
    "INSERT INTO activations (license_key,device_id,device_fingerprint,plan_days,activated_at,expires_at,updated_at) VALUES (?,?,?,?,?,?,?)"
  ).bind(normalizedKey, device_id, device_fingerprint || "", planDays, now, expires, now).run();

  // 同步更新 generated_keys 状态
  await env.DB.prepare("UPDATE generated_keys SET status='activated' WHERE license_key=?").bind(normalizedKey).run();

  return jsonResponse({ success: true, message: "激活成功", activated_at: now, expires_at: expires, plan_days: planDays });
}

async function handleVerify(request, env) {
  let body;
  try { body = await request.json(); } catch { return errorResponse("请求体 JSON 格式错误", 400); }

  const { key, device_id } = body || {};
  if (!key || !device_id) return errorResponse("缺少必填参数：key、device_id", 400);

  const normalizedKey = key.trim().toUpperCase();

  // 检查卡密是否已被吊销
  const genRecord = await env.DB.prepare("SELECT status FROM generated_keys WHERE license_key = ?").bind(normalizedKey).first();
  if (!genRecord) {
    return jsonResponse({ success: false, message: "该卡密已被吊销", expired: true });
  }
  if (genRecord && genRecord.status === "revoked") {
    await deleteLicenseKeyRecords(env, normalizedKey);
    return jsonResponse({ success: false, message: "该卡密已被吊销", expired: true });
  }

  const record = await env.DB.prepare("SELECT * FROM activations WHERE license_key=?").bind(normalizedKey).first();
  if (!record) return errorResponse("该卡密尚未激活", 404);
  if (record.device_id !== device_id) return errorResponse("设备不匹配：该卡密已绑定其他设备", 403);

  if (new Date() > new Date(record.expires_at)) {
    return jsonResponse({ success: false, message: "授权已过期", expires_at: record.expires_at, expired: true });
  }
  return jsonResponse({ success: true, message: "授权有效", expires_at: record.expires_at, plan_days: record.plan_days, activated_at: record.activated_at });
}

// ---------------------------------------------------------------------------
// 管理员 API
// ---------------------------------------------------------------------------

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

  return jsonResponse({ success: true, keys, plan_days: planDays, count: keys.length });
}

async function handleAdminList(request, env) {
  if (!checkAdmin(request, env)) return errorResponse("管理员密钥错误", 401);

  await purgeRevokedKeys(env);

  const generated = await env.DB.prepare(
    "SELECT g.*, a.device_id, a.device_fingerprint, a.activated_at, a.expires_at FROM generated_keys g LEFT JOIN activations a ON g.license_key = a.license_key ORDER BY g.id DESC LIMIT 200"
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

  await deleteLicenseKeyRecords(env, key);
  return jsonResponse({ success: true, message: "卡密已吊销并删除记录" });
}

// ---------------------------------------------------------------------------
// 管理页面 HTML
// ---------------------------------------------------------------------------

const ADMIN_HTML = `<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="color-scheme" content="dark">
<link rel="icon" href="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 32 32'%3E%3Crect width='32' height='32' rx='9' fill='%23020617'/%3E%3Cpath d='M18 4a6 6 0 0 0-5.65 7.99L3 21.34V28h6.66l2-2h2.67l2-2v-2.66l3.01-3.01A6 6 0 1 0 18 4Zm2 6.66h.01' fill='none' stroke='%2338bdf8' stroke-width='2.2' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E">
<title>TLS-shipinhao 卡密管理</title>
<style>
:root{
  --bg:#020617;
  --bg-soft:#0f172a;
  --panel:rgba(15,23,42,.86);
  --panel-strong:rgba(8,15,31,.94);
  --panel-soft:rgba(30,41,59,.72);
  --stroke:rgba(148,163,184,.16);
  --stroke-strong:rgba(56,189,248,.34);
  --text:#f8fafc;
  --muted:#94a3b8;
  --cyan:#38bdf8;
  --cyan-soft:rgba(56,189,248,.18);
  --green:#22c55e;
  --green-soft:rgba(34,197,94,.18);
  --amber:#f59e0b;
  --red:#f87171;
  --red-soft:rgba(248,113,113,.18);
  --shadow:0 24px 80px rgba(2,6,23,.55);
  --radius-xl:26px;
  --radius-lg:18px;
  --radius-md:14px;
  --mono:"SFMono-Regular","Menlo","Consolas","Liberation Mono",monospace;
  --sans:"SF Pro Display","PingFang SC","Hiragino Sans GB","Microsoft YaHei","Segoe UI",sans-serif;
}

*{box-sizing:border-box}
html,body{height:100%}
body{
  margin:0;
  font-family:var(--sans);
  color:var(--text);
  background:
    radial-gradient(circle at top left, rgba(34,197,94,.14), transparent 28%),
    radial-gradient(circle at top right, rgba(56,189,248,.17), transparent 26%),
    linear-gradient(135deg, #020617 0%, #081120 46%, #030712 100%);
  overflow:hidden;
}
body::before{
  content:"";
  position:fixed;
  inset:0;
  background:
    linear-gradient(rgba(148,163,184,.05) 1px, transparent 1px),
    linear-gradient(90deg, rgba(148,163,184,.05) 1px, transparent 1px);
  background-size:42px 42px;
  mask-image:linear-gradient(to bottom, rgba(0,0,0,.55), transparent 92%);
  pointer-events:none;
}
a{color:inherit}
button,input{font:inherit}
.hidden{display:none !important}
.shell{
  position:relative;
  z-index:1;
  max-width:1680px;
  width:100%;
  height:100vh;
  margin:0 auto;
  padding:18px;
  display:flex;
  flex-direction:column;
  align-items:center;
  gap:18px;
}
.masthead{
  width:100%;
  max-width:1120px;
  display:flex;
  align-items:center;
  justify-content:space-between;
  gap:20px;
  padding:18px 22px;
  border:1px solid var(--stroke);
  border-radius:28px;
  background:linear-gradient(180deg, rgba(15,23,42,.92), rgba(7,14,27,.92));
  box-shadow:var(--shadow);
  backdrop-filter:blur(20px);
}
.brand{
  display:flex;
  align-items:center;
  gap:16px;
  min-width:0;
}
.brand-icon{
  width:52px;
  height:52px;
  display:grid;
  place-items:center;
  border-radius:16px;
  background:linear-gradient(135deg, rgba(56,189,248,.16), rgba(34,197,94,.18));
  border:1px solid rgba(56,189,248,.28);
  box-shadow:0 0 0 1px rgba(56,189,248,.08), 0 12px 28px rgba(15,23,42,.45);
  color:var(--cyan);
}
.brand-copy{
  min-width:0;
}
.eyebrow{
  display:inline-flex;
  align-items:center;
  gap:8px;
  margin-bottom:8px;
  font-size:.72rem;
  font-weight:600;
  letter-spacing:.18em;
  text-transform:uppercase;
  color:var(--cyan);
}
.eyebrow::before{
  content:"";
  width:22px;
  height:1px;
  background:linear-gradient(90deg, rgba(56,189,248,0), rgba(56,189,248,.9));
}
.brand h1{
  margin:0;
  font-size:2rem;
  line-height:1.05;
  letter-spacing:-.03em;
}
.brand p{
  margin:8px 0 0;
  color:var(--muted);
  font-size:.98rem;
}
.masthead-meta{
  display:grid;
  grid-template-columns:repeat(2,minmax(140px,1fr));
  gap:12px;
  min-width:320px;
}
.meta-chip{
  padding:12px 14px;
  border-radius:16px;
  border:1px solid var(--stroke);
  background:rgba(15,23,42,.68);
}
.meta-chip span{
  display:block;
  font-size:.72rem;
  color:var(--muted);
  letter-spacing:.08em;
  text-transform:uppercase;
}
.meta-chip strong{
  display:block;
  margin-top:6px;
  font-size:1rem;
  color:var(--text);
}
.login-shell{
  flex:1;
  display:grid;
  width:100%;
  max-width:1120px;
  margin:0 auto;
  gap:12px;
  align-content:center;
  justify-items:center;
  min-height:0;
}
.login-hero{
  width:100%;
  gap:18px;
  padding:20px 22px;
  justify-content:center;
  align-items:center;
  text-align:center;
}
.login-hero-copy{
  max-width:760px;
}
.login-hero-copy h2{
  margin:0;
  font-size:2rem;
  line-height:1.05;
  letter-spacing:-.04em;
}
.login-hero-copy p{
  margin:12px auto 0;
  max-width:52ch;
  color:var(--muted);
  font-size:.96rem;
  line-height:1.65;
}
.login-feature-grid{
  display:grid;
  grid-template-columns:repeat(3,minmax(0,1fr));
  gap:12px;
  width:100%;
}
.feature-card{
  padding:14px;
  border-radius:20px;
  border:1px solid var(--stroke);
  background:linear-gradient(180deg, rgba(15,23,42,.8), rgba(8,15,31,.72));
  box-shadow:inset 0 0 0 1px rgba(56,189,248,.05);
}
.feature-label{
  display:inline-flex;
  align-items:center;
  gap:8px;
  font-size:.72rem;
  font-weight:700;
  letter-spacing:.12em;
  text-transform:uppercase;
  color:var(--cyan);
}
.feature-label::before{
  content:"";
  width:18px;
  height:1px;
  background:linear-gradient(90deg, rgba(56,189,248,0), rgba(56,189,248,.9));
}
.feature-card strong{
  display:block;
  margin-top:10px;
  font-size:.98rem;
}
.feature-card p{
  margin:6px 0 0;
  color:var(--muted);
  font-size:.82rem;
  line-height:1.55;
}
.workspace{
  display:flex;
  flex-direction:column;
  gap:18px;
  flex:1;
  width:100%;
  max-width:1120px;
  min-height:0;
}
.top-grid{
  display:grid;
  grid-template-columns:minmax(0,1.16fr) minmax(360px,.84fr);
  gap:18px;
  min-height:320px;
  height:clamp(320px, 36vh, 390px);
}
.card{
  position:relative;
  display:flex;
  flex-direction:column;
  min-height:0;
  overflow:hidden;
  padding:20px;
  border-radius:var(--radius-xl);
  border:1px solid var(--stroke);
  background:
    linear-gradient(180deg, rgba(15,23,42,.92), rgba(8,15,31,.88));
  box-shadow:var(--shadow);
  backdrop-filter:blur(22px);
}
.card::after{
  content:"";
  position:absolute;
  inset:0;
  border-radius:inherit;
  padding:1px;
  background:linear-gradient(135deg, rgba(56,189,248,.32), rgba(148,163,184,.06), rgba(34,197,94,.26));
  mask:linear-gradient(#fff 0 0) content-box, linear-gradient(#fff 0 0);
  mask-composite:exclude;
  -webkit-mask:linear-gradient(#fff 0 0) content-box, linear-gradient(#fff 0 0);
  -webkit-mask-composite:xor;
  pointer-events:none;
}
.panel-head{
  display:flex;
  align-items:flex-start;
  justify-content:space-between;
  gap:18px;
  margin-bottom:18px;
}
.panel-head h2{
  margin:0;
  font-size:1.35rem;
  line-height:1.08;
  letter-spacing:-.03em;
}
.panel-head p{
  margin:8px 0 0;
  max-width:56ch;
  color:var(--muted);
  font-size:.92rem;
  line-height:1.55;
}
.panel-head-tight{
  margin-bottom:14px;
}
.status-pill,.legend-item{
  display:inline-flex;
  align-items:center;
  gap:8px;
  padding:10px 12px;
  border-radius:999px;
  border:1px solid rgba(34,197,94,.24);
  background:rgba(34,197,94,.09);
  color:#d1fae5;
  white-space:nowrap;
  font-size:.8rem;
  font-weight:600;
}
.status-pill::before,.dot{
  content:"";
  width:8px;
  height:8px;
  border-radius:999px;
  background:currentColor;
  box-shadow:0 0 12px currentColor;
}
.dot{
  display:inline-block;
  background:var(--cyan);
  color:var(--cyan);
}
.dot-green{background:var(--green);color:var(--green)}
.dot-amber{background:var(--amber);color:var(--amber)}
.legend{
  display:flex;
  align-items:center;
  justify-content:flex-end;
  gap:10px;
  flex-wrap:wrap;
}
.field-grid{
  display:grid;
  grid-template-columns:repeat(3,minmax(0,1fr));
  gap:14px;
}
.field{
  display:block;
}
.field span{
  display:block;
  margin-bottom:8px;
  color:var(--muted);
  font-size:.82rem;
  letter-spacing:.04em;
}
input{
  width:100%;
  height:48px;
  padding:0 14px;
  border-radius:14px;
  border:1px solid rgba(71,85,105,.82);
  background:rgba(8,15,31,.9);
  color:var(--text);
  transition:border-color .18s ease, box-shadow .18s ease, transform .18s ease;
}
input::placeholder{color:rgba(148,163,184,.72)}
input:hover{border-color:rgba(56,189,248,.38)}
input:focus-visible,
button:focus-visible{
  outline:none;
  border-color:var(--cyan);
  box-shadow:0 0 0 3px rgba(56,189,248,.18);
}
.action-row{
  display:flex;
  align-items:center;
  gap:12px;
  margin-top:16px;
}
.btn{
  display:inline-flex;
  align-items:center;
  justify-content:center;
  gap:10px;
  min-height:44px;
  padding:0 18px;
  border:1px solid transparent;
  border-radius:14px;
  cursor:pointer;
  font-weight:700;
  letter-spacing:.01em;
  transition:transform .18s ease, filter .18s ease, background .18s ease, border-color .18s ease;
}
.btn:hover{transform:translateY(-1px);filter:brightness(1.04)}
.btn-primary{
  background:linear-gradient(135deg, #0ea5e9, #2563eb);
  color:#eff6ff;
  box-shadow:0 12px 26px rgba(37,99,235,.32);
}
.btn-secondary{
  border-color:rgba(56,189,248,.22);
  background:rgba(15,23,42,.78);
  color:#dbeafe;
}
.btn-ghost{
  border-color:rgba(148,163,184,.2);
  background:rgba(255,255,255,.02);
  color:var(--text);
}
.btn-danger{
  min-height:36px;
  padding:0 14px;
  border-radius:12px;
  border-color:rgba(248,113,113,.3);
  background:rgba(127,29,29,.62);
  color:#fecaca;
  box-shadow:none;
}
.btn-sm{
  min-height:38px;
  padding:0 14px;
  font-size:.84rem;
}
.keys-output{
  margin-top:16px;
  flex:1;
  min-height:0;
  padding:16px;
  border-radius:18px;
  border:1px solid rgba(34,197,94,.18);
  background:
    linear-gradient(180deg, rgba(7,18,20,.88), rgba(3,10,18,.92));
  color:#bbf7d0;
  font-family:var(--mono);
  font-size:.84rem;
  line-height:1.7;
  white-space:pre-wrap;
  word-break:break-all;
  overflow:auto;
  box-shadow:inset 0 0 0 1px rgba(34,197,94,.05);
}
.stats{
  display:grid;
  grid-template-columns:repeat(3,minmax(0,1fr));
  gap:14px;
}
.stat-item{
  padding:18px 16px;
  border-radius:20px;
  border:1px solid var(--stroke);
  background:linear-gradient(180deg, rgba(15,23,42,.92), rgba(10,15,28,.8));
}
.stat-item:nth-child(1){box-shadow:inset 0 0 0 1px rgba(56,189,248,.08)}
.stat-item:nth-child(2){box-shadow:inset 0 0 0 1px rgba(34,197,94,.08)}
.stat-item:nth-child(3){box-shadow:inset 0 0 0 1px rgba(245,158,11,.08)}
.stat-label{
  display:block;
  font-size:.8rem;
  letter-spacing:.06em;
  text-transform:uppercase;
  color:var(--muted);
}
.stat-num{
  display:block;
  margin-top:10px;
  font-size:2rem;
  font-weight:700;
  line-height:1;
  letter-spacing:-.05em;
}
.stat-item:nth-child(1) .stat-num{color:var(--cyan)}
.stat-item:nth-child(2) .stat-num{color:#4ade80}
.stat-item:nth-child(3) .stat-num{color:#fbbf24}
.table-card{
  flex:1;
}
.table-wrap{
  flex:1;
  min-height:0;
  overflow:auto;
  border-radius:20px;
  border:1px solid var(--stroke);
  background:rgba(4,10,22,.72);
}
table{
  width:100%;
  border-collapse:separate;
  border-spacing:0;
  table-layout:fixed;
  font-size:.8rem;
}
thead th{
  position:sticky;
  top:0;
  z-index:2;
  padding:14px 12px;
  text-align:left;
  font-size:.75rem;
  letter-spacing:.08em;
  text-transform:uppercase;
  color:var(--muted);
  background:rgba(2,6,23,.96);
  border-bottom:1px solid var(--stroke);
}
tbody tr{
  transition:background .18s ease, transform .18s ease;
}
tbody tr:hover{
  background:rgba(15,23,42,.66);
}
tbody td{
  padding:14px 12px;
  border-bottom:1px solid rgba(148,163,184,.08);
  vertical-align:top;
  color:#dbe4f3;
  line-height:1.45;
  word-break:break-word;
}
th:nth-child(1),td:nth-child(1){width:16%}
th:nth-child(2),td:nth-child(2){width:7%}
th:nth-child(3),td:nth-child(3){width:10%}
th:nth-child(4),td:nth-child(4){width:10%}
th:nth-child(5),td:nth-child(5){width:13%}
th:nth-child(6),td:nth-child(6){width:14%}
th:nth-child(7),td:nth-child(7){width:12%}
th:nth-child(8),td:nth-child(8){width:12%}
th:nth-child(9),td:nth-child(9){width:6%}
.mono{
  font-family:var(--mono);
}
.badge{
  display:inline-flex;
  align-items:center;
  gap:8px;
  padding:5px 10px;
  border-radius:999px;
  border:1px solid transparent;
  font-size:.74rem;
  font-weight:700;
  white-space:nowrap;
}
.badge::before{
  content:"";
  width:7px;
  height:7px;
  border-radius:999px;
  background:currentColor;
  box-shadow:0 0 12px currentColor;
}
.badge-green{
  color:#4ade80;
  background:rgba(20,83,45,.28);
  border-color:rgba(74,222,128,.16);
}
.badge-gray{
  color:#cbd5e1;
  background:rgba(51,65,85,.38);
  border-color:rgba(148,163,184,.12);
}
.badge-red{
  color:#fca5a5;
  background:rgba(127,29,29,.3);
  border-color:rgba(248,113,113,.14);
}
.cell-sub{
  margin-top:6px;
  color:var(--muted);
  font-size:.74rem;
}
.empty-state{
  padding:30px 18px !important;
  text-align:center;
  color:var(--muted);
}
.msg{
  margin-bottom:14px;
  padding:12px 14px;
  border-radius:14px;
  font-size:.85rem;
  line-height:1.5;
  border:1px solid transparent;
}
.msg-err{
  background:rgba(127,29,29,.22);
  color:#fecaca;
  border-color:rgba(248,113,113,.16);
}
.msg-ok{
  background:rgba(20,83,45,.22);
  color:#d1fae5;
  border-color:rgba(74,222,128,.16);
}
.login-card{
  width:min(100%, 560px);
  padding:20px;
  justify-self:center;
  align-self:start;
}
.login-card .panel-head{
  margin-bottom:14px;
}
.login-form{
  display:grid;
  gap:12px;
}
.login-card .btn{
  width:100%;
  margin-top:2px;
}
.login-footnote{
  margin-top:10px;
  color:var(--muted);
  font-size:.82rem;
  line-height:1.5;
}

@media (max-width: 1240px){
  .masthead{
    flex-direction:column;
    align-items:flex-start;
  }
  .masthead-meta{
    width:100%;
    min-width:0;
  }
  .top-grid{
    grid-template-columns:1fr;
    min-height:auto;
    height:auto;
  }
  .login-feature-grid{
    grid-template-columns:1fr;
    margin-top:20px;
  }
}

@media (max-width: 980px){
  body{overflow:auto}
  .shell{
    height:auto;
    min-height:100vh;
  }
  .workspace{
    min-height:auto;
  }
  .stats,
  .masthead-meta,
  .field-grid{
    grid-template-columns:1fr;
  }
  .login-shell{
    width:100%;
  }
  .table-card{
    min-height:580px;
  }
  .legend{
    justify-content:flex-start;
  }
}

@media (max-height: 720px){
  body{overflow:auto}
  .shell{
    height:auto;
    min-height:100vh;
  }
  .workspace{
    min-height:auto;
  }
  .table-card{
    min-height:540px;
  }
}

@media (prefers-reduced-motion: reduce){
  *,*::before,*::after{
    animation:none !important;
    transition:none !important;
    scroll-behavior:auto !important;
  }
}
</style>
</head>
<body>
<div class="shell">
  <header class="masthead">
    <div class="brand">
      <div class="brand-icon" aria-hidden="true">
        <svg width="28" height="28" viewBox="0 0 24 24" fill="none" role="presentation">
          <path d="M13.5 2a4.5 4.5 0 0 0-4.24 5.99L2 15.25V20h4.75l1.5-1.5h2l1.5-1.5v-2l2.26-2.26A4.5 4.5 0 1 0 13.5 2Z" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>
          <path d="M15.5 7.5h.01" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"/>
        </svg>
      </div>
      <div class="brand-copy">
        <span class="eyebrow">Admin Control Grid</span>
        <h1>TLS-shipinhao License Console</h1>
        <p>单屏管理卡密生成、状态总览与记录维护，默认在桌面端完整展示全部核心模块。</p>
      </div>
    </div>
    <div class="masthead-meta">
      <div class="meta-chip">
        <span>当前模式</span>
        <strong>Secure Admin</strong>
      </div>
      <div class="meta-chip">
        <span>布局策略</span>
        <strong>Desktop Single Screen</strong>
      </div>
    </div>
  </header>

  <div id="loginView" class="login-shell">
    <section class="card login-hero">
      <div class="login-hero-copy">
        <span class="eyebrow">Operator Launchpad</span>
        <h2>统一完成卡密生成、吊销与激活记录审查</h2>
        <p>登录后直接进入单屏控制台，生成配置、状态统计和卡密明细会同时进入视野，不再因为浏览器可视高度变化而意外切成窄屏布局。</p>
      </div>
      <div class="login-feature-grid">
        <div class="feature-card">
          <span class="feature-label">Batch</span>
          <strong>批量发卡更直接</strong>
          <p>数量、有效期和备注固定保持桌面端同排配置，减少来回扫视。</p>
        </div>
        <div class="feature-card">
          <span class="feature-label">Live</span>
          <strong>统计与列表同屏</strong>
          <p>总量、未使用、已激活会跟明细区联动刷新，定位记录更快。</p>
        </div>
        <div class="feature-card">
          <span class="feature-label">Audit</span>
          <strong>吊销后即时清除</strong>
          <p>管理员操作直接落到 Worker 与 D1，避免旧数据残留造成误判。</p>
        </div>
      </div>
    </section>
    <section class="card login-card">
      <div class="panel-head">
        <div>
          <span class="eyebrow">Admin Access</span>
          <h2>管理员登录</h2>
          <p>输入管理员密钥后进入卡密控制台，所有操作均通过同源接口完成。</p>
        </div>
        <div class="status-pill">安全校验中</div>
      </div>
      <div id="loginMsg"></div>
      <div class="login-form">
        <label class="field">
          <span>管理密钥</span>
          <input type="password" id="adminSecret" placeholder="请输入 ADMIN_SECRET" autocomplete="current-password">
        </label>
        <button class="btn btn-primary" onclick="doLogin()">进入控制台</button>
      </div>
      <div class="login-footnote">建议使用桌面浏览器登录。按下回车也可以直接提交，不需要额外点击。</div>
    </section>
  </div>

  <main id="mainView" class="workspace hidden">
    <section class="top-grid">
      <article class="card">
        <div class="panel-head">
          <div>
            <span class="eyebrow">Key Forge</span>
            <h2>批量生成卡密</h2>
            <p>在同一视野内完成数量、有效期和备注配置，生成结果会固定显示在下方输出区。</p>
          </div>
          <div class="status-pill">同源安全连接</div>
        </div>
        <div id="genMsg"></div>
        <div class="field-grid">
          <label class="field">
            <span>数量（1-50）</span>
            <input type="number" id="genCount" value="5" min="1" max="50">
          </label>
          <label class="field">
            <span>有效期（天）</span>
            <input type="number" id="genDays" value="30" min="1">
          </label>
          <label class="field">
            <span>备注</span>
            <input type="text" id="genNote" placeholder="可选备注">
          </label>
        </div>
        <div class="action-row">
          <button class="btn btn-primary" onclick="doGenerate()">生成卡密</button>
          <button class="btn btn-secondary hidden" onclick="copyKeys()" id="copyBtn">复制全部</button>
        </div>
        <div id="keysOutput" class="keys-output hidden"></div>
      </article>

      <article class="card">
        <div class="panel-head">
          <div>
            <span class="eyebrow">License Telemetry</span>
            <h2>卡密总览</h2>
            <p>实时查看总量、未使用与已激活数量，并快速刷新列表，避免来回滚动查看关键信息。</p>
          </div>
          <button class="btn btn-ghost btn-sm" onclick="loadList()">刷新列表</button>
        </div>
        <div id="statsArea" class="stats"></div>
      </article>
    </section>

    <section class="card table-card">
      <div class="panel-head panel-head-tight">
        <div>
          <span class="eyebrow">License Registry</span>
          <h2>卡密明细</h2>
          <p>列表区域自动占满剩余高度，桌面端默认无需页面滚动即可查看所有控制区。</p>
        </div>
        <div class="legend">
          <span class="legend-item"><span class="dot"></span>未使用</span>
          <span class="legend-item"><span class="dot dot-green"></span>已激活</span>
          <span class="legend-item"><span class="dot dot-amber"></span>已过期</span>
        </div>
      </div>
      <div class="table-wrap">
        <table>
          <thead>
            <tr>
              <th>卡密</th>
              <th>有效期</th>
              <th>状态</th>
              <th>备注</th>
              <th>生成时间</th>
              <th>设备 ID</th>
              <th>激活时间</th>
              <th>过期时间</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody id="keysList"></tbody>
        </table>
      </div>
    </section>
  </main>
</div>
<script>
let SECRET = "";
const API = location.origin;

function api(path, body) {
  return fetch(API + path, {
    method: "POST",
    headers: { "Content-Type": "application/json", "X-Admin-Secret": SECRET },
    body: JSON.stringify(body)
  }).then(res => res.json());
}

function doLogin() {
  SECRET = document.getElementById("adminSecret").value;
  if (!SECRET) {
    showMsg("loginMsg", "请输入管理密钥", true);
    return;
  }
  api("/api/admin/list", {}).then(res => {
    if (!res.success) {
      showMsg("loginMsg", "密钥错误", true);
      SECRET = "";
      return;
    }
    document.getElementById("loginView").classList.add("hidden");
    document.getElementById("mainView").classList.remove("hidden");
    renderList(res);
  }).catch(() => {
    showMsg("loginMsg", "登录请求失败，请稍后重试", true);
    SECRET = "";
  });
}

function doGenerate() {
  const count = parseInt(document.getElementById("genCount").value, 10) || 5;
  const plan_days = parseInt(document.getElementById("genDays").value, 10) || 30;
  const note = document.getElementById("genNote").value;
  api("/api/admin/generate", { count, plan_days, note }).then(res => {
    if (!res.success) {
      showMsg("genMsg", res.message, true);
      return;
    }
    showMsg("genMsg", "成功生成 " + res.keys.length + " 个卡密", false);
    const out = document.getElementById("keysOutput");
    out.textContent = res.keys.join("\\n");
    out.classList.remove("hidden");
    document.getElementById("copyBtn").classList.remove("hidden");
    loadList();
  }).catch(() => {
    showMsg("genMsg", "生成请求失败，请稍后重试", true);
  });
}

function copyKeys() {
  const text = document.getElementById("keysOutput").textContent;
  navigator.clipboard.writeText(text).then(() => {
    const btn = document.getElementById("copyBtn");
    btn.textContent = "已复制";
    setTimeout(() => { btn.textContent = "复制全部"; }, 1500);
  });
}

function doRevoke(key) {
  if (!confirm("确定要吊销卡密 " + key + " 吗？此操作不可逆！")) return;
  api("/api/admin/revoke", { key }).then(res => {
    if (!res.success) {
      alert(res.message);
      return;
    }
    loadList();
  });
}

function loadList() {
  api("/api/admin/list", {}).then(res => {
    if (res.success) renderList(res);
  });
}

function renderList(res) {
  const stats = res.stats || [];
  const total = stats.reduce((sum, row) => sum + row.cnt, 0);
  const unused = stats.find(row => row.status === "unused")?.cnt || 0;
  const activated = stats.find(row => row.status === "activated")?.cnt || 0;
  document.getElementById("statsArea").innerHTML = [
    stat(total, "总计"),
    stat(unused, "未使用"),
    stat(activated, "已激活")
  ].join("");

  const rows = res.keys || [];
  const tbody = document.getElementById("keysList");
  if (!rows.length) {
    tbody.innerHTML = '<tr><td colspan="9" class="empty-state">当前还没有卡密记录，生成后会在这里立即展示。</td></tr>';
    return;
  }

  tbody.innerHTML = rows.map(row => {
    const expired = !!(row.expires_at && new Date() > new Date(row.expires_at));
    const badge = row.status === "activated"
      ? '<span class="badge badge-green">已激活</span>'
      : row.status === "revoked"
        ? '<span class="badge badge-red">已吊销</span>'
        : '<span class="badge badge-gray">未使用</span>';
    const expireTag = expired ? '<div class="cell-sub"><span class="badge badge-red">已过期</span></div>' : '';
    const revokeBtn = row.status !== "revoked"
      ? '<button class="btn btn-danger btn-sm" onclick="doRevoke(&quot;' + esc(row.license_key) + '&quot;)">吊销</button>'
      : "-";

    return '<tr>'
      + '<td class="mono">' + esc(row.license_key) + '</td>'
      + '<td>' + row.plan_days + ' 天</td>'
      + '<td>' + badge + expireTag + '</td>'
      + '<td>' + esc(row.note || "-") + '</td>'
      + '<td>' + fmt(row.created_at) + '</td>'
      + '<td class="mono">' + esc(row.device_id || "-") + '</td>'
      + '<td>' + fmt(row.activated_at) + '</td>'
      + '<td>' + fmt(row.expires_at) + '</td>'
      + '<td>' + revokeBtn + '</td>'
      + '</tr>';
  }).join("");
}

function stat(n, label) {
  return '<div class="stat-item"><span class="stat-label">' + label + '</span><span class="stat-num">' + n + '</span></div>';
}

function fmt(value) {
  return value ? value.replace(/T/, " ").replace(/\\+00:00$/, "") : "-";
}

function esc(value) {
  const node = document.createElement("div");
  node.textContent = value;
  return node.innerHTML;
}

function showMsg(id, msg, err) {
  document.getElementById(id).innerHTML = '<div class="msg ' + (err ? "msg-err" : "msg-ok") + '">' + esc(msg) + '</div>';
  setTimeout(() => {
    document.getElementById(id).innerHTML = "";
  }, 4000);
}

document.getElementById("adminSecret").addEventListener("keydown", event => {
  if (event.key === "Enter") doLogin();
});
</script>
</body>
</html>`;

function serveAdminPage() {
  return htmlResponse(ADMIN_HTML);
}

// ---------------------------------------------------------------------------
// 入口
// ---------------------------------------------------------------------------

export default {
  async fetch(request, env) {
    // CORS 预检
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

    // 管理页面（GET）
    if (request.method === "GET" && (url.pathname === "/admin" || url.pathname === "/admin/")) {
      return serveAdminPage();
    }

    if (request.method !== "POST") {
      return errorResponse("仅支持 POST 请求", 405);
    }

    // 检查 HMAC_SECRET 是否配置
    if (!env.HMAC_SECRET) {
      return errorResponse("服务器配置错误：缺少 HMAC_SECRET", 500);
    }

    switch (url.pathname) {
      case "/api/activate":
        return handleActivate(request, env);
      case "/api/verify":
        return handleVerify(request, env);
      case "/api/admin/generate":
      case "/api/admin/list":
      case "/api/admin/revoke": {
        let resp;
        if (url.pathname === "/api/admin/generate") resp = await handleAdminGenerate(request, env);
        else if (url.pathname === "/api/admin/list") resp = await handleAdminList(request, env);
        else resp = await handleAdminRevoke(request, env);
        // 管理员端点不开放 CORS，仅允许同源访问
        resp.headers.delete("Access-Control-Allow-Origin");
        return resp;
      }
      default:
        return errorResponse("未知路由", 404);
    }
  },
};
