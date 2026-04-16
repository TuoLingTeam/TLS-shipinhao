<script setup lang="ts">
import { computed, ref } from "vue";
import { useDelivery } from "../composables/useDelivery";
import { useDeliveryStore } from "../stores/delivery";
import { useAppStore } from "../stores/app";

const store = useDeliveryStore();
const appStore = useAppStore();
const { updateDelivery, batchDelivery } = useDelivery();
const licenseBlocked = computed(() => !appStore.isLicensed);

const orderId = ref("");
const trackingNumber = ref("");
const carrierCode = ref("JT");

const batchText = ref("");

async function handleSingleDelivery() {
  if (licenseBlocked.value) {
    store.error = "请先激活授权后再使用发货功能";
    return;
  }
  if (!orderId.value || !trackingNumber.value) return;
  await updateDelivery(orderId.value, trackingNumber.value, carrierCode.value);
}

async function handleBatchDelivery() {
  if (licenseBlocked.value) {
    store.error = "请先激活授权后再使用发货功能";
    return;
  }
  const lines = batchText.value
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean);
  const items = lines.map((line) => {
    const [oid, tn] = line.split(/[\t,]/).map((s) => s.trim());
    return { order_id: oid, tracking_number: tn };
  }).filter((i) => i.order_id && i.tracking_number);
  if (items.length === 0) return;
  await batchDelivery(items);
}
</script>

<template>
  <div>
    <h2 class="text-xl font-semibold text-slate-700 mb-4">发货管理</h2>

    <div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
      <div class="bg-white rounded-lg p-4 shadow-sm border border-slate-200">
        <h3 class="font-medium text-slate-700 mb-3">单个发货</h3>
        <div class="space-y-3">
          <div>
            <label class="block text-sm text-slate-600 mb-1">订单号</label>
            <input
              v-model="orderId"
              class="w-full px-3 py-1.5 border border-slate-300 rounded text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
              placeholder="输入订单号"
            />
          </div>
          <div>
            <label class="block text-sm text-slate-600 mb-1">快递单号</label>
            <input
              v-model="trackingNumber"
              class="w-full px-3 py-1.5 border border-slate-300 rounded text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
              placeholder="输入快递单号"
            />
          </div>
          <div>
            <label class="block text-sm text-slate-600 mb-1">快递公司</label>
            <select
              v-model="carrierCode"
              class="w-full px-3 py-1.5 border border-slate-300 rounded text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
            >
              <option value="JT">极兔速递</option>
              <option value="YTO">圆通速递</option>
              <option value="ZTO">中通快递</option>
              <option value="STO">申通快递</option>
              <option value="YD">韵达快递</option>
              <option value="SF">顺丰速运</option>
            </select>
          </div>
          <button
            :disabled="store.loading || licenseBlocked"
            class="w-full px-4 py-2 bg-blue-500 text-white text-sm rounded hover:bg-blue-600 disabled:opacity-50 transition-colors"
            @click="handleSingleDelivery"
          >
            {{ store.loading ? "发货中..." : "确认发货" }}
          </button>
        </div>
      </div>

      <div class="bg-white rounded-lg p-4 shadow-sm border border-slate-200">
        <h3 class="font-medium text-slate-700 mb-3">批量发货</h3>
        <div class="space-y-3">
          <div>
            <label class="block text-sm text-slate-600 mb-1">
              粘贴数据（每行：订单号,快递单号）
            </label>
            <textarea
              v-model="batchText"
              rows="6"
              class="w-full px-3 py-1.5 border border-slate-300 rounded text-sm font-mono focus:outline-none focus:ring-2 focus:ring-blue-500"
              placeholder="3735560095122745088,JT00000001&#10;3735560095122745089,JT00000002"
            />
          </div>
          <button
            :disabled="store.loading || licenseBlocked"
            class="w-full px-4 py-2 bg-green-500 text-white text-sm rounded hover:bg-green-600 disabled:opacity-50 transition-colors"
            @click="handleBatchDelivery"
          >
            {{ store.loading ? "批量发货中..." : "开始批量发货" }}
          </button>
        </div>

        <div
          v-if="store.batchProgress"
          class="mt-4 p-3 rounded text-sm"
          :class="store.batchProgress.fatalError ? 'bg-red-50 text-red-700' : 'bg-green-50 text-green-700'"
        >
          <div>总计：{{ store.batchProgress.totalCount }}</div>
          <div>成功：{{ store.batchProgress.successCount }}</div>
          <div>失败：{{ store.batchProgress.failureCount }}</div>
          <div v-if="store.batchProgress.fatalError" class="mt-1 font-medium">
            致命错误：{{ store.batchProgress.fatalError }}
          </div>
        </div>
      </div>
    </div>

    <div v-if="licenseBlocked" class="mt-4 p-3 bg-amber-50 text-amber-700 text-sm rounded border border-amber-200">
      当前未激活授权，发货功能不可用。
    </div>

    <div v-if="store.error" class="mt-4 p-3 bg-red-50 text-red-600 text-sm rounded border border-red-200">
      {{ store.error }}
    </div>
  </div>
</template>
