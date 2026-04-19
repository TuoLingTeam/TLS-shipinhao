<script setup lang="ts">
import { ref, computed } from "vue";

const emit = defineEmits<{
  submit: [items: { order_id: string; tracking_number: string }[]];
}>();

const batchText = ref("");

const parsedItems = computed(() =>
  batchText.value
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean)
    .map((line) => {
      const [oid, tn] = line.split(/[\t,]/).map((s) => s.trim());
      return { order_id: oid ?? "", tracking_number: tn ?? "" };
    })
    .filter((i) => i.order_id && i.tracking_number)
);

function handleSubmit() {
  if (parsedItems.value.length === 0) return;
  emit("submit", parsedItems.value);
}
</script>

<template>
  <div class="space-y-app">
    <div>
      <label class="block text-sm text-slate-600 mb-1">
        粘贴数据（每行：订单号,快递单号）
      </label>
      <textarea
        v-model="batchText"
        rows="6"
        class="w-full px-3 py-1.5 border border-slate-300 rounded text-sm font-mono focus:outline-none focus:ring-2 focus:ring-brand/35"
        placeholder="3735560095122745088,JT00000001&#10;3735560095122745089,JT00000002"
      />
    </div>
    <div class="text-xs text-slate-500">
      已识别 <strong>{{ parsedItems.length }}</strong> 条有效数据
    </div>
    <button
      :disabled="parsedItems.length === 0"
      class="w-full px-4 py-2 bg-brand-soft0 text-white text-sm rounded hover:bg-brand-deep disabled:opacity-50 transition-colors"
      @click="handleSubmit"
    >
      开始批量发货
    </button>
  </div>
</template>
