<script setup lang="ts">
import { useRoute } from "vue-router";
import { computed } from "vue";

const route = useRoute();

const titleMap: Record<string, string> = {
  dashboard: "仪表盘",
  review: "中差评管理",
  order: "订单管理",
  delivery: "发货管理",
  license: "授权管理",
  settings: "设置",
};

const pageTitle = computed(
  () => titleMap[route.name as string] ?? "TLS-shipinhao"
);
</script>

<template>
  <header
    class="h-14 border-b border-slate-200 bg-white flex items-center justify-between px-6"
  >
    <h1 class="text-lg font-semibold text-slate-800">{{ pageTitle }}</h1>
    <div class="flex items-center gap-4 text-sm text-slate-500">
      <span class="inline-flex items-center gap-1.5">
        <span class="w-2 h-2 rounded-full bg-green-400"></span>
        已授权
      </span>
    </div>
  </header>
</template>
