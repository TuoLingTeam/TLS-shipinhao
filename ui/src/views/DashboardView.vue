<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useTauriInvoke } from "../composables/useTauriInvoke";

const appInfo = useTauriInvoke<{ name: string; version: string; runtime: string }>("get_app_info");
const info = ref<{ name: string; version: string; runtime: string } | null>(null);

onMounted(async () => {
  info.value = await appInfo.execute();
});
</script>

<template>
  <div>
    <h2 class="text-xl font-semibold text-slate-700 mb-4">概览</h2>

    <div class="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
      <div class="bg-white rounded-lg p-5 shadow-sm border border-slate-200">
        <div class="text-sm text-slate-500">今日评价匹配</div>
        <div class="mt-1 text-2xl font-bold text-slate-800">--</div>
      </div>
      <div class="bg-white rounded-lg p-5 shadow-sm border border-slate-200">
        <div class="text-sm text-slate-500">订单缓存数</div>
        <div class="mt-1 text-2xl font-bold text-slate-800">--</div>
      </div>
      <div class="bg-white rounded-lg p-5 shadow-sm border border-slate-200">
        <div class="text-sm text-slate-500">发货任务</div>
        <div class="mt-1 text-2xl font-bold text-slate-800">--</div>
      </div>
    </div>

    <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
      <RouterLink
        to="/review"
        class="block bg-white rounded-lg p-5 shadow-sm border border-slate-200 hover:border-blue-300 hover:shadow transition-all"
      >
        <div class="text-lg font-medium text-slate-700">中差评查找</div>
        <div class="text-sm text-slate-500 mt-1">查找并匹配中差评订单</div>
      </RouterLink>
      <RouterLink
        to="/delivery"
        class="block bg-white rounded-lg p-5 shadow-sm border border-slate-200 hover:border-blue-300 hover:shadow transition-all"
      >
        <div class="text-lg font-medium text-slate-700">批量发货</div>
        <div class="text-sm text-slate-500 mt-1">批量更新物流信息</div>
      </RouterLink>
    </div>

    <div v-if="info" class="mt-6 text-xs text-slate-400">
      {{ info.name }} v{{ info.version }} · {{ info.runtime }}
    </div>
  </div>
</template>
