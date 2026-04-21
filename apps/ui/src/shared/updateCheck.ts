import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { formatDateTime } from "./format";
import { toErrorMessage } from "./toErrorMessage";

/** 与后端 `check_for_update` / `update-available` 对齐的最小快照（多余字段 JSON 可忽略） */
export interface UpdateCheckSnapshot {
  app: string;
  version: string;
  build: number;
  mandatory: boolean;
  platform: string;
  download_url: string;
  tutorial_url: string;
  notes: string[];
  has_update: boolean;
  raw_payload: unknown;
}

const DISMISS_KEY = "update_dismiss_until";
const DISMISS_DURATION_MS = 24 * 60 * 60 * 1000;
const RETRY_DELAY_MS = 3000;

function readDismissedUntil(): number {
  if (typeof window === "undefined") return 0;
  const raw = Number(window.localStorage.getItem(DISMISS_KEY) ?? "0");
  return Number.isFinite(raw) ? raw : 0;
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    window.setTimeout(resolve, ms);
  });
}

export const useUpdateCheckStore = defineStore("update-check", () => {
  const latestInfo = ref<UpdateCheckSnapshot | null>(null);
  const lastError = ref<string | null>(null);
  /** 用户点击「打开下载页」失败时展示（与检查更新失败 lastError 区分） */
  const downloadActionError = ref<string | null>(null);
  const tutorialActionError = ref<string | null>(null);
  const lastCheckAt = ref<number | null>(null);
  const checking = ref(false);
  const dismissedUntil = ref(readDismissedUntil());

  let unlisten: UnlistenFn | null = null;
  let started = false;

  const hasUpdateAvailable = computed(() => latestInfo.value?.has_update === true);

  const bannerVisible = computed(() => {
    if (!latestInfo.value?.has_update) return false;
    if (latestInfo.value.mandatory) return true;
    return dismissedUntil.value <= Date.now();
  });

  /** 非强制更新且处于「稍后提醒」冷却：横幅隐藏，但设置页仍展示提示 */
  const isSnoozed = computed(() => {
    if (!latestInfo.value?.has_update || latestInfo.value.mandatory) return false;
    return dismissedUntil.value > Date.now();
  });

  const snoozeUntilText = computed(() => {
    if (!isSnoozed.value) return "";
    return formatDateTime(new Date(dismissedUntil.value).toISOString());
  });

  function dismissBanner() {
    if (!latestInfo.value || latestInfo.value.mandatory || typeof window === "undefined") return;
    const until = Date.now() + DISMISS_DURATION_MS;
    window.localStorage.setItem(DISMISS_KEY, String(until));
    dismissedUntil.value = until;
  }

  /** 清除「稍后提醒」，使顶部横幅在下次满足条件时立即出现 */
  function clearSnooze() {
    if (typeof window === "undefined") return;
    window.localStorage.removeItem(DISMISS_KEY);
    dismissedUntil.value = 0;
  }

  async function fetchOnce(): Promise<UpdateCheckSnapshot> {
    return invoke<UpdateCheckSnapshot>("check_for_update");
  }

  async function refresh() {
    if (checking.value) return;
    checking.value = true;
    lastError.value = null;
    try {
      const result = await fetchOnce();
      latestInfo.value = result;
      dismissedUntil.value = readDismissedUntil();
      lastCheckAt.value = Date.now();
      downloadActionError.value = null;
      tutorialActionError.value = null;
    } catch (err) {
      lastError.value = toErrorMessage(err);
      await sleep(RETRY_DELAY_MS);
      try {
        const result = await fetchOnce();
        latestInfo.value = result;
        dismissedUntil.value = readDismissedUntil();
        lastCheckAt.value = Date.now();
        lastError.value = null;
        downloadActionError.value = null;
        tutorialActionError.value = null;
      } catch (err2) {
        lastError.value = toErrorMessage(err2);
      }
    } finally {
      checking.value = false;
    }
  }

  function applyEventPayload(payload: UpdateCheckSnapshot) {
    latestInfo.value = payload;
    dismissedUntil.value = readDismissedUntil();
    lastCheckAt.value = Date.now();
    lastError.value = null;
    downloadActionError.value = null;
    tutorialActionError.value = null;
  }

  /** 从仪表盘 / 设置内嵌入口打开网盘或下载页 */
  async function openDownloadUrl() {
    downloadActionError.value = null;
    const url = latestInfo.value?.download_url?.trim();
    if (!url) {
      downloadActionError.value = "下载链接为空";
      return;
    }
    try {
      await invoke("open_external_url", { url });
    } catch (err) {
      downloadActionError.value = toErrorMessage(err);
    }
  }

  /** 打开 version.json 中的教程链接 */
  async function openTutorialUrl() {
    tutorialActionError.value = null;
    const url = latestInfo.value?.tutorial_url?.trim();
    if (!url) {
      tutorialActionError.value = "暂无教程链接";
      return;
    }
    try {
      await invoke("open_external_url", { url });
    } catch (err) {
      tutorialActionError.value = toErrorMessage(err);
    }
  }

  async function start() {
    if (started) return;
    started = true;
    try {
      unlisten = await listen<UpdateCheckSnapshot>("update-available", (event) => {
        applyEventPayload(event.payload);
      });
    } catch {
      unlisten = null;
    }
    void refresh();
  }

  function stop() {
    started = false;
    if (unlisten) {
      void unlisten();
      unlisten = null;
    }
  }

  return {
    latestInfo,
    lastError,
    downloadActionError,
    tutorialActionError,
    lastCheckAt,
    checking,
    dismissedUntil,
    hasUpdateAvailable,
    bannerVisible,
    isSnoozed,
    snoozeUntilText,
    refresh,
    dismissBanner,
    clearSnooze,
    openDownloadUrl,
    openTutorialUrl,
    start,
    stop,
  };
});
