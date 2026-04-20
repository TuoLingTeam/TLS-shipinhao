<script setup lang="ts">
import { useRoute } from "vue-router";
import AppNavIcon from "./AppNavIcon.vue";
import { APP_NAME } from "../shared/brand";
import { navGroups } from "./navigation";

const route = useRoute();

function isActive(path: string): boolean {
  if (path === "/") return route.path === "/";
  return route.path.startsWith(path);
}
</script>

<template>
  <aside
    class="app-sidebar-shell flex w-[60px] shrink-0 flex-col self-stretch overflow-hidden rounded-[18px] border border-[var(--color-sidebar-line)] bg-[image:var(--color-sidebar)] shadow-[0_28px_72px_-38px_rgba(15,23,42,0.82)] sm:w-[64px] lg:sticky lg:w-[228px] lg:rounded-[22px]"
  >
    <!-- 仅宽侧栏展示应用名；窄栏为纯图标带，避免标题被截成竖条 -->
    <div
      class="hidden min-h-[3.25rem] shrink-0 items-center justify-center border-b border-white/10 bg-white/6 px-3 py-3.5 backdrop-blur-xl lg:flex"
    >
      <div class="min-w-0 max-w-full px-1 text-center">
        <div class="truncate text-[1.02rem] font-semibold leading-snug tracking-tight text-white">
          {{ APP_NAME }}
        </div>
      </div>
    </div>

    <nav class="mt-2 flex-1 space-y-app overflow-y-auto px-1.5 pb-2 pr-1.5 pt-3 lg:mt-3 lg:px-2.5 lg:pb-3 lg:pr-2 lg:pt-0">
      <section v-for="group in navGroups" :key="group.id">
        <div class="hidden px-1 text-[9px] font-semibold uppercase tracking-[0.2em] text-emerald-100/42 lg:block">
          {{ group.label }}
        </div>
        <div class="mt-0 space-y-app lg:mt-2">
          <RouterLink
            v-for="item in group.items"
            :key="item.path"
            :to="item.path"
            class="sidebar-link sidebar-link-collapsed cursor-pointer"
            :class="{ active: isActive(item.path) }"
            :title="item.label"
          >
            <span class="sidebar-icon-shell">
              <AppNavIcon :name="item.icon" icon-class="h-[17px] w-[17px]" />
            </span>
            <span class="sidebar-link-label">{{ item.label }}</span>
          </RouterLink>
        </div>
      </section>
    </nav>
  </aside>
</template>
