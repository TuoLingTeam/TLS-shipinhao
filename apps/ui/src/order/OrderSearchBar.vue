<script setup lang="ts">
import { ref } from "vue";

const emit = defineEmits<{ search: [keyword: string] }>();
const keyword = ref("");
</script>

<template>
  <div class="rounded-[18px] border border-white/65 bg-white/80 p-3 shadow-[0_18px_40px_-32px_rgba(15,23,42,0.2)] backdrop-blur">
    <div class="grid gap-2.5 lg:grid-cols-[minmax(0,1fr)_120px] lg:items-end">
      <div class="min-w-0">
        <label class="field-label">本地订单检索</label>
        <input
          v-model.trim="keyword"
          type="text"
          placeholder="搜索订单号、买家昵称或收件人"
          class="field-input"
          @keyup.enter="emit('search', keyword)"
        />
        <p class="mt-1.5 text-[11px] leading-5 text-slate-500">
          仅筛选本地订单，不触发远端请求。
        </p>
      </div>
      <button class="action-btn action-btn-primary min-h-[40px] w-full" @click="emit('search', keyword)">
        搜索
      </button>
    </div>
  </div>
</template>
