<script setup lang="ts">
// 全局 Toast 浮层容器：与 useNotification 配套。
// AppLayout 顶层挂一次，其它任何组件 useNotification().show(message, type)
// 即可弹出 3 秒自动关闭的小提示，避免在 form 下方 inline alert 撑开布局。
import { useNotification } from "./useNotification";

const { toasts, dismiss } = useNotification();

const toneClass: Record<"success" | "error" | "info", string> = {
  success: "border-emerald-200 bg-emerald-50/95 text-emerald-900",
  error: "border-red-200 bg-red-50/95 text-red-900",
  info: "border-slate-200 bg-white/95 text-slate-800",
};
</script>

<template>
  <Teleport to="body">
    <div
      class="toast-container pointer-events-none fixed right-4 top-4 z-50 flex max-w-[min(420px,calc(100vw-2rem))] flex-col gap-2"
      role="region"
      aria-live="polite"
      aria-atomic="true"
    >
      <transition-group name="toast">
        <button
          v-for="toast in toasts"
          :key="toast.id"
          type="button"
          class="toast pointer-events-auto rounded-xl border px-4 py-3 text-left text-sm font-medium shadow-lg backdrop-blur transition hover:shadow-xl"
          :class="toneClass[toast.type]"
          :title="`点击关闭 · 默认 3 秒后自动消失`"
          @click="dismiss(toast.id)"
        >
          {{ toast.message }}
        </button>
      </transition-group>
    </div>
  </Teleport>
</template>

<style scoped>
.toast-enter-active,
.toast-leave-active {
  transition:
    opacity 200ms ease,
    transform 200ms ease;
}
.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translateY(-8px);
}
.toast-leave-active {
  position: absolute;
  right: 0;
}
</style>
