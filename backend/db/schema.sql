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
