<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useDelivery } from "../delivery/useDelivery";
import { useDeliveryStore } from "../delivery/delivery.store";
import { useAppStore } from "../app.store";
import EmptyState from "../shared/EmptyState.vue";
import ConfirmDialog from "../shared/ConfirmDialog.vue";

const store = useDeliveryStore();
const appStore = useAppStore();
const { updateDelivery, batchDelivery, cancelBatchDelivery, retryFailedItems, exportFailedCsv } = useDelivery();
const licenseBlocked = computed(() => !appStore.isLicensed);

const orderId = ref("");
const trackingNumber = ref("");
const carrierCode = ref("JT");
const batchText = ref("");
const confirmOpen = ref(false);
const retryConfirmOpen = ref(false);

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

const progress = computed(() => store.batchProgress);
const progressPercent = computed(() => {
  const info = progress.value;
  if (!info || info.totalCount === 0) return 0;
  return Math.min(100, Math.round((info.processedCount / info.totalCount) * 100));
});
const failedSteps = computed(() => progress.value?.steps.filter((step) => step.status === "failed") ?? []);
const recentSteps = computed(() => {
  const steps = progress.value?.steps ?? [];
  return steps.slice(-20).reverse();
});
const canRetryFailed = computed(() =>
  Boolean(progress.value && !progress.value.running && failedSteps.value.length > 0),
);
const overviewCards = computed(() => [
  {
    label: "待发订单",
    value: orderId.value || "未带入",
    hint: store.draftSource ? `来源：${store.draftSource}` : "可从评价匹配结果自动带入",
  },
  {
    label: "批量条数",
    value: parsedBatchItems.value.length ? `${parsedBatchItems.value.length} 条` : "0 条",
    hint: parsedBatchItems.value.length ? "支持确认后统一提交" : "粘贴订单号和单号后自动识别",
  },
  {
    label: "执行进度",
    value: progress.value ? `${progressPercent.value}%` : "待开始",
    hint: progress.value ? `成功 ${progress.value.successCount} · 失败 ${progress.value.failureCount}` : "尚未启动批量发货任务",
  },
]);
const confirmMessage = computed(() => {
  const count = parsedBatchItems.value.length;
  const eta = Math.max(1, Math.ceil(count * 0.5));
  return `即将提交 ${count} 条批量发货，预计耗时约 ${eta} 秒。提交后将无法撤销已发送至小店的条目，请确认无误。`;
});

async function handleSingleDelivery() {
  if (licenseBlocked.value) {
    store.error = "请先激活授权后再使用发货功能";
    return;
  }
  if (!orderId.value || !trackingNumber.value) return;
  await updateDelivery(orderId.value, trackingNumber.value, carrierCode.value);
}

function requestBatchDelivery() {
  if (licenseBlocked.value) {
    store.error = "请先激活授权后再使用发货功能";
    return;
  }
  if (parsedBatchItems.value.length === 0) return;
  confirmOpen.value = true;
}

async function confirmBatchDelivery() {
  confirmOpen.value = false;
  await batchDelivery(parsedBatchItems.value);
}

async function confirmRetryFailed() {
  retryConfirmOpen.value = false;
  await retryFailedItems();
}

async function handleCancelBatch() {
  await cancelBatchDelivery();
}
</script>

