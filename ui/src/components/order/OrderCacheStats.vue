<script setup lang="ts">
import { formatDate } from "../../utils/format";

defineProps<{
  count: number;
  lastSyncAt: string | null;
}>();
</script>

<template>
  <div class="surface-panel p-5 lg:p-6">
    <div class="flex items-start justify-between gap-4">
      <div>
        <div class="text-xs font-semibold uppercase tracking-[0.22em] text-slate-400">Cache Snapshot</div>
        <h3 class="mt-1 text-2xl font-semibold tracking-tight text-slate-900">缓存统计</h3>
      </div>
      <div class="rounded-2xl bg-amber-50 px-3 py-2 text-[11px] font-semibold uppercase tracking-[0.18em] text-amber-700">
        Local Store
      </div>
    </div>

    <div class="mt-5 space-y-4 text-sm">
      <div class="flex items-center justify-between rounded-2xl bg-slate-50 px-4 py-3">
        <span class="text-slate-500">缓存订单数</span>
        <span class="text-xl font-semibold tracking-tight text-slate-900">{{ count }}</span>
      </div>
      <div class="flex items-center justify-between rounded-2xl bg-slate-50 px-4 py-3">
        <span class="text-slate-500">最后同步</span>
        <span class="font-medium text-slate-700">{{ lastSyncAt ? formatDate(lastSyncAt) : "从未" }}</span>
      </div>
    </div>
  </div>
</template>
