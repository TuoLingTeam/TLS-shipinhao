<script setup lang="ts">
import { computed, ref } from "vue";
import { useRouter } from "vue-router";
import { useReview } from "../review/useReview";
import { useReviewStore } from "../review/review.store";
import { useOrderStore } from "../order/order.store";
import { useAppStore } from "../app.store";
import LoadingState from "../shared/LoadingState.vue";
import ReviewMatchStrategyBadge from "../review/ReviewMatchStrategyBadge.vue";
import { useLayout } from "../layout/useLayout";
import type { ReviewRangePresetKey } from "../shared/format";
import { getReviewRangeFromPreset } from "../shared/format";

const router = useRouter();
const { mode } = useLayout();
const store = useReviewStore();
const orderStore = useOrderStore();
const appStore = useAppStore();
const { findReviews, findQualityRefundOrders, prefillMatchedOrder } = useReview();

const rangePreset = ref<ReviewRangePresetKey>("last_30_days");
const rangePresetOptions: { value: ReviewRangePresetKey; label: string }[] = [
  { value: "today", label: "今天" },
  { value: "yesterday", label: "昨天" },
  { value: "last_7_days", label: "近 7 天" },
  { value: "last_30_days", label: "近 30 天" },
];
const licenseBlocked = computed(() => !appStore.isLicensed);
const isQualityRefundMode = computed(() => store.lastMode === "quality_refund");
const isCompactLayout = computed(() => ["compact", "high_dpi_compact"].includes(mode.value));

const loadingTitle = computed(() =>
  orderStore.syncSource === "review_query"
    ? "正在准备订单缓存并执行评分匹配"
    : isQualityRefundMode.value
      ? "正在获取品退订单并匹配缓存订单"
      : "正在获取差评并执行订单评分匹配",
);
const loadingDescription = computed(() =>
  orderStore.syncSource === "review_query"
    ? orderStore.syncMessage || "后端会先按所选日期范围保障订单缓存可用，再执行评分匹配。"
    : isQualityRefundMode.value
      ? "品退接口会直接返回订单号，成功后可直接带入发货页。"
      : "差评会先确保缓存覆盖，再按商品、SKU、昵称与时间执行评分匹配。",
);
const idColumnLabel = computed(() => (isQualityRefundMode.value ? "订单号" : "评价ID"));
const tableEmptyColspan = computed(() => (isQualityRefundMode.value ? 4 : 5));
const emptyTableHint = computed(() => {
  if (store.lastQuery) {
    return isQualityRefundMode.value
      ? "本次查询暂无品退订单，可调整日期范围后重试。"
      : "本次查询暂无差评结果，可调整日期范围后重试。";
  }
  return "暂无数据。请选择日期后点击「获取差评」或「获取品退」。";
});

function buildReviewWindow(): { days: number; startAt: string; endAt: string } {
  store.setError(null);
  const r = getReviewRangeFromPreset(rangePreset.value);
  return { days: r.days, startAt: r.startAt, endAt: r.endAt };
}

async function handleSearch() {
  if (licenseBlocked.value) {
    store.setError("请先激活授权后再使用评价管理");
    return;
  }
  const w = buildReviewWindow();
  await findReviews(w.days, w.startAt, w.endAt);
}

async function handleQualityRefundSearch() {
  if (licenseBlocked.value) {
    store.setError("请先激活授权后再使用品退订单");
    return;
  }
  const w = buildReviewWindow();
  await findQualityRefundOrders(w.days, w.startAt, w.endAt);
}

function handleUseMatchedOrder(orderId: string) {
  if (!orderId.trim()) return;
  prefillMatchedOrder(orderId, isQualityRefundMode.value ? "品退匹配" : "评价匹配");
  void router.push("/delivery");
}

