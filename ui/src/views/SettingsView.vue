<script setup lang="ts">
import { ref } from "vue";

const cookieHeader = ref("");
const saved = ref(false);

function handleSave() {
  // TODO: 调用 Tauri command 保存 Cookie 到 AppState
  saved.value = true;
  setTimeout(() => (saved.value = false), 2000);
}
</script>

<template>
  <div>
    <h2 class="text-xl font-semibold text-slate-700 mb-4">设置</h2>

    <div class="max-w-2xl space-y-6">
      <div class="bg-white rounded-lg p-4 shadow-sm border border-slate-200">
        <h3 class="font-medium text-slate-700 mb-3">Cookie 配置</h3>
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
