<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import { APP_VERSION, AUTHOR_WECHAT } from "../shared/brand";
import { useLicense } from "../license/useLicense";
import { useAppStore } from "../app.store";
import { formatDateTime } from "../shared/format";
import { LICENSE_STATE_LABELS } from "../license/license.types";
import { useCookieHealthStore } from "../shared/cookieHealth";
import { isSettingsSection } from "../layout/navigation";
import type { SettingsSectionId } from "../layout/navigation";

const appStore = useAppStore();
const cookieHealth = useCookieHealthStore();
const route = useRoute();
const { activateLicense, verifyLicense, activateLoading, verifyLoading } = useLicense();

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

const licenseKey = ref("");
const licenseMessage = ref<string | null>(null);
const licenseMessageType = ref<"success" | "error">("success");

const activeSection = computed<SettingsSectionId>(() => {
  const raw = Array.isArray(route.query.section) ? route.query.section[0] : route.query.section;
  return isSettingsSection(raw) ? raw : "cookie";
});

const currentStateText = computed(() => LICENSE_STATE_LABELS[appStore.licenseState] ?? LICENSE_STATE_LABELS.unknown);
const cookiePathText = computed(() => cookiePath.value || "未设置保存目录");
const licenseExpiresText = computed(() => formatDateTime(appStore.licenseExpiresAt));
const licenseVerifiedText = computed(() => formatDateTime(appStore.lastVerifiedAt));

async function refreshCookieHealth() {
  try {
    await cookieHealth.refreshSilently();
  } catch {
    // 忽略刷新异常，页面上已有错误态文案
  }
}

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

function flashSaved() {
  saved.value = true;
  setTimeout(() => {
    saved.value = false;
  }, 2200);
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
    flashSaved();
    await refreshCookieHealth();
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
    flashSaved();
    await refreshCookieHealth();
  } catch (e) {
    saveError.value = typeof e === "string" ? e : String(e);
  } finally {
    extractLoading.value = false;
  }
}

async function handleActivate() {
  if (!licenseKey.value) return;
  const result = await activateLicense(licenseKey.value);
  if (result) {
    licenseMessage.value = result.message ?? null;
    licenseMessageType.value = result.success ? "success" : "error";
  }
}

async function handleRefresh() {
  const key = appStore.licenseKey || licenseKey.value;
  if (!key) {
    licenseMessage.value = "暂无已保存卡密，无法刷新状态";
    licenseMessageType.value = "error";
    return;
  }
  const result = await verifyLicense(key);
  if (result) {
    licenseMessage.value = result.message ?? "状态已刷新";
    licenseMessageType.value = result.success ? "success" : "error";
  }
}

async function revealSection(section: SettingsSectionId) {
  await nextTick();
  document.getElementById(`settings-section-${section}`)?.scrollIntoView({
    behavior: "smooth",
    block: "start",
  });
}

watch(activeSection, (section, previous) => {
  if (section === previous) return;
  void revealSection(section);
});

onMounted(() => {
  void Promise.all([loadCookieStatus(), refreshCookieHealth()]).finally(() => {
    void revealSection(activeSection.value);
  });
});
</script>