function unmatchedReason(record: {
  order_id: string;
  candidate_count: number;
  top_score: number;
}) {
  if (isQualityRefundMode.value) {
    if (!record.order_id.trim()) {
      return "品退接口未返回订单号，暂时无法自动带入发货。";
    }
    return "接口已返回订单号，但当前结果未能自动带入，请重试。";
  }
  if (record.candidate_count === 0) {
    return "已完成评分匹配，但当前缓存覆盖范围内没有找到同商品/SKU候选订单。";
  }
  return `已找到 ${record.candidate_count} 个候选订单，最高得分 ${record.top_score}，未达到自动匹配阈值。`;
}

function displayId(record: { evaluation_id: string; order_id: string }) {
  return isQualityRefundMode.value ? record.order_id || record.evaluation_id : record.evaluation_id;
}

function matchedHint(record: { confidence_score: number; strategy: string }) {
  if (isQualityRefundMode.value) {
    return "官方已返回订单号 · 点击上方策略徽章可查看命中说明，点击本行可自动带入发货页。";
  }
  if (record.strategy === "exact_match") {
    return "评分 100 · 已加入自动发货候选，点击上方策略徽章可查看说明。";
  }
  if (record.strategy === "high_confidence") {
    return `评分 ${record.confidence_score} · 达到高置信阈值，建议复核后带入发货页。`;
  }
  if (record.strategy === "probable_match") {
    return `评分 ${record.confidence_score} · 可能匹配，建议人工核对后再带入。`;
  }
  return `评分 ${record.confidence_score} · 当前结果仅供参考。`;
}

function formatReplyDeadline(value: string | null) {
  if (!value) return "未知";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}
</script>

