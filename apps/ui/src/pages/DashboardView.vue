<script setup lang="ts">
import { computed, onMounted } from "vue";
import { RouterLink, type RouteLocationRaw } from "vue-router";

import { useOrderStore } from "@/stores/order";
import { useOrder } from "@/services/order";
import { useCookieHealthStore } from "@/stores/cookieHealth";
import { useUpdateCheckStore } from "@/stores/updateCheck";
import { useAppStore } from "@/stores/app";
import AppNavIcon from "../layout/AppNavIcon.vue";
import { buildSettingsLocation } from "../layout/navigation";
import { AUTHOR_WECHAT } from "../shared/brand";
import { useRuntimeClock } from "../shared/useRuntimeClock";
import { useNotification } from "../shared/useNotification";

const appStore = useAppStore();
const orderStore = useOrderStore();
const cookieHealth = useCookieHealthStore();
const updateCheck = useUpdateCheckStore();
const { loadCacheCounts, loadCacheStatus } = useOrder();
const { show: showToast } = useNotification();

type Tone = "success" | "warn" | "error" | "idle";
type ShortcutTone = "brand" | "sky" | "amber" | "slate";

interface MetricTile {
  key: string;
  label: string;
  value: string;
  hint: string;
  tone: Tone;
}

type CacheMetricKey = "today" | "yesterday" | "last_7_days" | "last_30_days";

function buildCacheMetric(key: string, label: string, count: number | null, hint: string): MetricTile {
  return {
    key,
    label,
    value: count === null ? "--" : String(count),
    hint,
    tone: count === null ? "idle" : count > 0 ? "success" : "warn",
  };
}

const cacheCounts = computed(() => ({
  today: orderStore.cacheCounts?.today_count ?? orderStore.cacheStatus?.today_count ?? null,
  yesterday: orderStore.cacheCounts?.yesterday_count ?? orderStore.cacheStatus?.yesterday_count ?? null,
  last7: orderStore.cacheCounts?.last_7_days_count ?? orderStore.cacheStatus?.last_7_days_count ?? null,
  last30:
    orderStore.cacheCounts?.last_30_days_count ??
    orderStore.cacheStatus?.last_30_days_count ??
    orderStore.cacheStatus?.cached_order_count ??
    null,
  todayLatestOrderAt:
    orderStore.cacheCounts?.today_latest_order_at ??
    orderStore.cacheStatus?.today_latest_order_at ??
    null,
}));

function startOfLocalDay(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate(), 0, 0, 0, 0);
}

function endOfLocalDay(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate(), 23, 59, 59, 999);
}

function addLocalDays(date: Date, days: number): Date {
  const next = new Date(date);
  next.setDate(next.getDate() + days);
  return next;
}

function isSameLocalDay(a: Date, b: Date): boolean {
  return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
}

function formatClock(date: Date): string {
  const hour = String(date.getHours()).padStart(2, "0");
  const minute = String(date.getMinutes()).padStart(2, "0");
  const second = String(date.getSeconds()).padStart(2, "0");
  return `${hour}:${minute}:${second}`;
}

function formatMonthDayTime(date: Date): string {
  return `${date.getMonth() + 1}.${date.getDate()} ${formatClock(date)}`;
}

function formatCacheRange(start: Date, end: Date): string {
  if (isSameLocalDay(start, end)) {
    return `${formatMonthDayTime(start)}-${formatClock(end)}`;
  }
  return `${formatMonthDayTime(start)}-${formatMonthDayTime(end)}`;
}

function parseValidDate(value: string | null): Date | null {
  if (!value) return null;
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? null : date;
}

function buildCacheRangeHints(
  now: Date,
  todayCount: number | null,
  todayLatestOrderAt: string | null,
): Record<CacheMetricKey, string> {
  const todayStart = startOfLocalDay(now);
  const todayEnd = endOfLocalDay(now);
  const yesterday = addLocalDays(now, -1);
  const yesterdayStart = startOfLocalDay(yesterday);
  const yesterdayEnd = endOfLocalDay(yesterday);
  const last7Start = startOfLocalDay(addLocalDays(now, -7));
  const last30Start = startOfLocalDay(addLocalDays(now, -30));
  const latestTodayOrder = parseValidDate(todayLatestOrderAt);
  const todayRangeEnd =
    latestTodayOrder && isSameLocalDay(latestTodayOrder, todayStart) && latestTodayOrder <= todayEnd
      ? latestTodayOrder
      : null;

  return {
    today:
      todayRangeEnd === null
        ? `${formatMonthDayTime(todayStart)}-${todayCount === null ? "待加载" : "暂无今日缓存"}`
        : formatCacheRange(todayStart, todayRangeEnd),
    yesterday: formatCacheRange(yesterdayStart, yesterdayEnd),
    last_7_days: formatCacheRange(last7Start, yesterdayEnd),
    last_30_days: formatCacheRange(last30Start, yesterdayEnd),
  };
}

