<script setup lang="ts">
import { useRoute } from "vue-router";
import AppNavIcon from "./AppNavIcon.vue";
import { APP_NAME, APP_NAME_EN, APP_VERSION } from "../shared/brand";
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
    <div class="flex min-h-[68px] items-center justify-center border-b border-white/10 bg-white/6 px-2 py-2.5 backdrop-blur-xl lg:min-h-[84px] lg:justify-start lg:px-3 lg:py-3">
      <div class="flex min-w-0 items-center gap-app">
        <div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-[14px] bg-white/10 text-amber-300 ring-1 ring-white/10 lg:h-10 lg:w-10 lg:rounded-[16px]">
          <AppNavIcon name="spark" icon-class="h-[18px] w-[18px] lg:h-5 lg:w-5" />
        </div>
        <div class="hidden min-w-0 flex-1 lg:block">
          <div class="truncate text-[1.05rem] font-semibold leading-snug tracking-tight text-white">{{ APP_NAME }}</div>
        </div>
      </div>
    </div>

    <nav class="mt-2 flex-1 space-y-app overflow-y-auto px-1.5 pr-1.5 lg:mt-4 lg:px-2.5 lg:pr-2">
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

    <div class="hidden px-2.5 pb-3 pt-2 text-[10px] leading-tight text-emerald-50/42 lg:block">{{ APP_NAME_EN }} v{{ APP_VERSION }}</div>
  </aside>
</template>
