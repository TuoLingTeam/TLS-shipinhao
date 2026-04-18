<script setup lang="ts">
import { useRoute } from "vue-router";
import AppNavIcon from "./AppNavIcon.vue";
import { APP_NAME, APP_NAME_EN, APP_VERSION, AUTHOR_WECHAT } from "../shared/brand";
import { navGroups } from "./navigation";

const route = useRoute();

function isActive(path: string): boolean {
  if (path === "/") return route.path === "/";
  return route.path.startsWith(path);
}
</script>

<template>
  <aside class="hidden h-[calc(100dvh-2rem)] w-[288px] shrink-0 self-start overflow-hidden rounded-[28px] border border-[var(--color-sidebar-line)] bg-[image:var(--color-sidebar)] p-4 shadow-[0_28px_72px_-38px_rgba(15,23,42,0.82)] lg:sticky lg:top-5 lg:flex lg:h-[calc(100dvh-2.5rem)] lg:flex-col">
    <div class="rounded-[24px] border border-white/10 bg-white/6 px-4 py-4 backdrop-blur-xl">
      <div class="flex items-center gap-3">
        <div class="flex h-12 w-12 items-center justify-center rounded-[18px] bg-white/10 text-amber-300 ring-1 ring-white/10">
          <AppNavIcon name="spark" icon-class="h-5 w-5" />
        </div>
        <div class="min-w-0 flex-1">
          <div class="truncate text-[1.22rem] font-semibold tracking-tight text-white">{{ APP_NAME }}</div>
          <div class="mt-1 flex items-center gap-2 text-xs text-emerald-50/68">
            <span>{{ AUTHOR_WECHAT }}</span>
            <span>·</span>
            <span>v{{ APP_VERSION }}</span>
          </div>
        </div>
      </div>
    </div>

    <nav class="mt-5 flex-1 space-y-4 overflow-y-auto pr-1">
      <section v-for="group in navGroups" :key="group.id">
        <div class="px-2 text-[10px] font-semibold uppercase tracking-[0.24em] text-emerald-100/42">
          {{ group.label }}
        </div>
        <div class="mt-2 space-y-1.5">
          <RouterLink
            v-for="item in group.items"
            :key="item.path"
            :to="item.path"
            class="sidebar-link cursor-pointer"
            :class="{ active: isActive(item.path) }"
          >
            <span class="sidebar-icon-shell">
              <AppNavIcon :name="item.icon" icon-class="h-[17px] w-[17px]" />
            </span>
            <span class="min-w-0 flex-1 truncate text-sm font-semibold tracking-[0.01em]">{{ item.label }}</span>
          </RouterLink>
        </div>
      </section>
    </nav>

    <div class="pt-3 text-[11px] text-emerald-50/42">{{ APP_NAME_EN }} v{{ APP_VERSION }}</div>
  </aside>
</template>
