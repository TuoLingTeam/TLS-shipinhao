import { defineStore } from "pinia";
import { ref } from "vue";
import type { LicenseState } from "../types/license";

export const useAppStore = defineStore("app", () => {
  const licenseState = ref<LicenseState>("invalid");
  const isLicensed = ref(false);
  const appVersion = ref("5.0.0");
  const licenseKey = ref("");
  const licenseExpiresAt = ref<string | null>(null);
  const lastVerifiedAt = ref<string | null>(null);

  function setLicenseState(state: LicenseState) {
    licenseState.value = state;
    isLicensed.value = state === "active" || state === "renewal_due";
  }

  function setLicenseInfo(payload: {
    license_state: LicenseState;
    license_key?: string | null;
    license_expires_at?: string | null;
    last_verified_at?: string | null;
  }) {
    setLicenseState(payload.license_state);
    licenseKey.value = payload.license_key ?? "";
    licenseExpiresAt.value = payload.license_expires_at ?? null;
    lastVerifiedAt.value = payload.last_verified_at ?? null;
  }

  return {
    licenseState,
    isLicensed,
    appVersion,
    licenseKey,
    licenseExpiresAt,
    lastVerifiedAt,
    setLicenseState,
    setLicenseInfo,
  };
});
