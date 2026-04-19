<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useOrder } from "../order/useOrder";
import { useOrderStore } from "../order/order.store";
import { useAppStore } from "../app.store";
import OrderSearchBar from "../order/OrderSearchBar.vue";
import { formatCent } from "../shared/format";
import EmptyState from "../shared/EmptyState.vue";

const store = useOrderStore();
const appStore = useAppStore();
const { syncRecentCache, loadRecentCache, loadCacheStatus } = useOrder();
const licenseBlocked = computed(() => !appStore.isLicensed);
const searchKeyword = ref("");
const totalAmount = computed(() =>
  store.cachedOrders.reduce((sum, item) => sum + (item.amount_cent ?? 0), 0),
);
const uniqueBuyerCount = computed(() => new Set(store.cachedOrders.map((item) => item.buyer_name)).size);
const visibleOrders = computed(() => {
  const keyword = searchKeyword.value.trim().toLowerCase();
  if (!keyword) return store.cachedOrders;
  return store.cachedOrders.filter((item) =>
    [item.order_id, item.buyer_name, item.receiver_name].some((field) =>
      field.toLowerCase().includes(keyword),
    ),
  );
});
const activeCoverageLabel = computed(() => {
  if (!store.cacheStatus) return "等待建立缓存";
  return store.cacheStatus.coverage_complete
    ? "近 30 天（不含今天）缓存完整，可直接支撑评价匹配。"
    : `近 30 天（不含今天）存在 ${store.cacheStatus.missing_segment_count} 个覆盖缺口，建议立即同步。`;
});
const cacheStatusTone = computed<"success" | "warn" | "idle">(() => {
  if (!store.cacheStatus) return "idle";
  return store.cacheStatus.coverage_complete ? "success" : "warn";
});
const cacheStatusBadgeLabel = computed(() => {
  if (!store.cacheStatus) return "缓存待建立";
  return store.cacheStatus.coverage_complete ? "缓存完整" : `缓存缺口 ${store.cacheStatus.missing_segment_count}`;
});
const cacheStatusBadgeClass = computed(() => {
  if (cacheStatusTone.value === "success") return "order-cache-chip--success";
  if (cacheStatusTone.value === "warn") return "order-cache-chip--warn";
  return "order-cache-chip--idle";
});
const syncSteps = computed(() => [
  {
    key: "ensure_recent_cache",
    title: "维护缓存",
    status:
      store.syncPhase === "ensure_recent_cache"
        ? "进行中"
        : ["refresh_light_cache", "completed"].includes(store.syncPhase || "")
          ? "已完成"
          : "等待中",
  },
  {
    key: "refresh_light_cache",
    title: "刷新列表",
    status:
      store.syncPhase === "refresh_light_cache"
        ? "进行中"
        : ["completed"].includes(store.syncPhase || "")
          ? "已完成"
          : "等待中",
  },
  {
    key: "completed",
    title: "同步完成",
    status: store.syncPhase === "completed" ? "已完成" : "等待中",
  },
]);
const syncStepTone = (status: string) => {
  if (status === "已完成") return "border-brand-tint bg-brand-soft/70 text-brand-deep";
  if (status === "进行中") return "border-brand-tint bg-brand-soft/60 text-slate-800";
  return "border-slate-200 bg-white text-slate-500";
};

async function handleSync() {
  if (licenseBlocked.value) {
    store.error = "请先激活授权后再使用订单同步";
    return;
  }
  await syncRecentCache();
}

function handleSearch(keyword: string) {
  searchKeyword.value = keyword;
}

onMounted(async () => {
  await Promise.all([loadRecentCache(), loadCacheStatus()]);
});
</script>

