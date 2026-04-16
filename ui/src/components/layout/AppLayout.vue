<script setup lang="ts">
import { onMounted } from "vue";
import AppSidebar from "./AppSidebar.vue";
import AppHeader from "./AppHeader.vue";
import { useLicense } from "../../composables/useLicense";

const { refreshStoredLicenseStatus } = useLicense();

onMounted(() => {
  void refreshStoredLicenseStatus();
});
</script>

<template>
  <div class="flex h-screen overflow-hidden">
    <AppSidebar />
    <div class="flex-1 flex flex-col overflow-hidden">
      <AppHeader />
      <main class="flex-1 overflow-y-auto p-6 bg-slate-50">
        <RouterView />
      </main>
    </div>
  </div>
</template>
