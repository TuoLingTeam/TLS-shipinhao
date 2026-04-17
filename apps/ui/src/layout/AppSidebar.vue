<script setup lang="ts">
import { useRoute } from "vue-router";
import AppNavIcon from "./AppNavIcon.vue";
import { APP_NAME_EN, APP_VERSION, AUTHOR_WECHAT } from "../shared/brand";

const route = useRoute();

const navItems = [
  { path: "/", label: "仪表盘", icon: "dashboard" },
  { path: "/review", label: "评价管理", icon: "review" },
  { path: "/order", label: "订单管理", icon: "order" },
  { path: "/delivery", label: "发货管理", icon: "delivery" },
  { path: "/license", label: "授权管理", icon: "license" },
  { path: "/settings", label: "设置", icon: "settings" },
] as const;

function isActive(path: string): boolean {
  if (path === "/") return route.path === "/";
  return route.path.startsWith(path);
}
</script>

<template>
  <aside class="hidden h-full w-[280px] shrink-0 overflow-hidden rounded-[30px] border border-[var(--color-sidebar-line)] bg-[image:var(--color-sidebar)] p-5 shadow-[0_28px_60px_-32px_rgba(15,23,42,0.85)] lg:flex lg:flex-col">
    <div class="rounded-[24px] border border-white/10 bg-white/5 p-4 backdrop-blur">
      <div class="flex items-center gap-3">
        <div class="flex h-11 w-11 items-center justify-center rounded-2xl bg-white/10 text-amber-300 ring-1 ring-white/10">
          <AppNavIcon name="spark" icon-class="h-5 w-5" />
        </div>
        <div>
          <div class="text-lg font-semibold tracking-tight text-white">驼铃·视频小店差评处理</div>
          <div class="mt-1 text-xs text-slate-300">作者微信 {{ AUTHOR_WECHAT }}</div>
        </div>
      </div>
    </div>

    <nav class="mt-6 flex-1 space-y-2 overflow-y-auto pr-1">
      <RouterLink
        v-for="item in navItems"
        :key="item.path"
        :to="item.path"
        class="sidebar-link cursor-pointer"
        :class="{ active: isActive(item.path) }"
      >
        <span class="sidebar-icon-shell">
          <AppNavIcon :name="item.icon" icon-class="h-[18px] w-[18px]" />
        </span>
        <span class="text-sm font-medium tracking-[0.01em]">{{ item.label }}</span>
      </RouterLink>
    </nav>

    <div class="pt-4 text-xs text-slate-400">{{ APP_NAME_EN }} v{{ APP_VERSION }}</div>
  </aside>
</template>
