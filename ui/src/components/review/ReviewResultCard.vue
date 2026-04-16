<script setup lang="ts">
import type { OrderMatchResult } from "../../types/review";
import MatchScoreBadge from "./MatchScoreBadge.vue";
import StatusBadge from "../common/StatusBadge.vue";

defineProps<{ result: OrderMatchResult }>();
</script>

<template>
  <div class="bg-white rounded-lg p-4 border border-slate-200 hover:shadow-sm transition-shadow">
    <div class="flex items-center justify-between">
      <div class="space-y-1">
        <div class="text-sm font-medium text-slate-700">
          订单：<span class="font-mono">{{ result.order_id }}</span>
        </div>
        <div class="text-xs text-slate-500">
          买家：{{ result.buyer_nickname || "-" }}
        </div>
        <div class="text-xs text-slate-500">
          SKU：{{ result.sku_name || result.sku_id || "-" }} / 商品ID：{{ result.product_id || "-" }}
        </div>
        <div class="text-xs text-slate-500 line-clamp-2">
          评价：{{ result.evaluation_content || "（无评价内容）" }}
        </div>
        <div class="text-xs text-slate-500">
          评价ID：{{ result.evaluation_id }}
        </div>
        <div class="text-xs text-slate-500">
          来源：{{ result.source }}
        </div>
      </div>
      <div class="flex items-center gap-2">
        <MatchScoreBadge :score="result.confidence_score" />
        <StatusBadge
          :label="result.matched ? '已匹配' : '未匹配'"
          :variant="result.matched ? 'success' : 'neutral'"
        />
      </div>
    </div>
  </div>
</template>
