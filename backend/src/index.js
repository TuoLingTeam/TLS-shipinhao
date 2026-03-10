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
  if (!genRecord) return errorResponse("卡密未注册：该卡密不在系统记录中", 403);
  if (genRecord.status === "revoked") return errorResponse("该卡密已被吊销，无法使用", 403);

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
  if (genRecord && genRecord.status === "revoked") {
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
  if (record.status === "revoked") return errorResponse("该卡密已被吊销", 400);

  await env.DB.prepare("UPDATE generated_keys SET status = 'revoked' WHERE license_key = ?").bind(key).run();
  return jsonResponse({ success: true, message: "卡密已吊销" });
}

// ---------------------------------------------------------------------------
// 管理页面 HTML
// ---------------------------------------------------------------------------

const ADMIN_HTML = `<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>TLS-shipinhao 卡密管理</title>
<style>
*{box-sizing:border-box;margin:0;padding:0}
body{font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;background:#0f172a;color:#e2e8f0;min-height:100vh;padding:24px}
.container{max-width:960px;margin:0 auto}
h1{font-size:1.5rem;margin-bottom:24px;color:#38bdf8}
.card{background:#1e293b;border-radius:12px;padding:20px;margin-bottom:20px;border:1px solid #334155}
.card h2{font-size:1.1rem;margin-bottom:16px;color:#94a3b8}
label{display:block;font-size:.85rem;color:#94a3b8;margin-bottom:4px}
input,select,textarea{width:100%;padding:10px 12px;border:1px solid #475569;border-radius:8px;background:#0f172a;color:#e2e8f0;font-size:.9rem;margin-bottom:12px}
input:focus,textarea:focus{outline:none;border-color:#38bdf8}
.row{display:flex;gap:12px}
.row>*{flex:1}
btn,button,.btn{display:inline-block;padding:10px 20px;border:none;border-radius:8px;font-size:.9rem;cursor:pointer;font-weight:600}
.btn-primary{background:#2563eb;color:#fff}.btn-primary:hover{background:#1d4ed8}
.btn-sm{padding:6px 14px;font-size:.8rem}
.login-box{max-width:400px;margin:80px auto}
table{width:100%;border-collapse:collapse;font-size:.82rem}
th,td{padding:8px 10px;text-align:left;border-bottom:1px solid #334155}
th{color:#94a3b8;font-weight:600;position:sticky;top:0;background:#1e293b}
.badge{display:inline-block;padding:2px 8px;border-radius:4px;font-size:.75rem;font-weight:600}
.badge-green{background:#064e3b;color:#34d399}
.badge-gray{background:#374151;color:#9ca3af}
.badge-red{background:#7f1d1d;color:#fca5a5}
.keys-output{background:#0f172a;border:1px solid #475569;border-radius:8px;padding:12px;font-family:monospace;font-size:.85rem;white-space:pre-wrap;word-break:break-all;max-height:200px;overflow-y:auto;margin-top:12px;color:#34d399}
.stats{display:flex;gap:16px;margin-bottom:16px}
.stat-item{background:#0f172a;border-radius:8px;padding:12px 16px;text-align:center;border:1px solid #334155}
.stat-num{font-size:1.5rem;font-weight:700;color:#38bdf8}
.stat-label{font-size:.75rem;color:#94a3b8}
.table-wrap{max-height:500px;overflow-y:auto;border-radius:8px;border:1px solid #334155}
.hidden{display:none}
.msg{padding:10px;border-radius:8px;margin-bottom:12px;font-size:.85rem}
.msg-err{background:#7f1d1d;color:#fca5a5}
.msg-ok{background:#064e3b;color:#34d399}
.copy-btn{background:#475569;color:#e2e8f0;border:none;padding:4px 10px;border-radius:4px;cursor:pointer;font-size:.75rem;margin-left:8px}
.copy-btn:hover{background:#64748b}
</style>
</head>
<body>
<div class="container">
  <h1>🔑 TLS-shipinhao 卡密管理</h1>
  <div id="loginView" class="login-box">
    <div class="card">
      <h2>管理员登录</h2>
      <div id="loginMsg"></div>
      <label>管理密钥</label>
      <input type="password" id="adminSecret" placeholder="请输入 ADMIN_SECRET">
      <button class="btn btn-primary" style="width:100%" onclick="doLogin()">登录</button>
    </div>
  </div>
  <div id="mainView" class="hidden">
    <div class="card">
      <h2>批量生成卡密</h2>
      <div id="genMsg"></div>
      <div class="row">
        <div><label>数量（1-50）</label><input type="number" id="genCount" value="5" min="1" max="50"></div>
        <div><label>有效期（天）</label><input type="number" id="genDays" value="30" min="1"></div>
      </div>
      <label>备注</label>
      <input type="text" id="genNote" placeholder="可选备注">
      <button class="btn btn-primary" onclick="doGenerate()">生成卡密</button>
      <button class="copy-btn" onclick="copyKeys()" id="copyBtn" style="display:none">复制全部</button>
      <div id="keysOutput" class="keys-output hidden"></div>
    </div>
    <div class="card">
      <h2>卡密总览</h2>
      <div id="statsArea" class="stats"></div>
      <button class="btn btn-primary btn-sm" onclick="loadList()" style="margin-bottom:16px">刷新列表</button>
      <div class="table-wrap">
        <table>
          <thead><tr><th>卡密</th><th>有效期</th><th>状态</th><th>备注</th><th>生成时间</th><th>设备ID</th><th>激活时间</th><th>过期时间</th><th>操作</th></tr></thead>
          <tbody id="keysList"></tbody>
        </table>
      </div>
    </div>
  </div>
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
  });
}

function doGenerate() {
  const count = parseInt(document.getElementById("genCount").value) || 5;
  const plan_days = parseInt(document.getElementById("genDays").value) || 30;
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
    document.getElementById("copyBtn").style.display = "inline-block";
    loadList();
  });
}

function copyKeys() {
  const text = document.getElementById("keysOutput").textContent;
  navigator.clipboard.writeText(text).then(() => {
    const btn = document.getElementById("copyBtn");
    btn.textContent = "已复制!";
    setTimeout(() => btn.textContent = "复制全部", 1500);
  });
}

function doRevoke(key) {
  if (!confirm('确定要吊销卡密 ' + key + ' 吗？此操作不可逆！')) return;
  api('/api/admin/revoke', { key }).then(res => {
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
  const sa = document.getElementById("statsArea");
  const total = (res.stats || []).reduce((s, r) => s + r.cnt, 0);
  const unused = (res.stats || []).find(r => r.status === "unused")?.cnt || 0;
  const activated = (res.stats || []).find(r => r.status === "activated")?.cnt || 0;
  const revoked = (res.stats || []).find(r => r.status === "revoked")?.cnt || 0;
  sa.innerHTML = [
    stat(total, "总计"), stat(unused, "未使用"), stat(activated, "已激活"), stat(revoked, "已吊销")
  ].join("");
  
  const tbody = document.getElementById("keysList");
  tbody.innerHTML = (res.keys || []).map(r => {
    const st = r.status === "activated" ? '<span class="badge badge-green">已激活</span>'
      : r.status === "revoked" ? '<span class="badge badge-red">已吊销</span>'
      : '<span class="badge badge-gray">未使用</span>';
    const expired = r.status !== "revoked" && r.expires_at && new Date() > new Date(r.expires_at);
    const expBadge = expired ? ' <span class="badge badge-red">已过期</span>' : '';
    const revokeBtn = r.status !== "revoked"
      ? '<button class="copy-btn" style="background:#7f1d1d;color:#fca5a5" onclick="doRevoke(&quot;' + esc(r.license_key) + '&quot;)">吊销</button>'
      : '-';
    return '<tr>'
      + '<td style="font-family:monospace;font-size:.78rem">' + esc(r.license_key) + '</td>'
      + '<td>' + r.plan_days + '天</td>'
      + '<td>' + st + expBadge + '</td>'
      + '<td>' + esc(r.note || '') + '</td>'
      + '<td>' + fmt(r.created_at) + '</td>'
      + '<td style="font-size:.75rem">' + esc(r.device_id || '-') + '</td>'
      + '<td>' + fmt(r.activated_at) + '</td>'
      + '<td>' + fmt(r.expires_at) + '</td>'
      + '<td>' + revokeBtn + '</td>'
      + '</tr>';
  }).join("");
}

function stat(n, label) {
  return '<div class="stat-item"><div class="stat-num">' + n + '</div><div class="stat-label">' + label + '</div></div>';
}

function fmt(s) { 
  return s ? s.replace(/T/, ' ').replace(/\\+00:00$/, '') : '-'; 
}

function esc(s) { 
  const d = document.createElement('div'); 
  d.textContent = s; 
  return d.innerHTML; 
}

function showMsg(id, msg, err) {
  document.getElementById(id).innerHTML = '<div class="msg ' + (err ? 'msg-err' : 'msg-ok') + '">' + esc(msg) + '</div>';
  setTimeout(() => document.getElementById(id).innerHTML = "", 4000);
}

document.getElementById("adminSecret").addEventListener("keydown", e => { 
  if (e.key === "Enter") doLogin(); 
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
