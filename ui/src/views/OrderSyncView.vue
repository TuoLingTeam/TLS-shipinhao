<script setup lang="ts">
import { useOrder } from "../composables/useOrder";
import { useOrderStore } from "../stores/order";
import { useAppStore } from "../stores/app";
import OrderSearchBar from "../components/order/OrderSearchBar.vue";
import OrderCacheStats from "../components/order/OrderCacheStats.vue";
import { formatCent } from "../utils/format";
import { computed } from "vue";

const store = useOrderStore();
const appStore = useAppStore();
const { syncOrders } = useOrder();
const licenseBlocked = computed(() => !appStore.isLicensed);

function todayISO(): string {
  return new Date().toISOString().split("T")[0] + "T23:59:59Z";
}

function daysAgoISO(n: number): string {
  const d = new Date();
  d.setDate(d.getDate() - n);
  return d.toISOString().split("T")[0] + "T00:00:00Z";
}

async function handleSync() {
  if (licenseBlocked.value) {
    store.error = "请先激活授权后再使用订单同步";
    return;
  }
  await syncOrders(daysAgoISO(30), todayISO());
}

function handleSearch(keyword: string) {
  // TODO: 前端过滤 store.cachedOrders
  void keyword;
}
</script>

<template>
  <div>
    <h2 class="text-xl font-semibold text-slate-700 mb-4">订单管理</h2>

    <div class="grid grid-cols-1 lg:grid-cols-3 gap-4 mb-4">
      <div class="lg:col-span-2">
        <OrderSearchBar @search="handleSearch" />
      </div>
      <OrderCacheStats
        :count="store.cachedOrders.length"
        :last-sync-at="store.lastSyncAt"
      />
    </div>

    <div class="mb-4">
      <button
        :disabled="store.loading || licenseBlocked"
        class="px-4 py-1.5 bg-blue-500 text-white text-sm rounded hover:bg-blue-600 disabled:opacity-50 transition-colors"
        @click="handleSync"
      >
        {{ store.loading ? "同步中..." : "同步订单" }}
      </button>
    </div>

    <div v-if="licenseBlocked" class="mb-4 p-3 bg-amber-50 text-amber-700 text-sm rounded border border-amber-200">
      当前未激活授权，订单同步不可用。
    </div>

    <div v-if="store.error" class="mb-4 p-3 bg-red-50 text-red-600 text-sm rounded border border-red-200">
      {{ store.error }}
    </div>

    <div v-if="store.cachedOrders.length > 0" class="bg-white rounded-lg shadow-sm border border-slate-200 overflow-hidden">
      <table class="w-full text-sm">
        <thead class="bg-slate-50 text-slate-600">
          <tr>
            <th class="text-left px-4 py-2.5 font-medium">订单号</th>
            <th class="text-left px-4 py-2.5 font-medium">买家</th>
            <th class="text-left px-4 py-2.5 font-medium">收件人</th>
            <th class="text-right px-4 py-2.5 font-medium">金额</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="o in store.cachedOrders"
            :key="o.order_id"
            class="border-t border-slate-100 hover:bg-slate-50 transition-colors"
          >
            <td class="px-4 py-2.5 font-mono text-xs">{{ o.order_id }}</td>
            <td class="px-4 py-2.5">{{ o.buyer_name }}</td>
            <td class="px-4 py-2.5">{{ o.receiver_name }}</td>
            <td class="px-4 py-2.5 text-right">{{ formatCent(o.amount_cent) }}</td>
          </tr>
        </tbody>
      </table>
    </div>

    <div v-else-if="!store.loading" class="text-center py-12 text-slate-400">
      暂无缓存数据，请先同步订单
    </div>
  </div>
</template>
