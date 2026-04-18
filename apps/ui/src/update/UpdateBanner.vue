<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

interface UpdateInfo {
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

const updateInfo = ref<UpdateInfo | null>(null);
const dismissedUntil = ref(readDismissedUntil());
let unlisten: UnlistenFn | null = null;

const visible = computed(() => {
  if (!updateInfo.value?.has_update) return false;
  if (updateInfo.value.mandatory) return true;
  return dismissedUntil.value <= Date.now();
});

function readDismissedUntil(): number {
  if (typeof window === "undefined") return 0;
  const raw = Number(window.localStorage.getItem(DISMISS_KEY) ?? "0");
  return Number.isFinite(raw) ? raw : 0;
}

function applyUpdateInfo(next: UpdateInfo | null) {
  if (!next?.has_update) return;
  updateInfo.value = next;
  dismissedUntil.value = readDismissedUntil();
}

function dismiss() {
  if (!updateInfo.value || updateInfo.value.mandatory || typeof window === "undefined") return;
  const until = Date.now() + DISMISS_DURATION_MS;
  window.localStorage.setItem(DISMISS_KEY, String(until));
  dismissedUntil.value = until;
}

async function refreshUpdateInfo() {
  try {
    const result = await invoke<UpdateInfo>("check_for_update");
    applyUpdateInfo(result);
  } catch {
    // 非 Tauri 环境或网络失败时静默降级，不能阻塞启动。
  }
}

type ExternalSlot = "download" | "tutorial";

const opening = ref<ExternalSlot | null>(null);
const openError = ref<string | null>(null);

async function openExternal(url: string, slot: ExternalSlot) {
  if (!url) {
    openError.value = "链接为空，请稍后重试";
    return;
  }
  opening.value = slot;
  openError.value = null;
  try {
    await invoke("open_external_url", { url });
  } catch (err) {
    openError.value = typeof err === "string" ? err : String(err);
  } finally {
    opening.value = null;
  }
}

onMounted(async () => {
  try {
    unlisten = await listen<UpdateInfo>("update-available", (event) => {
      applyUpdateInfo(event.payload);
    });
  } catch {
    unlisten = null;
  }
  await refreshUpdateInfo();
});

onUnmounted(() => {
  if (unlisten) {
    void unlisten();
    unlisten = null;
  }
});
</script>

<template>
  <section v-if="visible && updateInfo" class="hero-panel border border-brand-tint/90 px-5 py-4">
    <div class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
      <div class="min-w-0">
        <div class="flex flex-wrap items-center gap-2">
          <span class="status-badge status-badge-active">发现新版本 {{ updateInfo.version }}</span>
          <span v-if="updateInfo.mandatory" class="status-badge status-badge-danger">强制更新</span>
        </div>
        <div class="mt-2 text-sm leading-6 text-brand-deep">
          已检测到可用更新，建议尽快升级以获取最新修复与兼容能力。
        </div>
        <ul v-if="updateInfo.notes.length" class="mt-3 space-y-1 text-sm leading-6 text-slate-700">
          <li v-for="(note, index) in updateInfo.notes" :key="`${updateInfo.version}-${index}`" class="flex gap-2">
            <span class="mt-[2px] text-brand">•</span>
            <span>{{ note }}</span>
          </li>
        </ul>
      </div>

      <div class="flex shrink-0 flex-wrap gap-2 lg:justify-end">
        <button
          type="button"
          class="action-btn action-btn-primary"
          :disabled="opening === 'download'"
          @click="openExternal(updateInfo.download_url, 'download')"
        >
          {{ opening === 'download' ? '打开中...' : '下载更新' }}
        </button>
        <button
          v-if="updateInfo.tutorial_url"
          type="button"
          class="action-btn action-btn-secondary"
          :disabled="opening === 'tutorial'"
          @click="openExternal(updateInfo.tutorial_url, 'tutorial')"
        >
          {{ opening === 'tutorial' ? '打开中...' : '查看教程' }}
        </button>
        <button v-if="!updateInfo.mandatory" type="button" class="action-btn action-btn-secondary" @click="dismiss">
          稍后提醒
        </button>
      </div>
      <p v-if="openError" class="mt-2 w-full text-xs text-red-600">
        {{ openError }}
      </p>
    </div>
  </section>
</template>
