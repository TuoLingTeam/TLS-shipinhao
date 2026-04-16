<script setup lang="ts">
import { ref } from "vue";

const emit = defineEmits<{ search: [keyword: string] }>();
const keyword = ref("");
</script>

<template>
  <div class="surface-panel flex flex-col gap-4 p-4 lg:flex-row lg:items-center lg:p-5">
    <div class="min-w-0 flex-1">
      <label class="field-label">搜索订单</label>
      <input
        v-model.trim="keyword"
        type="text"
        placeholder="搜索订单号、买家昵称或收件人"
        class="field-input"
        @keyup.enter="emit('search', keyword)"
      />
    </div>
    <button class="action-btn action-btn-primary min-w-[108px]" @click="emit('search', keyword)">
      搜索
    </button>
  </div>
</template>
