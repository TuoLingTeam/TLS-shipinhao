<script setup lang="ts">
import { computed } from "vue";
import { useOrder } from "../composables/useOrder";
import { useOrderStore } from "../stores/order";
import { useAppStore } from "../stores/app";
import OrderSearchBar from "../components/order/OrderSearchBar.vue";
import OrderCacheStats from "../components/order/OrderCacheStats.vue";
import { formatCent } from "../utils/format";
import EmptyState from "../components/common/EmptyState.vue";
import LoadingState from "../components/common/LoadingState.vue";

const store = useOrderStore();
const appStore = useAppStore();
const { syncOrders } = useOrder();
const licenseBlocked = computed(() => !appStore.isLicensed);
const totalAmount = computed(() =>
  store.cachedOrders.reduce((sum, item) => sum + (item.amount_cent ?? 0), 0),
);
const syncSteps = computed(() => [
  {
    key: "request_remote",
    title: "拉取远端订单",
    status:
      store.syncPhase === "request_remote"
        ? "进行中"
        : ["write_cache", "load_cache", "completed"].includes(store.syncPhase || "")
          ? "已完成"
          : "等待中",
  },
  {
    key: "write_cache",
    title: "写入本地缓存",
    status:
      store.syncPhase === "write_cache"
        ? "进行中"
        : ["load_cache", "completed"].includes(store.syncPhase || "")
          ? "已完成"
          : "等待中",
  },
  {
    key: "load_cache",
    title: "刷新缓存视图",
    status:
      store.syncPhase === "load_cache"
        ? "进行中"
        : store.syncPhase === "completed"
          ? "已完成"
          : "等待中",
  },
]);

function todayISO(): string {
  return `${new Date().toISOString().split("T")[0]}T23:59:59Z`;
}

function daysAgoISO(n: number): string {
  const d = new Date();
  d.setDate(d.getDate() - n);
  return `${d.toISOString().split("T")[0]}T00:00:00Z`;
}

async function handleSync() {
  if (licenseBlocked.value) {
    store.error = "请先激活授权后再使用订单同步";
    return;
  }
  await syncOrders(daysAgoISO(30), todayISO());
}

function handleSearch(keyword: string) {
  void keyword;
}
</script>

<template>
  <div class="space-y-5">
    <section class="grid grid-cols-1 gap-4 xl:grid-cols-[1.7fr_0.9fr]">
      <div class="space-y-4">
        <div>
          <span class="card-eyebrow">ORDER CACHE</span>
          <h2 class="mt-3 text-2xl font-semibold tracking-tight text-slate-900">订单同步与本地检索</h2>
          <p class="mt-2 text-sm leading-6 text-slate-500">
            同步后的订单缓存会用于评价匹配、履约核对等流程。建议定期拉取近 30 天数据保持链路完整。
          </p>
        </div>
        <OrderSearchBar @search="handleSearch" />
      </div>
      <OrderCacheStats :count="store.cachedOrders.length" :last-sync-at="store.lastSyncAt" />
    </section>

    <section class="grid grid-cols-1 gap-4 lg:grid-cols-[1.2fr_0.8fr]">
      <div class="surface-panel flex flex-col gap-4 p-5 lg:flex-row lg:items-center lg:justify-between lg:p-6">
        <div>
          <div class="text-base font-semibold text-slate-900">立即同步近 30 天订单</div>
          <p class="mt-1 text-sm leading-6 text-slate-500">
            建议在开始评价查找前先同步一次，保证 SKU、商品与买家信息尽量完整。
          </p>
        </div>
        <button
          :disabled="store.loading || licenseBlocked"
          class="action-btn action-btn-primary min-w-[140px]"
          @click="handleSync"
        >
          {{ store.loading ? "同步中..." : "同步订单" }}
        </button>
      </div>

      <div class="surface-panel p-5 lg:p-6">
        <div class="text-xs font-semibold uppercase tracking-[0.22em] text-slate-400">Density Snapshot</div>
        <div class="mt-4 grid grid-cols-2 gap-3">
          <div class="rounded-2xl bg-slate-50 px-4 py-3">
            <div class="text-xs text-slate-400">买家数</div>
            <div class="mt-1 text-xl font-semibold tracking-tight text-slate-900">{{ store.cachedOrders.length || '--' }}</div>
          </div>
          <div class="rounded-2xl bg-slate-50 px-4 py-3">
            <div class="text-xs text-slate-400">总金额</div>
            <div class="mt-1 text-xl font-semibold tracking-tight text-slate-900">{{ store.cachedOrders.length ? formatCent(totalAmount) : '--' }}</div>
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
      :description="store.syncMessage || '会先调用远端拉单接口，再加载本地 SQLite 缓存结果。'"
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
          class="h-full rounded-full bg-blue-600 transition-all duration-300"
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
              ? 'border-green-200 bg-green-50/70'
              : step.status === '进行中'
                ? 'border-blue-200 bg-blue-50/70'
                : 'border-slate-200 bg-white'
          "
        >
          <div class="text-sm font-semibold text-slate-800">{{ step.title }}</div>
          <div class="mt-1 text-xs text-slate-500">{{ step.status }}</div>
        </div>
      </div>
    </section>

    <section v-else-if="store.cachedOrders.length > 0" class="data-table-shell overflow-x-auto">
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
            v-for="o in store.cachedOrders"
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
    </section>

    <EmptyState
      v-else
      title="暂无缓存数据"
      description="点击上方“同步订单”，将最近订单拉入本地缓存后再进行搜索或评价匹配。"
      @action="handleSync"
    >
      立即同步订单
    </EmptyState>
  </div>
</template>
