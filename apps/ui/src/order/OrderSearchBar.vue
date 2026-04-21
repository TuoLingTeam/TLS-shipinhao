<script setup lang="ts">
import { ref } from "vue";

defineProps<{
  syncDisabled?: boolean;
  syncLabel?: string;
}>();

const emit = defineEmits<{
  search: [keyword: string];
  sync: [];
}>();
const keyword = ref("");
</script>

<template>
  <div class="order-search-shell flex flex-col gap-app sm:flex-row sm:items-center sm:gap-app">
    <label class="field-affix field-affix--leading min-h-10 w-full min-w-0 sm:flex-1" for="order-local-search-input">
      <svg class="field-affix-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <circle cx="11" cy="11" r="7" />
        <path d="m20 20-3.5-3.5" />
      </svg>
      <input
        id="order-local-search-input"
        v-model.trim="keyword"
        type="text"
        aria-label="搜索本地订单缓存"
        placeholder="搜索订单号或买家昵称"
        class="field-input field-input--with-leading-icon min-h-10 w-full min-w-0"
        @keyup.enter="emit('search', keyword)"
      />
    </label>
    <div class="flex w-full gap-app sm:w-auto sm:shrink-0">
      <button
        type="button"
        class="order-search-action action-btn action-btn-primary min-h-10 flex-1 sm:min-w-[112px] sm:flex-initial"
        @click="emit('search', keyword)"
      >
        搜索
      </button>
      <button
        type="button"
        class="order-search-action action-btn action-btn-secondary min-h-10 flex-1 sm:min-w-[128px] sm:flex-initial"
        :disabled="syncDisabled"
        @click="emit('sync')"
      >
        {{ syncLabel || "同步缓存" }}
      </button>
    </div>
  </div>
</template>
