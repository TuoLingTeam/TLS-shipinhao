<script setup lang="ts">
import AppLayout from "./layout/AppLayout.vue";
import MigrationBanner from "./migration/MigrationBanner.vue";
import UpdateBanner from "./update/UpdateBanner.vue";
import { useLayout } from "./layout/useLayout";
import { useUiScale } from "./layout/useUiScale";

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
