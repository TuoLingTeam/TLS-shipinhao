import { defineStore } from "pinia";
import { ref } from "vue";
import type { LicenseState } from "../types/license";

export const useAppStore = defineStore("app", () => {
  const licenseState = ref<LicenseState>("invalid");
  const isLicensed = ref(false);
  const appVersion = ref("5.0.0");

  function setLicenseState(state: LicenseState) {
    licenseState.value = state;
    isLicensed.value = state === "active";
  }

  return { licenseState, isLicensed, appVersion, setLicenseState };
});
