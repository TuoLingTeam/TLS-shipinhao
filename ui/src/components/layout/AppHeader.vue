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

const pageTitle = computed(() => titleMap[route.name as string] ?? "TLS-shipinhao");
const licenseLabel = computed(() => (appStore.isLicensed ? "已授权" : "未激活"));
</script>

<template>
  <header class="surface-panel px-5 py-4 lg:px-6 lg:py-4">
    <div class="flex items-center justify-between gap-4">
      <div class="min-w-0">
        <p class="text-xs font-semibold uppercase tracking-[0.22em] text-slate-400">TLS · VIDEO COMMERCE DESK</p>
        <div class="mt-2 flex items-center gap-3">
          <h1 class="truncate text-2xl font-bold tracking-tight text-slate-900">{{ pageTitle }}</h1>
          <span class="hidden rounded-full bg-amber-100 px-2.5 py-1 text-[11px] font-semibold text-amber-700 sm:inline-flex">
            v{{ appStore.appVersion }}
          </span>
        </div>
      </div>

      <div class="status-chip shrink-0">
        <span class="status-dot" :class="appStore.isLicensed ? 'success' : ''"></span>
        <div class="text-sm font-semibold text-slate-700">{{ licenseLabel }}</div>
      </div>
    </div>
  </header>
</template>
