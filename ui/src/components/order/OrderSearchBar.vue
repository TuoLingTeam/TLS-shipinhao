<script setup lang="ts">
import { ref } from "vue";

const emit = defineEmits<{ search: [keyword: string] }>();
const keyword = ref("");
</script>

<template>
  <div class="flex flex-col gap-3 lg:flex-row lg:items-end">
    <div class="min-w-0 flex-1">
      <label class="field-label">本地订单检索</label>
      <input
        v-model.trim="keyword"
        type="text"
        placeholder="搜索订单号、买家昵称或收件人"
        class="field-input"
        @keyup.enter="emit('search', keyword)"
      />
      <p class="mt-2 text-xs leading-5 text-slate-500">
        支持按订单号、买家昵称、收件人本地筛选，不会触发远端接口请求。
      </p>
    </div>
    <button class="action-btn action-btn-primary min-w-[108px]" @click="emit('search', keyword)">
      搜索
    </button>
  </div>
</template>
