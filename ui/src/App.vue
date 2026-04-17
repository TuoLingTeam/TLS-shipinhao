<script setup lang="ts">
import AppLayout from "./components/layout/AppLayout.vue";
import { useLayout } from "./composables/useLayout";
import { useUiScale } from "./composables/useUiScale";

useUiScale();
const { mode } = useLayout();
</script>

<template>
  <div class="app-shell" :data-layout="mode">
    <AppLayout />
  </div>
</template>
