<script setup lang="ts">
defineProps<{ score: number }>();

function color(score: number): string {
  if (score >= 80) return "text-brand bg-brand-soft";
  if (score >= 50) return "text-yellow-600 bg-yellow-50";
  return "text-red-600 bg-red-50";
}
</script>

<template>
  <span
    class="inline-block px-2 py-0.5 text-xs font-semibold rounded"
    :class="color(score)"
  >
    {{ score }}%
  </span>
</template>
