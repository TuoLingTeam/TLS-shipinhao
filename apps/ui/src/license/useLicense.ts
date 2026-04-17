import { useAppStore } from "../app.store";
import { useTauriInvoke } from "../shared/useTauriInvoke";
import type { LicenseState } from "./license.types";

type LicensePayload = {
  success?: boolean;
  message?: string;
  configured?: boolean;
  license_state: LicenseState;
  license_key?: string | null;
  license_expires_at?: string | null;
  last_verified_at?: string | null;
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
      last_verified_at: result.last_verified_at,
    });
  }

  async function activateLicense(licenseKey: string) {
    const result = await activate.execute({ license_key: licenseKey });
    applyLicensePayload(result);
    return result;
  }

  async function verifyLicense(licenseKey: string) {
    const result = await verify.execute({ license_key: licenseKey });
    applyLicensePayload(result);
    return result;
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

  return {
    activateLicense,
    verifyLicense,
    loadStoredLicenseStatus,
    refreshStoredLicenseStatus,
    activateLoading: activate.loading,
    verifyLoading: verify.loading,
    loadStoredLoading: loadStored.loading,
  };
}
