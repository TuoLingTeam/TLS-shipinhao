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

#[derive(Debug, Deserialize)]
struct RevokeBody {
    key: String,
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
        Ok(s) => s.to_string(),
        Err(_) => {
            return Ok(Some(json_err(503, "ADMIN_SECRET 未配置，拒绝管理员接口")?));
        }
    };
    let got = headers.get("X-Admin-Secret")?.unwrap_or_default();
    if got != expected {
        return Ok(Some(json_err(401, "unauthorized")?));
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
    let req: RevokeBody = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => return json_err(400, format!("invalid_json: {e}")),
    };
    let key = req.key.trim().to_uppercase();
    if key.is_empty() {
        return json_err(400, "empty_key");
    }

    let sql = "UPDATE generated_keys SET status = 'revoked' WHERE license_key = ?";
    let stmt = db.prepare(sql).bind(&[JsValue::from_str(&key)])?;
    stmt.run().await?;

    Response::from_json(&serde_json::json!({ "success": true }))
}

fn js_iso_timestamp() -> String {
    js_sys::Date::new_0().to_iso_string().into()
}

fn random_license_key() -> String {
    let mut buf = [0u8; 8];
    if getrandom::getrandom(&mut buf).is_err() {
        return format!("TLS-FALLBACK-{:020}", js_sys::Date::now() as u64);
    }
    format!(
        "TLS-{}",
        buf.iter()
            .fold(String::new(), |acc, b| acc + &format!("{:02X}", b))
    )
}

pub async fn serve_admin_html() -> Result<Response> {
    const HTML: &str = include_str!("../../../backend/src/admin/admin.html");
    Response::from_html(HTML)
}