<template>
  <div class="review-match-view flex min-h-0 flex-1 flex-col gap-app">
    <section
      data-testid="review-control-shell"
      class="hero-panel subsystem-hero review-config-panel relative overflow-hidden p-3 lg:p-3.5"
    >
      <div class="pointer-events-none absolute -right-20 -top-16 h-40 w-40 rounded-full bg-[radial-gradient(circle,rgba(167,243,208,0.4),transparent_72%)]"></div>

      <div class="config-panel-eyebrow relative z-[1]">
        <span class="config-panel-eyebrow-dot" aria-hidden="true"></span>
        <span class="config-panel-eyebrow-label">评价查询</span>
      </div>

      <div
        data-testid="review-config-actions"
        class="relative z-[1] flex w-full min-w-0 flex-col gap-3 sm:flex-row sm:items-stretch sm:gap-3"
      >
        <label class="field-affix field-affix--leading flex-1" for="review-range-preset">
          <svg class="field-affix-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <rect x="3" y="5" width="18" height="16" rx="3" />
            <path d="M3 10h18" />
            <path d="M8 3v4" />
            <path d="M16 3v4" />
          </svg>
          <select
            id="review-range-preset"
            v-model="rangePreset"
            data-testid="review-range-preset"
            aria-label="选择日期范围"
            class="field-input field-input--with-leading-icon box-border min-h-[40px] min-w-0 w-full"
          >
            <option v-for="opt in rangePresetOptions" :key="opt.value" :value="opt.value">
              {{ opt.label }}
            </option>
          </select>
        </label>
        <button
          data-testid="review-fetch-bad"
          type="button"
          :disabled="store.loading || licenseBlocked"
          class="action-btn action-btn-primary box-border min-h-[40px] min-w-0 flex-1 cursor-pointer"
          @click="handleSearch"
        >
          {{ store.loading && !isQualityRefundMode ? "处理中..." : "获取差评" }}
        </button>
        <button
          data-testid="review-fetch-quality"
          type="button"
          :disabled="store.loading || licenseBlocked"
          class="action-btn action-btn-secondary box-border min-h-[40px] min-w-0 flex-1 cursor-pointer border border-slate-200/90"
          @click="handleQualityRefundSearch"
        >
          {{ store.loading && isQualityRefundMode ? "处理中..." : "获取品退" }}
        </button>
      </div>
    </section>

    <div v-if="licenseBlocked" class="soft-alert warn">
      当前未激活授权，评价管理不可用。请先前往设置中心完成激活。
    </div>

    <div v-if="store.error" class="soft-alert error">
      {{ store.error }}
    </div>

    <div
      v-if="store.loading"
      class="flex min-h-0 min-w-0 flex-1 flex-col gap-app"
    >
      <LoadingState
        class="min-h-0 flex-1"
        :title="loadingTitle"
        :description="loadingDescription"
      />
      <div
        v-if="orderStore.syncSource === 'review_query'"
        class="surface-panel shrink-0 space-y-app px-4 py-4"
      >
        <div class="flex items-center justify-between text-sm">
          <span class="font-semibold text-slate-800">自动同步进度</span>
          <span class="font-mono text-slate-500">{{ orderStore.syncProgress }}%</span>
        </div>
        <div class="h-2 overflow-hidden rounded-full bg-slate-100">
          <div
            class="h-full rounded-full bg-brand transition-all duration-300"
            :style="{ width: `${orderStore.syncProgress}%` }"
          ></div>
        </div>
        <div
          class="grid grid-cols-1 gap-app text-xs text-slate-500"
          :class="isCompactLayout ? 'sm:grid-cols-1' : 'md:grid-cols-3'"
        >
          <div class="rounded-[16px] border border-slate-200/80 px-3.5 py-3">
            <div class="font-semibold text-slate-700">1. 缓存保障</div>
            <div class="mt-1">{{ ['ensure_recent_cache', 'match_reviews', 'completed'].includes(orderStore.syncPhase || '') ? '进行中/完成' : '等待中' }}</div>
          </div>
          <div class="rounded-[16px] border border-slate-200/80 px-3.5 py-3">
            <div class="font-semibold text-slate-700">2. 评分匹配</div>
            <div class="mt-1">
              {{ ['match_reviews', 'completed'].includes(orderStore.syncPhase || '') ? '进行中/完成' : '等待中' }}
            </div>
          </div>
          <div class="rounded-[16px] border border-slate-200/80 px-3.5 py-3">
            <div class="font-semibold text-slate-700">3. 完成结果</div>
            <div class="mt-1">
              {{ orderStore.syncPhase === 'completed' ? '已完成' : '等待中' }}
            </div>
          </div>
        </div>
      </div>
    </div>

    <div v-else-if="!store.loading" class="flex min-h-0 min-w-0 flex-1 flex-col gap-app">
      <div v-if="store.cacheWarnings.length && !isQualityRefundMode" class="shrink-0 soft-alert warn">
        {{ store.cacheWarnings.join("；") }}
      </div>

      <section class="data-table-shell flex min-h-0 min-w-0 flex-1 flex-col overflow-x-auto overflow-y-auto">
        <table class="w-full text-sm" :class="isCompactLayout ? 'min-w-[820px]' : 'min-w-[980px]'">
          <thead class="table-head text-slate-600">
            <tr>
              <th
                v-if="!isQualityRefundMode"
                class="table-head-sticky px-3 py-2.5 text-left text-xs font-semibold sm:px-5 sm:py-3 sm:text-sm"
              >
                买家昵称
              </th>
              <th
                v-if="!isQualityRefundMode"
                class="table-head-sticky px-3 py-2.5 text-left text-xs font-semibold sm:px-5 sm:py-3 sm:text-sm"
              >
                评价内容
              </th>
              <th v-if="isQualityRefundMode" class="table-head-sticky px-3 py-2.5 text-left text-xs font-semibold sm:px-5 sm:py-3 sm:text-sm">品退原因</th>
              <th class="table-head-sticky px-3 py-2.5 text-left text-xs font-semibold sm:px-5 sm:py-3 sm:text-sm">订单详情</th>
              <th class="table-head-sticky px-3 py-2.5 text-left text-xs font-semibold sm:px-5 sm:py-3 sm:text-sm">{{ idColumnLabel }}</th>
              <th class="table-head-sticky px-3 py-2.5 text-center text-xs font-semibold sm:px-5 sm:py-3 sm:text-sm">匹配</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="r in store.results"
              :key="r.evaluation_id"
              class="table-row border-t border-slate-100/80 align-top transition-colors"
              :class="[
                r.matched && r.order_id ? 'cursor-pointer' : '',
                !isQualityRefundMode && !r.replyable ? 'bg-slate-50/80 text-slate-400' : '',
              ]"
              @click="r.matched && r.order_id ? handleUseMatchedOrder(r.order_id) : undefined"
            >
              <td v-if="!isQualityRefundMode" class="px-3 py-2.5 sm:px-5 sm:py-3">
                <div class="font-semibold text-slate-800">{{ r.buyer_nickname || "-" }}</div>
              </td>
              <td v-if="!isQualityRefundMode" class="max-w-md px-3 py-2.5 sm:px-5 sm:py-3">
                <div class="whitespace-pre-wrap break-all text-sm leading-6 text-slate-700 sm:text-[14.5px] sm:leading-[1.6]">
                  {{ r.evaluation_content || "（无评价内容）" }}
                </div>
              </td>
              <td v-if="isQualityRefundMode" class="px-3 py-2.5 sm:px-5 sm:py-3">
                <div class="text-sm leading-6 text-slate-700">
                  {{ r.quality_refund_info?.reason || "—" }}
                </div>
              </td>
              <td class="px-3 py-2.5 sm:px-5 sm:py-3">
                <div class="space-y-1 text-xs leading-5 text-slate-600">
                  <div><span class="font-semibold text-slate-700">SKU：</span>{{ r.sku_name || r.sku_id || "-" }}</div>
                  <div>
                    <span class="font-semibold text-slate-700">商品ID：</span>
                    <span class="font-mono">{{ r.product_id || "-" }}</span>
                  </div>
                  <div v-if="r.product_name" class="text-slate-500">{{ r.product_name }}</div>
                </div>
              </td>
              <td class="px-3 py-2.5 font-mono text-xs text-slate-700 sm:px-5 sm:py-3">{{ displayId(r) }}</td>
              <td class="px-3 py-2.5 text-center sm:px-5 sm:py-3">
                <div class="flex flex-col items-center gap-1.5">
                  <span
                    class="inline-flex rounded-full px-3 py-1 text-xs font-semibold"
                    :class="r.matched ? 'bg-brand-soft text-brand-deep' : 'bg-slate-100 text-slate-500'"
                  >
                    {{ r.matched ? "已匹配" : "未匹配" }}
                  </span>
                  <ReviewMatchStrategyBadge
                    :strategy="r.strategy"
                    :reasons="r.match_reasons"
                    :candidate-count="r.candidate_count"
                    :top-score="r.top_score"
                  />
                  <span
                    v-if="!isQualityRefundMode && !r.replyable"
                    class="inline-flex rounded-full bg-slate-200 px-3 py-1 text-[11px] font-semibold text-slate-600"
                    :title="`回复截止：${formatReplyDeadline(r.reply_deadline)}`"
                  >
                    已超期
                  </span>
                  <div
                    class="max-w-[180px] text-center text-[11px] leading-5"
                    :class="r.matched ? 'text-slate-500' : 'text-amber-700'"
                  >
                    {{ r.matched ? matchedHint(r) : unmatchedReason(r) }}
                  </div>
                </div>
              </td>
            </tr>
            <tr v-if="store.results.length === 0" class="table-row border-t border-slate-100/80">
              <td
                class="px-4 py-14 text-center text-sm leading-6 text-slate-500 sm:px-6"
                :colspan="tableEmptyColspan"
              >
                {{ emptyTableHint }}
              </td>
            </tr>
          </tbody>
        </table>
      </section>
    </div>
  </div>
</template>
