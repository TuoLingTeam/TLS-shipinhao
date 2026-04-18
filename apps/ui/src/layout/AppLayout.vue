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
  <div class="app-frame flex min-h-dvh overflow-hidden bg-transparent p-4 text-slate-900 lg:p-5">
    <AppSidebar />
    <div class="app-content-shell ml-0 flex min-w-0 flex-1 flex-col gap-4 overflow-hidden lg:ml-4">
      <slot />
      <AppHeader />
      <main class="min-h-0 flex-1 overflow-y-auto pr-1">
        <div class="app-page-shell mx-auto flex w-full max-w-[1480px] flex-col gap-5 pb-8">
          <RouterView />
        </div>
      </main>
    </div>
  </div>
</template>
