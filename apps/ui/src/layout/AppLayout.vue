<script setup lang="ts">
import { onMounted } from "vue";
import AppSidebar from "./AppSidebar.vue";
import AppHeader from "./AppHeader.vue";
import { useLicense } from "../license/useLicense";

const { refreshStoredLicenseStatus } = useLicense();

onMounted(() => {
  void refreshStoredLicenseStatus();
});
</script>

<template>
  <div
    class="app-frame flex h-dvh min-h-0 overflow-hidden bg-transparent p-2 text-slate-900 sm:p-3 md:p-4 lg:p-5"
    style="padding-left: max(0.5rem, env(safe-area-inset-left)); padding-right: max(0.5rem, env(safe-area-inset-right))"
  >
    <AppSidebar />
    <div class="app-content-shell ml-2 flex min-w-0 flex-1 flex-col gap-5 overflow-hidden sm:ml-3 lg:ml-4 lg:gap-6">
      <slot />
      <AppHeader />
    <main class="flex min-h-0 min-w-0 flex-1 flex-col overflow-y-auto pr-0.5 sm:pr-1">
      <div class="app-page-shell mx-auto flex w-full min-w-0 max-w-[1480px] flex-1 min-h-0 flex-col gap-app pb-4 sm:pb-6 lg:pb-8">
        <RouterView />
      </div>
    </main>
    </div>
  </div>
</template>
