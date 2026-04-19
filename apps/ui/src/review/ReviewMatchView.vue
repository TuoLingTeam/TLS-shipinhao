<script setup lang="ts">
import { computed, ref } from "vue";
import { useRouter } from "vue-router";
import { useReview } from "../review/useReview";
import { useReviewStore } from "../review/review.store";
import { useOrderStore } from "../order/order.store";
import { useAppStore } from "../app.store";
import EmptyState from "../shared/EmptyState.vue";
import LoadingState from "../shared/LoadingState.vue";
import ReviewMatchStrategyBadge from "../review/ReviewMatchStrategyBadge.vue";
import { useLayout } from "../layout/useLayout";
import { localDaysAgoStartIso, localYesterdayEndIso } from "../shared/format";

const router = useRouter();
const { mode } = useLayout();
const store = useReviewStore();
const orderStore = useOrderStore();
const appStore = useAppStore();
const { findReviews, findQualityRefundOrders, prefillMatchedOrder } = useReview();

const days = ref(30);
const qualityReasonFilter = ref("");
const licenseBlocked = computed(() => !appStore.isLicensed);
const isQualityRefundMode = computed(() => store.lastMode === "quality_refund");
const filteredResults = computed(() => {
  if (!isQualityRefundMode.value) return store.results;
  const keyword = qualityReasonFilter.value.trim();
  if (!keyword) return store.results;
  return store.results.filter((item) =>
    item.quality_refund_info?.reason?.includes(keyword),
  );
});
const matchedCount = computed(() => store.results.filter((item) => item.matched).length);
const isCompactLayout = computed(() => ["compact", "high_dpi_compact"].includes(mode.value));
const unmatchedCount = computed(() => store.results.length - matchedCount.value);

const heroEyebrow = computed(() =>
  isQualityRefundMode.value ? "TLS · QUALITY REFUND" : "TLS · BAD REVIEW",
);

const heroTitle = computed(() =>
  isQualityRefundMode.value ? "品退订单直连" : "差评评分匹配",
);

const heroLead = computed(() =>
  isQualityRefundMode.value
    ? "品退接口直接返回订单号，匹配成功即可一键带入发货。"
    : "先补齐缓存，再按商品 / SKU / 昵称 / 时间多维度评分匹配。",
);

const summaryCards = computed(() => [
  {
    label: "当前模式",
    value: isQualityRefundMode.value ? "品退直连" : "差评评分匹配",
    hint: isQualityRefundMode.value ? "优先使用接口直返订单号" : "依赖缓存评分寻找最优订单",
    tone: "brand",
  },
  {
    label: "最近结果",
    value: store.results.length ? `${store.results.length} 条` : "等待查询",
    hint: store.lastQuery ? `${store.lastQuery.days} 天范围内的数据` : "尚未发起检索",
    tone: "slate",
  },
  {
    label: "已匹配",
    value: store.results.length ? `${matchedCount.value} / ${store.results.length}` : "--",
    hint: unmatchedCount.value > 0 ? `${unmatchedCount.value} 条待人工核实` : "命中后可直接带入发货",
    tone: unmatchedCount.value > 0 && store.results.length ? "amber" : "success",
  },
]);

const summaryCardAccent: Record<string, string> = {
  brand: "review-summary-card--brand",
  slate: "review-summary-card--slate",
  amber: "review-summary-card--amber",
  success: "review-summary-card--success",
};

