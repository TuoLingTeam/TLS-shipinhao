/**
 * 与 Rust `api_contracts::LicenseState` 严格一一对齐。
 * 后端 serde 序列化永远落在下列 10 个取值之一；若未来新增状态，必须同时更新：
 *   1. `api-contracts/src/lib.rs::LicenseState` 枚举变体
 *   2. `api-contracts/src/lib.rs::LICENSE_STATE_SERDE_LABELS` 常量
 *   3. 本文件与 `LICENSE_STATE_LABELS`
 *
 * 后端单测 `license_state_serde_labels_cover_all_variants` 守住 #1 与 #2 的一致性；
 * 前端这一组取值与 Rust 取值的关系靠 PR 评审与本注释维护。
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

/**
 * `api_contracts::RuntimeState` 在 TS 端的只读镜像。
 *
 * 当前 `useLicense` 内的 `LicensePayload` 只覆盖了 IPC `get_license_status` /
 * `activate_license` / `verify_license` 这一组命令实际暴露的子集；如果未来
 * 后端在不破坏向后兼容的前提下补充了字段（如 `task_policy` / `risk_level` /
 * `compromised`），TS 这边没有任何编译期信号会提醒。
 *
 * 这个接口定义保持「全字段、全可选」与 Rust serde 的 `#[serde(default)]`
 * 配套：
 * - 字段名严格按 `api_contracts::RuntimeState` 的 snake_case
 * - 字段全部 optional，模拟「老前端读到新字段时什么都不做」的安全语义
 * - 不直接绑定到 `useTauriInvoke` 上以避免改动现有命令契约
 *
 * 想要在某个组件里访问扩展字段时，按需把命令返回值断言为
 * `LicensePayload & RuntimeStateView`，无需改 IPC 实现。
 */
export interface RuntimeStateView {
  license_key?: string;
  device_id?: string;
  reason?: LicenseState;
  status_hint?: LicenseState;
  license_expires_at?: string;
  lease_expires_at?: string;
  renew_after?: string;
  task_policy?: string[];
  risk_level?: string;
  runtime_backend?: string;
  compromised?: boolean;
  last_verify_at?: string;
}
