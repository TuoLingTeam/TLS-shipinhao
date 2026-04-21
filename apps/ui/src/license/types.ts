/**
 * 与 Rust `api_contracts::LicenseState` 严格一一对齐。
 * 后端 serde 序列化永远落在下列 10 个取值之一；若未来新增状态，必须同时更新
 * `api-contracts/src/lib.rs::LicenseState` 与本文件与 LICENSE_STATE_LABELS。
 */
export type LicenseState =
  | "active"
  | "not_found"
  | "renewal_due"
  | "expired"
  | "revoked"
  | "device_mismatch"
  | "reactivation_required"
  | "online_refresh_required"
  | "invalid"
  | "compromised";

export const LICENSE_STATE_LABELS: Record<LicenseState, string> = {
  active: "已激活",
  not_found: "未发现租约",
  renewal_due: "待续期",
  expired: "已过期",
  revoked: "已吊销",
  device_mismatch: "设备不匹配",
  reactivation_required: "需重新激活",
  online_refresh_required: "需联网刷新",
  invalid: "未激活",
  compromised: "异常",
};

export type RiskLevel = "low" | "medium" | "high";

export interface LicenseLease {
  license_key: string;
  device_id: string;
  license_status: LicenseState;
  license_expires_at: string;
  lease_expires_at: string;
  renew_after: string;
  task_policy: string[];
  keyset_version: number;
  binding_version: number;
  issued_at: string;
}

export interface RuntimeGrant {
  task_type: string;
  granted: boolean;
  grant_id: string;
  valid_until: string;
  risk_level: RiskLevel | null;
  degraded_reason: string | null;
}
