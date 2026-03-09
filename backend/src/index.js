/**
 * TLS-shipinhao 卡密验证后端
 * Cloudflare Workers + D1
 */

// ---------------------------------------------------------------------------
// 工具函数
// ---------------------------------------------------------------------------

/** Base32 解码（RFC 4648，兼容无填充输入） */
function base32Decode(input) {
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
  let raw = input.toUpperCase().replace(/=+$/, "");
  let bits = 0;
  let value = 0;
  const output = [];

  for (const ch of raw) {
    const idx = alphabet.indexOf(ch);
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

/** HMAC-SHA256 签名（使用 Web Crypto API） */
async function hmacSha256(secret, data) {
  const key = await crypto.subtle.importKey(
    "raw",
    secret,
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"]
  );
  const sig = await crypto.subtle.sign("HMAC", key, data);
  return new Uint8Array(sig);
}

/** 恒定时间比较 */
function constantTimeEqual(a, b) {
  if (a.length !== b.length) return false;
  let result = 0;
  for (let i = 0; i < a.length; i++) {
    result |= a[i] ^ b[i];
  }
  return result === 0;
}

// ---------------------------------------------------------------------------
// 卡密验证（与 Python license.py 逻辑一致）
// ---------------------------------------------------------------------------

const KEY_PREFIX = "TLS-";
const PAYLOAD_LEN = 10; // 2 (days) + 2 (salt) + 6 (hmac truncated)

/**
 * 校验卡密并返回 { valid, planDays }
 * @param {string} key 卡密字符串
 * @param {Uint8Array} secretBytes HMAC 密钥
 */
async function validateKey(key, secretBytes) {
  try {
    let body = key.trim().toUpperCase();
    if (body.startsWith(KEY_PREFIX)) {
      body = body.slice(KEY_PREFIX.length);
    }
    const raw = body.replace(/-/g, "");
    const padding = (8 - (raw.length % 8)) % 8;
    const decoded = base32Decode(raw + "=".repeat(padding));

    if (decoded.length !== PAYLOAD_LEN) {
      return { valid: false, planDays: 0 };
    }

    const daysBytes = decoded.slice(0, 2);
    const salt = decoded.slice(2, 4);
    const sigStored = decoded.slice(4, 10);

    // 拼接 daysBytes + salt 作为签名输入
    const sigInput = new Uint8Array(4);
    sigInput.set(daysBytes, 0);
    sigInput.set(salt, 2);

    const fullSig = await hmacSha256(secretBytes, sigInput);
    const sigExpected = fullSig.slice(0, 6);

    if (!constantTimeEqual(sigStored, sigExpected)) {
      return { valid: false, planDays: 0 };
    }

    const planDays = (daysBytes[0] << 8) | daysBytes[1]; // big-endian uint16
    return { valid: true, planDays };
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
    headers: {
      "Content-Type": "application/json; charset=utf-8",
      "Access-Control-Allow-Origin": "*",
    },
  });
}

function errorResponse(message, status = 400) {
  return jsonResponse({ success: false, message }, status);
}

// ---------------------------------------------------------------------------
// API 路由
// ---------------------------------------------------------------------------

async function handleActivate(request, env) {
  let body;
  try {
    body = await request.json();
  } catch {
    return errorResponse("请求体 JSON 格式错误", 400);
  }

  const { key, device_id, device_fingerprint } = body || {};
  if (!key || !device_id) {
    return errorResponse("缺少必填参数：key、device_id", 400);
  }

  // 1. 验证卡密签名
  const secretBytes = new TextEncoder().encode(env.HMAC_SECRET);
  const { valid, planDays } = await validateKey(key, secretBytes);
  if (!valid) {
    return errorResponse("卡密无效：格式错误或签名不匹配", 403);
  }
  if (planDays <= 0) {
    return errorResponse("卡密无效：有效期异常", 403);
  }

  const normalizedKey = key.trim().toUpperCase();

  // 2. 查询 D1 是否已激活
  const existing = await env.DB.prepare(
    "SELECT * FROM activations WHERE license_key = ?"
  )
    .bind(normalizedKey)
    .first();

  const now = new Date();
  const expiresAt = new Date(now.getTime() + planDays * 24 * 60 * 60 * 1000);
  const nowISO = now.toISOString().replace(/\.\d{3}Z$/, "+00:00");
  const expiresISO = expiresAt.toISOString().replace(/\.\d{3}Z$/, "+00:00");

  if (existing) {
    // 3a. 已存在记录：检查设备是否一致
    if (existing.device_id !== device_id) {
      return errorResponse(
        "该卡密已在其他设备激活，不允许更换设备。如需帮助请联系作者。",
        403
      );
    }

    // 同设备重新激活 → 更新时间
    await env.DB.prepare(
      `UPDATE activations 
       SET activated_at = ?, expires_at = ?, updated_at = ?, device_fingerprint = ?
       WHERE license_key = ?`
    )
      .bind(nowISO, expiresISO, nowISO, device_fingerprint || "", normalizedKey)
      .run();

    return jsonResponse({
      success: true,
      message: "重新激活成功",
      activated_at: nowISO,
      expires_at: expiresISO,
      plan_days: planDays,
    });
  }

  // 3b. 新激活 → 插入记录
  await env.DB.prepare(
    `INSERT INTO activations (license_key, device_id, device_fingerprint, plan_days, activated_at, expires_at, updated_at)
     VALUES (?, ?, ?, ?, ?, ?, ?)`
  )
    .bind(
      normalizedKey,
      device_id,
      device_fingerprint || "",
      planDays,
      nowISO,
      expiresISO,
      nowISO
    )
    .run();

  return jsonResponse({
    success: true,
    message: "激活成功",
    activated_at: nowISO,
    expires_at: expiresISO,
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
  if (!key || !device_id) {
    return errorResponse("缺少必填参数：key、device_id", 400);
  }

  const normalizedKey = key.trim().toUpperCase();

  const record = await env.DB.prepare(
    "SELECT * FROM activations WHERE license_key = ?"
  )
    .bind(normalizedKey)
    .first();

  if (!record) {
    return errorResponse("该卡密尚未激活", 404);
  }

  if (record.device_id !== device_id) {
    return errorResponse("设备不匹配：该卡密已绑定其他设备", 403);
  }

  // 检查是否过期
  const expiresAt = new Date(record.expires_at);
  if (new Date() > expiresAt) {
    return jsonResponse({
      success: false,
      message: "授权已过期",
      expires_at: record.expires_at,
      expired: true,
    });
  }

  return jsonResponse({
    success: true,
    message: "授权有效",
    expires_at: record.expires_at,
    plan_days: record.plan_days,
    activated_at: record.activated_at,
  });
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
          "Access-Control-Allow-Methods": "POST, OPTIONS",
          "Access-Control-Allow-Headers": "Content-Type",
          "Access-Control-Max-Age": "86400",
        },
      });
    }

    const url = new URL(request.url);

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
      default:
        return errorResponse("未知路由", 404);
    }
  },
};
