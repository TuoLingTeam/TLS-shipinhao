<script setup lang="ts">
import { computed, ref } from "vue";
import { useRouter } from "vue-router";
import { useReview } from "../composables/useReview";
import { useReviewStore } from "../stores/review";
import { useOrderStore } from "../stores/order";
import { useAppStore } from "../stores/app";
import EmptyState from "../components/common/EmptyState.vue";
import LoadingState from "../components/common/LoadingState.vue";

const router = useRouter();
const store = useReviewStore();
const orderStore = useOrderStore();
const appStore = useAppStore();
const { findReviews, findQualityRefundOrders, prefillMatchedOrder } = useReview();

const days = ref(30);
const licenseBlocked = computed(() => !appStore.isLicensed);
const isQualityRefundMode = computed(() => store.lastMode === "quality_refund");
const matchedCount = computed(() => store.results.filter((item) => item.matched).length);
const unmatchedCount = computed(() => store.results.length - matchedCount.value);
const loadingTitle = computed(() =>
  orderStore.syncSource === "review_query"
    ? "正在准备订单缓存并执行评分匹配"
    : isQualityRefundMode.value
      ? "正在获取品退订单并匹配缓存订单"
      : "正在获取差评并执行订单评分匹配",
);
const loadingDescription = computed(() =>
  orderStore.syncSource === "review_query"
    ? orderStore.syncMessage || "后端会先保障最近 30 天订单缓存可用，再执行评分匹配。"
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
  return `本次共获取 ${store.results.length} 条${sourceLabel}，其中 ${matchedCount.value} 条已完成匹配，${unmatchedCount.value} 条未达到匹配阈值。${cacheNote}当前缓存覆盖 ${store.cacheCoverageStart || "-"} 至 ${store.cacheCoverageEnd || "-" }。`;
});
const idColumnLabel = computed(() => (isQualityRefundMode.value ? "订单号" : "评价ID"));

function switchMode(mode: "bad_review" | "quality_refund") {
  store.setLastMode(mode);
  store.setError(null);
}

function todayISO(): string {
  return `${new Date().toISOString().split("T")[0]}T23:59:59Z`;
}

function daysAgoISO(n: number): string {
  const d = new Date();
  d.setDate(d.getDate() - n);
  return `${d.toISOString().split("T")[0]}T00:00:00Z`;
}

async function handleSearch() {
  if (licenseBlocked.value) {
    store.setError("请先激活授权后再使用评价管理");
    return;
  }
  await findReviews(days.value, daysAgoISO(days.value), todayISO());
}

