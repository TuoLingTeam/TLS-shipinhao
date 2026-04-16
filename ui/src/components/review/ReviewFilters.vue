<script setup lang="ts">
import { ref } from "vue";

const emit = defineEmits<{ search: [days: number] }>();
const days = ref(30);
</script>

<template>
  <div class="bg-white rounded-lg p-4 shadow-sm border border-slate-200">
    <div class="flex items-end gap-4">
      <div>
        <label class="block text-sm text-slate-600 mb-1">查询天数</label>
        <input
          v-model.number="days"
          type="number"
          min="1"
          max="90"
          class="w-24 px-3 py-1.5 border border-slate-300 rounded text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
        />
      </div>
      <button
        class="px-4 py-1.5 bg-blue-500 text-white text-sm rounded hover:bg-blue-600 transition-colors"
        @click="emit('search', days)"
      >
        开始查找
      </button>
    </div>
  </div>
</template>
