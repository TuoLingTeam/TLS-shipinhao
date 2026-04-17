<script setup lang="ts">
import { computed, ref } from "vue";
import type { MatchStrategy } from "../../types/review";

const props = defineProps<{ strategy: MatchStrategy }>();
const tooltipOpen = ref(false);

const meta = computed(() => {
  switch (props.strategy) {
    case "exact_match":
      return {
        text: "精确匹配",
        badgeClass: "bg-brand-soft text-brand-deep ring-1 ring-brand-tint",
        description: "评分 100：昵称、商品与 SKU 关键信号完全命中，可直接自动带入。",
      };
    case "high_confidence":
      return {
        text: "高置信",
        badgeClass: "bg-brand-soft/80 text-brand ring-1 ring-brand-tint",
        description: "评分达到自动带入阈值，命中度很高，建议优先复核后使用。",
      };
    case "probable_match":
      return {
        text: "可能匹配",
        badgeClass: "bg-amber-100 text-amber-700 ring-1 ring-amber-200",
        description: "评分达到最低匹配阈值，但仍建议人工确认订单后再使用。",
      };
    case "fallback":
    default:
      return {
        text: "仅供参考",
        badgeClass: "bg-slate-100 text-slate-600 ring-1 ring-slate-200",
        description: "未达到自动匹配阈值，仅作为兜底候选展示。",
      };
  }
});
</script>

<template>
  <div class="relative inline-flex">
    <button
      type="button"
      class="inline-flex items-center rounded-full px-3 py-1 text-xs font-semibold transition hover:opacity-90 focus:outline-none focus:ring-2 focus:ring-brand-tint"
      :class="meta.badgeClass"
      :title="`${meta.text}：${meta.description}`"
      @click.stop="tooltipOpen = !tooltipOpen"
      @blur="tooltipOpen = false"
    >
      {{ meta.text }}
    </button>
    <div
      v-if="tooltipOpen"
      class="absolute right-0 top-[calc(100%+8px)] z-10 w-64 rounded-2xl border border-slate-200 bg-white px-3 py-2 text-left text-xs leading-5 text-slate-600 shadow-xl"
    >
      {{ meta.description }}
    </div>
  </div>
</template>
