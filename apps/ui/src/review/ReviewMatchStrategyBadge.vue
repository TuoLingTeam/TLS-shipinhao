<script setup lang="ts">
import { computed, ref } from "vue";
import type { MatchStrategy } from "./review.types";

const props = defineProps<{
  strategy: MatchStrategy;
  /** 后端 build_nickname_reason / build_product_reason / build_time_reason 生成的评分明细。 */
  reasons?: string[];
  /** 候选订单数量，便于排查「候选很多但分数上不去」的情况。 */
  candidateCount?: number;
  /** 候选中的最高基分，辅助诊断「最高分 0」这类极端场景。 */
  topScore?: number;
}>();
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

const hasReasons = computed(() => Array.isArray(props.reasons) && props.reasons.length > 0);
const hasCandidateMeta = computed(
  () => typeof props.candidateCount === "number" || typeof props.topScore === "number",
);
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
      class="absolute right-0 top-[calc(100%+8px)] z-20 w-80 space-y-2 rounded-2xl border border-slate-200 bg-white px-3 py-2.5 text-left text-xs leading-5 text-slate-600 shadow-xl"
    >
      <p class="font-semibold text-slate-700">{{ meta.text }}</p>
      <p class="text-slate-500">{{ meta.description }}</p>
      <div v-if="hasReasons" class="border-t border-slate-100 pt-2">
        <p class="mb-1 font-semibold text-slate-700">评分明细</p>
        <ul class="space-y-1 text-slate-600">
          <li v-for="(reason, idx) in props.reasons" :key="idx" class="break-words">
            · {{ reason }}
          </li>
        </ul>
      </div>
      <div
        v-if="hasCandidateMeta"
        class="flex gap-3 border-t border-slate-100 pt-2 text-[11px] text-slate-500"
      >
        <span v-if="typeof props.candidateCount === 'number'">
          候选 <span class="font-semibold text-slate-700">{{ props.candidateCount }}</span> 条
        </span>
        <span v-if="typeof props.topScore === 'number'">
          最高基分 <span class="font-semibold text-slate-700">{{ props.topScore }}</span>
        </span>
      </div>
    </div>
  </div>
</template>
