import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import { computed, ref } from "vue";
import { toErrorMessage } from "./toErrorMessage";

export interface CookieHealthSnapshot {
  healthy: boolean;
  configured: boolean;
  has_biz_magic: boolean;
  last_checked_at: string | null;
  hint: string | null;
}

const POLL_INTERVAL_MS = 30 * 60 * 1000;
const STARTUP_DELAY_MS = 8 * 1000;

export const useCookieHealthStore = defineStore("cookie-health", () => {
  const snapshot = ref<CookieHealthSnapshot>({
    healthy: false,
    configured: false,
    has_biz_magic: false,
    last_checked_at: null,
    hint: null,
  });
  const loading = ref(false);
  const error = ref<string | null>(null);
  let timer: number | null = null;
  let started = false;
  let startupHandle: number | null = null;

  const status = computed<"unknown" | "healthy" | "unhealthy" | "unconfigured">(() => {
    if (!snapshot.value.last_checked_at) return "unknown";
    if (!snapshot.value.configured) return "unconfigured";
    return snapshot.value.healthy ? "healthy" : "unhealthy";
  });

  async function refreshSilently() {
    try {
      const current = await invoke<CookieHealthSnapshot>("get_cookie_health");
      snapshot.value = current;
    } catch (err) {
      error.value = toErrorMessage(err);
    }
  }

  async function probe() {
    if (loading.value) return;
    loading.value = true;
    error.value = null;
    try {
      const result = await invoke<CookieHealthSnapshot>("check_cookie_health");
      snapshot.value = result;
    } catch (err) {
      error.value = toErrorMessage(err);
    } finally {
      loading.value = false;
    }
  }

  function start() {
    if (started) return;
    started = true;
    void refreshSilently();
    startupHandle = window.setTimeout(() => {
      void probe();
    }, STARTUP_DELAY_MS);
    timer = window.setInterval(() => {
      void probe();
    }, POLL_INTERVAL_MS);
  }

  function stop() {
    started = false;
    if (timer !== null) {
      window.clearInterval(timer);
      timer = null;
    }
    if (startupHandle !== null) {
      window.clearTimeout(startupHandle);
      startupHandle = null;
    }
  }

  return {
    snapshot,
    loading,
    error,
    status,
    probe,
    refreshSilently,
    start,
    stop,
  };
});
