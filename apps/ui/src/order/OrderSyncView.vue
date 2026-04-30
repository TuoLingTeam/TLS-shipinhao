<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useOrder } from "../order/useOrder";
import { useOrderStore } from "../order/store";
import { useAppStore } from "../app.store";
import OrderSearchBar from "../order/OrderSearchBar.vue";
import { formatCent } from "../shared/format";
import EmptyState from "../shared/EmptyState.vue";
import { useNotification } from "../shared/useNotification";

const store = useOrderStore();
const appStore = useAppStore();
const { syncRecentCache, loadRecentCache, loadCacheStatus } = useOrder();
const { show: showToast } = useNotification();
const licenseBlocked = computed(() => !appStore.isLicensed);
const searchKeyword = ref("");

// 大列表默认懒渲染，避免路由切换时一次性挂载上万条订单。
const INITIAL_DISPLAY_LIMIT = 100;
const showAllOrders = ref(false);

const cacheListLoaded = computed(() => store.cachedOrders.length > 0);
const totalAmount = computed(() =>
  store.cachedOrders.reduce((sum, item) => sum + (item.amount_cent ?? 0), 0),
);
const uniqueBuyerCount = computed(() => new Set(store.cachedOrders.map((item) => item.buyer_name)).size);

const filteredOrders = computed(() => {
  const keyword = searchKeyword.value.trim().toLowerCase();
  if (!keyword) return store.cachedOrders;
  return store.cachedOrders.filter((item) =>
    [item.order_id, item.buyer_name].some((field) =>
      field.toLowerCase().includes(keyword),
    ),
  );
});

const visibleOrders = computed(() => {
  const list = filteredOrders.value;
  if (showAllOrders.value || searchKeyword.value.trim()) return list;
  return list.length > INITIAL_DISPLAY_LIMIT ? list.slice(0, INITIAL_DISPLAY_LIMIT) : list;
});

const isTruncated = computed(() => visibleOrders.value.length < filteredOrders.value.length);
const totalOrderCount = computed(() => filteredOrders.value.length);

function handleShowAllOrders() {
  showAllOrders.value = true;
}
const cachedOrderCount = computed(() => store.cacheStatus?.cached_order_count ?? 0);
const hasLocalCache = computed(() => cachedOrderCount.value > 0);
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
    showToast(store.error, "error");
    return;
  }
  const result = await syncRecentCache();
  if (result) {
    showToast(`订单缓存同步完成，本次保存 ${result.orders_saved} 条`, "success");
    return;
  }
  showToast(store.error ?? "订单缓存同步失败", "error");
}

async function ensureCacheListLoaded() {
  if (cacheListLoaded.value || store.loading) return;
  await loadRecentCache();
}

async function handleSearch(keyword: string) {
  searchKeyword.value = keyword;
  if (hasLocalCache.value) {
    await ensureCacheListLoaded();
  }
}

async function handleLoadCacheList() {
  if (licenseBlocked.value) {
    showToast("请先激活授权后再加载订单列表", "error");
    return;
  }
  await ensureCacheListLoaded();
  showToast(store.error ? store.error : "订单列表已加载", store.error ? "error" : "success");
}

const emptyStateTitle = computed(() => {
  if (searchKeyword.value) return "未找到匹配订单";
  if (hasLocalCache.value && !cacheListLoaded.value) return `本地缓存 ${cachedOrderCount.value} 条订单`;
  return "暂无缓存数据";
});

const emptyStateDescription = computed(() => {
  if (searchKeyword.value) return "当前关键词没有命中任何本地订单，可更换关键词或清空筛选。";
  if (hasLocalCache.value && !cacheListLoaded.value)
    return "列表较大，默认按需加载以保障切换流畅。点击下方按钮加载完整订单列表。";
  return "点击下方按钮，将最近订单拉入本地缓存后再进行搜索或评价匹配。";
});

const emptyStateButtonLabel = computed(() => {
  if (searchKeyword.value) return "同步缓存";
  if (hasLocalCache.value && !cacheListLoaded.value) return "加载订单列表";
  return "立即同步缓存";
});

function handleEmptyAction() {
  if (searchKeyword.value) {
    void handleSync();
    return;
  }
  if (hasLocalCache.value && !cacheListLoaded.value) {
    void handleLoadCacheList();
    return;
  }
  void handleSync();
}

onMounted(async () => {
  await loadCacheStatus();
});
</script>

