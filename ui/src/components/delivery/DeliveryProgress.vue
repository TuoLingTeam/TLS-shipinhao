<script setup lang="ts">
import { computed } from "vue";
import ProgressBar from "../common/ProgressBar.vue";
import type { BatchProgress } from "../../stores/delivery";

const props = defineProps<{ progress: BatchProgress }>();

const percent = computed(() => {
  if (props.progress.totalCount === 0) return 0;
  return (
    ((props.progress.successCount + props.progress.failureCount) /
      props.progress.totalCount) *
    100
  );
});
</script>

<template>
  <div class="space-y-3">
    <ProgressBar :percent="percent" label="批量发货进度" />
    <div class="grid grid-cols-3 gap-2 text-center text-sm">
      <div class="bg-slate-50 p-2 rounded">
        <div class="text-slate-500">总计</div>
        <div class="font-bold text-slate-800">{{ progress.totalCount }}</div>
      </div>
      <div class="bg-brand-soft p-2 rounded">
        <div class="text-brand">成功</div>
        <div class="font-bold text-brand-deep">{{ progress.successCount }}</div>
      </div>
      <div class="bg-red-50 p-2 rounded">
        <div class="text-red-600">失败</div>
        <div class="font-bold text-red-700">{{ progress.failureCount }}</div>
      </div>
    </div>
    <div
      v-if="progress.fatalError"
      class="p-2 bg-red-50 text-red-700 text-sm rounded"
    >
      {{ progress.fatalError }}
    </div>
  </div>
</template>
