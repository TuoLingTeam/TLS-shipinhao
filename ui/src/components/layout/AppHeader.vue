<script setup lang="ts">
import { useRoute } from "vue-router";
import { computed } from "vue";
import { useAppStore } from "../../stores/app";

const route = useRoute();
const appStore = useAppStore();

const titleMap: Record<string, string> = {
  dashboard: "仪表盘",
  review: "评价管理",
  order: "订单管理",
  delivery: "发货管理",
  license: "授权管理",
  settings: "设置",
};

const pageTitle = computed(
  () => titleMap[route.name as string] ?? "TLS-shipinhao"
);

const licenseLabel = computed(() =>
  appStore.isLicensed ? "已授权" : "未激活"
);
</script>

<template>
  <header
    class="h-14 border-b border-slate-200 bg-white flex items-center justify-between px-6"
  >
    <h1 class="text-lg font-semibold text-slate-800">{{ pageTitle }}</h1>
    <div class="flex items-center gap-4 text-sm text-slate-500">
      <span class="inline-flex items-center gap-1.5">
        <span
          class="w-2 h-2 rounded-full"
          :class="appStore.isLicensed ? 'bg-green-400' : 'bg-slate-300'"
        ></span>
        {{ licenseLabel }}
      </span>
    </div>
  </header>
</template>
