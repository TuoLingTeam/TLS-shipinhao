<script setup lang="ts">
import { formatDate } from "../../utils/format";

defineProps<{
  count: number;
  lastSyncAt: string | null;
}>();
</script>

<template>
  <div class="bg-white rounded-lg p-4 shadow-sm border border-slate-200">
    <h3 class="font-medium text-slate-700 mb-3">缓存统计</h3>
    <div class="space-y-2 text-sm">
      <div class="flex justify-between">
        <span class="text-slate-500">缓存订单数</span>
        <span class="font-semibold text-slate-800">{{ count }}</span>
      </div>
      <div class="flex justify-between">
        <span class="text-slate-500">最后同步</span>
        <span class="text-slate-600">{{ lastSyncAt ? formatDate(lastSyncAt) : "从未" }}</span>
      </div>
    </div>
  </div>
</template>