async function handleQualityRefundSearch() {
  if (licenseBlocked.value) {
    store.setError("请先激活授权后再使用品退订单");
    return;
  }
  await findQualityRefundOrders(days.value, daysAgoISO(days.value), todayISO());
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
</script>

<template>
  <div class="space-y-5">
    <section class="surface-panel flex flex-col gap-4 p-5 lg:flex-row lg:items-end lg:justify-between lg:p-6">
      <div class="min-w-0">
        <h2 class="text-xl font-semibold tracking-tight text-slate-900">评价检索</h2>
        <div class="mt-3 inline-flex rounded-2xl bg-slate-100/90 p-1">
          <button
            class="min-w-[104px] rounded-xl px-4 py-2 text-sm font-semibold transition"
            :class="
              !isQualityRefundMode
                ? 'bg-white text-slate-900 shadow-sm'
                : 'text-slate-500 hover:text-slate-700'
            "
            @click="switchMode('bad_review')"
          >
            差评
          </button>
          <button
            class="min-w-[104px] rounded-xl px-4 py-2 text-sm font-semibold transition"
            :class="
              isQualityRefundMode
                ? 'bg-white text-slate-900 shadow-sm'
                : 'text-slate-500 hover:text-slate-700'
            "
            @click="switchMode('quality_refund')"
          >
            品退
          </button>
        </div>
      </div>
      <div class="flex flex-wrap items-end gap-3">
        <div>
          <label class="field-label">查询天数</label>
          <input v-model.number="days" type="number" min="1" max="90" class="field-input w-28" />
        </div>
        <button
          :disabled="store.loading || licenseBlocked"
          class="action-btn action-btn-primary min-w-[128px]"
          @click="handleFetchCurrentMode"
        >
          {{
            store.loading
              ? "处理中..."
              : isQualityRefundMode
                ? "获取品退订单"
                : "获取差评订单"
          }}
        </button>
      </div>
    </section>

    <div v-if="licenseBlocked" class="soft-alert warn">
      当前未激活授权，评价管理不可用。请先前往授权管理完成激活。
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
      class="surface-panel space-y-4 px-5 py-5"
    >
      <div class="flex items-center justify-between text-sm">
        <span class="font-semibold text-slate-800">自动同步进度</span>
        <span class="font-mono text-slate-500">{{ orderStore.syncProgress }}%</span>
      </div>
      <div class="h-2 overflow-hidden rounded-full bg-slate-100">
        <div
          class="h-full rounded-full bg-blue-600 transition-all duration-300"
          :style="{ width: `${orderStore.syncProgress}%` }"
        ></div>
      </div>
      <div class="grid grid-cols-1 gap-3 text-xs text-slate-500 md:grid-cols-3">
        <div class="rounded-2xl border border-slate-200/80 px-4 py-3">
          <div class="font-semibold text-slate-700">1. 缓存保障</div>
          <div class="mt-1">{{ ['ensure_recent_cache', 'match_reviews', 'completed'].includes(orderStore.syncPhase || '') ? '进行中/完成' : '等待中' }}</div>
        </div>
        <div class="rounded-2xl border border-slate-200/80 px-4 py-3">
          <div class="font-semibold text-slate-700">2. 评分匹配</div>
          <div class="mt-1">
            {{ ['match_reviews', 'completed'].includes(orderStore.syncPhase || '') ? '进行中/完成' : '等待中' }}
          </div>
        </div>
        <div class="rounded-2xl border border-slate-200/80 px-4 py-3">
          <div class="font-semibold text-slate-700">3. 完成结果</div>
          <div class="mt-1">
            {{ orderStore.syncPhase === 'completed' ? '已完成' : '等待中' }}
          </div>
        </div>
      </div>
    </div>

    <div v-else-if="store.results.length > 0" class="space-y-4">
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
      <table class="w-full min-w-[980px] text-sm">
        <thead class="table-head text-slate-600">
          <tr>
            <th
              v-if="!isQualityRefundMode"
              class="table-head-sticky px-5 py-4 text-left font-semibold"
            >
              买家昵称
            </th>
            <th class="table-head-sticky px-5 py-4 text-left font-semibold">评价内容</th>
            <th class="table-head-sticky px-5 py-4 text-left font-semibold">订单详情</th>
            <th class="table-head-sticky px-5 py-4 text-left font-semibold">{{ idColumnLabel }}</th>
            <th class="table-head-sticky px-5 py-4 text-center font-semibold">匹配</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="r in store.results"
            :key="r.evaluation_id"
            class="table-row border-t border-slate-100/80 align-top transition-colors"
            :class="r.matched && r.order_id ? 'cursor-pointer' : ''"
            @click="r.matched && r.order_id ? handleUseMatchedOrder(r.order_id) : undefined"
          >
            <td v-if="!isQualityRefundMode" class="px-5 py-4">
              <div class="font-semibold text-slate-800">{{ r.buyer_nickname || "-" }}</div>
            </td>
            <td class="max-w-md px-5 py-4">
              <div class="whitespace-pre-wrap break-all text-[15px] leading-7 text-slate-700">
                {{ r.evaluation_content || "（无评价内容）" }}
              </div>
            </td>
            <td class="px-5 py-4">
              <div class="space-y-1.5 text-xs leading-6 text-slate-600">
                <div><span class="font-semibold text-slate-700">SKU：</span>{{ r.sku_name || r.sku_id || "-" }}</div>
                <div>
                  <span class="font-semibold text-slate-700">商品ID：</span>
                  <span class="font-mono">{{ r.product_id || "-" }}</span>
                </div>
                <div v-if="r.product_name" class="text-slate-500">{{ r.product_name }}</div>
              </div>
            </td>
            <td class="px-5 py-4 font-mono text-xs text-slate-700">{{ displayId(r) }}</td>
            <td class="px-5 py-4 text-center">
              <div class="flex flex-col items-center gap-2">
                <span
                  class="inline-flex rounded-full px-3 py-1 text-xs font-semibold"
                  :class="r.matched ? 'bg-green-100 text-green-700' : 'bg-slate-100 text-slate-500'"
                >
                  {{ r.matched ? "已匹配" : "未匹配" }}
                </span>
                <div
                  class="max-w-[180px] text-center text-[11px] leading-5"
                  :class="r.matched ? 'text-slate-500' : 'text-amber-700'"
                >
                  {{ r.matched ? `评分 ${r.confidence_score} · 点击本行可自动带入发货页。` : unmatchedReason(r) }}
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
