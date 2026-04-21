import { defineStore } from "pinia";
import { ref } from "vue";
import type { LicenseState } from "./license/types";
import { APP_VERSION } from "./shared/brand";

export const useAppStore = defineStore("app", () => {
  const licenseState = ref<LicenseState>("invalid");
  const isLicensed = ref(false);
  const appVersion = ref(APP_VERSION);
  const licenseKey = ref("");
  // 卡密硬过期：通常 100 年，UI 标签「卡密有效期」。后端来自 REST 响应的 license_expires_at。
  const licenseExpiresAt = ref<string | null>(null);
  // Lease Token 过期（3 天左右）：短效执行 token，到期必须联网续约。
  // 后端来自已签名 Lease payload 的 exp 字段 → runtime.lease_expires_at。
  const leaseExpiresAt = ref<string | null>(null);
  const lastVerifiedAt = ref<string | null>(null);

  function setLicenseState(state: LicenseState) {
    licenseState.value = state;
    isLicensed.value = state === "active" || state === "renewal_due";
  }

  function setLicenseInfo(payload: {
    license_state: LicenseState;
    license_key?: string | null;
    license_expires_at?: string | null;
    lease_expires_at?: string | null;
    last_verified_at?: string | null;
  }) {
    setLicenseState(payload.license_state);
    licenseKey.value = payload.license_key ?? "";
    licenseExpiresAt.value = payload.license_expires_at ?? null;
    leaseExpiresAt.value = payload.lease_expires_at ?? null;
    lastVerifiedAt.value = payload.last_verified_at ?? null;
  }

  return {
    licenseState,
    isLicensed,
    appVersion,
    licenseKey,
    licenseExpiresAt,
    leaseExpiresAt,
    lastVerifiedAt,
    setLicenseState,
    setLicenseInfo,
  };
});
