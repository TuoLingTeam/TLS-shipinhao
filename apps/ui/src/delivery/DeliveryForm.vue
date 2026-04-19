<script setup lang="ts">
import { ref } from "vue";

const emit = defineEmits<{
  submit: [orderId: string, trackingNumber: string, carrierCode: string];
}>();

const orderId = ref("");
const trackingNumber = ref("");
const carrierCode = ref("JT");

function handleSubmit() {
  if (!orderId.value || !trackingNumber.value) return;
  emit("submit", orderId.value, trackingNumber.value, carrierCode.value);
}

const carriers = [
  { code: "JT", label: "极兔速递" },
  { code: "YTO", label: "圆通速递" },
  { code: "ZTO", label: "中通快递" },
  { code: "STO", label: "申通快递" },
  { code: "YD", label: "韵达快递" },
  { code: "SF", label: "顺丰速运" },
];
</script>

<template>
  <div class="space-y-app">
    <div>
      <label class="block text-sm text-slate-600 mb-1">订单号</label>
      <input
        v-model="orderId"
        class="w-full px-3 py-1.5 border border-slate-300 rounded text-sm focus:outline-none focus:ring-2 focus:ring-brand/35"
        placeholder="输入订单号"
      />
    </div>
    <div>
      <label class="block text-sm text-slate-600 mb-1">快递单号</label>
      <input
        v-model="trackingNumber"
        class="w-full px-3 py-1.5 border border-slate-300 rounded text-sm focus:outline-none focus:ring-2 focus:ring-brand/35"
        placeholder="输入快递单号"
      />
    </div>
    <div>
      <label class="block text-sm text-slate-600 mb-1">快递公司</label>
      <select
        v-model="carrierCode"
        class="w-full px-3 py-1.5 border border-slate-300 rounded text-sm focus:outline-none focus:ring-2 focus:ring-brand/35"
      >
        <option v-for="c in carriers" :key="c.code" :value="c.code">{{ c.label }}</option>
      </select>
    </div>
    <button
      class="w-full px-4 py-2 bg-brand text-white text-sm rounded hover:bg-brand transition-colors"
      @click="handleSubmit"
    >
      确认发货
    </button>
  </div>
</template>
