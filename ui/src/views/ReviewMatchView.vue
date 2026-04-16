<script setup lang="ts">
import { computed, ref } from "vue";
import { useReview } from "../composables/useReview";
import { useReviewStore } from "../stores/review";
import { useAppStore } from "../stores/app";

const store = useReviewStore();
const appStore = useAppStore();
const { findReviews } = useReview();

const days = ref(30);
const licenseBlocked = computed(() => !appStore.isLicensed);

function todayISO(): string {
  return new Date().toISOString().split("T")[0] + "T23:59:59Z";
}

function daysAgoISO(n: number): string {
  const d = new Date();
  d.setDate(d.getDate() - n);
  return d.toISOString().split("T")[0] + "T00:00:00Z";
}

async function handleSearch() {
  if (licenseBlocked.value) {
    store.setError("请先激活授权后再使用评价管理");
    return;
  }
  await findReviews(days.value, daysAgoISO(days.value), todayISO());
}

</script>

<template>
  <div>
    <h2 class="text-xl font-semibold text-slate-700 mb-4">评价管理</h2>

    <div class="bg-white rounded-lg p-4 shadow-sm border border-slate-200 mb-4">
      <div class="flex items-end gap-4">
        <div>
          <label class="block text-sm text-slate-600 mb-1">查询天数</label>
          <input
            v-model.number="days"
            type="number"
            min="1"
            max="90"
            class="w-24 px-3 py-1.5 border border-slate-300 rounded text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        </div>
        <button
          :disabled="store.loading || licenseBlocked"
          class="px-4 py-1.5 bg-blue-500 text-white text-sm rounded hover:bg-blue-600 disabled:opacity-50 transition-colors"
          @click="handleSearch"
        >
          {{ store.loading ? "查找中..." : "开始查找" }}
        </button>
      </div>
    </div>

    <div v-if="licenseBlocked" class="mb-4 p-3 bg-amber-50 text-amber-700 text-sm rounded border border-amber-200">
      当前未激活授权，评价管理不可用。
    </div>

    <div v-if="store.error" class="mb-4 p-3 bg-red-50 text-red-600 text-sm rounded border border-red-200">
      {{ store.error }}
    </div>

    <div v-if="store.results.length > 0" class="bg-white rounded-lg shadow-sm border border-slate-200 overflow-hidden">
      <table class="w-full text-sm">
        <thead class="bg-slate-50 text-slate-600">
          <tr>
            <th class="text-left px-4 py-2.5 font-medium">买家昵称</th>
            <th class="text-left px-4 py-2.5 font-medium">评价内容</th>
            <th class="text-left px-4 py-2.5 font-medium">订单详情</th>
            <th class="text-left px-4 py-2.5 font-medium">评价ID</th>
            <th class="text-center px-4 py-2.5 font-medium">匹配</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="r in store.results"
            :key="r.evaluation_id"
            class="border-t border-slate-100 hover:bg-slate-50 transition-colors"
          >
            <td class="px-4 py-2.5">
              <div class="font-medium text-slate-700">{{ r.buyer_nickname || "-" }}</div>
            </td>
            <td class="px-4 py-2.5 max-w-sm">
              <div class="text-slate-700 whitespace-pre-wrap break-all">
                {{ r.evaluation_content || "（无评价内容）" }}
              </div>
            </td>
            <td class="px-4 py-2.5">
              <div class="space-y-1 text-xs text-slate-600">
                <div>SKU：{{ r.sku_name || r.sku_id || "-" }}</div>
                <div>商品ID：<span class="font-mono">{{ r.product_id || "-" }}</span></div>
                <div v-if="r.product_name" class="text-slate-500">{{ r.product_name }}</div>
              </div>
            </td>
            <td class="px-4 py-2.5 font-mono text-xs">{{ r.evaluation_id }}</td>
            <td class="px-4 py-2.5 text-center">
              <span
                class="inline-block px-2 py-0.5 text-xs rounded-full"
                :class="r.matched ? 'bg-green-100 text-green-700' : 'bg-slate-100 text-slate-500'"
              >
                {{ r.matched ? "已匹配" : "未匹配" }}
              </span>
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <div v-else-if="!store.loading && store.lastQuery" class="text-center py-12 text-slate-400">
      未找到匹配结果
    </div>
  </div>
</template>
