//! 管理员接口：D1 与 `backend/db/schema.sql` 中 `generated_keys` / `activations` 对齐。

use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;
use worker::{D1Database, Env, Request, Response, Result};

const DB: &str = "DB";

#[derive(Debug, Deserialize)]
struct GenerateBody {
    count: u32,
    plan_days: u32,
    #[serde(default)]
    note: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct StatRow {
    status: String,
    cnt: i64,
}

#[derive(Debug, Deserialize, Serialize)]
struct ListKeyRow {
    license_key: String,
    plan_days: i64,
    status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    note: Option<String>,
    created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    device_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    activated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expires_at: Option<String>,
}

pub async fn handle_admin_request(mut req: Request, env: &Env) -> Result<Response> {
    if let Some(resp) = check_admin(req.headers(), env)? {
        return Ok(resp);
    }

    let db = env.d1(DB)?;
    let path = req.path();
    let body = req.text().await.unwrap_or_default();

    match path.as_str() {
        "/api/admin/list" => admin_list(&db).await,
        "/api/admin/generate" => admin_generate(&db, &body).await,
        "/api/admin/revoke" => admin_revoke(&db, &body).await,
        _ => Response::error("not_found", 404),
    }
}

pub(crate) fn check_admin(headers: &worker::Headers, env: &Env) -> Result<Option<Response>> {
    let expected = match env.secret("ADMIN_SECRET") {
        Ok(s) => Some(s.to_string()),
        Err(_) => None,
    };
    let got = headers.get("X-Admin-Secret")?.unwrap_or_default();
    let auth_error = crate::admin_auth_error_contract(
        expected.is_some(),
        expected
            .as_deref()
            .map(|value| value == got)
            .unwrap_or(false),
    );
    if let Some((status, message)) = auth_error {
        return Ok(Some(json_err(status, message)?));
    }
    Ok(None)
}

fn json_err(status: u16, message: impl AsRef<str>) -> Result<Response> {
    Ok(Response::from_json(&serde_json::json!({
        "success": false,
        "message": message.as_ref(),
    }))?
    .with_status(status))
}

async fn admin_list(db: &D1Database) -> Result<Response> {
    let stats_stmt =
        db.prepare("SELECT status, COUNT(*) as cnt FROM generated_keys GROUP BY status");
    let stats_res = stats_stmt.all().await?;
    let stats: Vec<StatRow> = stats_res.results().unwrap_or_default();

    let list_sql = r#"
        SELECT g.license_key, g.plan_days, g.status, g.note, g.created_at,
               a.device_id, a.activated_at, a.expires_at
        FROM generated_keys g
        LEFT JOIN activations a ON a.license_key = g.license_key
        ORDER BY g.created_at DESC
    "#;
    let list_stmt = db.prepare(list_sql);
    let list_res = list_stmt.all().await?;
    let keys: Vec<ListKeyRow> = list_res.results().unwrap_or_default();

    Response::from_json(&serde_json::json!({
        "success": true,
        "stats": stats,
        "keys": keys,
    }))
}

async fn admin_generate(db: &D1Database, body: &str) -> Result<Response> {
    let req: GenerateBody = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => return json_err(400, format!("invalid_json: {e}")),
    };
    let count = req.count.clamp(1, 100);
    let note = req.note.clone();
    let now = js_iso_timestamp();

    let mut keys_out = Vec::new();
    for _ in 0..count {
        let key = random_license_key();
        let sql = "INSERT INTO generated_keys (license_key, plan_days, status, created_at, note) VALUES (?, ?, 'unused', ?, ?)";
        let stmt = db.prepare(sql).bind(&[
            JsValue::from_str(&key),
            JsValue::from(req.plan_days),
            JsValue::from_str(&now),
            JsValue::from_str(&note),
        ])?;
        if let Err(e) = stmt.run().await {
            return Response::from_json(&serde_json::json!({
                "success": false,
                "message": format!("insert_failed: {e}"),
            }));
        }
        keys_out.push(key);
    }

    Response::from_json(&serde_json::json!({
        "success": true,
        "keys": keys_out,
    }))
}

async fn admin_revoke(db: &D1Database, body: &str) -> Result<Response> {
    let repo = crate::runtime::D1RuntimeRepo::new(db);
    let payload = match crate::handle_admin_revoke_json(&repo, body, chrono::Utc::now()).await {
        Ok(payload) => payload,
        Err(err) => {
            let (status, message) = crate::revoke_error_contract(&err.to_string());
            return json_err(status, message);
        }
    };
    let value: crate::SignedLicenseApiResponse =
        serde_json::from_str(&payload).map_err(|e| worker::Error::RustError(e.to_string()))?;
    Response::from_json(&value).map(|resp| resp.with_status(crate::revoke_response_status(&value)))
}

fn js_iso_timestamp() -> String {
    js_sys::Date::new_0().to_iso_string().into()
}

/// 生成 `TLS-XXXX-XXXX-XXXX-XXXX` 格式的卡密。
///
/// - 10 字节熵源 = 80 bit，按 Crockford Base32（32 字符、剔除 I/L/O/U）编码恰好 16 字符
/// - 16 字符拆成 4 组，便于用户抄写时分组核对
/// - 熵值 80 bit 远高于先前 hex (64 bit) 方案，暴力破解搜索空间多 65,536 倍
fn random_license_key() -> String {
    const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    let mut buf = [0u8; 10];
    if getrandom::getrandom(&mut buf).is_err() {
        // getrandom 理论上不应失败（Worker 运行时有 crypto.getRandomValues），
        // 给个可识别的兜底：仍维持 TLS- 前缀 + 4 组 4 字符的长度骨架
        let ts = (js_sys::Date::now() as u64).to_be_bytes();
        return format!("TLS-{}", encode_crockford_base32(&ts, ALPHABET));
    }
    format!("TLS-{}", encode_crockford_base32(&buf, ALPHABET))
}

/// 把 10 字节（80 bit）编码成 16 字符 Crockford Base32，并按 4-4-4-4 分组。
///
/// `bytes` 长度必须是 5 的倍数；此处固定接收 10 字节，5 bit/字符 × 16 = 80 bit 一一对应。
fn encode_crockford_base32(bytes: &[u8], alphabet: &[u8; 32]) -> String {
    let mut out = String::with_capacity(19); // 16 字符 + 3 连字符
    let mut bit_buf: u64 = 0;
    let mut bit_count: u32 = 0;
    let mut char_count: u32 = 0;
    for &b in bytes {
        bit_buf = (bit_buf << 8) | b as u64;
        bit_count += 8;
        while bit_count >= 5 {
            bit_count -= 5;
            let idx = ((bit_buf >> bit_count) & 0x1F) as usize;
            if char_count > 0 && char_count % 4 == 0 {
                out.push('-');
            }
            out.push(alphabet[idx] as char);
            char_count += 1;
        }
    }
    out
}

pub async fn serve_admin_html() -> Result<Response> {
    const HTML: &str = include_str!("../../assets/admin.html");
    Response::from_html(HTML)
}
