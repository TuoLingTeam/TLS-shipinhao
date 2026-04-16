import { useAppStore } from "../stores/app";
import { useTauriInvoke } from "./useTauriInvoke";

export function useLicense() {
  const appStore = useAppStore();
  const activate = useTauriInvoke<{ success: boolean; message: string }>("activate_license");
  const verify = useTauriInvoke<{ success: boolean; message: string }>("verify_license");

  async function activateLicense(licenseKey: string, deviceId: string) {
    const result = await activate.execute({ license_key: licenseKey, device_id: deviceId });
    if (result?.success) {
      appStore.setLicenseState("active");
    }
    return result;
  }

  async function verifyLicense(licenseKey: string, deviceId: string) {
    const result = await verify.execute({ license_key: licenseKey, device_id: deviceId });
    if (result?.success) {
      appStore.setLicenseState("active");
    }
    return result;
  }

  return { activateLicense, verifyLicense, activateLoading: activate.loading, verifyLoading: verify.loading };
}
