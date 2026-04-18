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
  <header class="surface-panel relative overflow-hidden px-4 py-4 lg:px-5 lg:py-4">
    <div class="pointer-events-none absolute inset-y-0 right-0 w-[220px] bg-[radial-gradient(circle_at_top_right,rgba(167,243,208,0.24),transparent_70%)]"></div>
    <div class="relative flex flex-col gap-4 xl:flex-row xl:items-center xl:justify-between">
      <div class="min-w-0">
        <p class="text-[11px] font-semibold uppercase tracking-[0.24em] text-slate-400">{{ pageMeta.eyebrow }}</p>
        <div class="mt-2 flex flex-wrap items-center gap-2.5">
          <h1 class="truncate text-[1.7rem] font-bold tracking-tight text-slate-950">{{ pageMeta.title }}</h1>
          <span class="inline-flex rounded-full bg-amber-100 px-2.5 py-0.5 text-[11px] font-semibold text-amber-700 shadow-sm">
            v{{ appStore.appVersion }}
          </span>
        </div>
        <p class="mt-1 max-w-2xl text-[13px] leading-5 text-slate-500">{{ pageMeta.description }}</p>
      </div>

      <div class="flex flex-wrap items-center gap-2 xl:justify-end">
        <div class="hidden items-center gap-1 rounded-full border border-brand-tint/80 bg-white/90 px-1.5 py-1 shadow-sm lg:flex">
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
