<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useOrder } from "../composables/useOrder";
import { useOrderStore } from "../stores/order";
import { useAppStore } from "../stores/app";
import OrderSearchBar from "../components/order/OrderSearchBar.vue";
import OrderCacheStats from "../components/order/OrderCacheStats.vue";
import { formatCent } from "../utils/format";
import EmptyState from "../components/common/EmptyState.vue";
import LoadingState from "../components/common/LoadingState.vue";
import { useLayout } from "../composables/useLayout";

const store = useOrderStore();
const { mode } = useLayout();
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
  if (!store.cacheStatus) return "等待建立";
  return store.cacheStatus.coverage_complete
    ? "缓存覆盖完整，可直接支撑差评评分匹配。"
    : `存在 ${store.cacheStatus.missing_segment_count} 个覆盖缺口，建议立即维护缓存。`;
});
const isCompactLayout = computed(() => ["compact", "high_dpi_compact"].includes(mode.value));
const isWideLayout = computed(() => mode.value === "wide");
const syncSteps = computed(() => [
  {
    key: "ensure_recent_cache",
    title: "维护最近 30 天缓存",
    status:
      store.syncPhase === "ensure_recent_cache"
        ? "进行中"
        : ["refresh_light_cache", "completed"].includes(store.syncPhase || "")
          ? "已完成"
          : "等待中",
  },
  {
    key: "refresh_light_cache",
    title: "刷新订单列表视图",
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
    status:
      store.syncPhase === "completed"
          ? "已完成"
          : "等待中",
  },
]);

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
  <div class="space-y-5">
    <section class="grid grid-cols-1 gap-4" :class="isWideLayout ? 'xl:grid-cols-[1.4fr_0.9fr]' : 'xl:grid-cols-1'">
      <div class="hero-panel p-5 lg:p-6">
        <div class="flex flex-col gap-5">
          <div class="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
            <div>
              <span class="card-eyebrow">ORDER CACHE</span>
              <h2 class="mt-3 text-2xl font-semibold tracking-tight text-slate-900">订单管理与缓存维护</h2>
              <p class="mt-2 text-sm leading-6 text-slate-500">
                这里同时承担两类职责：维护近 30 天富缓存，以及对当前订单副本做快速本地检索。
              </p>
            </div>
            <div class="rounded-2xl border border-brand-tint bg-white/75 px-4 py-3 text-sm text-brand-deep lg:max-w-[280px]">
              <div class="font-semibold text-brand-deep">当前状态</div>
              <div class="mt-1 leading-6">{{ activeCoverageLabel }}</div>
            </div>
          </div>
          <OrderSearchBar @search="handleSearch" />
        </div>
      </div>
      <OrderCacheStats
        :count="store.cacheStatus?.cached_order_count ?? store.cachedOrders.length"
        :last-sync-at="store.cacheStatus?.last_sync_at ?? store.lastSyncAt"
        :coverage-start="store.cacheStatus?.coverage_start"
        :coverage-end="store.cacheStatus?.coverage_end"
        :coverage-complete="store.cacheStatus?.coverage_complete"
        :missing-segment-count="store.cacheStatus?.missing_segment_count"
      />
    </section>

    <section class="grid grid-cols-1 gap-4" :class="isWideLayout ? 'xl:grid-cols-[1.15fr_0.85fr]' : 'xl:grid-cols-1'">
      <div class="surface-panel p-5 lg:p-6">
        <div class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
          <div class="min-w-0">
            <div class="text-base font-semibold text-slate-900">同步最近 30 天缓存</div>
            <p class="mt-1 text-sm leading-6 text-slate-500">
              仅维护缓存，不改变评分规则。完成后差评匹配会直接复用这份富缓存。
            </p>
          </div>
          <button
            :disabled="store.loading || licenseBlocked"
            class="action-btn action-btn-primary min-w-[164px]"
            @click="handleSync"
          >
            {{ store.loading ? "同步中..." : "立即同步缓存" }}
          </button>
        </div>
        <div class="mt-5 grid grid-cols-1 gap-3" :class="isCompactLayout ? 'sm:grid-cols-1' : 'md:grid-cols-3'">
          <div class="rounded-2xl border border-slate-200/80 bg-slate-50 px-4 py-3">
            <div class="text-xs text-slate-400">当前列表</div>
            <div class="mt-1 text-xl font-semibold tracking-tight text-slate-900">{{ visibleOrders.length }}</div>
          </div>
          <div class="rounded-2xl border border-slate-200/80 bg-slate-50 px-4 py-3">
            <div class="text-xs text-slate-400">买家数</div>
            <div class="mt-1 text-xl font-semibold tracking-tight text-slate-900">{{ uniqueBuyerCount || "--" }}</div>
          </div>
          <div class="rounded-2xl border border-slate-200/80 bg-slate-50 px-4 py-3">
            <div class="text-xs text-slate-400">总金额</div>
            <div class="mt-1 text-xl font-semibold tracking-tight text-slate-900">{{ store.cachedOrders.length ? formatCent(totalAmount) : "--" }}</div>
          </div>
        </div>
      </div>

      <div class="surface-panel p-5 lg:p-6">
        <div class="flex items-center justify-between gap-4">
          <div>
            <div class="text-xs font-semibold uppercase tracking-[0.22em] text-slate-400">Search State</div>
            <h3 class="mt-1 text-xl font-semibold tracking-tight text-slate-900">本地检索状态</h3>
          </div>
          <div class="rounded-2xl bg-slate-50 px-3 py-2 text-xs font-semibold text-slate-500">
            {{ searchKeyword ? "已筛选" : "全部订单" }}
          </div>
        </div>
        <div class="mt-5 space-y-3">
          <div v-if="store.cacheStatus?.last_error" class="soft-alert warn">
            最近一次缓存维护提示：{{ store.cacheStatus.last_error }}
          </div>
          <div class="rounded-2xl border border-slate-200/80 bg-slate-50 px-4 py-3">
            <div class="text-xs text-slate-400">筛选关键词</div>
            <div class="mt-1 text-sm font-semibold text-slate-800">
              {{ searchKeyword || "未输入，当前展示全部本地订单。" }}
            </div>
          </div>
          <div class="rounded-2xl border border-slate-200/80 bg-slate-50 px-4 py-3">
            <div class="text-xs text-slate-400">匹配链路说明</div>
            <div class="mt-1 text-sm leading-6 text-slate-600">
              差评评分匹配依赖富缓存；本页列表只负责展示轻量副本，方便核对订单号、买家与收件人。
            </div>
          </div>
        </div>
      </div>
    </section>

    <div v-if="licenseBlocked" class="soft-alert warn">
      当前未激活授权，订单同步不可用。请先前往授权管理完成激活。
    </div>

    <div v-if="store.error" class="soft-alert error">
      {{ store.error }}
    </div>

    <LoadingState
      v-if="store.loading"
      title="正在同步订单并刷新缓存"
      :description="store.syncMessage || '后端会先维护最近 30 天富缓存，再刷新当前订单列表。'"
    />

    <section v-if="store.loading" class="surface-panel space-y-4 p-5 lg:p-6">
      <div class="flex items-center justify-between">
        <div>
          <div class="text-base font-semibold text-slate-900">实时同步进度</div>
          <div class="mt-1 text-sm text-slate-500">{{ store.syncMessage || "正在准备同步任务…" }}</div>
        </div>
        <div class="text-2xl font-semibold tracking-tight text-slate-900">{{ store.syncProgress }}%</div>
      </div>
      <div class="h-2 overflow-hidden rounded-full bg-slate-100">
        <div
          class="h-full rounded-full bg-brand transition-all duration-300"
          :style="{ width: `${store.syncProgress}%` }"
        ></div>
      </div>
      <div class="grid grid-cols-1 gap-3 lg:grid-cols-3">
        <div
          v-for="step in syncSteps"
          :key="step.key"
          class="rounded-2xl border px-4 py-3"
          :class="
            step.status === '已完成'
              ? 'border-brand-tint bg-brand-soft/70'
              : step.status === '进行中'
                ? 'border-brand-tint bg-brand-soft/70'
                : 'border-slate-200 bg-white'
          "
        >
          <div class="text-sm font-semibold text-slate-800">{{ step.title }}</div>
          <div class="mt-1 text-xs text-slate-500">{{ step.status }}</div>
        </div>
      </div>
    </section>

    <section v-else-if="visibleOrders.length > 0" class="surface-panel overflow-hidden">
      <div class="flex flex-col gap-3 border-b border-slate-200/70 px-5 py-4 lg:flex-row lg:items-center lg:justify-between">
        <div>
          <div class="text-base font-semibold text-slate-900">本地订单列表</div>
          <div class="mt-1 text-sm text-slate-500">
            当前展示 {{ visibleOrders.length }} 条{{ searchKeyword ? "筛选结果" : "缓存订单" }}。
          </div>
        </div>
        <div class="flex flex-wrap gap-2 text-xs">
          <span class="rounded-full bg-slate-100 px-3 py-1.5 font-medium text-slate-600">
            订单数 {{ visibleOrders.length }}
          </span>
          <span class="rounded-full bg-slate-100 px-3 py-1.5 font-medium text-slate-600">
            买家 {{ uniqueBuyerCount }}
          </span>
          <span class="rounded-full bg-slate-100 px-3 py-1.5 font-medium text-slate-600">
            金额 {{ store.cachedOrders.length ? formatCent(totalAmount) : "--" }}
          </span>
        </div>
      </div>
      <div class="data-table-shell overflow-x-auto border-0 shadow-none">
      <table class="w-full min-w-[860px] text-sm">
        <thead class="table-head text-slate-600">
          <tr>
            <th class="table-head-sticky px-5 py-4 text-left font-semibold">订单号</th>
            <th class="table-head-sticky px-5 py-4 text-left font-semibold">买家</th>
            <th class="table-head-sticky px-5 py-4 text-left font-semibold">收件人</th>
            <th class="table-head-sticky px-5 py-4 text-right font-semibold">金额</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="o in visibleOrders"
            :key="o.order_id"
            class="table-row border-t border-slate-100/80 transition-colors"
          >
            <td class="px-5 py-4 font-mono text-xs text-slate-700">{{ o.order_id }}</td>
            <td class="px-5 py-4 font-medium text-slate-800">{{ o.buyer_name }}</td>
            <td class="px-5 py-4 text-slate-700">{{ o.receiver_name }}</td>
            <td class="px-5 py-4 text-right font-semibold text-slate-900">{{ formatCent(o.amount_cent) }}</td>
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
          : '点击上方“同步订单”，将最近订单拉入本地缓存后再进行搜索或评价匹配。'
      "
      @action="handleSync"
    >
      {{ searchKeyword ? "同步最近 30 天缓存" : "立即同步订单" }}
    </EmptyState>
  </div>
</template>
