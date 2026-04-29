<script setup lang="ts">
import { computed, onMounted } from "vue";
import { RouterLink, type RouteLocationRaw } from "vue-router";

// 仪表盘冗余横幅已移除（2026-04）：5 个状态卡片自身的 tone + hint 已能完整
// 表达「续费提醒 / Cookie 失效 / 缓存缺口 / 批量发货失败」等异常。
// hero 横幅的「有 N 项需要处理」改为基于 metrics tone 数计算。
import { useAppStore } from "../app.store";
import { useOrderStore } from "../order/store";
import { useReviewStore } from "../review/store";
import { useDeliveryStore } from "../delivery/store";
import { useOrder } from "../order/useOrder";
import { useCookieHealthStore } from "../shared/cookieHealth";
import { useUpdateCheckStore } from "../shared/updateCheck";
import AppNavIcon from "../layout/AppNavIcon.vue";
import { formatDateTime } from "../shared/format";
import { LICENSE_STATE_LABELS } from "../license/types";
import { buildSettingsLocation } from "../layout/navigation";
import { AUTHOR_WECHAT } from "../shared/brand";
import { useRuntimeClock } from "../shared/useRuntimeClock";

const appStore = useAppStore();
const orderStore = useOrderStore();
const reviewStore = useReviewStore();
const deliveryStore = useDeliveryStore();
const cookieHealth = useCookieHealthStore();
const updateCheck = useUpdateCheckStore();
const { loadCacheStatus } = useOrder();

type Tone = "success" | "warn" | "error" | "idle";
type ShortcutTone = "brand" | "sky" | "amber" | "slate";

const daysUntilLicenseExpires = computed<number | null>(() => {
  const iso = appStore.licenseExpiresAt;
  if (!iso) return null;
  const expire = Date.parse(iso);
  if (!Number.isFinite(expire)) return null;
  const diffMs = expire - Date.now();
  return Math.ceil(diffMs / (24 * 60 * 60 * 1000));
});

const licenseTone = computed<Tone>(() => {
  if (!appStore.isLicensed) return "error";
  if ((daysUntilLicenseExpires.value ?? Infinity) <= 7) return "warn";
  return "success";
});

const licenseText = computed(() => LICENSE_STATE_LABELS[appStore.licenseState] ?? "未知状态");

const cacheCount = computed(() => orderStore.cacheStatus?.cached_order_count ?? orderStore.cachedOrders.length);
const missingSegments = computed(() => orderStore.cacheStatus?.missing_segment_count ?? 0);
const lastSyncAt = computed(() => orderStore.cacheStatus?.last_sync_at ?? orderStore.lastSyncAt);

const matchedReviewCount = computed(() => reviewStore.results.filter((item) => item.matched).length);
const unmatchedReviewCount = computed(() => reviewStore.results.length - matchedReviewCount.value);

interface MetricTile {
  key: string;
  label: string;
  value: string;
  hint: string;
  tone: Tone;
}

const licenseMetric = computed<MetricTile>(() => {
  const days = daysUntilLicenseExpires.value;
  const hint = (() => {
    if (!appStore.isLicensed) {
      return days !== null && days < 0
        ? `已过期 ${Math.abs(days)} 天，请前往设置中心续费`
        : "请前往设置中心激活";
    }
    if (days !== null && days < 0) {
      return `授权已过期 ${Math.abs(days)} 天，请尽快续费`;
    }
    if (days !== null && days <= 7) {
      return `授权将在 ${days} 天后到期，建议提前续费`;
    }
    return "已激活";
  })();
  return { key: "license", label: "授权状态", value: licenseText.value, hint, tone: licenseTone.value };
});

const cookieMetric = computed<MetricTile>(() => {
  const status = cookieHealth.status;
  const lastCheckedAt = cookieHealth.snapshot.last_checked_at;
  const hint = (() => {
    if (status === "unhealthy") {
      return cookieHealth.snapshot.hint || "Cookie 已失效，请重新登录小店";
    }
    if (status === "unconfigured") {
      return "尚未配置，前往设置中心完成登录";
    }
    return lastCheckedAt ? `最近探测：${formatDateTime(lastCheckedAt)}` : "启动后自动探测";
  })();
  return {
    key: "cookie",
    label: "Cookie 状态",
    value:
      status === "healthy"
        ? "可用"
        : status === "unhealthy"
          ? "已失效"
          : status === "unconfigured"
            ? "未配置"
            : "待探测",
    hint,
    tone:
      status === "healthy"
        ? "success"
        : status === "unhealthy"
          ? "error"
          : status === "unconfigured"
            ? "warn"
            : "idle",
  };
});

const cacheMetric = computed<MetricTile>(() => ({
  key: "cache",
  label: "最近 30 天缓存",
  value: cacheCount.value > 0 ? String(cacheCount.value) : "--",
  hint:
    cacheCount.value === 0
      ? "点击下方缓存同步建立本地订单副本"
      : missingSegments.value > 0
        ? `存在 ${missingSegments.value} 个缺口，建议同步补齐`
        : lastSyncAt.value
          ? `最近同步：${formatDateTime(lastSyncAt.value)}`
          : "已建立，建议定期刷新",
  tone:
    cacheCount.value === 0 ? "warn" : missingSegments.value > 0 ? "warn" : "success",
}));

