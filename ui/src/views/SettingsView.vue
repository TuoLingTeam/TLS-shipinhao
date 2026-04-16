<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

const cookieHeader = ref("");
const saved = ref(false);
const saveError = ref<string | null>(null);
const loadError = ref<string | null>(null);
const hasBizMagic = ref(false);
const cookieConfigured = ref(false);
const cookiePath = ref("");
const loginLoading = ref(false);
const extractLoading = ref(false);
const pickDirLoading = ref(false);

async function loadCookieStatus() {
  loadError.value = null;
  try {
    const status = await invoke<{
      configured: boolean;
      has_biz_magic: boolean;
      cookie_path: string;
    }>("get_cookie_status");
    hasBizMagic.value = status.has_biz_magic;
    cookieConfigured.value = status.configured;
    cookiePath.value = status.cookie_path;
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
    const res = await invoke<{ success: boolean; biz_magic: string | null; cookie_path: string }>("set_cookie", {
      cookie_header: raw,
    });
    hasBizMagic.value = Boolean(res.biz_magic);
    cookieConfigured.value = true;
    cookiePath.value = res.cookie_path;
    saved.value = true;
    setTimeout(() => {
      saved.value = false;
    }, 2000);
  } catch (e) {
    saveError.value = typeof e === "string" ? e : String(e);
  }
}

async function handlePickSaveDir() {
  pickDirLoading.value = true;
  saveError.value = null;
  try {
    const result = await invoke<{ selected: boolean; cookie_path: string }>("pick_cookie_save_dir");
    cookiePath.value = result.cookie_path;
  } catch (e) {
    saveError.value = typeof e === "string" ? e : String(e);
  } finally {
    pickDirLoading.value = false;
  }
}

async function handleOpenLogin() {
  loginLoading.value = true;
  saveError.value = null;
  try {
    await invoke("open_cookie_login");
  } catch (e) {
    saveError.value = typeof e === "string" ? e : String(e);
  } finally {
    loginLoading.value = false;
  }
}

async function handleExtractCookie() {
  extractLoading.value = true;
  saveError.value = null;
  try {
    const result = await invoke<{
      success: boolean;
      biz_magic: string | null;
      cookie_header: string;
      cookie_path: string;
    }>("extract_cookie_from_login");
    cookieHeader.value = result.cookie_header;
    cookiePath.value = result.cookie_path;
    hasBizMagic.value = Boolean(result.biz_magic);
    cookieConfigured.value = true;
    saved.value = true;
    setTimeout(() => {
      saved.value = false;
    }, 2000);
  } catch (e) {
    saveError.value = typeof e === "string" ? e : String(e);
  } finally {
    extractLoading.value = false;
  }
}

onMounted(() => {
  void loadCookieStatus();
});
</script>

<template>
  <div class="space-y-5">
    <section class="surface-panel p-5 lg:p-6">
      <div class="flex items-center justify-between gap-4">
        <h2 class="text-xl font-semibold tracking-tight text-slate-900">Cookie 配置</h2>
        <div
          class="rounded-2xl px-3 py-2 text-xs font-semibold uppercase tracking-[0.18em]"
          :class="cookieConfigured ? 'bg-green-100 text-green-700' : 'bg-slate-100 text-slate-500'"
        >
          {{ cookieConfigured ? '已配置' : '未配置' }}
        </div>
      </div>

      <div class="mt-5 space-y-4">
        <div class="rounded-[20px] border border-slate-200 bg-slate-50/80 p-4">
          <div class="text-sm font-semibold text-slate-700">当前保存位置</div>
          <div class="mt-2 break-all font-mono text-xs leading-6 text-slate-500">{{ cookiePath || "未设置" }}</div>
        </div>

        <div class="flex flex-wrap gap-3">
          <button class="action-btn action-btn-secondary" :disabled="pickDirLoading" @click="handlePickSaveDir">
            {{ pickDirLoading ? "选择中..." : "选择保存目录" }}
          </button>
          <button class="action-btn action-btn-secondary" :disabled="loginLoading" @click="handleOpenLogin">
            {{ loginLoading ? "打开登录页中..." : "打开登录页" }}
          </button>
          <button class="action-btn action-btn-primary" :disabled="extractLoading" @click="handleExtractCookie">
            {{ extractLoading ? "提取中..." : "自动提取" }}
          </button>
        </div>

        <div>
          <label class="field-label">手动覆盖 Cookie</label>
          <textarea
            v-model.trim="cookieHeader"
            rows="5"
            class="field-textarea font-mono text-sm"
            placeholder="粘贴完整的 Cookie 字符串..."
          />
        </div>

        <div class="flex items-center gap-3">
          <button class="action-btn action-btn-primary" @click="handleSave">保存</button>
          <span v-if="saved" class="text-sm font-semibold text-green-600">已保存</span>
          <span v-if="hasBizMagic" class="text-sm text-slate-500">已识别 biz_magic</span>
        </div>

        <p v-if="loadError" class="text-sm text-amber-600">{{ loadError }}</p>
        <p v-if="saveError" class="text-sm text-red-600">{{ saveError }}</p>
      </div>
    </section>
  </div>
</template>
