<script setup lang="ts">
import { onMounted, onUnmounted } from "vue";
import AppLayout from "./layout/AppLayout.vue";
import MigrationBanner from "./shared/MigrationBanner.vue";
import { useLayout } from "./layout/useLayout";
import { useUiScale } from "./layout/useUiScale";
import { useCookieHealthStore } from "@/stores/cookieHealth";
import { useStoreContextStore } from "@/stores/storeContext";
import { useUpdateCheckStore } from "@/stores/updateCheck";
import { useLicense } from "@/services/license";

useUiScale();
const { mode } = useLayout();
const cookieHealth = useCookieHealthStore();
const storeContext = useStoreContextStore();
const updateCheck = useUpdateCheckStore();
const { restoreStoredLicenseIfNeeded } = useLicense();

onMounted(async () => {
  cookieHealth.start();
  updateCheck.start();
  void storeContext.refresh().catch(() => {
    // 启动阶段允许静默失败，设置页/顶部店铺选择器会继续提供手动刷新入口。
  });
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
  updateCheck.stop();
});
</script>

<template>
  <div class="app-shell" :data-layout="mode">
    <AppLayout>
      <MigrationBanner />
    </AppLayout>
  </div>
</template>