const cacheRangeHints = computed(() =>
  buildCacheRangeHints(new Date(), cacheCounts.value.today, cacheCounts.value.todayLatestOrderAt),
);

const metrics = computed<MetricTile[]>(() => [
  buildCacheMetric(
    "today",
    "今天缓存",
    cacheCounts.value.today,
    cacheRangeHints.value.today,
  ),
  buildCacheMetric(
    "yesterday",
    "昨天缓存",
    cacheCounts.value.yesterday,
    cacheRangeHints.value.yesterday,
  ),
  buildCacheMetric(
    "last_7_days",
    "近 7 天缓存",
    cacheCounts.value.last7,
    cacheRangeHints.value.last_7_days,
  ),
  buildCacheMetric(
    "last_30_days",
    "近 30 天缓存",
    cacheCounts.value.last30,
    cacheRangeHints.value.last_30_days,
  ),
]);

const quickLinks: readonly {
  to: RouteLocationRaw;
  title: string;
  icon: "review" | "order" | "delivery" | "settings";
  description: string;
  tone: ShortcutTone;
}[] = [
  { to: "/review", title: "中差评/品退", icon: "review", description: "一键匹配订单并带入发货", tone: "brand" },
  { to: "/order", title: "订单缓存同步", icon: "order", description: "维护近 30 天（不含今天）缓存", tone: "sky" },
  { to: "/delivery", title: "批量发货", icon: "delivery", description: "逐条进度·失败明细·支持取消", tone: "amber" },
  { to: buildSettingsLocation("license"), title: "设置中心", icon: "settings", description: "授权、Cookie 与版本信息", tone: "slate" },
] as const;

const toneBadgeClass: Record<Tone, string> = {
  success: "bg-brand-soft text-brand-deep",
  warn: "bg-amber-50 text-amber-700",
  error: "bg-red-50 text-red-700",
  idle: "bg-slate-100 text-slate-500",
};

const toneBadgeLabel: Record<Tone, string> = {
  success: "有缓存",
  warn: "空",
  error: "异常",
  idle: "待加载",
};

const metricAccentClass: Record<Tone, string> = {
  success: "dashboard-metric-card--success",
  warn: "dashboard-metric-card--warn",
  error: "dashboard-metric-card--error",
  idle: "dashboard-metric-card--idle",
};

const shortcutClass: Record<ShortcutTone, string> = {
  brand: "dashboard-shortcut--brand",
  sky: "dashboard-shortcut--sky",
  amber: "dashboard-shortcut--amber",
  slate: "dashboard-shortcut--slate",
};

const greeting = computed(() => {
  const hour = new Date().getHours();
  if (hour < 6) return "深夜好，辛苦啦！";
  if (hour < 11) return "早上好，新的一天开工";
  if (hour < 14) return "中午好，别忘了吃饭";
  if (hour < 18) return "下午好，继续加油";
  return "晚上好，收尾时间啦";
});

const todayText = computed(() =>
  new Date().toLocaleDateString("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    weekday: "long",
  }),
);

const heroSummaryText = computed(() => {
  if (cacheCounts.value.last30 === null) return "正在读取订单缓存统计";
  return `近 30 天（不含今天）缓存 ${cacheCounts.value.last30} 条，可直接进入业务流程`;
});

const { clockText } = useRuntimeClock();

const hasTutorialUrl = computed(() => Boolean(updateCheck.latestInfo?.tutorial_url?.trim()));

async function handleOpenDownloadUrl() {
  await updateCheck.openDownloadUrl();
  if (updateCheck.downloadActionError) {
    showToast(updateCheck.downloadActionError, "error");
    return;
  }
  showToast("下载页已打开", "success");
}

async function handleOpenTutorialUrl() {
  await updateCheck.openTutorialUrl();
  if (updateCheck.tutorialActionError) {
    showToast(updateCheck.tutorialActionError, "error");
    return;
  }
  showToast("教程已打开", "success");
}

onMounted(async () => {
  void loadCacheStatus();
  void loadCacheCounts();
  void cookieHealth.refreshSilently();
});
</script>