<template>
  <div class="settings-view-shell flex flex-col gap-app">
    <section
      data-testid="settings-panels"
      class="settings-layout"
    >
      <article
        id="settings-section-cookie"
        data-testid="settings-section-cookie"
        class="surface-panel settings-section-card settings-section-card--cookie p-4 lg:p-5"
      >
        <header class="settings-section-head">
          <div class="flex min-w-0 items-start gap-3">
            <span class="settings-card-badge settings-card-badge--cookie" aria-hidden="true">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="h-4 w-4">
                <path d="M21 12a9 9 0 1 1-9-9c.4 0 .7.4.55.78a3 3 0 0 0 3.88 3.88c.36-.14.74.16.73.53a3 3 0 0 0 3.58 3.06c.35-.09.74.18.73.55.01.07.01.14.01.2Z" />
                <path d="M8.5 9.5h.01" />
                <path d="M11.5 13.5h.01" />
                <path d="M15.5 15.5h.01" />
                <path d="M8 15h.01" />
              </svg>
            </span>
            <div class="min-w-0">
              <h3 class="settings-section-title">Cookie 配置</h3>
            </div>
          </div>
          <div v-if="!cookieConfigured || !hasBizMagic" class="subsystem-chipbar">
            <span v-if="!cookieConfigured" class="subsystem-chip subsystem-chip--warn">未配置</span>
            <span v-if="!hasBizMagic" class="subsystem-chip subsystem-chip--warn">待识别 biz_magic</span>
          </div>
        </header>

        <div class="settings-cookie-body">
          <label class="field-label">手动覆盖 Cookie</label>
          <textarea
            data-testid="settings-cookie-textarea"
            v-model.trim="cookieHeader"
            class="field-textarea settings-cookie-textarea font-mono text-sm"
            placeholder="粘贴完整的 Cookie 字符串..."
          />
          <div v-if="saved" class="settings-field-footer">
            <span class="settings-inline-note is-success">Cookie 已保存</span>
          </div>

          <div data-testid="settings-cookie-path" class="settings-cookie-path-box">
            <div class="settings-cookie-path-label">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="h-3.5 w-3.5">
                <path d="M3 7.5A1.5 1.5 0 0 1 4.5 6h4.2l1.5 1.8h9.3a1.5 1.5 0 0 1 1.5 1.5v7.2a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 16.5Z" />
              </svg>
              保存位置
            </div>
            <div class="settings-cookie-path-value font-mono">{{ cookiePathText }}</div>
          </div>

          <div
            data-testid="settings-cookie-actions"
            class="settings-action-card"
          >
            <div class="settings-action-buttons-grid settings-action-buttons-grid--2x2">
              <button type="button" class="action-btn action-btn-primary min-h-10" @click="handleSave">
                保存 Cookie
              </button>
              <button
                type="button"
                class="action-btn action-btn-primary min-h-10"
                :disabled="extractLoading"
                @click="handleExtractCookie"
              >
                {{ extractLoading ? "提取中..." : "自动提取 Cookie" }}
              </button>
              <button
                type="button"
                class="action-btn action-btn-secondary min-h-10"
                :disabled="loginLoading"
                @click="handleOpenLogin"
              >
                {{ loginLoading ? "打开登录页中..." : "打开登录页" }}
              </button>
              <button
                type="button"
                class="action-btn action-btn-secondary min-h-10"
                :disabled="pickDirLoading"
                @click="handlePickSaveDir"
              >
                {{ pickDirLoading ? "选择中..." : "选择保存目录" }}
              </button>
            </div>
          </div>
        </div>

        <p v-if="loadError" class="text-xs text-amber-600">{{ loadError }}</p>
        <p v-if="saveError" class="text-xs text-red-600">{{ saveError }}</p>
      </article>

      <article
        id="settings-section-license"
        data-testid="settings-section-license"
        class="surface-panel settings-section-card settings-section-card--license p-4 lg:p-5"
        :class="{ 'is-active': activeSection === 'license' }"
      >
        <header class="settings-section-head">
          <div class="flex min-w-0 items-start gap-3">
            <span class="settings-card-badge settings-card-badge--license" aria-hidden="true">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="h-4 w-4">
                <path d="M12 3 19 6v5c0 4.4-2.83 8.45-7 9.75C7.83 19.45 5 15.4 5 11V6z" />
                <path d="m9.4 11.8 1.7 1.7 3.5-3.8" />
              </svg>
            </span>
            <div class="min-w-0">
              <h3 class="settings-section-title">授权信息</h3>
            </div>
          </div>
          <span class="settings-badge" :class="appStore.isLicensed ? 'is-positive' : 'is-warning'">{{ currentStateText }}</span>
        </header>

        <div class="settings-info-grid settings-info-grid--single">
          <div class="settings-info-item">
            <span class="settings-info-label">状态</span>
            <span class="settings-info-value">{{ currentStateText }}</span>
          </div>
          <div class="settings-info-item">
            <span class="settings-info-label">到期</span>
            <span class="settings-info-value">{{ licenseExpiresText }}</span>
          </div>
          <div class="settings-info-item">
            <span class="settings-info-label">校验</span>
            <span class="settings-info-value">{{ licenseVerifiedText }}</span>
          </div>
          <div class="settings-info-item">
            <span class="settings-info-label">卡密</span>
            <span class="settings-info-value settings-info-value--mono">{{ appStore.licenseKey || "未保存" }}</span>
          </div>
        </div>

        <div
          data-testid="settings-license-actions"
          class="settings-action-card"
        >
          <input
            v-model.trim="licenseKey"
            class="field-input settings-license-field min-h-10 w-full min-w-0"
            placeholder="输入卡密"
            aria-label="卡密"
          />
          <div class="settings-action-row">
            <button
              :disabled="activateLoading"
              class="action-btn action-btn-primary min-h-10 min-w-0 flex-1 cursor-pointer"
              @click="handleActivate"
            >
              {{ activateLoading ? "激活中..." : "立即激活" }}
            </button>
            <button
              :disabled="verifyLoading"
              class="action-btn action-btn-secondary min-h-10 min-w-0 flex-1 cursor-pointer"
              @click="handleRefresh"
            >
              {{ verifyLoading ? "刷新中..." : "刷新状态" }}
            </button>
          </div>
        </div>

        <div v-if="licenseMessage" class="soft-alert" :class="licenseMessageType === 'success' ? 'success' : 'error'">
          {{ licenseMessage }}
        </div>
      </article>

      <article
        id="settings-section-about"
        data-testid="settings-section-about"
        class="surface-panel settings-section-card settings-section-card--about p-4 lg:p-5"
        :class="{ 'is-active': activeSection === 'about' }"
      >
        <header class="settings-section-head">
          <div class="flex min-w-0 items-start gap-3">
            <span class="settings-card-badge settings-card-badge--about" aria-hidden="true">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="h-4 w-4">
                <circle cx="12" cy="12" r="9" />
                <path d="M12 8.5v4" />
                <path d="M12 15.5h.01" />
              </svg>
            </span>
            <div class="min-w-0">
              <h3 class="settings-section-title">应用信息</h3>
            </div>
          </div>
        </header>

        <div data-testid="settings-about-meta" class="settings-info-grid">
          <div class="settings-info-item">
            <span class="settings-info-label">版本</span>
            <span class="settings-info-value settings-info-value--mono">v{{ APP_VERSION || appStore.appVersion }}</span>
          </div>
          <div class="settings-info-item">
            <span class="settings-info-label">作者微信</span>
            <span class="settings-info-value settings-info-value--mono">{{ AUTHOR_WECHAT }}</span>
          </div>
        </div>
      </article>
    </section>
  </div>
</template>
