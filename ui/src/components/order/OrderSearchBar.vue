<script setup lang="ts">
import { ref } from "vue";

const emit = defineEmits<{ search: [keyword: string] }>();
const keyword = ref("");
</script>

<template>
  <div class="flex gap-2">
    <input
      v-model="keyword"
      type="text"
      placeholder="搜索订单号或买家"
      class="flex-1 px-3 py-1.5 border border-slate-300 rounded text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
      @keyup.enter="emit('search', keyword)"
    />
    <button
      class="px-4 py-1.5 bg-blue-500 text-white text-sm rounded hover:bg-blue-600 transition-colors"
      @click="emit('search', keyword)"
    >
      搜索
    </button>
  </div>
</template>
