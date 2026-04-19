<script setup lang="ts">
import { useRoute, useRouter } from "vue-router";
import { computed } from "vue";
import { useAppStore } from "../app.store";
import { useUiScale } from "../layout/useUiScale";
import { useCookieHealthStore } from "../shared/cookieHealth";
import { buildSettingsLocation, pageMetaMap, type PageName } from "./navigation";

const route = useRoute();
const router = useRouter();
const appStore = useAppStore();
const cookieHealth = useCookieHealthStore();

const pageMeta = computed(() => pageMetaMap[(route.name as PageName) || "dashboard"] ?? pageMetaMap.dashboard);
const licenseLabel = computed(() => (appStore.isLicensed ? "已授权" : "未激活"));
const { scalePercent, increment, decrement, reset } = useUiScale();

const cookieChip = computed(() => {
  switch (cookieHealth.status) {
    case "healthy":
      return { label: "Cookie 正常", tone: "success", hint: "Cookie 已主动探测可用" };
    case "unhealthy":
      return { label: "Cookie 失效", tone: "error", hint: cookieHealth.snapshot.hint ?? "请重新登录" };
    case "unconfigured":
      return { label: "未配置 Cookie", tone: "warn", hint: "请前往设置中心完成登录" };
    default:
      return { label: "Cookie 待探测", tone: "idle", hint: "尚未完成首次探测" };
  }
});

function handleCookieChipClick() {
  if (cookieHealth.status === "healthy" || cookieHealth.status === "unknown") {
    void cookieHealth.probe();
    return;
  }
  void router.push(buildSettingsLocation("cookie"));
}

function handleLicenseChipClick() {
  void router.push(buildSettingsLocation("license"));
}
</script>

<template>
  <header class="surface-panel relative overflow-hidden px-3 py-3 sm:px-4 sm:py-4 lg:px-5">
    <div class="pointer-events-none absolute inset-y-0 right-0 w-[220px] bg-[radial-gradient(circle_at_top_right,rgba(167,243,208,0.24),transparent_70%)]"></div>
    <div class="relative flex flex-row flex-nowrap items-center justify-between gap-app">
      <div class="min-w-0 flex-1 pr-2">
        <h1 class="truncate text-xl font-bold tracking-tight text-slate-950 sm:text-2xl lg:text-[1.7rem]">{{ pageMeta.title }}</h1>
      </div>

      <div class="flex min-w-0 shrink-0 flex-nowrap items-center gap-app overflow-x-auto">
        <div class="flex items-center gap-0.5 rounded-full border border-brand-tint/80 bg-white/90 px-1 py-1 shadow-sm sm:gap-1 sm:px-1.5">
          <button type="button" class="flex h-7 w-7 items-center justify-center rounded-full text-sm font-semibold text-brand transition hover:bg-brand-soft" title="缩小界面（Ctrl/Cmd -）" @click="decrement">－</button>
          <button type="button" class="rounded-full px-2.5 py-1 text-[11px] font-semibold tracking-[0.12em] text-brand-deep transition hover:bg-brand-soft" title="恢复默认缩放（Ctrl/Cmd 0）" @click="reset">{{ scalePercent }}</button>
          <button type="button" class="flex h-7 w-7 items-center justify-center rounded-full text-sm font-semibold text-brand transition hover:bg-brand-soft" title="放大界面（Ctrl/Cmd +）" @click="increment">＋</button>
        </div>
        <button
          type="button"
          class="status-chip shrink-0 cursor-pointer transition hover:border-brand-tint hover:bg-white"
          :title="cookieChip.hint"
          @click="handleCookieChipClick"
        >
          <span
            class="status-dot"
            :class="{
              success: cookieChip.tone === 'success',
              warn: cookieChip.tone === 'warn',
              error: cookieChip.tone === 'error',
            }"
          ></span>
          <div class="text-[13px] font-semibold text-slate-700">{{ cookieChip.label }}</div>
        </button>
        <button type="button" class="status-chip shrink-0 cursor-pointer transition hover:border-brand-tint hover:bg-white" @click="handleLicenseChipClick">
          <span class="status-dot" :class="appStore.isLicensed ? 'success' : 'warn'"></span>
          <div class="text-[13px] font-semibold text-slate-700">{{ licenseLabel }}</div>
        </button>
      </div>
    </div>
  </header>
</template>
