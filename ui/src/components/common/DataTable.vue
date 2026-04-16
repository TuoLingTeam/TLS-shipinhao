<script setup lang="ts" generic="T extends Record<string, unknown>">
defineProps<{
  columns: { key: string; label: string; align?: "left" | "center" | "right" }[];
  rows: T[];
  emptyText?: string;
}>();
</script>

<template>
  <div class="bg-white rounded-lg shadow-sm border border-slate-200 overflow-hidden">
    <table class="w-full text-sm">
      <thead class="bg-slate-50 text-slate-600">
        <tr>
          <th
            v-for="col in columns"
            :key="col.key"
            class="px-4 py-2.5 font-medium"
            :class="{
              'text-left': col.align !== 'center' && col.align !== 'right',
              'text-center': col.align === 'center',
              'text-right': col.align === 'right',
            }"
          >
            {{ col.label }}
          </th>
        </tr>
      </thead>
      <tbody>
        <tr
          v-for="(row, i) in rows"
          :key="i"
          class="border-t border-slate-100 hover:bg-slate-50 transition-colors"
        >
          <td
            v-for="col in columns"
            :key="col.key"
            class="px-4 py-2.5"
            :class="{
              'text-center': col.align === 'center',
              'text-right': col.align === 'right',
            }"
          >
            <slot :name="col.key" :row="row" :value="row[col.key]">
              {{ row[col.key] ?? "--" }}
            </slot>
          </td>
        </tr>
        <tr v-if="rows.length === 0">
          <td :colspan="columns.length" class="px-4 py-8 text-center text-slate-400">
            {{ emptyText ?? "暂无数据" }}
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>
