<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { RouterLink } from "vue-router";
import { useTauriInvoke } from "../composables/useTauriInvoke";
import { useAppStore } from "../stores/app";
import { useOrderStore } from "../stores/order";
import { useReviewStore } from "../stores/review";
import { useDeliveryStore } from "../stores/delivery";
import AppNavIcon from "../components/layout/AppNavIcon.vue";
import { AUTHOR_WECHAT } from "../constants/brand";

const appStore = useAppStore();
const orderStore = useOrderStore();
const reviewStore = useReviewStore();
const deliveryStore = useDeliveryStore();

const appInfo = useTauriInvoke<{ name: string; name_en: string; version: string; author_wechat: string; window_title: string; runtime: string }>("get_app_info");
const info = ref<{ name: string; name_en: string; version: string; author_wechat: string; window_title: string; runtime: string } | null>(null);

const metrics = computed(() => [
  {
    label: "评价匹配",
    value: reviewStore.results.length > 0 ? String(reviewStore.results.filter((item) => item.matched).length) : "--",
    hint: reviewStore.results.length > 0 ? `${reviewStore.results.length} 条结果` : "等待查询",
  },
  {
    label: "订单缓存",
    value: orderStore.cachedOrders.length > 0 ? String(orderStore.cachedOrders.length) : "--",
    hint: orderStore.lastSyncAt ? "已同步" : "未同步",
  },
  {
    label: "发货任务",
    value: deliveryStore.batchProgress ? String(deliveryStore.batchProgress.totalCount) : "--",
    hint: deliveryStore.batchProgress ? `${deliveryStore.batchProgress.successCount} 成功` : "待执行",
  },
]);

const quickLinks = [
  { to: "/review", title: "中差评查找", icon: "review" },
  { to: "/order", title: "订单缓存同步", icon: "order" },
  { to: "/delivery", title: "批量发货", icon: "delivery" },
] as const;

onMounted(async () => {
  info.value = await appInfo.execute();
});
</script>

<template>
  <div class="space-y-5">
    <section class="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-3">
      <article v-for="metric in metrics" :key="metric.label" class="surface-panel metric-card p-5 lg:p-6">
        <div class="metric-label">{{ metric.label }}</div>
        <div class="metric-value">{{ metric.value }}</div>
        <div class="metric-hint">{{ metric.hint }}</div>
      </article>
    </section>

    <section class="surface-panel p-5 lg:p-6">
      <div class="mb-4 flex items-center justify-between">
        <h3 class="text-xl font-semibold tracking-tight text-slate-900">高频入口</h3>
        <div class="text-xs text-slate-400">{{ info?.runtime ?? 'tauri' }} · v{{ info?.version ?? appStore.appVersion }} · 作者微信 {{ info?.author_wechat ?? AUTHOR_WECHAT }}</div>
      </div>

      <div class="grid grid-cols-1 gap-4 lg:grid-cols-3">
        <RouterLink
          v-for="item in quickLinks"
          :key="item.to"
          :to="item.to"
          class="quick-link surface-panel-strong flex min-h-[144px] flex-col justify-between p-5"
        >
          <div>
            <div class="flex h-11 w-11 items-center justify-center rounded-2xl bg-slate-900 text-white shadow-lg shadow-slate-900/10">
              <AppNavIcon :name="item.icon" icon-class="h-5 w-5" />
            </div>
            <h4 class="mt-4 text-lg font-semibold text-slate-900">{{ item.title }}</h4>
          </div>
          <div class="mt-4 inline-flex items-center gap-2 text-sm font-semibold text-blue-600">
            进入
            <span aria-hidden="true">→</span>
          </div>
        </RouterLink>
      </div>
    </section>
  </div>
</template>
