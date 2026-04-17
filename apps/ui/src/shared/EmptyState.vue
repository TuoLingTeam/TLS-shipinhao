<script setup lang="ts">
withDefaults(
  defineProps<{
    title: string;
    description?: string;
    compact?: boolean;
  }>(),
  {
    description: '',
    compact: false,
  },
);

defineEmits<{ action: [] }>();
</script>

<template>
  <div class="surface-panel text-center" :class="compact ? 'px-5 py-10' : 'px-6 py-16 lg:px-8 lg:py-18'">
    <div class="mx-auto flex h-16 w-16 items-center justify-center rounded-[22px] bg-slate-100 text-slate-500 shadow-inner">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="h-7 w-7" aria-hidden="true">
        <path d="M4 6.5h16" />
        <path d="M4 12h16" />
        <path d="M4 17.5h10" />
      </svg>
    </div>
    <div class="mt-5 text-lg font-semibold text-slate-800">{{ title }}</div>
    <div v-if="description" class="mx-auto mt-2 max-w-md text-sm leading-6 text-slate-500">{{ description }}</div>
    <slot name="action">
      <button
        v-if="$slots.default"
        class="action-btn action-btn-primary mx-auto mt-6"
        @click="$emit('action')"
      >
        <slot />
      </button>
    </slot>
  </div>
</template>
