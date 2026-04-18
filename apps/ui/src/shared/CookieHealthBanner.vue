<script setup lang="ts">
import { computed } from "vue";
import { RouterLink } from "vue-router";
import { useCookieHealthStore } from "./cookieHealth";
import { formatDateTime } from "./format";

const store = useCookieHealthStore();

const visible = computed(() => {
  if (!store.snapshot.last_checked_at) return false;
  return !store.snapshot.healthy;
});

const tone = computed(() =>
  store.snapshot.configured ? "bg-red-50 border-red-200 text-red-700" : "bg-amber-50 border-amber-200 text-amber-800",
);
const title = computed(() =>
  store.snapshot.configured ? "Cookie 可能已失效" : "尚未配置 Cookie",
);
const description = computed(
  () => store.snapshot.hint || "请前往设置中心重新登录小店并保存最新 Cookie。",
);
</script>

<template>
  <section
    v-if="visible"
    class="rounded-[20px] border px-5 py-4 text-sm shadow-sm"
    :class="tone"
  >
    <div class="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
      <div class="min-w-0">
        <div class="text-base font-semibold">
          {{ title }}
        </div>
        <div class="mt-1 leading-6">{{ description }}</div>
        <div v-if="store.snapshot.last_checked_at" class="mt-1 text-xs opacity-75">
          最近探测：{{ formatDateTime(store.snapshot.last_checked_at) }}
        </div>
      </div>
      <div class="flex shrink-0 flex-wrap gap-2 lg:justify-end">
        <button
          type="button"
          class="action-btn action-btn-secondary"
          :disabled="store.loading"
          @click="store.probe()"
        >
          {{ store.loading ? "探测中..." : "立即重新探测" }}
        </button>
        <RouterLink to="/settings" class="action-btn action-btn-primary">前往设置</RouterLink>
      </div>
    </div>
  </section>
</template>
