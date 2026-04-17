<script setup lang="ts">
import AppLayout from "./components/layout/AppLayout.vue";
import MigrationBanner from "./components/layout/MigrationBanner.vue";
import UpdateBanner from "./components/layout/UpdateBanner.vue";
import { useLayout } from "./composables/useLayout";
import { useUiScale } from "./composables/useUiScale";

useUiScale();
const { mode } = useLayout();
</script>

<template>
  <div class="app-shell" :data-layout="mode">
    <AppLayout>
      <MigrationBanner />
      <UpdateBanner />
    </AppLayout>
  </div>
</template>
