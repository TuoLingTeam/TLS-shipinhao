<script setup lang="ts">
defineProps<{
  open: boolean;
  title: string;
  message: string;
  confirmText?: string;
  cancelText?: string;
}>();

defineEmits<{
  confirm: [];
  cancel: [];
}>();
</script>

<template>
  <Teleport to="body">
    <div
      v-if="open"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/40"
      @click.self="$emit('cancel')"
    >
      <div class="bg-white rounded-lg shadow-xl p-6 w-96 max-w-[90vw]">
        <h3 class="text-lg font-semibold text-slate-800">{{ title }}</h3>
        <p class="mt-2 text-sm text-slate-600">{{ message }}</p>
        <div class="mt-app flex justify-end gap-app">
          <button
            class="px-4 py-1.5 text-sm border border-slate-300 rounded text-slate-600 hover:bg-slate-50 transition-colors"
            @click="$emit('cancel')"
          >
            {{ cancelText ?? "取消" }}
          </button>
          <button
            class="px-4 py-1.5 text-sm bg-brand text-white rounded hover:bg-brand transition-colors"
            @click="$emit('confirm')"
          >
            {{ confirmText ?? "确认" }}
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>