const loadingTitle = computed(() =>
  orderStore.syncSource === "review_query"
    ? "正在准备订单缓存并执行评分匹配"
    : isQualityRefundMode.value
      ? "正在获取品退订单并匹配缓存订单"
      : "正在获取差评并执行订单评分匹配",
);
const loadingDescription = computed(() =>
  orderStore.syncSource === "review_query"
    ? orderStore.syncMessage || "后端会先保障近 30 天（不含今天）订单缓存可用，再执行评分匹配。"
    : isQualityRefundMode.value
      ? "品退接口会直接返回订单号，成功后可直接带入发货页。"
      : "差评会先确保缓存覆盖，再按商品、SKU、昵称与时间执行评分匹配。",
);
const emptyTitle = computed(() => (isQualityRefundMode.value ? "未找到品退匹配订单" : "未找到匹配结果"));
const emptyDescription = computed(() =>
  isQualityRefundMode.value
    ? "请先确认订单缓存已同步，再尝试扩大查询天数。"
    : "已完成缓存保障但仍未找到足够高分订单，可尝试扩大查询天数。",
);
const resultSummary = computed(() => {
  if (!store.results.length) return "";
  const sourceLabel = isQualityRefundMode.value ? "品退订单" : "差评订单";
  if (isQualityRefundMode.value) {
    if (!unmatchedCount.value) {
      return `本次共获取 ${store.results.length} 条${sourceLabel}，官方接口已直接返回订单号，可直接带入发货页。`;
    }
    return `本次共获取 ${store.results.length} 条${sourceLabel}，其中 ${matchedCount.value} 条可直接带入发货，${unmatchedCount.value} 条因接口缺少订单号暂时无法自动带入。`;
  }
  if (!unmatchedCount.value) {
    return `本次共获取 ${store.results.length} 条${sourceLabel}，全部命中订单缓存，可直接带入发货页。`;
  }
  const cacheNote = store.cacheSyncPerformed
    ? `本次已自动补齐 ${store.cacheSyncWrittenCount} 条缓存订单，`
    : "本次已完成缓存保障，";
  return `本次共获取 ${store.results.length} 条${sourceLabel}，其中 ${matchedCount.value} 条已完成匹配，${unmatchedCount.value} 条未达到匹配阈值。${cacheNote}当前缓存覆盖 ${store.cacheCoverageStart || "-"} 至 ${store.cacheCoverageEnd || "-"}。`;
});
const idColumnLabel = computed(() => (isQualityRefundMode.value ? "订单号" : "评价ID"));

function switchMode(mode: "bad_review" | "quality_refund") {
  store.setLastMode(mode);
  store.setError(null);
  qualityReasonFilter.value = "";
}

async function handleSearch() {
  if (licenseBlocked.value) {
    store.setError("请先激活授权后再使用评价管理");
    return;
  }
  await findReviews(days.value, localDaysAgoStartIso(days.value), localYesterdayEndIso());
}

async function handleQualityRefundSearch() {
  if (licenseBlocked.value) {
    store.setError("请先激活授权后再使用品退订单");
    return;
  }
  await findQualityRefundOrders(
    days.value,
    localDaysAgoStartIso(days.value),
    localYesterdayEndIso(),
  );
}