<template>
  <div class="order-sync-view flex min-h-0 flex-1 flex-col">
    <section
      data-testid="order-hero-shell"
      class="hero-panel subsystem-hero order-hero-shell relative flex shrink-0 flex-col gap-3 overflow-hidden p-3 lg:p-3.5"
    >
      <div class="pointer-events-none absolute -right-20 -top-16 h-40 w-40 rounded-full bg-[radial-gradient(circle,rgba(167,243,208,0.38),transparent_72%)]"></div>

      <div class="config-panel-eyebrow relative z-[1]">
        <span class="config-panel-eyebrow-dot" aria-hidden="true"></span>
        <span class="config-panel-eyebrow-label">订单检索</span>
      </div>

      <div v-if="store.cacheStatus?.last_error" class="relative z-[1] soft-alert warn">
        最近一次缓存维护提示：{{ store.cacheStatus.last_error }}
      </div>

      <div class="relative z-[1]">
        <OrderSearchBar
          :sync-disabled="store.loading || licenseBlocked"
          :sync-label="store.loading ? '同步中...' : '同步缓存'"
          @search="handleSearch"
          @sync="handleSync"
        />
      </div>
    </section>

    <div v-if="licenseBlocked" class="shrink-0 soft-alert warn">
      当前未激活授权，订单同步不可用。请先前往设置中心完成激活。
    </div>

    <div v-if="store.error" class="shrink-0 soft-alert error">
      {{ store.error }}
    </div>

    <section
      v-if="store.loading"
      data-testid="order-sync-progress"
      class="order-progress-shell surface-panel min-h-0 min-w-0 flex-1 p-3 lg:p-3.5"
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
      class="order-table-shell surface-panel flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden"
    >
      <div class="order-list-header flex shrink-0 flex-col gap-2 border-b border-slate-200/70 px-4 py-2.5 lg:flex-row lg:items-center lg:justify-between">
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
          <span
            v-if="isTruncated"
            class="rounded-full bg-amber-50 px-2 py-0.5 text-[11px] font-semibold text-amber-700"
            :title="`已渲染前 ${visibleOrders.length} 条以保证流畅，点击右侧按钮查看全部 ${totalOrderCount} 条`"
          >
            预览模式
          </span>
        </div>
        <div class="flex flex-wrap items-center gap-2 text-xs">
          <span class="order-stat-chip order-stat-chip--count">
            <span class="order-stat-chip-label">订单</span>
            <span class="order-stat-chip-value">
              {{ isTruncated ? `${visibleOrders.length} / ${totalOrderCount}` : visibleOrders.length }}
            </span>
          </span>
          <span class="order-stat-chip order-stat-chip--buyer">
            <span class="order-stat-chip-label">买家</span>
            <span class="order-stat-chip-value">{{ uniqueBuyerCount }}</span>
          </span>
          <span class="order-stat-chip order-stat-chip--amount">
            <span class="order-stat-chip-label">金额</span>
            <span class="order-stat-chip-value">{{ store.cachedOrders.length ? formatCent(totalAmount) : "--" }}</span>
          </span>
          <button
            v-if="isTruncated"
            data-testid="order-show-all"
            type="button"
            class="action-btn action-btn-secondary action-btn-compact cursor-pointer"
            :title="`一次性渲染 ${totalOrderCount} 条订单，可能略有卡顿`"
            @click="handleShowAllOrders"
          >
            显示完整订单（{{ totalOrderCount }}）
          </button>
        </div>
      </div>
      <div class="data-table-shell min-h-0 flex-1 overflow-auto border-0 shadow-none">
        <table class="order-table w-full min-w-[768px] text-sm">
          <thead class="table-head text-slate-600">
            <tr>
              <th class="table-head-sticky px-4 py-2.5 text-left font-semibold">订单号</th>
              <th class="table-head-sticky px-4 py-2.5 text-left font-semibold">买家</th>
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
              <td class="px-4 py-2.5 text-right font-semibold text-slate-900">{{ formatCent(o.amount_cent) }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </section>

    <div v-else class="order-sync-empty flex min-h-0 min-w-0 flex-1 flex-col">
      <EmptyState
        class="min-h-0 flex flex-1 flex-col justify-center"
        :title="emptyStateTitle"
        :description="emptyStateDescription"
        @action="handleEmptyAction"
      >
        {{ emptyStateButtonLabel }}
      </EmptyState>
    </div>
  </div>
</template>
