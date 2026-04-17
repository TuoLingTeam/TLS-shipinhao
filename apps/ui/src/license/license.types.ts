export type LicenseState =
  | "active"
  | "renewal_due"
  | "expired"
  | "revoked"
  | "device_mismatch"
  | "invalid"
  | "compromised"
  | "offline_grace"
  | "pending_activation"
  | "unknown";

export const LICENSE_STATE_LABELS: Record<LicenseState, string> = {
  active: "已激活",
  renewal_due: "待续期",
  expired: "已过期",
  revoked: "已吊销",
  device_mismatch: "设备不匹配",
  invalid: "未激活",
  compromised: "异常",
  offline_grace: "离线宽限",
  pending_activation: "待激活",
  unknown: "未知状态",
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
