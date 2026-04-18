<script setup lang="ts">
import { computed } from "vue";
import { formatDate, formatDateTime } from "../shared/format";

const props = defineProps<{
  count: number;
  lastSyncAt: string | null;
  coverageStart?: string | null;
  coverageEnd?: string | null;
  coverageComplete?: boolean;
  missingSegmentCount?: number;
}>();

const statCards = computed(() => [
  {
    label: "缓存订单数",
    value: String(props.count || 0),
    hint: "用于评价匹配与订单核对",
  },
  {
    label: "覆盖状态",
    value: props.coverageComplete ? "完整" : `缺口 ${props.missingSegmentCount || 0}`,
    hint: props.coverageComplete ? "可直接支撑差评评分匹配" : "建议立即同步补齐覆盖范围",
  },
  {
    label: "最后同步",
    value: props.lastSyncAt ? formatDateTime(props.lastSyncAt) : "从未",
    hint: props.lastSyncAt ? "保留最近一次完成时间" : "尚未建立本地缓存",
  },
]);
</script>

<template>
  <div class="surface-panel p-5 lg:p-6">
    <div class="flex items-start justify-between gap-4">
      <div>
        <div class="text-xs font-semibold uppercase tracking-[0.22em] text-slate-400">Cache Snapshot</div>
        <h3 class="mt-2 text-2xl font-semibold tracking-tight text-slate-900">缓存统计</h3>
        <p class="mt-2 text-sm leading-6 text-slate-500">
          该状态来自富订单缓存，用于差评评分匹配与覆盖诊断。
        </p>
      </div>
      <div
        class="rounded-full px-3 py-2 text-[11px] font-semibold uppercase tracking-[0.18em]"
        :class="coverageComplete ? 'bg-brand-soft text-brand-deep' : 'bg-amber-50 text-amber-700'"
      >
        {{ coverageComplete ? 'Coverage OK' : 'Need Refresh' }}
      </div>
    </div>

    <div class="mt-6 grid grid-cols-1 gap-3 lg:grid-cols-3">
      <article
        v-for="card in statCards"
        :key="card.label"
        class="rounded-[22px] border border-slate-200/80 bg-slate-50 px-4 py-4"
      >
        <div class="text-xs text-slate-400">{{ card.label }}</div>
        <div class="mt-2 text-lg font-semibold tracking-tight text-slate-900">{{ card.value }}</div>
        <div class="mt-2 text-sm leading-6 text-slate-500">{{ card.hint }}</div>
      </article>
    </div>

    <div class="mt-4 rounded-[22px] border border-slate-200/80 bg-slate-50 px-4 py-4">
      <div class="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
        <span class="text-sm font-semibold text-slate-700">覆盖区间</span>
        <span class="text-sm text-slate-600">
          {{ coverageStart && coverageEnd ? `${formatDate(coverageStart)} ~ ${formatDate(coverageEnd)}` : '未建立' }}
        </span>
      </div>
    </div>
  </div>
</template>
