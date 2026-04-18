<script setup lang="ts">
import { onMounted, onUnmounted } from "vue";
import AppLayout from "./layout/AppLayout.vue";
import MigrationBanner from "./migration/MigrationBanner.vue";
import UpdateBanner from "./update/UpdateBanner.vue";
import CookieHealthBanner from "./shared/CookieHealthBanner.vue";
import { useLayout } from "./layout/useLayout";
import { useUiScale } from "./layout/useUiScale";
import { useCookieHealthStore } from "./shared/cookieHealth";
import { useLicense } from "./license/useLicense";

useUiScale();
const { mode } = useLayout();
const cookieHealth = useCookieHealthStore();
const { restoreStoredLicenseIfNeeded } = useLicense();

onMounted(async () => {
  cookieHealth.start();
  // 启动时尝试用已保存卡密自动恢复 Lease：
  // 仅在后端判定为"半孤立 profile"时发起一次远端 verify，避免打扰正常用户。
  try {
    await restoreStoredLicenseIfNeeded();
  } catch {
    // 离线或网络异常时保持当前 UI 状态，用户也可手动点"刷新状态"。
  }
});
onUnmounted(() => {
  cookieHealth.stop();
});
</script>

<template>
  <div class="app-shell" :data-layout="mode">
    <AppLayout>
      <MigrationBanner />
      <UpdateBanner />
      <CookieHealthBanner />
    </AppLayout>
  </div>
</template>
