import type { Ref } from "vue";
import { useAppStore } from "../app.store";
import { useTauriInvoke } from "../shared/useTauriInvoke";
import type { LicenseState } from "./types";

type LicensePayload = {
  success?: boolean;
  message?: string;
  configured?: boolean;
  license_state: LicenseState;
  license_key?: string | null;
  /** 卡密硬过期（一般 100 年）；来自 REST 响应，UI 标签「卡密有效期」。 */
  license_expires_at?: string | null;
  /** Lease Token 硬过期（一般 3 天）；来自已签名 Lease payload.exp，UI 标签「下次续约」。 */
  lease_expires_at?: string | null;
  last_verified_at?: string | null;
  /** 后端探测到本地 profile 有卡密但 Lease 容器丢失时为 true，前端据此自动远端恢复。 */
  needs_restore?: boolean;
};

export function useLicense() {
  const appStore = useAppStore();
  const activate = useTauriInvoke<LicensePayload>("activate_license");
  const verify = useTauriInvoke<LicensePayload>("verify_license");
  const loadStored = useTauriInvoke<LicensePayload>("get_license_status");

  function applyLicensePayload(result: LicensePayload | null | undefined) {
    if (!result) return;
    appStore.setLicenseInfo({
      license_state: result.license_state ?? "invalid",
      license_key: result.license_key,
      license_expires_at: result.license_expires_at,
      lease_expires_at: result.lease_expires_at,
      last_verified_at: result.last_verified_at,
    });
  }

  // adapters/license.rs::activate 会把 "已在其他设备激活" 等非 active/renewal_due
  // 的合法业务响应重包装成 LicenseHttpError::InvalidResponse，到 Tauri 层表现为
  // rejected Promise；useTauriInvoke 只把错误写进 error ref、对调用方返回 null。
  // 结果是 LicenseView/SettingsView 的 `if (result)` 静默吞掉提示。这里兜住：
  // invoke reject 时重建成业务失败等价的 payload，上层显示逻辑不用再调整。
  function wrapInvokeFailure(
    errorRef: Ref<string | null>,
    fallback: string,
  ): LicensePayload {
    const message = errorRef.value?.trim() || fallback;
    return {
      success: false,
      message,
      license_state: appStore.licenseState,
    };
  }

  async function activateLicense(licenseKey: string) {
    const result = await activate.execute({ license_key: licenseKey });
    if (result) {
      applyLicensePayload(result);
      return result;
    }
    return wrapInvokeFailure(activate.error, "激活失败，请检查网络或稍后重试");
  }

  async function verifyLicense(licenseKey: string) {
    const result = await verify.execute({ license_key: licenseKey });
    if (result) {
      applyLicensePayload(result);
      return result;
    }
    return wrapInvokeFailure(verify.error, "刷新失败，请检查网络或稍后重试");
  }

  async function loadStoredLicenseStatus() {
    const result = await loadStored.execute();
    applyLicensePayload(result);
    return result;
  }

  async function refreshStoredLicenseStatus() {
    const result = await loadStoredLicenseStatus();
    if (result?.license_key) {
      const verified = await verifyLicense(result.license_key);
      return verified ?? result;
    }
    return result;
  }

  /**
   * 启动时调用一次：仅在后端判定为"半孤立 profile"时自动远端 verify 恢复 Lease。
   * - 正常已授权用户不会打扰（只读 get_license_status）
   * - 卡密还在有效期但 Lease 丢失的场景会在有网时无感恢复
   */
  async function restoreStoredLicenseIfNeeded() {
    const snapshot = await loadStoredLicenseStatus();
    if (!snapshot?.needs_restore || !snapshot.license_key) return snapshot;
    const verified = await verifyLicense(snapshot.license_key);
    return verified ?? snapshot;
  }

  return {
    activateLicense,
    verifyLicense,
    loadStoredLicenseStatus,
    refreshStoredLicenseStatus,
    restoreStoredLicenseIfNeeded,
    activateLoading: activate.loading,
    verifyLoading: verify.loading,
    loadStoredLoading: loadStored.loading,
  };
}
