<script setup lang="ts">
import { ref } from "vue";
import { useLicense } from "../composables/useLicense";
import { useAppStore } from "../stores/app";

const appStore = useAppStore();
const { activateLicense, activateLoading } = useLicense();

const licenseKey = ref("");
const deviceId = ref("auto-detect");
const message = ref<string | null>(null);
const messageType = ref<"success" | "error">("success");

async function handleActivate() {
  if (!licenseKey.value) return;
  const result = await activateLicense(licenseKey.value, deviceId.value);
  if (result) {
    message.value = result.message;
    messageType.value = result.success ? "success" : "error";
  }
}

const stateLabel: Record<string, string> = {
  active: "已激活",
  renewal_due: "待续期",
  expired: "已过期",
  revoked: "已吊销",
  device_mismatch: "设备不匹配",
  invalid: "未激活",
  compromised: "异常",
};
</script>

<template>
  <div>
    <h2 class="text-xl font-semibold text-slate-700 mb-4">授权管理</h2>

    <div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
      <div class="bg-white rounded-lg p-4 shadow-sm border border-slate-200">
        <h3 class="font-medium text-slate-700 mb-3">激活卡密</h3>
        <div class="space-y-3">
          <div>
            <label class="block text-sm text-slate-600 mb-1">卡密</label>
            <input
              v-model="licenseKey"
              class="w-full px-3 py-1.5 border border-slate-300 rounded text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
              placeholder="输入卡密"
            />
          </div>
          <button
            :disabled="activateLoading"
            class="w-full px-4 py-2 bg-blue-500 text-white text-sm rounded hover:bg-blue-600 disabled:opacity-50 transition-colors"
            @click="handleActivate"
          >
            {{ activateLoading ? "激活中..." : "激活" }}
          </button>
        </div>
        <div
          v-if="message"
          class="mt-3 p-2.5 text-sm rounded"
          :class="messageType === 'success' ? 'bg-green-50 text-green-700' : 'bg-red-50 text-red-700'"
        >
          {{ message }}
        </div>
      </div>

      <div class="bg-white rounded-lg p-4 shadow-sm border border-slate-200">
        <h3 class="font-medium text-slate-700 mb-3">授权状态</h3>
        <div class="space-y-2 text-sm">
          <div class="flex justify-between">
            <span class="text-slate-500">状态</span>
            <span
              class="font-medium"
              :class="appStore.isLicensed ? 'text-green-600' : 'text-slate-600'"
            >
              {{ stateLabel[appStore.licenseState] ?? appStore.licenseState }}
            </span>
          </div>
          <div class="flex justify-between">
            <span class="text-slate-500">版本</span>
            <span class="text-slate-700">{{ appStore.appVersion }}</span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
