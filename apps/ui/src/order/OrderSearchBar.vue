<script setup lang="ts">
import { ref } from "vue";

const emit = defineEmits<{ search: [keyword: string] }>();
const keyword = ref("");
</script>

<template>
  <div class="order-search-shell">
    <div class="order-search-grid">
      <div class="min-w-0">
        <label class="field-label">本地订单检索</label>
        <input
          v-model.trim="keyword"
          type="text"
          placeholder="搜索订单号、买家昵称或收件人"
          class="field-input"
          @keyup.enter="emit('search', keyword)"
        />
        <p class="mt-1 text-[10px] leading-4 text-slate-500">
          仅筛选本地订单，不触发远端请求。
        </p>
      </div>
      <button class="order-search-action action-btn action-btn-primary w-full" @click="emit('search', keyword)">
        搜索
      </button>
    </div>
  </div>
</template>
