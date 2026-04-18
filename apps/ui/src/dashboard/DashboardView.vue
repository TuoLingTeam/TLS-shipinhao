<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { RouterLink } from "vue-router";
import { useTauriInvoke } from "../shared/useTauriInvoke";
import { useAppStore } from "../app.store";
import { useOrderStore } from "../order/order.store";
import { useReviewStore } from "../review/review.store";
import { useDeliveryStore } from "../delivery/delivery.store";
import { useOrder } from "../order/useOrder";
import { useCookieHealthStore } from "../shared/cookieHealth";
import AppNavIcon from "../layout/AppNavIcon.vue";
import { AUTHOR_WECHAT } from "../shared/brand";
import { formatDateTime } from "../shared/format";
import { LICENSE_STATE_LABELS } from "../license/license.types";

const appStore = useAppStore();
const orderStore = useOrderStore();
const reviewStore = useReviewStore();
const deliveryStore = useDeliveryStore();
const cookieHealth = useCookieHealthStore();
const { loadCacheStatus } = useOrder();

const appInfo = useTauriInvoke<{ name: string; name_en: string; version: string; author_wechat: string; window_title: string; runtime: string }>("get_app_info");
const info = ref<{ name: string; name_en: string; version: string; author_wechat: string; window_title: string; runtime: string } | null>(null);

const daysUntilLicenseExpires = computed<number | null>(() => {
  const iso = appStore.licenseExpiresAt;
  if (!iso) return null;
  const expire = Date.parse(iso);
  if (!Number.isFinite(expire)) return null;
  const diffMs = expire - Date.now();
  return Math.ceil(diffMs / (24 * 60 * 60 * 1000));
});

