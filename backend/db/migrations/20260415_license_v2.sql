-- TLS-shipinhao 授权链路 V2 升级迁移
-- 用途：在保留旧卡密数据的前提下，为服务端短期票据模型补齐字段与表。

PRAGMA foreign_keys = OFF;

ALTER TABLE activations ADD COLUMN binding_version INTEGER NOT NULL DEFAULT 2;
ALTER TABLE activations ADD COLUMN status TEXT NOT NULL DEFAULT 'active';
ALTER TABLE activations ADD COLUMN last_verify_at TEXT DEFAULT '';
ALTER TABLE activations ADD COLUMN last_session_issued_at TEXT DEFAULT '';
ALTER TABLE activations ADD COLUMN last_offline_grant_issued_at TEXT DEFAULT '';

ALTER TABLE generated_keys ADD COLUMN status TEXT NOT NULL DEFAULT 'active';
ALTER TABLE generated_keys ADD COLUMN revoked_at TEXT DEFAULT '';
ALTER TABLE generated_keys ADD COLUMN revoke_reason TEXT DEFAULT '';

CREATE TABLE IF NOT EXISTS device_sessions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  license_key TEXT NOT NULL,
  device_id TEXT NOT NULL,
  session_id TEXT NOT NULL,
  task_type TEXT NOT NULL DEFAULT '',
  issued_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  revoked_at TEXT DEFAULT '',
  client_version TEXT DEFAULT '',
  ip_hash TEXT DEFAULT ''
);

CREATE INDEX IF NOT EXISTS idx_device_sessions_license_key
ON device_sessions(license_key, device_id, expires_at);

CREATE TABLE IF NOT EXISTS device_registrations (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  license_key TEXT NOT NULL,
  device_id TEXT NOT NULL,
  device_fingerprint_hash TEXT DEFAULT '',
  registered_at TEXT NOT NULL,
  last_seen_at TEXT NOT NULL,
  registration_status TEXT NOT NULL DEFAULT 'active'
);

CREATE INDEX IF NOT EXISTS idx_device_registrations_license_key
ON device_registrations(license_key, device_id);

CREATE TABLE IF NOT EXISTS license_audit_logs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  license_key TEXT NOT NULL,
  device_id TEXT DEFAULT '',
  action TEXT NOT NULL,
  action_reason TEXT DEFAULT '',
  created_at TEXT NOT NULL,
  operator TEXT DEFAULT '',
  meta_json TEXT DEFAULT ''
);

CREATE INDEX IF NOT EXISTS idx_license_audit_logs_license_key
ON license_audit_logs(license_key, created_at);

UPDATE activations
SET binding_version = CASE WHEN binding_version IS NULL OR binding_version = 0 THEN 2 ELSE binding_version END,
    status = CASE WHEN status IS NULL OR status = '' THEN 'active' ELSE status END;

UPDATE generated_keys
SET status = CASE WHEN status IS NULL OR status = '' THEN 'active' ELSE status END,
    revoked_at = COALESCE(revoked_at, ''),
    revoke_reason = COALESCE(revoke_reason, '');

PRAGMA foreign_keys = ON;