<template>
  <div class="space-y-app">
    <section class="hero-panel subsystem-hero relative overflow-hidden p-3 lg:p-3.5">
      <div class="pointer-events-none absolute -right-16 -top-16 h-40 w-40 rounded-full bg-[radial-gradient(circle,rgba(167,243,208,0.4),transparent_72%)]"></div>

      <div data-testid="order-hero-shell" class="order-hero-shell relative">
        <div class="flex flex-wrap items-center justify-between gap-3">
          <div class="min-w-0 flex-1">
            <h2 class="text-[1.05rem] font-bold tracking-tight text-slate-900 sm:text-[1.15rem] lg:text-[1.22rem]">
              订单缓存与本地检索
            </h2>
            <p class="mt-0.5 max-w-[44rem] text-[12px] leading-5 text-slate-500">
              {{ activeCoverageLabel }}
            </p>
          </div>

          <div class="order-cache-chip shrink-0" :class="cacheStatusBadgeClass">
            <span class="status-dot" :class="cacheStatusTone"></span>
            <span class="order-cache-chip-label">{{ cacheStatusBadgeLabel }}</span>
          </div>
        </div>

        <div v-if="store.cacheStatus?.last_error" class="soft-alert warn">
          最近一次缓存维护提示：{{ store.cacheStatus.last_error }}
        </div>

        <OrderSearchBar
          :sync-disabled="store.loading || licenseBlocked"
          :sync-label="store.loading ? '同步中...' : '同步缓存'"
          @search="handleSearch"
          @sync="handleSync"
        />
      </div>
    </section>

    <div v-if="licenseBlocked" class="soft-alert warn">
      当前未激活授权，订单同步不可用。请先前往设置中心完成激活。
    </div>

    <div v-if="store.error" class="soft-alert error">
      {{ store.error }}
    </div>

    <section
      v-if="store.loading"
      data-testid="order-sync-progress"
      class="order-progress-shell surface-panel p-3 lg:p-3.5"
    >
      <div class="order-progress-head">
        <div class="min-w-0">
          <div class="text-sm font-semibold text-slate-900">同步订单缓存</div>
          <div class="mt-0.5 text-[12px] leading-5 text-slate-500">
            {{ store.syncMessage || "正在准备同步任务…" }}
          </div>
        </div>
        <div class="order-progress-percent">{{ store.syncProgress }}%</div>
      </div>

      <div class="order-progress-bar">
        <div
          class="h-full rounded-full bg-brand transition-all duration-300"
          :style="{ width: `${store.syncProgress}%` }"
        ></div>
      </div>

      <div class="order-step-grid grid grid-cols-1 gap-app sm:grid-cols-3">
        <div
          v-for="step in syncSteps"
          :key="step.key"
          class="order-step-card rounded-[12px] border px-2.5 py-2"
          :class="syncStepTone(step.status)"
        >
          <div class="text-[12px] font-semibold">{{ step.title }}</div>
          <div class="mt-0.5 text-[11px]">{{ step.status }}</div>
        </div>
      </div>
    </section>

    <section
      v-else-if="visibleOrders.length > 0"
      data-testid="order-table-shell"
      class="order-table-shell surface-panel overflow-hidden"
    >
      <div class="order-list-header flex flex-col gap-2 border-b border-slate-200/70 px-4 py-2.5 lg:flex-row lg:items-center lg:justify-between">
        <div class="flex items-center gap-2">
          <span class="order-list-indicator" aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="h-3.5 w-3.5">
              <path d="M3 7.5 12 3l9 4.5v9L12 21l-9-4.5z" />
              <path d="M12 12 3 7.5" />
              <path d="M12 12l9-4.5" />
              <path d="M12 21v-9" />
            </svg>
          </span>
          <div class="text-sm font-semibold tracking-tight text-slate-900 sm:text-[0.95rem]">本地订单列表</div>
        </div>
        <div class="flex flex-wrap items-center gap-2 text-xs">
          <span class="order-stat-chip order-stat-chip--count">
            <span class="order-stat-chip-label">订单</span>
            <span class="order-stat-chip-value">{{ visibleOrders.length }}</span>
          </span>
          <span class="order-stat-chip order-stat-chip--buyer">
            <span class="order-stat-chip-label">买家</span>
            <span class="order-stat-chip-value">{{ uniqueBuyerCount }}</span>
          </span>
          <span class="order-stat-chip order-stat-chip--amount">
            <span class="order-stat-chip-label">金额</span>
            <span class="order-stat-chip-value">{{ store.cachedOrders.length ? formatCent(totalAmount) : "--" }}</span>
          </span>
        </div>
      </div>
      <div class="data-table-shell overflow-x-auto border-0 shadow-none">
        <table class="order-table w-full min-w-[860px] text-sm">
          <thead class="table-head text-slate-600">
            <tr>
              <th class="table-head-sticky px-4 py-2.5 text-left font-semibold">订单号</th>
              <th class="table-head-sticky px-4 py-2.5 text-left font-semibold">买家</th>
              <th class="table-head-sticky px-4 py-2.5 text-left font-semibold">收件人</th>
              <th class="table-head-sticky px-4 py-2.5 text-right font-semibold">金额</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="o in visibleOrders"
              :key="o.order_id"
              class="table-row border-t border-slate-100/80 transition-colors"
            >
              <td class="order-id-cell px-4 py-2.5 font-mono text-xs text-slate-700">{{ o.order_id }}</td>
              <td class="px-4 py-2.5 font-medium text-slate-800">{{ o.buyer_name }}</td>
              <td class="px-4 py-2.5 text-slate-700">{{ o.receiver_name }}</td>
              <td class="px-4 py-2.5 text-right font-semibold text-slate-900">{{ formatCent(o.amount_cent) }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </section>

    <EmptyState
      v-else
      title="暂无缓存数据"
      :description="
        searchKeyword
          ? '当前关键词没有命中任何本地订单，可更换关键词或清空筛选。'
          : '点击上方「同步缓存」，将最近订单拉入本地缓存后再进行搜索或评价匹配。'
      "
      @action="handleSync"
    >
      {{ searchKeyword ? "同步缓存" : "立即同步缓存" }}
    </EmptyState>
  </div>
</template>
