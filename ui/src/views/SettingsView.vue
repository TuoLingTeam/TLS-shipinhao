<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

const cookieHeader = ref("");
const saved = ref(false);
const saveError = ref<string | null>(null);
const loadError = ref<string | null>(null);
const hasBizMagic = ref(false);
const cookieConfigured = ref(false);

async function loadCookieStatus() {
  loadError.value = null;
  try {
    const status = await invoke<{
      configured: boolean;
      has_biz_magic: boolean;
    }>("get_cookie_status");
    hasBizMagic.value = status.has_biz_magic;
    cookieConfigured.value = status.configured;
  } catch (e) {
    loadError.value = typeof e === "string" ? e : String(e);
  }
}

async function handleSave() {
  saveError.value = null;
  const raw = cookieHeader.value.trim();
  if (!raw) {
    saveError.value = "请先粘贴 Cookie 字符串";
    return;
  }
  try {
    const res = await invoke<{ success: boolean; biz_magic: string | null }>(
      "set_cookie",
      { cookie_header: raw },
    );
    hasBizMagic.value = Boolean(res.biz_magic);
    cookieConfigured.value = true;
    saved.value = true;
    setTimeout(() => (saved.value = false), 2000);
  } catch (e) {
    saveError.value = typeof e === "string" ? e : String(e);
  }
}

onMounted(() => {
  void loadCookieStatus();
});
</script>

<template>
  <div>
    <h2 class="text-xl font-semibold text-slate-700 mb-4">设置</h2>

    <div class="max-w-2xl space-y-6">
      <div class="bg-white rounded-lg p-4 shadow-sm border border-slate-200">
        <h3 class="font-medium text-slate-700 mb-3">Cookie 配置</h3>
        <p v-if="cookieConfigured" class="text-xs text-green-700 mb-2">
          当前会话已保存 Cookie；下方粘贴新值可覆盖。
        </p>
        <div class="space-y-3">
          <div>
            <label class="block text-sm text-slate-600 mb-1">
              视频号商家后台 Cookie
            </label>
            <textarea
              v-model="cookieHeader"
              rows="4"
              class="w-full px-3 py-1.5 border border-slate-300 rounded text-sm font-mono focus:outline-none focus:ring-2 focus:ring-blue-500"
              placeholder="粘贴完整的 Cookie 字符串..."
            />
            <p class="mt-1 text-xs text-slate-400">
              从浏览器开发者工具中复制完整的 Cookie 请求头
            </p>
          </div>
          <button
            class="px-4 py-1.5 bg-blue-500 text-white text-sm rounded hover:bg-blue-600 transition-colors"
            @click="handleSave"
          >
            保存
          </button>
          <span v-if="saved" class="ml-2 text-sm text-green-600">已保存</span>
          <p v-if="hasBizMagic" class="text-xs text-slate-500">
            已解析到 biz_magic，发货等接口将使用该值。
          </p>
          <p v-if="loadError" class="text-xs text-amber-600">{{ loadError }}</p>
          <p v-if="saveError" class="text-xs text-red-600">{{ saveError }}</p>
        </div>
      </div>

      <div class="bg-white rounded-lg p-4 shadow-sm border border-slate-200">
        <h3 class="font-medium text-slate-700 mb-3">关于</h3>
        <div class="text-sm text-slate-600 space-y-1">
          <div>应用：TLS-shipinhao</div>
          <div>版本：5.0.0</div>
          <div>架构：Rust + Tauri 2.0 + Vue 3</div>
        </div>
      </div>
    </div>
  </div>
</template>
