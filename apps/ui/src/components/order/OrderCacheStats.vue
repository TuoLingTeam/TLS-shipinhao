<script setup lang="ts">
import { formatDate, formatDateTime } from "../../shared/format";

defineProps<{
  count: number;
  lastSyncAt: string | null;
  coverageStart?: string | null;
  coverageEnd?: string | null;
  coverageComplete?: boolean;
  missingSegmentCount?: number;
}>();
</script>

<template>
  <div class="surface-panel p-5 lg:p-6">
    <div class="flex items-start justify-between gap-4">
      <div>
        <div class="text-xs font-semibold uppercase tracking-[0.22em] text-slate-400">Cache Snapshot</div>
        <h3 class="mt-1 text-2xl font-semibold tracking-tight text-slate-900">缓存统计</h3>
        <p class="mt-2 text-sm leading-6 text-slate-500">
          该状态来自富订单缓存，用于差评评分匹配与覆盖诊断。
        </p>
      </div>
      <div
        class="rounded-2xl px-3 py-2 text-[11px] font-semibold uppercase tracking-[0.18em]"
        :class="
          coverageComplete
            ? 'bg-brand-soft text-brand-deep'
            : 'bg-amber-50 text-amber-700'
        "
      >
        {{ coverageComplete ? 'Coverage OK' : 'Need Refresh' }}
      </div>
    </div>

    <div class="mt-5 grid grid-cols-1 gap-3 text-sm">
      <div class="grid grid-cols-2 gap-3">
        <div class="rounded-2xl bg-slate-50 px-4 py-3">
          <div class="text-xs text-slate-400">缓存订单数</div>
          <div class="mt-1 text-xl font-semibold tracking-tight text-slate-900">{{ count }}</div>
        </div>
        <div class="rounded-2xl bg-slate-50 px-4 py-3">
          <div class="text-xs text-slate-400">覆盖状态</div>
          <div class="mt-1 text-base font-semibold" :class="coverageComplete ? 'text-brand-deep' : 'text-amber-700'">
            {{ coverageComplete ? "完整" : `缺口 ${missingSegmentCount || 0}` }}
          </div>
        </div>
      </div>
      <div class="flex items-center justify-between rounded-2xl bg-slate-50 px-4 py-3">
        <span class="text-slate-500">最后同步</span>
        <span class="font-medium text-slate-700">{{ lastSyncAt ? formatDateTime(lastSyncAt) : "从未" }}</span>
      </div>
      <div class="flex items-center justify-between rounded-2xl bg-slate-50 px-4 py-3">
        <span class="text-slate-500">覆盖区间</span>
        <span class="font-medium text-slate-700">
          {{ coverageStart && coverageEnd ? `${formatDate(coverageStart)} ~ ${formatDate(coverageEnd)}` : "未建立" }}
        </span>
      </div>
    </div>
  </div>
</template>
