<script setup lang="ts">
import { useRoute } from "vue-router";

const route = useRoute();

const navItems = [
  { path: "/", label: "仪表盘", icon: "📊" },
  { path: "/review", label: "中差评管理", icon: "⭐" },
  { path: "/order", label: "订单管理", icon: "📦" },
  { path: "/delivery", label: "发货管理", icon: "🚚" },
  { path: "/license", label: "授权管理", icon: "🔑" },
  { path: "/settings", label: "设置", icon: "⚙️" },
];

function isActive(path: string): boolean {
  if (path === "/") return route.path === "/";
  return route.path.startsWith(path);
}
</script>

<template>
  <aside class="w-56 min-h-screen bg-slate-800 text-slate-300 flex flex-col">
    <div class="px-5 py-4 text-lg font-bold text-white tracking-wide">
      TLS 视频号
    </div>
    <nav class="flex-1 mt-2">
      <RouterLink
        v-for="item in navItems"
        :key="item.path"
        :to="item.path"
        class="flex items-center gap-3 px-5 py-2.5 text-sm transition-colors hover:bg-slate-700 hover:text-white"
        :class="{ 'bg-slate-700 text-white': isActive(item.path) }"
      >
        <span class="text-base">{{ item.icon }}</span>
        <span>{{ item.label }}</span>
      </RouterLink>
    </nav>
    <div class="px-5 py-3 text-xs text-slate-500">v5.0.0</div>
  </aside>
</template>
