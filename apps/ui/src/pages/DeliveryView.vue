<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useDelivery } from "@/services/delivery";
import { useDeliveryStore } from "@/stores/delivery";
import { useAppStore } from "@/stores/app";
import ConfirmDialog from "../shared/ConfirmDialog.vue";
import { useNotification } from "../shared/useNotification";

const store = useDeliveryStore();
const appStore = useAppStore();
const { batchDelivery, cancelBatchDelivery, retryFailedItems, exportFailedCsv } = useDelivery();
const { show: showToast } = useNotification();
const licenseBlocked = computed(() => !appStore.isLicensed);

const orderIdsText = ref("");
const trackingNumbersText = ref("");
const confirmOpen = ref(false);
const retryConfirmOpen = ref(false);
const activeProgressTab = ref<"recent" | "failed">("recent");

function splitLines(raw: string): string[] {
  return raw.split("\n").map((line) => line.trim()).filter(Boolean);
}

watch(
  () => store.draftOrderId,
  (value) => {
    const incoming = splitLines(value ?? "");
    if (incoming.length === 0) return;
    const existing = splitLines(orderIdsText.value);
    const existingSet = new Set(existing);
    const incomingNew = incoming.filter((id) => !existingSet.has(id));
    const merged = [...incomingNew, ...existing];
    orderIdsText.value = merged.join("\n");
  },
  { immediate: true },
);

const orderIdLines = computed(() => splitLines(orderIdsText.value));
const trackingNumberLines = computed(() => splitLines(trackingNumbersText.value));
const pairCount = computed(() => Math.min(orderIdLines.value.length, trackingNumberLines.value.length));
const parsedBatchItems = computed(() => {
  const pairs: { order_id: string; tracking_number: string }[] = [];
  for (let i = 0; i < pairCount.value; i++) {
    const oid = orderIdLines.value[i];
    const tn = trackingNumberLines.value[i];
    if (oid && tn) pairs.push({ order_id: oid, tracking_number: tn });
  }
  return pairs;
});

const lineCountMismatch = computed(
  () => orderIdLines.value.length !== trackingNumberLines.value.length,
);

const progress = computed(() => store.batchProgress);
const progressPercent = computed(() => {
  const info = progress.value;
  if (!info || info.totalCount === 0) return 0;
  return Math.min(100, Math.round((info.processedCount / info.totalCount) * 100));
});
const failedSteps = computed(() => progress.value?.steps.filter((step) => step.status === "failed") ?? []);
const retryableFailedSteps = computed(() => failedSteps.value.filter((step) => step.retryable));
const recentSteps = computed(() => {
  const steps = progress.value?.steps ?? [];
  return steps.slice(-20).reverse();
});
const canRetryFailed = computed(() =>
  Boolean(progress.value && !progress.value.running && retryableFailedSteps.value.length > 0),
);
const confirmMessage = computed(() => {
  const count = parsedBatchItems.value.length;
  const eta = Math.max(1, Math.ceil(count * 0.5));
  return `即将提交 ${count} 条批量修改物流，预计耗时约 ${eta} 秒。提交后将无法撤销已发送至小店的条目，请确认无误。`;
});

function requestBatchDelivery() {
  if (licenseBlocked.value) {
    store.error = "请先激活授权后再使用发货功能";
    showToast(store.error, "error");
    return;
  }
  if (parsedBatchItems.value.length === 0) {
    showToast("请先填写订单号和快递单号", "error");
    return;
  }
  if (lineCountMismatch.value) {
    showToast(`订单与快递行数不一致，本次将提交前 ${parsedBatchItems.value.length} 条`, "info");
  }
  confirmOpen.value = true;
}

async function confirmBatchDelivery() {
  confirmOpen.value = false;
  await batchDelivery(parsedBatchItems.value);
  if (store.error) {
    showToast(store.error, "error");
    return;
  }
  const current = store.batchProgress;
  if (!current) return;
  showToast(
    `批量修改物流完成：成功 ${current.successCount} 条，失败 ${current.failureCount} 条`,
    current.failureCount > 0 || current.fatalError ? "error" : "success",
  );
}

