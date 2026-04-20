-- TLS-shipinhao V2 授权表结构

CREATE TABLE IF NOT EXISTS activations (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  license_key TEXT NOT NULL UNIQUE,
  device_id TEXT NOT NULL,
  device_fingerprint TEXT DEFAULT '',
  plan_days INTEGER NOT NULL,
  activated_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  binding_version INTEGER NOT NULL DEFAULT 2,
  status TEXT NOT NULL DEFAULT 'active',
  last_verify_at TEXT DEFAULT '',
  last_session_issued_at TEXT DEFAULT '',
  last_offline_grant_issued_at TEXT DEFAULT ''
);

CREATE INDEX IF NOT EXISTS idx_activations_license_key ON activations (license_key);
CREATE INDEX IF NOT EXISTS idx_activations_status ON activations (status);

CREATE TABLE IF NOT EXISTS generated_keys (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  license_key TEXT NOT NULL UNIQUE,
  plan_days INTEGER NOT NULL,
  status TEXT NOT NULL DEFAULT 'unused',
  created_at TEXT NOT NULL,
  note TEXT DEFAULT ''
);

CREATE INDEX IF NOT EXISTS idx_generated_keys_status ON generated_keys (status);

CREATE TABLE IF NOT EXISTS device_sessions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  license_key TEXT NOT NULL,
  device_id TEXT NOT NULL,
  session_id TEXT NOT NULL UNIQUE,
  issued_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  revoked_at TEXT DEFAULT NULL,
  client_version TEXT DEFAULT '',
  ip_hash TEXT DEFAULT ''
);

CREATE INDEX IF NOT EXISTS idx_device_sessions_license_key ON device_sessions (license_key);
CREATE INDEX IF NOT EXISTS idx_device_sessions_device_id ON device_sessions (device_id);
CREATE INDEX IF NOT EXISTS idx_device_sessions_revoked_at ON device_sessions (revoked_at);

CREATE TABLE IF NOT EXISTS device_registrations (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  license_key TEXT NOT NULL,
  device_id TEXT NOT NULL,
  device_fingerprint_hash TEXT DEFAULT '',
  registered_at TEXT NOT NULL,
  last_seen_at TEXT NOT NULL,
  registration_status TEXT NOT NULL DEFAULT 'active'
);

CREATE INDEX IF NOT EXISTS idx_device_registrations_license_key ON device_registrations (license_key);
CREATE INDEX IF NOT EXISTS idx_device_registrations_device_id ON device_registrations (device_id);

CREATE TABLE IF NOT EXISTS license_audit_logs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  license_key TEXT DEFAULT '',
  device_id TEXT DEFAULT '',
  action TEXT NOT NULL,
  action_reason TEXT DEFAULT '',
  created_at TEXT NOT NULL,
  operator TEXT DEFAULT 'system',
  meta_json TEXT DEFAULT ''
);

CREATE INDEX IF NOT EXISTS idx_license_audit_logs_license_key ON license_audit_logs (license_key);
CREATE INDEX IF NOT EXISTS idx_license_audit_logs_created_at ON license_audit_logs (created_at);
