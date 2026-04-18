<script setup lang="ts">
import { ref } from "vue";

const emit = defineEmits<{ search: [keyword: string] }>();
const keyword = ref("");
</script>

<template>
  <div class="rounded-[24px] border border-white/65 bg-white/80 p-4 shadow-[0_18px_40px_-30px_rgba(15,23,42,0.22)] backdrop-blur">
    <div class="grid gap-3 lg:grid-cols-[minmax(0,1fr)_132px] lg:items-end">
      <div class="min-w-0">
        <label class="field-label">本地订单检索</label>
        <input
          v-model.trim="keyword"
          type="text"
          placeholder="搜索订单号、买家昵称或收件人"
          class="field-input"
          @keyup.enter="emit('search', keyword)"
        />
        <p class="mt-2 text-xs leading-5 text-slate-500">
          仅对本地订单副本做筛选，不会触发远端接口请求，可快速核对订单号与收件人。
        </p>
      </div>
      <button class="action-btn action-btn-primary min-h-[48px] w-full" @click="emit('search', keyword)">
        搜索
      </button>
    </div>
  </div>
</template>