async function confirmRetryFailed() {
  retryConfirmOpen.value = false;
  await retryFailedItems();
  if (store.error) {
    showToast(store.error, "error");
    return;
  }
  const current = store.batchProgress;
  if (!current) return;
  showToast(
    `重试完成：成功 ${current.successCount} 条，失败 ${current.failureCount} 条`,
    current.failureCount > 0 || current.fatalError ? "error" : "success",
  );
}

async function handleCancelBatch() {
  await cancelBatchDelivery();
  showToast(store.error ? store.error : "已请求停止批量修改物流", store.error ? "error" : "info");
}

function handleClearInputs() {
  orderIdsText.value = "";
  trackingNumbersText.value = "";
  store.clearPrefillOrder();
  showToast("发货输入已清空", "info");
}

function handleExportFailedCsv() {
  exportFailedCsv();
  showToast(
    failedSteps.value.length > 0 ? "失败明细 CSV 已导出" : "暂无失败条目可导出",
    failedSteps.value.length > 0 ? "success" : "info",
  );
}
</script>

<template>
  <div class="delivery-view flex h-full min-h-0 flex-col">
    <div v-if="licenseBlocked" class="soft-alert warn delivery-license-banner text-xs">
      当前未激活授权，发货功能不可用，请先前往「设置中心」完成激活。
    </div>

    <section class="delivery-main-grid grid flex-1 min-h-0 gap-app lg:grid-cols-[6fr_7fr]">
      <article class="surface-panel delivery-input-panel p-3 lg:p-4 flex flex-col min-h-0">
        <div class="subsystem-section-header">
          <div class="min-w-0">
            <h3 class="text-base font-semibold tracking-tight text-slate-900">批量修改物流</h3>
          </div>
          <div v-if="lineCountMismatch" class="subsystem-chipbar">
            <span class="subsystem-chip subsystem-chip--warn">
              订单 {{ orderIdLines.length }} · 快递 {{ trackingNumberLines.length }}
            </span>
          </div>
        </div>

        <div
          data-testid="delivery-workspace"
          class="delivery-workspace mt-app grid flex-1 min-h-0 grid-cols-1 gap-app md:grid-cols-2"
        >
          <div class="delivery-input-column">
            <label class="field-label delivery-field-label">
              <span class="delivery-field-label-title">订单号</span>
              <span
                class="delivery-field-count"
                :class="{ 'delivery-field-count--active': orderIdLines.length > 0 }"
              >
                <span class="delivery-field-count-dot" aria-hidden="true"></span>
                <span class="delivery-field-count-num">{{ orderIdLines.length }}</span>
                <span class="delivery-field-count-unit">行</span>
              </span>
            </label>
            <textarea
              v-model="orderIdsText"
              class="field-textarea delivery-input-textarea font-mono text-sm"
              placeholder="3735560095122745088&#10;3735560095122745089&#10;3735560095122745090"
            />
          </div>

          <div class="delivery-input-column">
            <label class="field-label delivery-field-label">
              <span class="delivery-field-label-title">快递单号</span>
              <span
                class="delivery-field-count"
                :class="{ 'delivery-field-count--active': trackingNumberLines.length > 0 }"
              >
                <span class="delivery-field-count-dot" aria-hidden="true"></span>
                <span class="delivery-field-count-num">{{ trackingNumberLines.length }}</span>
                <span class="delivery-field-count-unit">行</span>
              </span>
            </label>
            <textarea
              v-model="trackingNumbersText"
              class="field-textarea delivery-input-textarea font-mono text-sm"
              placeholder="JT00000001&#10;JT00000002&#10;JT00000003"
            />
          </div>
        </div>

        <div class="delivery-action-bar">
          <button
            type="button"
            class="action-btn action-btn-secondary"
            :disabled="store.loading || (!orderIdsText && !trackingNumbersText) || progress?.running"
            @click="handleClearInputs"
          >
            清空输入
          </button>
          <button
            v-if="!progress?.running"
            :disabled="store.loading || licenseBlocked || parsedBatchItems.length === 0"
            class="action-btn action-btn-success flex-1"
            @click="requestBatchDelivery"
          >
            {{ store.loading ? "批量修改物流中..." : `开始批量修改物流（${parsedBatchItems.length} 条）` }}
          </button>
          <button
            v-else
            type="button"
            class="delivery-cancel-btn flex-1"
            :disabled="progress.cancelRequested"
            :class="{ 'is-pending': progress.cancelRequested }"
            @click="handleCancelBatch"
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="h-3.5 w-3.5">
              <rect x="6" y="6" width="12" height="12" rx="2" />
            </svg>
            {{
              progress.cancelRequested
                ? "停止中，将在当前条目完成后终止…"
                : `停止批量修改物流（${progress.processedCount}/${progress.totalCount}）`
            }}
          </button>
        </div>

        <p v-if="store.error" class="soft-alert error mt-2 py-1 text-xs">
          {{ store.error }}
        </p>
      </article>

      <article class="surface-panel delivery-progress-panel p-3 lg:p-4 flex flex-col min-h-0">
        <template v-if="progress">
          <div class="subsystem-section-header">
            <div class="min-w-0">
              <h3 class="text-base font-semibold text-slate-900">批量修改物流进度</h3>
              <div class="mt-0.5 text-xs text-slate-500">
                <template v-if="progress.running">正在处理 {{ progress.processedCount }} / {{ progress.totalCount }} 条</template>
                <template v-else-if="progress.stopped">已停止于第 {{ progress.processedCount }} 条（{{ progress.cancelRequested ? '用户取消' : '服务端中止' }}）</template>
                <template v-else>已完成全部 {{ progress.totalCount }} 条</template>
              </div>
            </div>
            <div class="subsystem-chipbar">
              <button
                v-if="!progress.running && failedSteps.length > 0"
                class="action-btn action-btn-secondary action-btn-compact"
                @click="handleExportFailedCsv"
              >
                导出 CSV
              </button>
              <button
                v-if="canRetryFailed"
                class="action-btn action-btn-primary action-btn-compact"
                @click="retryConfirmOpen = true"
              >
                重试 {{ retryableFailedSteps.length }} 条
              </button>
              <button
                v-if="!progress.running"
                class="action-btn action-btn-secondary action-btn-compact"
                @click="store.resetBatch"
              >
                清空结果
              </button>
            </div>
          </div>

          <div v-if="progress.running" class="delivery-cancel-banner">
            <div class="delivery-cancel-status">
              <span class="delivery-cancel-pulse" aria-hidden="true"></span>
              <span class="delivery-cancel-text">
                <strong class="text-slate-900">批量修改物流进行中</strong>
                <span class="text-slate-500">已处理 {{ progress.processedCount }} / {{ progress.totalCount }} 条</span>
              </span>
            </div>
            <button
              type="button"
              class="delivery-cancel-btn"
              :disabled="progress.cancelRequested"
              :class="{ 'is-pending': progress.cancelRequested }"
              @click="handleCancelBatch"
            >
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="h-3.5 w-3.5">
                <rect x="6" y="6" width="12" height="12" rx="2" />
              </svg>
              {{ progress.cancelRequested ? "停止中，将在当前条目完成后终止…" : "停止批量修改物流" }}
            </button>
          </div>

          <div class="mt-app h-1.5 overflow-hidden rounded-full bg-slate-100">
            <div
              class="h-full rounded-full transition-all duration-300"
              :class="progress.fatalError ? 'bg-red-400' : 'bg-brand'"
              :style="{ width: `${progressPercent}%` }"
            ></div>
          </div>

          <div class="delivery-progress-stats mt-app grid grid-cols-2 gap-app sm:grid-cols-4">
            <div class="delivery-stat">
              <div class="delivery-stat-label">总计</div>
              <div class="delivery-stat-value">{{ progress.totalCount }}</div>
            </div>
            <div class="delivery-stat">
              <div class="delivery-stat-label">成功</div>
              <div class="delivery-stat-value text-brand-deep">{{ progress.successCount }}</div>
            </div>
            <div class="delivery-stat">
              <div class="delivery-stat-label">失败</div>
              <div class="delivery-stat-value text-red-700">{{ progress.failureCount }}</div>
            </div>
            <div class="delivery-stat">
              <div class="delivery-stat-label">已处理</div>
              <div class="delivery-stat-value">{{ progress.processedCount }}/{{ progress.totalCount }}</div>
            </div>
          </div>

          <p v-if="progress.fatalError" class="soft-alert error mt-app py-1 text-xs">
            致命错误：{{ progress.fatalError }}
          </p>

          <div
            v-if="failedSteps.length > 0 || recentSteps.length > 0"
            class="delivery-progress-detail mt-app flex flex-1 min-h-0 flex-col"
          >
            <div class="delivery-tab-bar">
              <button
                type="button"
                class="delivery-tab"
                :class="{ 'delivery-tab--active': activeProgressTab === 'recent' }"
                @click="activeProgressTab = 'recent'"
              >
                最近处理 <span class="ml-1 text-[10px] text-slate-400">{{ recentSteps.length }}</span>
              </button>
              <button
                type="button"
                class="delivery-tab"
                :class="{ 'delivery-tab--active': activeProgressTab === 'failed' }"
                @click="activeProgressTab = 'failed'"
              >
                失败条目 <span class="ml-1 text-[10px] text-red-500">{{ failedSteps.length }}</span>
              </button>
            </div>

            <div class="delivery-detail-body flex-1 min-h-0 overflow-auto rounded-[14px] border border-slate-200/80 bg-white">
              <table v-if="activeProgressTab === 'failed'" class="w-full text-left text-xs">
                <thead class="sticky top-0 bg-slate-50 text-slate-500">
                  <tr>
                    <th class="px-2.5 py-2 font-semibold">#</th>
                    <th class="px-2.5 py-2 font-semibold">订单号</th>
                    <th class="px-2.5 py-2 font-semibold">快递单号</th>
                    <th class="px-2.5 py-2 font-semibold">错误</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-if="failedSteps.length === 0">
                    <td colspan="4" class="px-3 py-6 text-center text-slate-400">暂无失败条目</td>
                  </tr>
                  <tr v-for="item in failedSteps" :key="`failed-${item.index}`" class="border-t border-slate-100/80">
                    <td class="px-2.5 py-1.5 font-mono text-slate-500">{{ item.index }}</td>
                    <td class="px-2.5 py-1.5 font-mono text-slate-700">{{ item.orderId }}</td>
                    <td class="px-2.5 py-1.5 font-mono text-slate-700">{{ item.trackingNumber }}</td>
                    <td class="px-2.5 py-1.5 text-red-700">
                      <div class="flex items-center gap-1.5">
                        <span>{{ item.errorMessage || "未知错误" }}</span>
                        <span v-if="!item.retryable" class="rounded-full bg-slate-100 px-1.5 py-0.5 text-[10px] font-semibold text-slate-500">
                          不可重试
                        </span>
                      </div>
                    </td>
                  </tr>
                </tbody>
              </table>

              <table v-else class="w-full text-left text-xs">
                <thead class="sticky top-0 bg-slate-50 text-slate-500">
                  <tr>
                    <th class="px-2.5 py-2 font-semibold">#</th>
                    <th class="px-2.5 py-2 font-semibold">订单号</th>
                    <th class="px-2.5 py-2 font-semibold">结果</th>
                    <th class="px-2.5 py-2 font-semibold">备注</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-if="recentSteps.length === 0">
                    <td colspan="4" class="px-3 py-6 text-center text-slate-400">暂无记录</td>
                  </tr>
                  <tr v-for="item in recentSteps" :key="`recent-${item.index}`" class="border-t border-slate-100/80">
                    <td class="px-2.5 py-1.5 font-mono text-slate-500">{{ item.index }}</td>
                    <td class="px-2.5 py-1.5 font-mono text-slate-700">{{ item.orderId }}</td>
                    <td class="px-2.5 py-1.5">
                      <span
                        class="inline-flex rounded-full px-2 py-0.5 font-semibold"
                        :class="item.status === 'success' ? 'bg-brand-soft text-brand-deep' : 'bg-red-100 text-red-700'"
                      >
                        {{ item.status === "success" ? "成功" : "失败" }}
                      </span>
                    </td>
                    <td class="px-2.5 py-1.5 text-slate-500">
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
        </template>

        <template v-else>
          <div class="delivery-progress-placeholder flex-1 min-h-0 flex flex-col gap-3">
            <div class="flex items-start gap-3">
              <div class="delivery-placeholder-icon shrink-0">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                  <path d="M3 7h13l3 4v6a1 1 0 0 1-1 1h-2" />
                  <path d="M3 17h12" />
                  <circle cx="7" cy="17" r="2" />
                  <circle cx="17" cy="17" r="2" />
                </svg>
              </div>
              <div class="min-w-0">
                <h3 class="text-[15px] font-semibold tracking-tight text-slate-800">等待执行批量修改物流</h3>
                <p class="mt-1 text-[12px] leading-5 text-slate-500">
                  完成左侧两列粘贴后点击「开始批量修改物流」，此处会实时展示进度与结果。
                </p>
              </div>
            </div>

            <div class="delivery-placeholder-steps">
              <div class="delivery-placeholder-step">
                <span class="delivery-placeholder-step-index">1</span>
                <div class="min-w-0">
                  <div class="delivery-placeholder-step-title">粘贴订单号</div>
                  <p class="delivery-placeholder-step-copy">可从评价匹配或订单检索整列复制到左上输入框。</p>
                </div>
              </div>
              <div class="delivery-placeholder-step">
                <span class="delivery-placeholder-step-index">2</span>
                <div class="min-w-0">
                  <div class="delivery-placeholder-step-title">粘贴快递单号</div>
                  <p class="delivery-placeholder-step-copy">每行一个快递单号，按行号与左侧订单号顺序配对。</p>
                </div>
              </div>
              <div class="delivery-placeholder-step">
                <span class="delivery-placeholder-step-index">3</span>
                <div class="min-w-0">
                  <div class="delivery-placeholder-step-title">开始批量</div>
                  <p class="delivery-placeholder-step-copy">逐条实时进度、失败明细可导出，可恢复失败支持一键重试。</p>
                </div>
              </div>
            </div>

            <div class="delivery-placeholder-tips">
              <span class="delivery-placeholder-tip">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="h-3 w-3">
                  <path d="M12 2v2" /><path d="M12 20v2" /><path d="m4.93 4.93 1.41 1.41" /><path d="m17.66 17.66 1.41 1.41" /><path d="M2 12h2" /><path d="M20 12h2" /><path d="m6.34 17.66-1.41 1.41" /><path d="m19.07 4.93-1.41 1.41" /><circle cx="12" cy="12" r="4" />
                </svg>
                支持从剪贴板直接粘贴多行数据
              </span>
              <span class="delivery-placeholder-tip">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="h-3 w-3">
                  <circle cx="12" cy="12" r="9" /><path d="M12 7v5l3 3" />
                </svg>
                预计每条耗时约 0.5 秒
              </span>
            </div>
          </div>
        </template>
      </article>
    </section>

    <ConfirmDialog
      :open="confirmOpen"
      title="确认执行批量修改物流"
      :message="confirmMessage"
      confirm-text="立即执行"
      cancel-text="再检查一下"
      @confirm="confirmBatchDelivery"
      @cancel="confirmOpen = false"
    />
    <ConfirmDialog
      :open="retryConfirmOpen"
      title="仅重试失败条目"
      :message="`将重新提交 ${retryableFailedSteps.length} 条可重试失败订单，请确认已修正错误后再继续。`"
      confirm-text="开始重试"
      cancel-text="取消"
      @confirm="confirmRetryFailed"
      @cancel="retryConfirmOpen = false"
    />
  </div>
</template>