const licenseTone = computed<"success" | "warn" | "error">(() => {
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

const metrics = computed(() => [
  {
    key: "license",
    label: "授权状态",
    value: licenseText.value,
    hint:
      daysUntilLicenseExpires.value === null
        ? (appStore.isLicensed ? "已激活" : "请前往授权管理激活")
        : daysUntilLicenseExpires.value < 0
          ? `已过期 ${Math.abs(daysUntilLicenseExpires.value)} 天`
          : daysUntilLicenseExpires.value <= 7
            ? `剩余 ${daysUntilLicenseExpires.value} 天到期，建议续费`
            : `约 ${daysUntilLicenseExpires.value} 天后到期`,
    tone: licenseTone.value,
  },
  {
    key: "cookie",
    label: "Cookie 状态",
    value:
      cookieHealth.status === "healthy"
        ? "可用"
        : cookieHealth.status === "unhealthy"
          ? "已失效"
          : cookieHealth.status === "unconfigured"
            ? "未配置"
            : "待探测",
    hint:
      cookieHealth.snapshot.last_checked_at
        ? `最近探测：${formatDateTime(cookieHealth.snapshot.last_checked_at)}`
        : "启动后自动探测",
    tone:
      cookieHealth.status === "healthy"
        ? "success"
        : cookieHealth.status === "unhealthy"
          ? "error"
          : cookieHealth.status === "unconfigured"
            ? "warn"
            : "idle",
  },
  {
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
      cacheCount.value === 0
        ? "warn"
        : missingSegments.value > 0
          ? "warn"
          : "success",
  },
  {
    key: "review",
    label: "评价匹配",
    value: reviewStore.results.length > 0 ? `${matchedReviewCount.value}/${reviewStore.results.length}` : "--",
    hint:
      reviewStore.results.length === 0
        ? "未执行匹配"
        : unmatchedReviewCount.value > 0
          ? `${unmatchedReviewCount.value} 条待人工核实`
          : "全部匹配成功",
    tone: reviewStore.results.length === 0 ? "idle" : unmatchedReviewCount.value > 0 ? "warn" : "success",
  },
  {
    key: "delivery",
    label: "发货任务",
    value: deliveryStore.batchProgress ? String(deliveryStore.batchProgress.totalCount) : "--",
    hint: deliveryStore.batchProgress
      ? `成功 ${deliveryStore.batchProgress.successCount} · 失败 ${deliveryStore.batchProgress.failureCount}`
      : "本次启动尚未执行批量",
    tone: deliveryStore.batchProgress
      ? deliveryStore.batchProgress.failureCount > 0
        ? "warn"
        : "success"
      : "idle",
  },
]);

const alerts = computed(() => {
  const items: { key: string; text: string; to: string; tone: "warn" | "error" }[] = [];
  if (!appStore.isLicensed) {
    items.push({ key: "license-invalid", text: "授权尚未激活，所有业务功能已暂停", to: "/license", tone: "error" });
  } else if ((daysUntilLicenseExpires.value ?? Infinity) <= 7) {
    const days = daysUntilLicenseExpires.value ?? 0;
    items.push({
      key: "license-renewal",
      text: days < 0 ? `授权已过期 ${Math.abs(days)} 天，请尽快续费` : `授权将在 ${days} 天后到期，建议提前续费`,
      to: "/license",
      tone: days < 0 ? "error" : "warn",
    });
  }
  if (cookieHealth.status === "unhealthy") {
    items.push({
      key: "cookie-unhealthy",
      text: cookieHealth.snapshot.hint || "Cookie 可能已失效，请重新登录小店",
      to: "/settings",
      tone: "error",
    });
  } else if (cookieHealth.status === "unconfigured") {
    items.push({
      key: "cookie-missing",
      text: "尚未配置 Cookie，前往设置完成登录后才可一键发货",
      to: "/settings",
      tone: "warn",
    });
  }
  if (missingSegments.value > 0) {
    items.push({
      key: "cache-gap",
      text: `订单缓存存在 ${missingSegments.value} 个覆盖缺口，建议立即同步以保障评分匹配效果`,
      to: "/order",
      tone: "warn",
    });
  }
  if (deliveryStore.batchProgress && deliveryStore.batchProgress.failureCount > 0 && !deliveryStore.batchProgress.running) {
    items.push({
      key: "delivery-failed",
      text: `最近一批批量发货有 ${deliveryStore.batchProgress.failureCount} 条失败，点击查看明细并重试`,
      to: "/delivery",
      tone: "warn",
    });
  }
  return items;
});

const quickLinks = [
  { to: "/review", title: "中差评/品退", icon: "review", description: "一键匹配订单并带入发货" },
  { to: "/order", title: "订单缓存同步", icon: "order", description: "维护 30 天订单缓存与本地检索" },
  { to: "/delivery", title: "批量发货", icon: "delivery", description: "逐条进度·失败明细·支持取消" },
  { to: "/license", title: "授权管理", icon: "license", description: "激活卡密与查看到期信息" },
] as const;

const toneClass: Record<string, string> = {
  success: "bg-brand-soft text-brand-deep",
  warn: "bg-amber-50 text-amber-700",
  error: "bg-red-50 text-red-700",
  idle: "bg-slate-100 text-slate-500",
};

const alertToneClass: Record<"warn" | "error", string> = {
  warn: "border-amber-200 bg-amber-50 text-amber-800",
  error: "border-red-200 bg-red-50 text-red-700",
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

onMounted(async () => {
  info.value = await appInfo.execute();
  void loadCacheStatus();
  void cookieHealth.refreshSilently();
});
</script>

<template>
  <div class="space-y-5">
    <section class="hero-panel flex flex-col gap-3 p-5 lg:flex-row lg:items-end lg:justify-between lg:p-6">
      <div>
        <p class="text-xs font-semibold uppercase tracking-[0.22em] text-slate-400">TLS 工作台</p>
        <h2 class="mt-2 text-2xl font-semibold tracking-tight text-slate-900">{{ greeting }}</h2>
        <div class="mt-1 text-sm text-slate-500">{{ todayText }} · {{ info?.name ?? "驼铃·视频小店差评处理" }}</div>
      </div>
      <div class="text-xs text-slate-400">
        {{ info?.runtime ?? 'tauri' }} · v{{ info?.version ?? appStore.appVersion }} · 作者微信 {{ info?.author_wechat ?? AUTHOR_WECHAT }}
      </div>
    </section>

    <section v-if="alerts.length > 0" class="space-y-2">
      <RouterLink
        v-for="alert in alerts"
        :key="alert.key"
        :to="alert.to"
        class="flex items-center justify-between gap-3 rounded-[18px] border px-5 py-3 text-sm font-medium transition hover:opacity-90"
        :class="alertToneClass[alert.tone]"
      >
        <span class="min-w-0 truncate">{{ alert.text }}</span>
        <span class="shrink-0 text-xs opacity-70">前往处理 →</span>
      </RouterLink>
    </section>

    <section class="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-3">
      <article
        v-for="metric in metrics"
        :key="metric.key"
        class="surface-panel metric-card p-5 lg:p-6"
      >
        <div class="flex items-center justify-between">
          <div class="metric-label">{{ metric.label }}</div>
          <span
            class="rounded-full px-2.5 py-0.5 text-[11px] font-semibold uppercase tracking-[0.12em]"
            :class="toneClass[metric.tone]"
          >
            {{ metric.tone === 'success' ? '正常' : metric.tone === 'warn' ? '注意' : metric.tone === 'error' ? '需处理' : '待更新' }}
          </span>
        </div>
        <div class="metric-value">{{ metric.value }}</div>
        <div class="metric-hint">{{ metric.hint }}</div>
      </article>
    </section>

    <section class="hero-panel p-5 lg:p-6">
      <div class="mb-4 flex items-center justify-between">
        <h3 class="text-xl font-semibold tracking-tight text-slate-900">高频入口</h3>
        <div class="text-xs text-slate-400">点击卡片跳转对应模块</div>
      </div>

      <div class="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-4">
        <RouterLink
          v-for="item in quickLinks"
          :key="item.to"
          :to="item.to"
          class="quick-link surface-panel-strong flex min-h-[144px] flex-col justify-between p-5"
        >
          <div>
            <div class="flex h-11 w-11 items-center justify-center rounded-2xl bg-brand text-white shadow-lg shadow-brand/20">
              <AppNavIcon :name="item.icon" icon-class="h-5 w-5" />
            </div>
            <h4 class="mt-4 text-lg font-semibold text-slate-900">{{ item.title }}</h4>
            <p class="mt-1 text-xs leading-5 text-slate-500">{{ item.description }}</p>
          </div>
          <div class="mt-4 inline-flex items-center gap-2 text-sm font-semibold text-brand">
            进入
            <span aria-hidden="true">→</span>
          </div>
        </RouterLink>
      </div>
    </section>
  </div>
</template>
