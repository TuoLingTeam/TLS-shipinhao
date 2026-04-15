-- TLS-shipinhao 卡密激活记录表
CREATE TABLE IF NOT EXISTS activations (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  license_key TEXT NOT NULL UNIQUE,
  device_id TEXT NOT NULL,
  device_fingerprint TEXT DEFAULT '',
  plan_days INTEGER NOT NULL,
  activated_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

-- 按卡密快速查询
CREATE INDEX IF NOT EXISTS idx_activations_license_key ON activations (license_key);

-- 已生成的卡密记录表
CREATE TABLE IF NOT EXISTS generated_keys (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  license_key TEXT NOT NULL UNIQUE,
  plan_days INTEGER NOT NULL,
  status TEXT NOT NULL DEFAULT 'unused',  -- unused / activated
  created_at TEXT NOT NULL,
  note TEXT DEFAULT ''
);

CREATE INDEX IF NOT EXISTS idx_generated_keys_status ON generated_keys (status);

-- 后台操作审计日志
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

-- 客户端授权校验事件
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
