<script setup lang="ts">
import { computed, ref } from "vue";
import { useLicense } from "../license/useLicense";
import { useAppStore } from "../stores/app";
import { formatDateTime } from "../shared/format";
import { LICENSE_STATE_LABELS } from "../license/license.types";

const appStore = useAppStore();
const { activateLicense, verifyLicense, activateLoading, verifyLoading } = useLicense();

const licenseKey = ref("");
const message = ref<string | null>(null);
const messageType = ref<"success" | "error">("success");

async function handleActivate() {
  if (!licenseKey.value) return;
  const result = await activateLicense(licenseKey.value);
  if (result) {
    message.value = result.message ?? null;
    messageType.value = result.success ? "success" : "error";
  }
}

async function handleRefresh() {
  const key = appStore.licenseKey || licenseKey.value;
  if (!key) {
    message.value = "暂无已保存卡密，无法刷新状态";
    messageType.value = "error";
    return;
  }
  const result = await verifyLicense(key);
  if (result) {
    message.value = result.message ?? "状态已刷新";
    messageType.value = result.success ? "success" : "error";
  }
}

const currentStateText = computed(() => LICENSE_STATE_LABELS[appStore.licenseState] ?? LICENSE_STATE_LABELS.unknown);
const stateTone = computed(() => (appStore.isLicensed ? 'bg-brand-soft text-brand-deep' : 'bg-slate-100 text-slate-600'));
</script>

<template>
  <div class="grid grid-cols-1 gap-4 xl:grid-cols-[1.1fr_0.95fr]">
    <section class="surface-panel p-5 lg:p-6">
      <div class="flex items-center justify-between gap-4">
        <h2 class="text-xl font-semibold tracking-tight text-slate-900">激活卡密</h2>
        <div class="rounded-2xl px-3 py-2 text-sm font-semibold" :class="stateTone">
          {{ currentStateText }}
        </div>
      </div>

      <div class="mt-5 space-y-4">
        <div>
          <label class="field-label">卡密</label>
          <input v-model.trim="licenseKey" class="field-input" placeholder="输入卡密" />
        </div>
        <div class="flex flex-col gap-3 sm:flex-row">
          <button :disabled="activateLoading" class="action-btn action-btn-primary flex-1" @click="handleActivate">
            {{ activateLoading ? "激活中..." : "激活" }}
          </button>
          <button :disabled="verifyLoading" class="action-btn action-btn-secondary sm:min-w-[140px]" @click="handleRefresh">
            {{ verifyLoading ? "刷新中..." : "刷新状态" }}
          </button>
        </div>
      </div>

      <div v-if="message" class="mt-5 soft-alert" :class="messageType === 'success' ? 'success' : 'error'">
        {{ message }}
      </div>
    </section>

    <section class="surface-panel p-5 lg:p-6">
      <h2 class="text-xl font-semibold tracking-tight text-slate-900">授权快照</h2>
      <div class="mt-5 space-y-3 text-sm">
        <div class="flex justify-between rounded-2xl bg-slate-50 px-4 py-3">
          <span class="text-slate-500">状态</span>
          <span class="font-semibold text-slate-900">{{ currentStateText }}</span>
        </div>
        <div class="flex justify-between rounded-2xl bg-slate-50 px-4 py-3">
          <span class="text-slate-500">版本</span>
          <span class="font-semibold text-slate-900">{{ appStore.appVersion }}</span>
        </div>
        <div class="flex justify-between gap-3 rounded-2xl bg-slate-50 px-4 py-3">
          <span class="text-slate-500">卡密</span>
          <span class="break-all text-right font-mono text-xs text-slate-700">{{ appStore.licenseKey || "未保存" }}</span>
        </div>
        <div class="flex justify-between gap-3 rounded-2xl bg-slate-50 px-4 py-3">
          <span class="text-slate-500">到期</span>
          <span class="text-right font-medium text-slate-700">{{ formatDateTime(appStore.licenseExpiresAt) }}</span>
        </div>
        <div class="flex justify-between gap-3 rounded-2xl bg-slate-50 px-4 py-3">
          <span class="text-slate-500">校验时间</span>
          <span class="text-right font-medium text-slate-700">{{ formatDateTime(appStore.lastVerifiedAt) }}</span>
        </div>
      </div>
    </section>
  </div>
</template>
