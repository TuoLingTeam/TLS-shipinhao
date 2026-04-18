<script setup lang="ts">
import { useRoute, useRouter } from "vue-router";
import { computed } from "vue";
import { useAppStore } from "../app.store";
import { APP_NAME } from "../shared/brand";
import { useUiScale } from "../layout/useUiScale";
import { useCookieHealthStore } from "../shared/cookieHealth";

const route = useRoute();
const router = useRouter();
const appStore = useAppStore();
const cookieHealth = useCookieHealthStore();

const titleMap: Record<string, string> = {
  dashboard: "仪表盘",
  review: "评价管理",
  order: "订单管理",
  delivery: "发货管理",
  license: "授权管理",
  settings: "设置",
};

const pageTitle = computed(() => titleMap[route.name as string] ?? APP_NAME);
const licenseLabel = computed(() => (appStore.isLicensed ? "已授权" : "未激活"));
const { scalePercent, increment, decrement, reset } = useUiScale();

const cookieChip = computed(() => {
  switch (cookieHealth.status) {
    case "healthy":
      return { label: "Cookie 正常", tone: "success", hint: "Cookie 已主动探测可用" };
    case "unhealthy":
      return { label: "Cookie 失效", tone: "error", hint: cookieHealth.snapshot.hint ?? "请重新登录" };
    case "unconfigured":
      return { label: "未配置 Cookie", tone: "warn", hint: "请前往设置完成登录" };
    default:
      return { label: "Cookie 待探测", tone: "idle", hint: "尚未完成首次探测" };
  }
});

function handleCookieChipClick() {
  if (cookieHealth.status === "healthy" || cookieHealth.status === "unknown") {
    void cookieHealth.probe();
    return;
  }
  void router.push("/settings");
}
</script>

<template>
  <header class="surface-panel px-5 py-4 lg:px-6 lg:py-4">
    <div class="flex items-center justify-between gap-4">
      <div class="min-w-0">
        <p class="text-xs font-semibold uppercase tracking-[0.22em] text-slate-400">TLS · VIDEO COMMERCE DESK</p>
        <div class="mt-2 flex items-center gap-3">
          <h1 class="truncate text-2xl font-bold tracking-tight text-slate-900">{{ pageTitle }}</h1>
          <span class="hidden rounded-full bg-amber-100 px-2.5 py-1 text-[11px] font-semibold text-amber-700 sm:inline-flex">
            v{{ appStore.appVersion }}
          </span>
        </div>
      </div>

      <div class="flex items-center gap-3">
        <div class="hidden items-center gap-1 rounded-full border border-brand-tint/80 bg-white/80 px-2 py-1 shadow-sm lg:flex">
          <button type="button" class="flex h-8 w-8 items-center justify-center rounded-full text-sm font-semibold text-brand transition hover:bg-brand-soft" title="缩小界面（Ctrl/Cmd -）" @click="decrement">－</button>
          <button type="button" class="rounded-full px-2 py-1 text-[11px] font-semibold tracking-[0.12em] text-brand-deep transition hover:bg-brand-soft" title="恢复默认缩放（Ctrl/Cmd 0）" @click="reset">{{ scalePercent }}</button>
          <button type="button" class="flex h-8 w-8 items-center justify-center rounded-full text-sm font-semibold text-brand transition hover:bg-brand-soft" title="放大界面（Ctrl/Cmd +）" @click="increment">＋</button>
        </div>
        <button
          type="button"
          class="status-chip shrink-0 cursor-pointer transition hover:bg-slate-50"
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
          <div class="text-sm font-semibold text-slate-700">{{ cookieChip.label }}</div>
        </button>
        <div class="status-chip shrink-0">
          <span class="status-dot" :class="appStore.isLicensed ? 'success' : ''"></span>
          <div class="text-sm font-semibold text-slate-700">{{ licenseLabel }}</div>
        </div>
      </div>
    </div>
  </header>
</template>
