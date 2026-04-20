<script setup lang="ts">
withDefaults(
  defineProps<{
    title?: string;
    description?: string;
    compact?: boolean;
  }>(),
  {
    title: '处理中',
    description: '正在拉取最新数据，请稍候…',
    compact: false,
  },
);
</script>

<template>
  <div
    class="surface-panel flex min-h-0 flex-col overflow-hidden"
    :class="compact ? 'p-5' : 'p-6 lg:p-7'"
  >
    <div class="flex shrink-0 flex-col gap-app lg:flex-row lg:items-center lg:justify-between">
      <div>
        <div class="text-lg font-semibold text-slate-900">{{ title }}</div>
        <p class="mt-2 text-sm leading-6 text-slate-500">{{ description }}</p>
      </div>
      <div class="inline-flex items-center gap-app rounded-full bg-brand-soft px-3 py-2 text-sm font-medium text-brand-deep">
        <span class="h-2.5 w-2.5 animate-pulse rounded-full bg-brand"></span>
        正在加载
      </div>
    </div>

    <div class="mt-6 flex min-h-0 flex-1 flex-col space-y-app">
      <div class="h-12 animate-pulse rounded-2xl bg-slate-100"></div>
      <div class="grid grid-cols-1 gap-app lg:grid-cols-3">
        <div class="h-28 animate-pulse rounded-3xl bg-slate-100"></div>
        <div class="h-28 animate-pulse rounded-3xl bg-slate-100"></div>
        <div class="h-28 animate-pulse rounded-3xl bg-slate-100"></div>
      </div>
      <div class="space-y-app">
        <div class="h-14 animate-pulse rounded-2xl bg-slate-100"></div>
        <div class="h-14 animate-pulse rounded-2xl bg-slate-100"></div>
        <div class="h-14 animate-pulse rounded-2xl bg-slate-100"></div>
      </div>
    </div>
  </div>
</template>
