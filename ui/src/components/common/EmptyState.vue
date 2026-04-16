<script setup lang="ts">
defineProps<{
  icon?: string;
  title: string;
  description?: string;
}>();

defineEmits<{ action: [] }>();
</script>

<template>
  <div class="flex flex-col items-center justify-center py-12 text-center">
    <div v-if="icon" class="text-4xl mb-3">{{ icon }}</div>
    <div class="text-sm font-medium text-slate-600">{{ title }}</div>
    <div v-if="description" class="text-xs text-slate-400 mt-1">{{ description }}</div>
    <slot name="action">
      <button
        v-if="$slots.default"
        class="mt-4 px-4 py-1.5 text-sm bg-blue-500 text-white rounded hover:bg-blue-600 transition-colors"
        @click="$emit('action')"
      >
        <slot />
      </button>
    </slot>
  </div>
</template>
