use crate::*;
use wasm_bindgen::JsValue;
use worker::D1Database;

pub(crate) struct D1RuntimeRepo<'a> {
    db: &'a D1Database,
}

impl<'a> D1RuntimeRepo<'a> {
    pub(crate) fn new(db: &'a D1Database) -> Self {
        Self { db }
    }
}

fn enum_text<T: serde::Serialize>(value: &T) -> anyhow::Result<String> {
    Ok(serde_json::to_string(value)?.trim_matches('"').to_string())
}

#[async_trait(?Send)]
impl AsyncRuntimeRepository for D1RuntimeRepo<'_> {
    async fn load_generated_key(
        &self,
        license_key: &str,
    ) -> anyhow::Result<Option<GeneratedKeyRecord>> {
        let stmt = self
            .db
            .prepare(
                "SELECT license_key, CAST(plan_days AS INTEGER) AS plan_days, status, COALESCE(created_at, '') AS created_at, COALESCE(note, '') AS note FROM generated_keys WHERE license_key = ? LIMIT 1",
            )
            .bind(&[JsValue::from_str(license_key)])?;
        let result = stmt.all().await?;
        let mut rows: Vec<GeneratedKeyRecord> = result.results().unwrap_or_default();
        Ok(rows.pop())
    }

    async fn save_generated_key(&self, record: &GeneratedKeyRecord) -> anyhow::Result<()> {
        self.db
            .prepare(
                "INSERT INTO generated_keys (license_key, plan_days, status, created_at, note) VALUES (?, ?, ?, ?, ?) \
                 ON CONFLICT(license_key) DO UPDATE SET plan_days = excluded.plan_days, status = excluded.status, created_at = excluded.created_at, note = excluded.note",
            )
            .bind(&[
                JsValue::from_str(&record.license_key),
                JsValue::from_f64(record.plan_days as f64),
                JsValue::from_str(&enum_text(&record.status)?),
                JsValue::from_str(&record.created_at),
                JsValue::from_str(&record.note),
            ])?
            .run()
            .await?;
        Ok(())
    }

    async fn load_license(&self, license_key: &str) -> anyhow::Result<Option<LicenseRecord>> {
        let stmt = self
            .db
            .prepare(
                "SELECT license_key, device_id, COALESCE(device_fingerprint, '') AS device_fingerprint, CAST(plan_days AS INTEGER) AS plan_days, activated_at, expires_at AS license_expires_at, updated_at, binding_version, status, COALESCE(last_verify_at, '') AS last_verify_at FROM activations WHERE license_key = ? LIMIT 1",
            )
            .bind(&[JsValue::from_str(license_key)])?;
        let result = stmt.all().await?;
        let mut rows: Vec<LicenseRecord> = result.results().unwrap_or_default();
        Ok(rows.pop())
    }

    async fn save_license(&self, record: &LicenseRecord) -> anyhow::Result<()> {
        self.db
            .prepare(
                "INSERT INTO activations (license_key, device_id, device_fingerprint, plan_days, activated_at, expires_at, updated_at, binding_version, status, last_verify_at, last_session_issued_at, last_offline_grant_issued_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, COALESCE((SELECT last_session_issued_at FROM activations WHERE license_key = ?), ''), COALESCE((SELECT last_offline_grant_issued_at FROM activations WHERE license_key = ?), '')) \
                 ON CONFLICT(license_key) DO UPDATE SET device_id = excluded.device_id, device_fingerprint = excluded.device_fingerprint, plan_days = excluded.plan_days, activated_at = excluded.activated_at, expires_at = excluded.expires_at, updated_at = excluded.updated_at, binding_version = excluded.binding_version, status = excluded.status, last_verify_at = excluded.last_verify_at",
            )
            .bind(&[
                JsValue::from_str(&record.license_key),
                JsValue::from_str(&record.device_id),
                JsValue::from_str(&record.device_fingerprint),
                JsValue::from_f64(record.plan_days as f64),
                JsValue::from_str(&record.activated_at),
                JsValue::from_str(&record.license_expires_at),
                JsValue::from_str(&record.updated_at),
                JsValue::from_f64(record.binding_version as f64),
                JsValue::from_str(&enum_text(&record.status)?),
                JsValue::from_str(&record.last_verify_at),
                JsValue::from_str(&record.license_key),
                JsValue::from_str(&record.license_key),
            ])?
            .run()
            .await?;
        Ok(())
    }

    async fn load_device_registration(
        &self,
        license_key: &str,
        device_id: &str,
    ) -> anyhow::Result<Option<DeviceRegistration>> {
        let stmt = self
            .db
            .prepare(
                "SELECT license_key, device_id, COALESCE(device_fingerprint_hash, '') AS device_fingerprint_hash, COALESCE(registered_at, '') AS registered_at, COALESCE(last_seen_at, '') AS last_seen_at, COALESCE(registration_status, 'active') AS registration_status FROM device_registrations WHERE license_key = ? AND device_id = ? LIMIT 1",
            )
            .bind(&[JsValue::from_str(license_key), JsValue::from_str(device_id)])?;
        let result = stmt.all().await?;
        let mut rows: Vec<DeviceRegistration> = result.results().unwrap_or_default();
        Ok(rows.pop())
    }

    async fn save_device_registration(&self, record: &DeviceRegistration) -> anyhow::Result<()> {
        if self
            .load_device_registration(&record.license_key, &record.device_id)
            .await?
            .is_some()
        {
            self.db
                .prepare(
                    "UPDATE device_registrations SET device_fingerprint_hash = ?, registered_at = ?, last_seen_at = ?, registration_status = ? WHERE license_key = ? AND device_id = ?",
                )
                .bind(&[
                    JsValue::from_str(&record.device_fingerprint_hash),
                    JsValue::from_str(&record.registered_at),
                    JsValue::from_str(&record.last_seen_at),
                    JsValue::from_str(&record.registration_status),
                    JsValue::from_str(&record.license_key),
                    JsValue::from_str(&record.device_id),
                ])?
                .run()
                .await?;
        } else {
            self.db
                .prepare(
                    "INSERT INTO device_registrations (license_key, device_id, device_fingerprint_hash, registered_at, last_seen_at, registration_status) VALUES (?, ?, ?, ?, ?, ?)",
                )
                .bind(&[
                    JsValue::from_str(&record.license_key),
                    JsValue::from_str(&record.device_id),
                    JsValue::from_str(&record.device_fingerprint_hash),
                    JsValue::from_str(&record.registered_at),
                    JsValue::from_str(&record.last_seen_at),
                    JsValue::from_str(&record.registration_status),
                ])?
                .run()
                .await?;
        }
        Ok(())
    }

    async fn append_audit_event(&self, event: &AuditEvent) -> anyhow::Result<()> {
        self.db
            .prepare(
                "INSERT INTO license_audit_logs (license_key, device_id, action, action_reason, created_at, operator, meta_json) VALUES (?, ?, ?, ?, ?, 'worker', '{}')",
            )
            .bind(&[
                JsValue::from_str(&event.license_key),
                JsValue::from_str(&event.device_id),
                JsValue::from_str(&event.action),
                JsValue::from_str(&event.reason),
                JsValue::from_str(&event.created_at),
            ])?
            .run()
            .await?;
        Ok(())
    }

    async fn update_runtime_markers(
        &self,
        license_key: &str,
        now_iso: &str,
        session_issued: bool,
        grant_issued: bool,
        new_status: Option<LicenseState>,
    ) -> anyhow::Result<()> {
        let session_sql = if session_issued {
            ", last_session_issued_at = ?"
        } else {
            ""
        };
        let grant_sql = if grant_issued {
            ", last_offline_grant_issued_at = ?"
        } else {
            ""
        };
        let status_sql = if new_status.is_some() {
            ", status = ?"
        } else {
            ""
        };
        let sql = format!(
            "UPDATE activations SET updated_at = ?, last_verify_at = ?{session_sql}{grant_sql}{status_sql} WHERE license_key = ?"
        );
        let mut binds = vec![JsValue::from_str(now_iso), JsValue::from_str(now_iso)];
        if session_issued {
            binds.push(JsValue::from_str(now_iso));
        }
        if grant_issued {
            binds.push(JsValue::from_str(now_iso));
        }
        if let Some(status) = new_status {
            binds.push(JsValue::from_str(&enum_text(&status)?));
        }
        binds.push(JsValue::from_str(license_key));
        self.db.prepare(&sql).bind(&binds)?.run().await?;
        Ok(())
    }

    async fn revoke_license(
        &self,
        license_key: &str,
        device_id: &str,
        reason: &str,
        revoked_at: &str,
    ) -> anyhow::Result<bool> {
        let Some(_key_record) = self.load_generated_key(license_key).await? else {
            return Ok(false);
        };

        let advanced_update = self
            .db
            .prepare(revoke_generated_key_update_sql(true))
            .bind(&[
                JsValue::from_str(revoked_at),
                JsValue::from_str(reason),
                JsValue::from_str(license_key),
            ])?
            .run()
            .await;
        if advanced_update.is_err() {
            self.db
                .prepare(revoke_generated_key_update_sql(false))
                .bind(&[JsValue::from_str(license_key)])?
                .run()
                .await?;
        }

        self.db
            .prepare(
                "UPDATE activations SET status = 'revoked', updated_at = ?, last_verify_at = ? WHERE license_key = ?",
            )
            .bind(&[
                JsValue::from_str(revoked_at),
                JsValue::from_str(revoked_at),
                JsValue::from_str(license_key),
            ])?
            .run()
            .await?;

        self.db
            .prepare(
                "UPDATE device_sessions SET revoked_at = ? WHERE license_key = ? AND (revoked_at IS NULL OR revoked_at = '')",
            )
            .bind(&[JsValue::from_str(revoked_at), JsValue::from_str(license_key)])?
            .run()
            .await?;

        self.db
            .prepare(
                "UPDATE device_registrations SET registration_status = 'revoked', last_seen_at = ? WHERE license_key = ? AND device_id = ?",
            )
            .bind(&[
                JsValue::from_str(revoked_at),
                JsValue::from_str(license_key),
                JsValue::from_str(device_id),
            ])?
            .run()
            .await?;
        Ok(true)
    }

    async fn revoke_license_by_key(
        &self,
        license_key: &str,
        reason: &str,
        revoked_at: &str,
    ) -> anyhow::Result<bool> {
        let Some(_key_record) = self.load_generated_key(license_key).await? else {
            return Ok(false);
        };

        let advanced_update = self
            .db
            .prepare(revoke_generated_key_update_sql(true))
            .bind(&[
                JsValue::from_str(revoked_at),
                JsValue::from_str(reason),
                JsValue::from_str(license_key),
            ])?
            .run()
            .await;
        if advanced_update.is_err() {
            self.db
                .prepare(revoke_generated_key_update_sql(false))
                .bind(&[JsValue::from_str(license_key)])?
                .run()
                .await?;
        }

        self.db
            .prepare(
                "UPDATE activations SET status = 'revoked', updated_at = ?, last_verify_at = ? WHERE license_key = ?",
            )
            .bind(&[
                JsValue::from_str(revoked_at),
                JsValue::from_str(revoked_at),
                JsValue::from_str(license_key),
            ])?
            .run()
            .await?;

        self.db
            .prepare(
                "UPDATE device_sessions SET revoked_at = ? WHERE license_key = ? AND (revoked_at IS NULL OR revoked_at = '')",
            )
            .bind(&[JsValue::from_str(revoked_at), JsValue::from_str(license_key)])?
            .run()
            .await?;

        self.db
            .prepare(
                "UPDATE device_registrations SET registration_status = 'revoked', last_seen_at = ? WHERE license_key = ?",
            )
            .bind(&[JsValue::from_str(revoked_at), JsValue::from_str(license_key)])?
            .run()
            .await?;
        Ok(true)
    }
}
