<script setup lang="ts">
defineProps<{
  label: string;
  variant?: "success" | "error" | "warning" | "info" | "neutral";
}>();

const classes: Record<string, string> = {
  success: "bg-green-100 text-green-700",
  error: "bg-red-100 text-red-700",
  warning: "bg-yellow-100 text-yellow-700",
  info: "bg-blue-100 text-blue-700",
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
