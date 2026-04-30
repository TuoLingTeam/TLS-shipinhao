<script setup lang="ts">
import { useRoute, useRouter } from "vue-router";
import { computed } from "vue";
import { storeToRefs } from "pinia";
import { useAppStore } from "@/stores/app";
import { useUiScale } from "../layout/useUiScale";
import { useCookieHealthStore } from "@/stores/cookieHealth";
import { useStoreContextStore } from "@/stores/storeContext";
import { toErrorMessage } from "../shared/toErrorMessage";
import { useNotification } from "../shared/useNotification";
import { buildSettingsLocation, pageMetaMap, type PageName } from "./navigation";

const route = useRoute();
const router = useRouter();
const appStore = useAppStore();
const cookieHealth = useCookieHealthStore();
const { status: cookieStatus, snapshot: cookieSnapshot } = storeToRefs(cookieHealth);
const storeContext = useStoreContextStore();
const { show: showToast } = useNotification();

const pageMeta = computed(() => pageMetaMap[(route.name as PageName) || "dashboard"] ?? pageMetaMap.dashboard);
const licenseLabel = computed(() => (appStore.isLicensed ? "已授权" : "未激活"));
const { scalePercent, increment, decrement, reset } = useUiScale();

const cookieDotModifier = computed(() => {
  switch (cookieStatus.value) {
    case "healthy":
      return "success";
    case "unhealthy":
      return "error";
    case "unconfigured":
      return "warn";
    default:
      return "";
  }
});

const cookieChip = computed(() => {
  switch (cookieStatus.value) {
    case "healthy":
      return { label: "Cookie 正常", tone: "success", hint: "Cookie 已主动探测可用" };
    case "unhealthy":
      return { label: "Cookie 失效", tone: "error", hint: cookieSnapshot.value.hint ?? "请重新登录" };
    case "unconfigured":
      return { label: "未配置 Cookie", tone: "warn", hint: "请前往设置中心完成登录" };
    default:
      return { label: "Cookie 待探测", tone: "idle", hint: "尚未完成首次探测" };
  }
});

async function handleCookieChipClick() {
  if (cookieStatus.value === "healthy" || cookieStatus.value === "unknown") {
    await cookieHealth.probe();
    if (cookieHealth.error) {
      showToast(cookieHealth.error, "error");
      return;
    }
    const nextCookieStatus = cookieStatus.value as "unknown" | "healthy" | "unhealthy" | "unconfigured";
    if (nextCookieStatus === "healthy") {
      showToast("Cookie 探测正常", "success");
    } else if (nextCookieStatus === "unhealthy") {
      showToast(cookieChip.value.hint, "error");
    } else {
      showToast("Cookie 探测已完成", "info");
    }
    return;
  }
  void router.push(buildSettingsLocation("cookie"));
}

function handleLicenseChipClick() {
  void router.push(buildSettingsLocation("license"));
}

function handleStoreFallbackClick() {
  void router.push(buildSettingsLocation("cookie"));
}

async function handleStoreSelect(event: Event) {
  const target = event.target as HTMLSelectElement | null;
  const nextStoreId = target?.value?.trim() ?? "";
  if (!target || !nextStoreId) return;
  try {
    const result = await storeContext.selectStore(nextStoreId);
    if (!result) {
      target.value = storeContext.activeStoreId;
      showToast("当前店铺未切换", "info");
      return;
    }
    showToast(`已切换到 ${result.store.store_name}`, "success");
  } catch (error) {
    target.value = storeContext.activeStoreId;
    showToast(toErrorMessage(error), "error");
  }
}
</script>

<template>
  <header class="surface-panel relative overflow-hidden px-3 py-3 sm:px-4 lg:px-5 lg:py-4">
    <div class="pointer-events-none absolute inset-y-0 right-0 w-[220px] bg-[radial-gradient(circle_at_top_right,rgba(167,243,208,0.24),transparent_70%)]"></div>
    <div class="relative flex flex-row flex-nowrap items-center justify-between gap-app">
      <div class="min-w-0 flex-1 pr-2">
        <h1 class="truncate text-xl font-bold tracking-tight text-slate-950 sm:text-2xl lg:text-[1.7rem]">{{ pageMeta.title }}</h1>
      </div>

      <div class="flex min-w-0 shrink-0 flex-nowrap items-center gap-app overflow-x-auto">
        <label
          v-if="storeContext.hasStores"
          class="field-affix field-affix--leading min-w-[220px] shrink-0 sm:min-w-[240px]"
          :title="`${storeContext.activeStoreName} · ${storeContext.activeStoreId || '未识别店铺 ID'}`"
        >
          <svg class="field-affix-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M4 7.5h16" />
            <path d="M5 7.5 6.6 4.8A1.5 1.5 0 0 1 7.9 4h8.2a1.5 1.5 0 0 1 1.29.8L19 7.5" />
            <rect x="4" y="7.5" width="16" height="12.5" rx="2.5" />
            <path d="M8 12h8" />
            <path d="M8 15.5h5" />
          </svg>
          <select
            aria-label="选择当前店铺"
            class="field-input field-input--with-leading-icon min-h-[38px] w-full min-w-0 cursor-pointer bg-transparent text-sm font-semibold text-slate-800"
            :value="storeContext.activeStoreId"
            :disabled="storeContext.busy"
            @change="handleStoreSelect"
          >
            <option
              v-for="store in storeContext.stores"
              :key="store.store_id"
              :value="store.store_id"
            >
              {{ store.store_name }}
            </option>
          </select>
        </label>
        <button
          v-else
          type="button"
          class="status-chip shrink-0 cursor-pointer transition hover:border-brand-tint hover:bg-white"
          title="尚未识别任何店铺，前往设置中心保存 Cookie 后即可建立店铺列表"
          @click="handleStoreFallbackClick"
        >
          <span class="status-dot warn"></span>
          <div class="text-[13px] font-semibold text-slate-700">未识别店铺</div>
        </button>
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
          <span class="status-dot" :class="cookieDotModifier"></span>
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
