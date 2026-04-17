<script setup lang="ts">
import AppLayout from "./components/layout/AppLayout.vue";
import { useUiScale } from "./composables/useUiScale";

useUiScale();
</script>

<template>
  <AppLayout />
</template>