<template>
  <div class="dashboard-view flex min-h-0 min-w-0 flex-1 flex-col overflow-y-auto pr-0.5">
    <section class="hero-panel subsystem-hero dashboard-hero relative shrink-0 overflow-hidden p-3 lg:p-4">
      <div class="pointer-events-none absolute -right-16 -top-20 h-56 w-56 rounded-full bg-[radial-gradient(circle,rgba(167,243,208,0.55),transparent_70%)]"></div>
      <div class="pointer-events-none absolute -left-10 bottom-0 h-40 w-40 rounded-full bg-[radial-gradient(circle,rgba(240,253,244,0.6),transparent_70%)]"></div>

      <div class="relative flex min-w-0 flex-col gap-1.5">
        <span class="card-eyebrow">TLS · OPERATIONS OVERVIEW</span>
        <h2 class="text-xl font-bold tracking-tight text-slate-900 sm:text-[1.55rem] lg:text-[1.7rem]">
          {{ greeting }}
        </h2>
        <div class="flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-slate-500 sm:text-[13px]">
          <span>{{ todayText }}</span>
          <span aria-hidden="true" class="h-1 w-1 rounded-full bg-slate-300"></span>
          <span>{{ heroSummaryText }}</span>
        </div>
      </div>
    </section>

    <div class="dashboard-content-grid flex min-h-0 flex-1 flex-col gap-app">
      <div class="dashboard-primary-stack flex min-h-0 shrink-0 flex-col gap-app">
        <section class="surface-panel dashboard-cache-panel shrink-0 p-3 lg:p-4" aria-labelledby="dashboard-cache-title">
          <div class="dashboard-panel-heading mb-3 flex flex-wrap items-start justify-between gap-3">
            <div>
              <span class="card-eyebrow">ORDER CACHE</span>
              <h3 id="dashboard-cache-title" class="mt-1 text-base font-bold tracking-tight text-slate-950">订单缓存概览</h3>
            </div>
            <span class="dashboard-panel-note rounded-full border border-emerald-100 bg-emerald-50 px-2.5 py-1 text-[11px] font-semibold text-emerald-700">
              按自然日统计
            </span>
          </div>

          <div
            data-testid="dashboard-metrics"
            class="dashboard-metrics grid shrink-0 grid-cols-1 gap-3 min-[420px]:grid-cols-2 min-[420px]:gap-app sm:grid-cols-2 lg:grid-cols-4 lg:gap-app xl:grid-cols-4 items-stretch"
          >
            <article
              v-for="metric in metrics"
              :key="metric.key"
              data-testid="dashboard-metric-tile"
              class="surface-panel metric-card dashboard-metric-tile dashboard-metric-card relative flex min-h-[5.25rem] flex-col gap-1 overflow-hidden p-3 transition-all sm:min-h-[5.5rem] sm:gap-1.5 lg:min-h-[5.75rem] lg:gap-1.5 lg:p-4"
              :class="[metricAccentClass[metric.tone], { 'dashboard-metric-card--today': metric.key === 'today' }]"
            >
              <div class="flex items-start justify-between gap-2">
                <div class="flex min-w-0 items-center gap-2">
                  <span class="status-dot shrink-0" :class="metric.tone !== 'idle' ? metric.tone : ''"></span>
                  <div class="metric-label dashboard-metric-label truncate">{{ metric.label }}</div>
                </div>
                <span
                  class="dashboard-metric-badge shrink-0 whitespace-nowrap rounded-full px-2 py-0.5 font-semibold leading-none"
                  :class="toneBadgeClass[metric.tone]"
                >
                  {{ toneBadgeLabel[metric.tone] }}
                </span>
              </div>
              <div class="metric-value dashboard-metric-value">{{ metric.value }}</div>
              <div class="metric-hint dashboard-metric-hint line-clamp-2">{{ metric.hint }}</div>
            </article>
          </div>
        </section>

        <section class="surface-panel dashboard-shortcuts-panel flex min-h-0 shrink-0 flex-col p-3 lg:p-4">
          <div class="subsystem-section-header mb-3 flex items-center gap-2">
            <h3 class="text-sm font-semibold tracking-tight text-slate-900">快捷入口</h3>
            <span class="text-[11px] text-slate-400">一键直达核心业务</span>
          </div>

          <div
            data-testid="dashboard-shortcuts"
            class="dashboard-shortcuts-grid subsystem-summary-strip grid grid-cols-4 gap-x-4 gap-y-3 sm:gap-x-5 sm:gap-y-4 lg:gap-y-5"
          >
            <RouterLink
              v-for="item in quickLinks"
              :key="item.title"
              :to="item.to"
              class="quick-link quick-link-compact surface-panel-strong dashboard-shortcut group relative flex h-full items-center gap-3 overflow-hidden sm:gap-3"
              :class="shortcutClass[item.tone]"
            >
              <div class="dashboard-shortcut-icon" aria-hidden="true">
                <AppNavIcon :name="item.icon" icon-class="h-[18px] w-[18px]" />
              </div>
              <div class="min-w-0 flex-1">
                <h4 class="text-[13px] font-semibold text-slate-900">{{ item.title }}</h4>
                <p class="mt-0.5 truncate text-[11px] leading-4 text-slate-500">{{ item.description }}</p>
              </div>
              <div class="dashboard-shortcut-arrow" aria-hidden="true">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" class="h-3.5 w-3.5 transition-transform group-hover:translate-x-0.5">
                  <path d="M5 12h14" />
                  <path d="m12 5 7 7-7 7" />
                </svg>
              </div>
            </RouterLink>
          </div>
        </section>
      </div>

      <aside
        data-testid="dashboard-meta-cards"
        class="surface-panel dashboard-side-rail dashboard-meta-cards flex min-h-0 flex-1 flex-col p-3 lg:p-4"
        aria-label="运行时元数据"
      >
        <div class="dashboard-side-rail-head mb-3">
          <span class="card-eyebrow">RUNTIME</span>
          <h3 class="mt-1 text-sm font-bold tracking-tight text-slate-950">运行信息</h3>
          <p class="mt-1 text-[11px] leading-4 text-slate-500">版本、教程与本机时间</p>
        </div>

        <div class="dashboard-meta-list grid gap-3">
          <article class="dashboard-meta-card dashboard-meta-card--version flex items-center gap-3 p-3">
            <span class="dashboard-meta-icon" aria-hidden="true">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="h-4 w-4">
                <path d="M12 2 4 7v10l8 5 8-5V7z" />
                <path d="M12 22V12" />
                <path d="m4 7 8 5 8-5" />
              </svg>
            </span>
            <div class="min-w-0 flex-1">
              <div class="dashboard-meta-label">版本</div>
              <button
                v-if="updateCheck.hasUpdateAvailable"
                type="button"
                data-testid="dashboard-update-hint"
                class="dashboard-meta-value dashboard-meta-update w-full cursor-pointer truncate text-left hover:underline"
                :title="updateCheck.downloadActionError || `当前 v${appStore.appVersion} · 打开下载页`"
                @click="handleOpenDownloadUrl"
              >
                有新版本 v{{ updateCheck.latestInfo?.version }}
              </button>
              <div v-else class="dashboard-meta-value">v{{ appStore.appVersion }}</div>
            </div>
          </article>

          <article class="dashboard-meta-card dashboard-meta-card--author flex items-center gap-3 p-3">
            <span class="dashboard-meta-icon" aria-hidden="true">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="h-4 w-4">
                <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
                <circle cx="12" cy="7" r="4" />
              </svg>
            </span>
            <div class="min-w-0 flex-1">
              <div class="dashboard-meta-label">作者微信</div>
              <div class="dashboard-meta-value dashboard-meta-value--mono">{{ AUTHOR_WECHAT }}</div>
            </div>
          </article>

          <article
            class="dashboard-meta-card dashboard-meta-card--tutorial flex items-center gap-3 p-3"
            data-testid="dashboard-meta-tutorial"
          >
            <span class="dashboard-meta-icon" aria-hidden="true">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="h-4 w-4">
                <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20" />
                <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z" />
                <path d="M8 7h8" />
                <path d="M8 11h6" />
              </svg>
            </span>
            <div class="min-w-0 flex-1">
              <div class="dashboard-meta-label">查看教程</div>
              <button
                v-if="hasTutorialUrl"
                type="button"
                class="dashboard-meta-value w-full cursor-pointer truncate text-left hover:underline"
                :title="updateCheck.tutorialActionError || '在浏览器中打开'"
                @click="handleOpenTutorialUrl"
              >
                点击打开
              </button>
              <div v-else class="dashboard-meta-value text-slate-400">暂无链接</div>
              <p v-if="updateCheck.tutorialActionError" class="mt-0.5 truncate text-[10px] text-red-600">
                {{ updateCheck.tutorialActionError }}
              </p>
            </div>
          </article>

          <article class="dashboard-meta-card dashboard-meta-card--clock flex items-center gap-3 p-3">
            <span class="dashboard-meta-icon" aria-hidden="true">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="h-4 w-4">
                <circle cx="12" cy="12" r="9" />
                <path d="M12 7v5l3 2" />
              </svg>
            </span>
            <div class="min-w-0 flex-1">
              <div class="dashboard-meta-label">当前时间</div>
              <div class="dashboard-meta-value dashboard-meta-value--mono">{{ clockText }}</div>
            </div>
          </article>
        </div>
      </aside>
    </div>

  </div>
</template>
