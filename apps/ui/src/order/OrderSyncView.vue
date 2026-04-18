<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useOrder } from "../order/useOrder";
import { useOrderStore } from "../order/order.store";
import { useAppStore } from "../app.store";
import OrderSearchBar from "../order/OrderSearchBar.vue";
import { formatCent, formatDateTime } from "../shared/format";
import EmptyState from "../shared/EmptyState.vue";
import LoadingState from "../shared/LoadingState.vue";
import { useLayout } from "../layout/useLayout";

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
const cacheCount = computed(() => store.cacheStatus?.cached_order_count ?? store.cachedOrders.length);
const syncLabel = computed(() =>
  store.cacheStatus?.last_sync_at ? formatDateTime(store.cacheStatus.last_sync_at) : "未同步",
);
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
    ? "最近 30 天缓存完整，可直接支撑评价匹配。"
    : `存在 ${store.cacheStatus.missing_segment_count} 个覆盖缺口，建议立即同步。`;
});
const isCompactLayout = computed(() => ["compact", "high_dpi_compact"].includes(mode.value));
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
  <div class="space-y-4">
    <section class="hero-panel p-4 lg:p-5">
      <div class="flex flex-col gap-4">
        <div class="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
          <div>
            <span class="card-eyebrow">ORDER CACHE</span>
            <h2 class="mt-3 text-2xl font-semibold tracking-tight text-slate-900">订单管理</h2>
            <p class="mt-1 text-sm leading-6 text-slate-500">
              保留缓存维护、本地检索与订单列表三项核心能力，减少无效占位。
            </p>
          </div>
          <div class="rounded-2xl border border-brand-tint bg-white/80 px-3.5 py-3 text-sm text-brand-deep lg:max-w-[320px]">
            <div class="font-semibold">缓存状态</div>
            <div class="mt-1 leading-5">{{ activeCoverageLabel }}</div>
          </div>
        </div>

        <div class="flex flex-wrap gap-2 text-xs">
          <span class="rounded-full bg-white/80 px-3 py-1.5 font-medium text-slate-600">
            缓存 {{ cacheCount }}
          </span>
          <span class="rounded-full bg-white/80 px-3 py-1.5 font-medium text-slate-600">
            同步 {{ syncLabel }}
          </span>
          <span class="rounded-full bg-white/80 px-3 py-1.5 font-medium text-slate-600">
            金额 {{ store.cachedOrders.length ? formatCent(totalAmount) : "--" }}
          </span>
        </div>

        <div v-if="store.cacheStatus?.last_error" class="soft-alert warn">
          最近一次缓存维护提示：{{ store.cacheStatus.last_error }}
        </div>

        <OrderSearchBar @search="handleSearch" />
      </div>
    </section>

    <section class="surface-panel p-4 lg:p-5">
      <div class="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
        <div class="min-w-0">
          <div class="text-base font-semibold text-slate-900">同步最近 30 天缓存</div>
          <p class="mt-1 text-sm leading-6 text-slate-500">
            仅维护缓存，不额外展示冗余面板。同步后评价匹配会直接复用这份缓存。
          </p>
        </div>
        <button
          :disabled="store.loading || licenseBlocked"
          class="action-btn action-btn-primary min-w-[148px]"
          @click="handleSync"
        >
          {{ store.loading ? "同步中..." : "立即同步缓存" }}
        </button>
      </div>
      <div class="mt-4 grid grid-cols-1 gap-2.5" :class="isCompactLayout ? 'sm:grid-cols-1' : 'md:grid-cols-3'">
        <div class="rounded-[16px] border border-slate-200/80 bg-slate-50 px-3.5 py-3">
          <div class="text-[11px] text-slate-400">当前列表</div>
          <div class="mt-1 text-lg font-semibold tracking-tight text-slate-900">{{ visibleOrders.length }}</div>
        </div>
        <div class="rounded-[16px] border border-slate-200/80 bg-slate-50 px-3.5 py-3">
          <div class="text-[11px] text-slate-400">买家数</div>
          <div class="mt-1 text-lg font-semibold tracking-tight text-slate-900">{{ uniqueBuyerCount || "--" }}</div>
        </div>
        <div class="rounded-[16px] border border-slate-200/80 bg-slate-50 px-3.5 py-3">
          <div class="text-[11px] text-slate-400">总金额</div>
          <div class="mt-1 text-lg font-semibold tracking-tight text-slate-900">{{ store.cachedOrders.length ? formatCent(totalAmount) : "--" }}</div>
        </div>
      </div>
    </section>

    <div v-if="licenseBlocked" class="soft-alert warn">
      当前未激活授权，订单同步不可用。请先前往设置中心完成激活。
    </div>

    <div v-if="store.error" class="soft-alert error">
      {{ store.error }}
    </div>

    <LoadingState
      v-if="store.loading"
      title="正在同步订单并刷新缓存"
      :description="store.syncMessage || '后端会先维护最近 30 天富缓存，再刷新当前订单列表。'"
    />

    <section v-if="store.loading" class="surface-panel space-y-4 p-4 lg:p-5">
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
      <div class="grid grid-cols-1 gap-2.5 lg:grid-cols-3">
        <div
          v-for="step in syncSteps"
          :key="step.key"
          class="rounded-[16px] border px-3.5 py-3"
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
      <div class="flex flex-col gap-3 border-b border-slate-200/70 px-4 py-3.5 lg:flex-row lg:items-center lg:justify-between">
        <div>
          <div class="text-base font-semibold text-slate-900">本地订单列表</div>
          <div class="mt-1 text-sm text-slate-500">
            当前展示 {{ visibleOrders.length }} 条{{ searchKeyword ? "筛选结果" : "缓存订单" }}。
          </div>
        </div>
        <div class="flex flex-wrap gap-2 text-xs">
          <span class="rounded-full bg-slate-100 px-3 py-1.5 font-medium text-slate-600">
            订单 {{ visibleOrders.length }}
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
              <th class="table-head-sticky px-4 py-3.5 text-left font-semibold">订单号</th>
              <th class="table-head-sticky px-4 py-3.5 text-left font-semibold">买家</th>
              <th class="table-head-sticky px-4 py-3.5 text-left font-semibold">收件人</th>
              <th class="table-head-sticky px-4 py-3.5 text-right font-semibold">金额</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="o in visibleOrders"
              :key="o.order_id"
              class="table-row border-t border-slate-100/80 transition-colors"
            >
              <td class="px-4 py-3.5 font-mono text-xs text-slate-700">{{ o.order_id }}</td>
              <td class="px-4 py-3.5 font-medium text-slate-800">{{ o.buyer_name }}</td>
              <td class="px-4 py-3.5 text-slate-700">{{ o.receiver_name }}</td>
              <td class="px-4 py-3.5 text-right font-semibold text-slate-900">{{ formatCent(o.amount_cent) }}</td>
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
