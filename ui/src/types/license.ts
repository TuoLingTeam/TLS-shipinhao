export type LicenseState =
  | "active"
  | "renewal_due"
  | "expired"
  | "revoked"
  | "device_mismatch"
  | "invalid"
  | "compromised";

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