async function handleFetchCurrentMode() {
  if (isQualityRefundMode.value) {
    await handleQualityRefundSearch();
    return;
  }
  await handleSearch();
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
  <div class="space-y-app">
    <section class="hero-panel subsystem-hero relative overflow-hidden p-3 lg:p-3.5">
      <div class="pointer-events-none absolute -right-20 -top-16 h-40 w-40 rounded-full bg-[radial-gradient(circle,rgba(167,243,208,0.4),transparent_72%)]"></div>

      <div
        data-testid="review-control-shell"
        class="review-control-shell relative flex flex-col gap-2.5 p-0 border-0 bg-transparent shadow-none backdrop-blur-none"
      >
        <div class="flex flex-wrap items-center justify-between gap-3">
          <div class="min-w-0 flex-1">
            <div class="flex flex-wrap items-baseline gap-x-2 gap-y-1">
              <h2 class="text-[1.05rem] font-bold tracking-tight text-slate-900 sm:text-[1.15rem] lg:text-[1.22rem]">
                {{ heroTitle }}
              </h2>
              <span class="text-[11px] font-semibold uppercase tracking-[0.12em] text-slate-400">
                {{ heroEyebrow }}
              </span>
            </div>
            <p class="mt-0.5 max-w-[44rem] text-[12px] leading-5 text-slate-500">{{ heroLead }}</p>
          </div>

          <div
            data-testid="review-mode-switch"
            class="review-mode-switch shrink-0"
          >
            <button
              type="button"
              class="review-mode-option cursor-pointer"
              :class="!isQualityRefundMode ? 'review-mode-option-active' : 'review-mode-option-idle'"
              @click="switchMode('bad_review')"
            >
              差评
            </button>
            <button
              type="button"
              class="review-mode-option cursor-pointer"
              :class="isQualityRefundMode ? 'review-mode-option-active' : 'review-mode-option-idle'"
              @click="switchMode('quality_refund')"
            >
              品退
            </button>
          </div>
        </div>

        <div
          data-testid="review-filter-grid"
          class="review-filter-grid"
        >
          <div>
            <label class="field-label">天数</label>
            <input v-model.number="days" type="number" min="1" max="90" class="field-input" />
          </div>
          <div class="review-filter-field">
            <label class="field-label">{{ isQualityRefundMode ? '原因过滤' : '匹配说明' }}</label>
            <input
              v-if="isQualityRefundMode"
              v-model.trim="qualityReasonFilter"
              type="text"
              placeholder="输入关键字"
              class="field-input"
            />
            <div v-else class="review-helper-card">
              先补齐缓存，再按商品、SKU、昵称与时间评分匹配。
            </div>
          </div>
          <button
            data-testid="review-primary-action"
            type="button"
            :disabled="store.loading || licenseBlocked"
            class="review-primary-action action-btn action-btn-primary cursor-pointer"
            @click="handleFetchCurrentMode"
          >
            {{
              store.loading
                ? "处理中..."
                : isQualityRefundMode
                  ? "获取品退"
                  : "获取差评"
            }}
          </button>
        </div>

        <div
          data-testid="review-summary-strip"
          class="subsystem-summary-strip grid gap-2 sm:grid-cols-3 xl:grid-cols-3"
        >
          <article
            v-for="card in summaryCards"
            :key="card.label"
            class="subsystem-summary-card review-summary-card"
            :class="summaryCardAccent[card.tone]"
          >
            <div class="subsystem-summary-label">{{ card.label }}</div>
            <div class="subsystem-summary-value">{{ card.value }}</div>
            <div class="subsystem-summary-hint">{{ card.hint }}</div>
          </article>
        </div>
      </div>
    </section>

    <div v-if="licenseBlocked" class="soft-alert warn">
      当前未激活授权，评价管理不可用。请先前往设置中心完成激活。
    </div>

    <div v-if="store.error" class="soft-alert error">
      {{ store.error }}
    </div>

    <LoadingState
      v-if="store.loading"
      :title="loadingTitle"
      :description="loadingDescription"
    />
    <div
      v-if="store.loading && orderStore.syncSource === 'review_query'"
      class="surface-panel space-y-app px-4 py-4"
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

    <div v-else-if="store.results.length > 0" class="space-y-app">
      <div
        class="soft-alert"
        :class="unmatchedCount > 0 ? 'warn' : 'success'"
      >
        {{ resultSummary }}
      </div>
      <div v-if="store.cacheWarnings.length && !isQualityRefundMode" class="soft-alert warn">
        {{ store.cacheWarnings.join("；") }}
      </div>

      <section class="data-table-shell overflow-x-auto">
        <table class="w-full text-sm" :class="isCompactLayout ? 'min-w-[820px]' : 'min-w-[980px]'">
          <thead class="table-head text-slate-600">
            <tr>
              <th
                v-if="!isQualityRefundMode"
                class="table-head-sticky px-3 py-2.5 text-left text-xs font-semibold sm:px-5 sm:py-3 sm:text-sm"
              >
                买家昵称
              </th>
              <th class="table-head-sticky px-3 py-2.5 text-left text-xs font-semibold sm:px-5 sm:py-3 sm:text-sm">评价内容</th>
              <th v-if="isQualityRefundMode" class="table-head-sticky px-3 py-2.5 text-left text-xs font-semibold sm:px-5 sm:py-3 sm:text-sm">品退原因</th>
              <th class="table-head-sticky px-3 py-2.5 text-left text-xs font-semibold sm:px-5 sm:py-3 sm:text-sm">订单详情</th>
              <th class="table-head-sticky px-3 py-2.5 text-left text-xs font-semibold sm:px-5 sm:py-3 sm:text-sm">{{ idColumnLabel }}</th>
              <th class="table-head-sticky px-3 py-2.5 text-center text-xs font-semibold sm:px-5 sm:py-3 sm:text-sm">匹配</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="r in filteredResults"
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
              <td class="max-w-md px-3 py-2.5 sm:px-5 sm:py-3">
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
          </tbody>
        </table>
      </section>
    </div>

    <EmptyState
      v-else-if="store.lastQuery"
      :title="emptyTitle"
      :description="emptyDescription"
      @action="handleFetchCurrentMode"
    >
      {{ isQualityRefundMode ? "再取一次品退订单" : "再查一次" }}
    </EmptyState>
  </div>
</template>
