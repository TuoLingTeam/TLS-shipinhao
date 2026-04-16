<script setup lang="ts">
defineProps<{
  percent: number;
  label?: string;
}>();
</script>

<template>
  <div>
    <div v-if="label" class="flex justify-between text-xs text-slate-500 mb-1">
      <span>{{ label }}</span>
      <span>{{ Math.round(percent) }}%</span>
    </div>
    <div class="w-full h-2 bg-slate-200 rounded-full overflow-hidden">
      <div
        class="h-full bg-blue-500 rounded-full transition-all duration-300"
        :style="{ width: `${Math.min(100, Math.max(0, percent))}%` }"
      />
    </div>
  </div>
</template>
