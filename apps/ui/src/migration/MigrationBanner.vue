<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

interface MigrationError {
  step: string;
  message: string;
}

interface MigrationReport {
  legacy_detected: boolean;
  cache_migrated: boolean;
  cookie_migrated: boolean;
  license_migrated: boolean;
  config_pointer_migrated: boolean;
  backup_dir: string | null;
  errors: MigrationError[];
}

const DISMISS_KEY = "legacy_migration_banner_dismissed_v1";

const loading = ref(true);
const report = ref<MigrationReport | null>(null);
const visible = ref(false);
const expanded = ref(false);

function readDismissed(): boolean {
  if (typeof window === "undefined") return false;
  return window.localStorage.getItem(DISMISS_KEY) === "1";
}

function dismiss() {
  visible.value = false;
  if (typeof window !== "undefined") {
    window.localStorage.setItem(DISMISS_KEY, "1");
  }
}

function migratedItems(result: MigrationReport): string[] {
  return [
    result.cache_migrated ? "订单缓存" : "",
    result.cookie_migrated ? "Cookie 配置" : "",
    result.license_migrated ? "授权信息" : "",
    result.config_pointer_migrated ? "配置目录指针" : "",
  ].filter(Boolean);
}

onMounted(async () => {
  if (readDismissed()) {
    loading.value = false;
    return;
  }

  try {
    const result = await invoke<MigrationReport>("start_legacy_migration");
    report.value = result;
    visible.value = result.legacy_detected;
  } catch {
    visible.value = false;
  } finally {
    loading.value = false;
  }
});
</script>

<template>
  <section v-if="!loading && visible && report" class="surface-panel border border-brand-tint/90 px-5 py-4">
    <div class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
      <div class="min-w-0">
        <div class="flex flex-wrap items-center gap-2">
          <span class="status-badge status-badge-active">检测到 Python 4.3.0 本地数据</span>
          <span v-if="report.errors.length" class="status-badge status-badge-warning">迁移中有 {{ report.errors.length }} 条提示</span>
          <span v-else class="status-badge status-badge-active">迁移检查完成</span>
        </div>

        <div class="mt-2 text-sm leading-6 text-brand-deep">
          已在启动时自动检查并迁移旧版本地数据，避免你首次进入 5.1.0 后还要重新同步订单或重新配置环境。
        </div>

        <div class="mt-3 text-sm leading-6 text-slate-700">
          <template v-if="migratedItems(report).length">
            本次已处理：{{ migratedItems(report).join("、") }}。
          </template>
          <template v-else>
            已检测到旧版目录，但当前新目录已有同名数据，因此未覆盖现有文件。
          </template>
        </div>

        <div v-if="report.backup_dir" class="mt-2 text-xs text-slate-500">
          备份目录：<span class="font-mono">{{ report.backup_dir }}</span>
        </div>

        <div v-if="expanded && report.errors.length" class="mt-4 rounded-2xl border border-amber-200 bg-amber-50/80 p-4 text-sm text-amber-900">
          <div class="font-semibold">迁移提示</div>
          <ul class="mt-2 space-y-2">
            <li v-for="item in report.errors" :key="`${item.step}-${item.message}`">
              <div class="font-mono text-xs text-amber-800">{{ item.step }}</div>
              <div class="mt-1 leading-6">{{ item.message }}</div>
            </li>
          </ul>
        </div>
      </div>

      <div class="flex shrink-0 flex-wrap gap-2 lg:justify-end">
        <button
          v-if="report.errors.length"
          type="button"
          class="action-btn action-btn-secondary"
          @click="expanded = !expanded"
        >
          {{ expanded ? "收起详情" : "查看详情" }}
        </button>
        <button type="button" class="action-btn action-btn-primary" @click="dismiss">我知道了</button>
      </div>
    </div>
  </section>
</template>