<template>
  <div class="space-y-5">
    <section class="hero-panel p-5 lg:p-6">
      <div class="flex flex-col gap-5">
        <div>
          <span class="card-eyebrow">DELIVERY DESK</span>
          <h2 class="mt-3 text-2xl font-semibold tracking-tight text-slate-900">发货操作台</h2>
          <p class="mt-2 max-w-2xl text-sm leading-6 text-slate-500">
            单个发货与批量提交共用同一工作台：左侧适合快速改单，右侧适合批量录入与统一确认。
          </p>
        </div>
        <div class="grid gap-3 md:grid-cols-3">
          <article
            v-for="card in overviewCards"
            :key="card.label"
            class="rounded-[22px] border border-white/60 bg-white/80 px-4 py-4 shadow-[0_18px_40px_-30px_rgba(15,23,42,0.22)] backdrop-blur"
          >
            <div class="text-xs uppercase tracking-[0.18em] text-slate-400">{{ card.label }}</div>
            <div class="mt-2 break-all text-lg font-semibold tracking-tight text-slate-900">{{ card.value }}</div>
            <div class="mt-2 text-sm leading-6 text-slate-500">{{ card.hint }}</div>
          </article>
        </div>
      </div>
    </section>

    <section class="grid grid-cols-1 gap-4 xl:grid-cols-[1fr_1fr]">
      <div class="hero-panel p-5 lg:p-6">
        <div class="flex items-center justify-between gap-4">
          <div>
            <h2 class="text-xl font-semibold tracking-tight text-slate-900">单个发货</h2>
            <p class="mt-1 text-sm text-slate-500">适合处理临时补发、复核后手动修正的订单。</p>
          </div>
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
          <div>
            <h2 class="text-xl font-semibold tracking-tight text-slate-900">批量发货</h2>
            <p class="mt-1 text-sm text-slate-500">一行一单，确认后统一执行，并自动记录失败明细。</p>
          </div>
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
            :disabled="store.loading || licenseBlocked || parsedBatchItems.length === 0"
            class="action-btn action-btn-success w-full"
            @click="requestBatchDelivery"
          >
            {{ store.loading ? "批量发货中..." : "开始批量发货" }}
          </button>
        </div>
      </div>
    </section>

    <section v-if="progress" class="surface-panel p-5 lg:p-6">
      <div class="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
        <div class="min-w-0">
          <h3 class="text-lg font-semibold text-slate-900">批量发货进度</h3>
          <div class="mt-1 text-sm text-slate-500">
            <template v-if="progress.running">正在处理 {{ progress.processedCount }} / {{ progress.totalCount }} 条</template>
            <template v-else-if="progress.stopped">已停止于第 {{ progress.processedCount }} 条（{{ progress.cancelRequested ? '用户取消' : '服务端中止' }}）</template>
            <template v-else>已完成全部 {{ progress.totalCount }} 条</template>
          </div>
        </div>
        <div class="flex flex-wrap gap-2">
          <button
            v-if="progress.running"
            class="action-btn action-btn-secondary"
            :disabled="progress.cancelRequested"
            @click="handleCancelBatch"
          >
            {{ progress.cancelRequested ? "正在停止..." : "停止剩余条目" }}
          </button>
          <button
            v-if="!progress.running && failedSteps.length > 0"
            class="action-btn action-btn-secondary"
            @click="exportFailedCsv"
          >
            导出失败明细（CSV）
          </button>
          <button
            v-if="canRetryFailed"
            class="action-btn action-btn-primary"
            @click="retryConfirmOpen = true"
          >
            仅重试 {{ failedSteps.length }} 条失败
          </button>
          <button
            v-if="!progress.running"
            class="action-btn action-btn-secondary"
            @click="store.resetBatch"
          >
            清空结果
          </button>
        </div>
      </div>

      <div class="mt-4 h-2 overflow-hidden rounded-full bg-slate-100">
        <div
          class="h-full rounded-full transition-all duration-300"
          :class="progress.fatalError ? 'bg-red-400' : 'bg-brand'"
          :style="{ width: `${progressPercent}%` }"
        ></div>
      </div>

      <div class="mt-4 grid grid-cols-2 gap-3 text-center sm:grid-cols-4">
        <div class="rounded-2xl border border-slate-200 bg-slate-50 px-4 py-3">
          <div class="text-xs uppercase tracking-[0.16em] text-slate-400">总计</div>
          <div class="mt-1 text-lg font-semibold text-slate-800">{{ progress.totalCount }}</div>
        </div>
        <div class="rounded-2xl border border-brand-tint bg-brand-soft/60 px-4 py-3">
          <div class="text-xs uppercase tracking-[0.16em] text-brand-deep/70">成功</div>
          <div class="mt-1 text-lg font-semibold text-brand-deep">{{ progress.successCount }}</div>
        </div>
        <div class="rounded-2xl border border-red-200 bg-red-50 px-4 py-3">
          <div class="text-xs uppercase tracking-[0.16em] text-red-500">失败</div>
          <div class="mt-1 text-lg font-semibold text-red-700">{{ progress.failureCount }}</div>
        </div>
        <div class="rounded-2xl border border-slate-200 bg-slate-50 px-4 py-3">
          <div class="text-xs uppercase tracking-[0.16em] text-slate-400">已处理</div>
          <div class="mt-1 text-lg font-semibold text-slate-800">{{ progress.processedCount }} / {{ progress.totalCount }}</div>
        </div>
      </div>

      <div v-if="progress.fatalError" class="mt-4 soft-alert error">
        致命错误：{{ progress.fatalError }}
      </div>

      <div v-if="failedSteps.length > 0" class="mt-5">
        <div class="flex items-center justify-between">
          <h4 class="text-sm font-semibold text-slate-900">失败条目明细（{{ failedSteps.length }}）</h4>
          <div class="text-xs text-slate-500">支持导出 CSV 与仅重试失败</div>
        </div>
        <div class="mt-3 max-h-[260px] overflow-auto rounded-2xl border border-slate-200/80">
          <table class="w-full min-w-[600px] text-left text-xs">
            <thead class="bg-slate-50 text-slate-500">
              <tr>
                <th class="px-3 py-2 font-semibold">#</th>
                <th class="px-3 py-2 font-semibold">订单号</th>
                <th class="px-3 py-2 font-semibold">快递单号</th>
                <th class="px-3 py-2 font-semibold">错误信息</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="item in failedSteps" :key="item.index" class="border-t border-slate-100/80">
                <td class="px-3 py-2 font-mono text-slate-500">{{ item.index }}</td>
                <td class="px-3 py-2 font-mono text-slate-700">{{ item.orderId }}</td>
                <td class="px-3 py-2 font-mono text-slate-700">{{ item.trackingNumber }}</td>
                <td class="px-3 py-2 text-red-700">{{ item.errorMessage || "未知错误" }}</td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>

      <div v-if="recentSteps.length > 0" class="mt-5">
        <h4 class="text-sm font-semibold text-slate-900">最近处理（{{ recentSteps.length }} 条）</h4>
        <div class="mt-3 max-h-[200px] overflow-auto rounded-2xl border border-slate-200/80">
          <table class="w-full min-w-[540px] text-left text-xs">
            <thead class="bg-slate-50 text-slate-500">
              <tr>
                <th class="px-3 py-2 font-semibold">#</th>
                <th class="px-3 py-2 font-semibold">订单号</th>
                <th class="px-3 py-2 font-semibold">结果</th>
                <th class="px-3 py-2 font-semibold">备注</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="item in recentSteps" :key="`recent-${item.index}`" class="border-t border-slate-100/80">
                <td class="px-3 py-2 font-mono text-slate-500">{{ item.index }}</td>
                <td class="px-3 py-2 font-mono text-slate-700">{{ item.orderId }}</td>
                <td class="px-3 py-2">
                  <span
                    class="inline-flex rounded-full px-2.5 py-0.5 font-semibold"
                    :class="item.status === 'success' ? 'bg-brand-soft text-brand-deep' : 'bg-red-100 text-red-700'"
                  >
                    {{ item.status === "success" ? "成功" : "失败" }}
                  </span>
                </td>
                <td class="px-3 py-2 text-slate-500">
                  <template v-if="item.status === 'success'">
                    {{ item.oldWaybill ? `旧单号：${item.oldWaybill}` : "已更新物流" }}
                  </template>
                  <template v-else>{{ item.errorMessage || "未知错误" }}</template>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </section>

    <div v-if="licenseBlocked" class="soft-alert warn">
      当前未激活授权，发货功能不可用。请先前往设置中心完成激活。
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

    <ConfirmDialog
      :open="confirmOpen"
      title="确认执行批量发货"
      :message="confirmMessage"
      confirm-text="立即执行"
      cancel-text="再检查一下"
      @confirm="confirmBatchDelivery"
      @cancel="confirmOpen = false"
    />
    <ConfirmDialog
      :open="retryConfirmOpen"
      title="仅重试失败条目"
      :message="`将重新提交 ${failedSteps.length} 条失败订单，请确认已修正错误后再继续。`"
      confirm-text="开始重试"
      cancel-text="取消"
      @confirm="confirmRetryFailed"
      @cancel="retryConfirmOpen = false"
    />
  </div>
</template>
