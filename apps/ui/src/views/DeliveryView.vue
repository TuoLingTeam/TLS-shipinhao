<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useDelivery } from "../composables/useDelivery";
import { useDeliveryStore } from "../stores/delivery";
import { useAppStore } from "../stores/app";
import EmptyState from "../components/common/EmptyState.vue";

const store = useDeliveryStore();
const appStore = useAppStore();
const { updateDelivery, batchDelivery } = useDelivery();
const licenseBlocked = computed(() => !appStore.isLicensed);

const orderId = ref("");
const trackingNumber = ref("");
const carrierCode = ref("JT");
const batchText = ref("");

watch(
  () => store.draftOrderId,
  (value) => {
    if (value?.trim()) {
      orderId.value = value;
    }
  },
  { immediate: true },
);

const batchLines = computed(() => batchText.value.split("\n").map((line) => line.trim()).filter(Boolean));
const parsedBatchItems = computed(() =>
  batchLines.value
    .map((line) => {
      const [oid, tn] = line.split(/[\t,]/).map((s) => s.trim());
      return { order_id: oid, tracking_number: tn };
    })
    .filter((item) => item.order_id && item.tracking_number),
);

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
  if (parsedBatchItems.value.length === 0) return;
  await batchDelivery(parsedBatchItems.value);
}
</script>

<template>
  <div class="space-y-5">
    <section class="grid grid-cols-1 gap-4 xl:grid-cols-[1fr_1fr]">
      <div class="hero-panel p-5 lg:p-6">
        <div class="flex items-center justify-between gap-4">
          <h2 class="text-xl font-semibold tracking-tight text-slate-900">单个发货</h2>
          <div v-if="store.draftOrderId" class="text-xs text-brand">
            已自动带入：{{ store.draftSource || '匹配订单' }}
          </div>
        </div>

        <div class="mt-5 space-y-4">
          <div>
            <label class="field-label">订单号</label>
            <input v-model.trim="orderId" class="field-input" placeholder="输入订单号" />
          </div>
          <div>
            <label class="field-label">快递单号</label>
            <input v-model.trim="trackingNumber" class="field-input" placeholder="输入快递单号" />
          </div>
          <div>
            <label class="field-label">快递公司</label>
            <select v-model="carrierCode" class="field-select">
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
            class="action-btn action-btn-primary w-full"
            @click="handleSingleDelivery"
          >
            {{ store.loading ? "发货中..." : "确认发货" }}
          </button>
        </div>
      </div>

      <div class="surface-panel p-5 lg:p-6">
        <div class="flex items-center justify-between gap-4">
          <h2 class="text-xl font-semibold tracking-tight text-slate-900">批量发货</h2>
          <div class="text-xs text-slate-400">{{ parsedBatchItems.length }} 条可提交</div>
        </div>

        <div class="mt-5 space-y-4">
          <div>
            <label class="field-label">批量数据</label>
            <textarea
              v-model.trim="batchText"
              rows="7"
              class="field-textarea font-mono text-sm"
              placeholder="3735560095122745088,JT00000001&#10;3735560095122745089,JT00000002"
            />
          </div>
          <button
            :disabled="store.loading || licenseBlocked"
            class="action-btn action-btn-success w-full"
            @click="handleBatchDelivery"
          >
            {{ store.loading ? "批量发货中..." : "开始批量发货" }}
          </button>
        </div>

        <div
          v-if="store.batchProgress"
          class="mt-5 rounded-[20px] border px-4 py-4 text-sm"
          :class="store.batchProgress.fatalError ? 'border-red-200 bg-red-50 text-red-700' : 'border-brand-tint bg-brand-soft text-brand-deep'"
        >
          <div class="grid grid-cols-3 gap-3 text-center">
            <div>
              <div class="text-xs uppercase tracking-[0.16em] opacity-70">总计</div>
              <div class="mt-1 text-lg font-semibold">{{ store.batchProgress.totalCount }}</div>
            </div>
            <div>
              <div class="text-xs uppercase tracking-[0.16em] opacity-70">成功</div>
              <div class="mt-1 text-lg font-semibold">{{ store.batchProgress.successCount }}</div>
            </div>
            <div>
              <div class="text-xs uppercase tracking-[0.16em] opacity-70">失败</div>
              <div class="mt-1 text-lg font-semibold">{{ store.batchProgress.failureCount }}</div>
            </div>
          </div>
          <div v-if="store.batchProgress.fatalError" class="mt-3 text-sm font-medium">
            致命错误：{{ store.batchProgress.fatalError }}
          </div>
        </div>
      </div>
    </section>

    <div v-if="licenseBlocked" class="soft-alert warn">
      当前未激活授权，发货功能不可用。请先前往授权管理完成激活。
    </div>

    <div v-if="store.error" class="soft-alert error">
      {{ store.error }}
    </div>

    <EmptyState
      v-if="!batchLines.length && !orderId && !trackingNumber && !store.batchProgress && !store.error"
      compact
      title="等待输入发货数据"
      description="支持单条或批量提交物流单号。"
    />
  </div>
</template>