const reviewMetric = computed<MetricTile>(() => {
  const total = reviewStore.results.length;
  return {
    key: "review",
    label: "评价匹配",
    value: total > 0 ? `${matchedReviewCount.value}/${total}` : "--",
    hint:
      total === 0
        ? "未执行匹配"
        : unmatchedReviewCount.value > 0
          ? `${unmatchedReviewCount.value} 条待人工核实`
          : "全部匹配成功",
    tone:
      total === 0 ? "idle" : unmatchedReviewCount.value > 0 ? "warn" : "success",
  };
});

const deliveryMetric = computed<MetricTile>(() => {
  const progress = deliveryStore.batchProgress;
  const hint = (() => {
    if (!progress) return "本次启动尚未执行批量";
    if (!progress.running && progress.failureCount > 0) {
      return `本次失败 ${progress.failureCount} 条，前往批量发货查看明细`;
    }
    return `成功 ${progress.successCount} · 失败 ${progress.failureCount}`;
  })();
  return {
    key: "delivery",
    label: "发货任务",
    value: progress ? String(progress.totalCount) : "--",
    hint,
    tone: progress ? (progress.failureCount > 0 ? "warn" : "success") : "idle",
  };
});

const metrics = computed<MetricTile[]>(() => [
  licenseMetric.value,
  cookieMetric.value,
  cacheMetric.value,
  reviewMetric.value,
  deliveryMetric.value,
]);

const quickLinks: readonly {
  to: RouteLocationRaw;
  title: string;
  icon: "review" | "order" | "delivery" | "settings";
  description: string;
  tone: ShortcutTone;
}[] = [
  { to: "/review", title: "中差评/品退", icon: "review", description: "一键匹配订单并带入发货", tone: "brand" },
  { to: "/order", title: "订单缓存同步", icon: "order", description: "维护 30 天订单缓存与本地检索", tone: "sky" },
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
  success: "正常",
  warn: "注意",
  error: "需处理",
  idle: "待更新",
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
  // 计数依据：5 个状态卡片中 tone 为 warn / error 的数量。idle（待更新）不计入，
  // 那是首次启动还没数据的中性状态，并非"待处理"。
  const pendingCount = metrics.value.filter((m) => m.tone === "warn" || m.tone === "error").length;
  if (pendingCount === 0) return "运营状态整体健康，可直接进入业务流程";
  if (pendingCount === 1) return "有 1 项需要处理，建议先清理再开始业务";
  return `有 ${pendingCount} 项需要处理，建议先清理再开始业务`;
});

// 当前时钟抽到 useRuntimeClock；会话时长已迁至设置页，底部元数据卡改为「查看教程」。
const { clockText } = useRuntimeClock();

const hasTutorialUrl = computed(() => Boolean(updateCheck.latestInfo?.tutorial_url?.trim()));

onMounted(async () => {
  void loadCacheStatus();
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

    <div
      data-testid="dashboard-metrics"
      class="dashboard-metrics grid shrink-0 grid-cols-1 gap-3 min-[420px]:grid-cols-2 min-[420px]:gap-app sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-5 lg:gap-app xl:grid-cols-5 items-stretch"
    >
      <article
        v-for="metric in metrics"
        :key="metric.key"
        data-testid="dashboard-metric-tile"
        class="surface-panel metric-card dashboard-metric-tile dashboard-metric-card relative flex min-h-[5.25rem] flex-col gap-1 overflow-hidden p-3 transition-all sm:min-h-[5.5rem] sm:gap-1.5 lg:min-h-[5.75rem] lg:gap-1.5 lg:p-4"
        :class="metricAccentClass[metric.tone]"
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

    <section class="surface-panel dashboard-shortcuts-panel flex min-h-0 shrink-0 flex-col p-3 lg:p-4">
      <div class="subsystem-section-header mb-3 flex items-center gap-2">
        <h3 class="text-sm font-semibold tracking-tight text-slate-900">快捷入口</h3>
        <span class="text-[11px] text-slate-400">一键直达核心业务</span>
      </div>

      <div
        data-testid="dashboard-shortcuts"
        class="dashboard-shortcuts-grid subsystem-summary-strip grid grid-cols-2 gap-x-4 gap-y-3 sm:gap-x-5 sm:gap-y-4 lg:gap-y-5"
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

    <!-- 底部元数据卡片：版本 / 作者微信 / 查看教程 / 当前时间 -->
    <section
      data-testid="dashboard-meta-cards"
      class="dashboard-meta-cards grid shrink-0 grid-cols-2 gap-3 sm:grid-cols-4 sm:gap-app"
      aria-label="运行时元数据"
    >
      <article class="surface-panel dashboard-meta-card dashboard-meta-card--version flex items-center gap-3 p-3 lg:p-4">
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
            @click="updateCheck.openDownloadUrl()"
          >
            有新版本 v{{ updateCheck.latestInfo?.version }}
          </button>
          <div v-else class="dashboard-meta-value">v{{ appStore.appVersion }}</div>
        </div>
      </article>

      <article class="surface-panel dashboard-meta-card dashboard-meta-card--author flex items-center gap-3 p-3 lg:p-4">
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
        class="surface-panel dashboard-meta-card dashboard-meta-card--tutorial flex items-center gap-3 p-3 lg:p-4"
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
            @click="updateCheck.openTutorialUrl()"
          >
            点击打开
          </button>
          <div v-else class="dashboard-meta-value text-slate-400">暂无链接</div>
          <p v-if="updateCheck.tutorialActionError" class="mt-0.5 truncate text-[10px] text-red-600">
            {{ updateCheck.tutorialActionError }}
          </p>
        </div>
      </article>

      <article class="surface-panel dashboard-meta-card dashboard-meta-card--clock flex items-center gap-3 p-3 lg:p-4">
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
    </section>

  </div>
</template>
