<script setup lang="ts">
defineProps<{
  label: string;
  variant?: "success" | "error" | "warning" | "info" | "neutral";
}>();

const classes: Record<string, string> = {
  success: "bg-brand-soft text-brand-deep",
  error: "bg-red-100 text-red-700",
  warning: "bg-yellow-100 text-yellow-700",
  info: "bg-brand-soft text-brand-deep",
  neutral: "bg-slate-100 text-slate-600",
};
</script>

<template>
  <span
    class="inline-block px-2 py-0.5 text-xs font-medium rounded-full"
    :class="classes[variant ?? 'neutral']"
  >
    {{ label }}
  </span>
</template>
