<script setup lang="ts">
import { computed, nextTick, onMounted, onBeforeUnmount, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import { APP_VERSION, AUTHOR_WECHAT } from "../shared/brand";
import { useLicense } from "../license/useLicense";
import { useAppStore } from "../app.store";
import { formatDateTime } from "../shared/format";
import { LICENSE_STATE_LABELS } from "../license/types";
import { useCookieHealthStore } from "../shared/cookieHealth";
import { useRuntimeClock } from "../shared/useRuntimeClock";
import { useUpdateCheckStore } from "../shared/updateCheck";
import { toErrorMessage } from "../shared/toErrorMessage";
import { isSettingsSection } from "../layout/navigation";
import type { SettingsSectionId } from "../layout/navigation";

/** 登录窗口打开后，前端轮询读取登录态的间隔（ms） */
const COOKIE_POLL_INTERVAL_MS = 1500;
/** 轮询最长持续时间（ms），到期后自动停止以避免无谓占用 */
const COOKIE_POLL_TIMEOUT_MS = 5 * 60 * 1000;

const appStore = useAppStore();
const cookieHealth = useCookieHealthStore();
const updateCheck = useUpdateCheckStore();
const route = useRoute();
const { activateLicense, verifyLicense, activateLoading, verifyLoading } = useLicense();
const { clockText, uptimeText } = useRuntimeClock();

/** 最近一次成功保存来源：`auto`=登录窗口轮询；`manual`=手动粘贴后保存 */
const saveNotice = ref<null | "auto" | "manual">(null);
const saveError = ref<string | null>(null);
const loadError = ref<string | null>(null);
const hasBizMagic = ref(false);
const cookieConfigured = ref(false);
const cookiePath = ref("");
const loginLoading = ref(false);
const pickDirLoading = ref(false);
const manualCookie = ref("");
const manualSaveLoading = ref(false);

function handleClearManualCookie() {
  manualCookie.value = "";
  saveError.value = null;
}

const licenseKey = ref("");
const licenseMessage = ref<string | null>(null);
const licenseMessageType = ref<"success" | "error">("success");

const activeSection = computed<SettingsSectionId>(() => {
  const raw = Array.isArray(route.query.section) ? route.query.section[0] : route.query.section;
  return isSettingsSection(raw) ? raw : "cookie";
});

const currentStateText = computed(() => LICENSE_STATE_LABELS[appStore.licenseState] ?? "未知状态");
const cookiePathText = computed(() => cookiePath.value || "未设置保存目录");
// 卡密有效期：100 年级别的硬过期；Lease TTL：3 天级别的短效 Token，到期需联网续约
const licenseExpiresText = computed(() => formatDateTime(appStore.licenseExpiresAt));
const leaseExpiresText = computed(() => formatDateTime(appStore.leaseExpiresAt));
const licenseVerifiedText = computed(() => formatDateTime(appStore.lastVerifiedAt));

let pollTimer: ReturnType<typeof setInterval> | null = null;
let pollDeadline = 0;

function stopCookiePoll() {
  if (pollTimer !== null) {
    clearInterval(pollTimer);
    pollTimer = null;
  }
}

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
    loadError.value = toErrorMessage(e);
  }
}

function flashSaveNotice(kind: "auto" | "manual") {
  saveNotice.value = kind;
  setTimeout(() => {
    if (saveNotice.value === kind) {
      saveNotice.value = null;
    }
  }, 2200);
}

async function handlePickSaveDir() {
  pickDirLoading.value = true;
  saveError.value = null;
  try {
    const result = await invoke<{ selected: boolean; cookie_path: string }>("pick_cookie_save_dir");
    cookiePath.value = result.cookie_path;
  } catch (e) {
    saveError.value = toErrorMessage(e);
  } finally {
    pickDirLoading.value = false;
  }
}

async function tryExtractCookieOnce() {
  try {
    const result = await invoke<{
      success: boolean;
      biz_magic: string | null;
      cookie_header: string;
      cookie_path: string;
    }>("extract_cookie_from_login");
    cookiePath.value = result.cookie_path;
    hasBizMagic.value = Boolean(result.biz_magic);
    cookieConfigured.value = true;
    return true;
  } catch {
    // 登录尚未完成或窗口已关闭：保持轮询继续，由调用方控制超时
    return false;
  }
}

function startCookiePoll() {
  stopCookiePoll();
  pollDeadline = Date.now() + COOKIE_POLL_TIMEOUT_MS;
  pollTimer = setInterval(async () => {
    if (Date.now() > pollDeadline) {
      stopCookiePoll();
      return;
    }
    const ok = await tryExtractCookieOnce();
    if (!ok) return;
    stopCookiePoll();
    flashSaveNotice("auto");
    await refreshCookieHealth();
    try {
      await invoke("close_cookie_login_window");
    } catch {
      // 关窗失败不影响保存结果，留给用户手动关闭
    }
  }, COOKIE_POLL_INTERVAL_MS);
}

async function handleSaveManualCookie() {
  const raw = manualCookie.value.trim();
  if (!raw) {
    saveError.value = "请粘贴 Cookie 内容后再保存";
    return;
  }
  manualSaveLoading.value = true;
  saveError.value = null;
  try {
    await invoke("set_cookie", { cookie_header: raw });
    await loadCookieStatus();
    await refreshCookieHealth();
    flashSaveNotice("manual");
    manualCookie.value = "";
  } catch (e) {
    saveError.value = toErrorMessage(e);
  } finally {
    manualSaveLoading.value = false;
  }
}

async function handleOpenLogin() {
  loginLoading.value = true;
  saveError.value = null;
  try {
    await invoke("open_cookie_login");
    startCookiePoll();
  } catch (e) {
    saveError.value = toErrorMessage(e);
  } finally {
    loginLoading.value = false;
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

onBeforeUnmount(() => {
  stopCookiePoll();
});
</script>

<template>
  <div class="settings-view-shell flex min-h-0 min-w-0 flex-col gap-6 lg:gap-7">
    <section
      data-testid="settings-panels"
      class="settings-layout"
    >
      <article
        id="settings-section-cookie"
        data-testid="settings-section-cookie"
        class="surface-panel settings-section-card settings-section-card--cookie flex min-h-0 h-full flex-col p-5 lg:p-6"
      >
        <header class="settings-section-head shrink-0">
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

        <div class="settings-cookie-body flex min-h-0 flex-1 flex-col">
          <div data-testid="settings-cookie-actions" class="settings-cookie-flows flex min-h-0 flex-1 flex-col">
            <div class="settings-cookie-subpanel settings-cookie-subpanel--auto shrink-0">
              <p class="settings-cookie-subpanel-title">方式一 · 浏览器登录（推荐）</p>
              <p class="settings-cookie-subpanel-hint">在弹出窗口完成登录后，应用会轮询并写入 Cookie；可用下方「选择保存路径」调整落盘目录。</p>
              <div class="settings-action-buttons-grid settings-action-buttons-grid--1x2">
                <button
                  type="button"
                  class="action-btn action-btn-primary min-h-10"
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
                  {{ pickDirLoading ? "选择中..." : "选择保存路径" }}
                </button>
              </div>
              <div data-testid="settings-cookie-path" class="settings-cookie-path-box shrink-0">
                <div class="settings-cookie-path-label">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="h-3.5 w-3.5">
                    <path d="M3 7.5A1.5 1.5 0 0 1 4.5 6h4.2l1.5 1.8h9.3a1.5 1.5 0 0 1 1.5 1.5v7.2a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 16.5Z" />
                  </svg>
                  保存位置
                </div>
                <div class="settings-cookie-path-value font-mono">{{ cookiePathText }}</div>
              </div>
              <div v-if="saveNotice === 'auto'" class="settings-field-footer pt-0.5">
                <span class="settings-inline-note is-success">Cookie 已自动保存</span>
              </div>
            </div>

            <div class="settings-cookie-subpanel settings-cookie-subpanel--manual flex min-h-0 min-w-0 flex-1 flex-col">
              <p class="settings-cookie-subpanel-title shrink-0">方式二 · 手动粘贴</p>
              <div class="settings-cookie-editor flex min-h-0 min-w-0 flex-1 flex-col">
                <textarea
                  id="settings-cookie-textarea"
                  v-model.trim="manualCookie"
                  data-testid="settings-cookie-textarea"
                  class="field-input field-textarea settings-cookie-textarea min-h-0 w-full min-w-0 flex-1 resize-y font-mono"
                  placeholder="粘贴浏览器中复制的完整 Cookie 请求头（含 biz_magic 等字段时将自动解析）"
                  aria-label="Cookie 请求头"
                  spellcheck="false"
                  autocomplete="off"
                />
                <div class="settings-action-buttons-grid settings-action-buttons-grid--1x2 shrink-0">
                  <button
                    type="button"
                    class="action-btn action-btn-ghost min-h-10"
                    data-testid="settings-cookie-clear-manual"
                    :disabled="manualSaveLoading || !manualCookie"
                    @click="handleClearManualCookie"
                  >
                    清除 Cookie
                  </button>
                  <button
                    type="button"
                    class="action-btn action-btn-secondary min-h-10"
                    data-testid="settings-cookie-save-manual"
                    :disabled="manualSaveLoading"
                    @click="handleSaveManualCookie"
                  >
                    {{ manualSaveLoading ? "保存中..." : "保存手动 Cookie" }}
                  </button>
                </div>
              </div>
              <div v-if="saveNotice === 'manual'" class="settings-field-footer pt-0.5">
                <span class="settings-inline-note is-success">手动 Cookie 已保存</span>
              </div>
            </div>
          </div>
        </div>

        <p v-if="loadError" class="text-xs text-amber-600">{{ loadError }}</p>
        <p v-if="saveError" class="text-xs text-red-600">{{ saveError }}</p>
      </article>

      <div class="settings-right-column flex min-h-0 min-w-0 flex-col gap-app">
      <article
        id="settings-section-license"
        data-testid="settings-section-license"
        class="surface-panel settings-section-card settings-section-card--license flex flex-col p-5 lg:p-6"
        :class="{ 'is-active': activeSection === 'license' }"
      >
        <header class="settings-section-head shrink-0">
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

        <div class="settings-info-grid settings-info-grid--single shrink-0">
          <div class="settings-info-item" title="卡密自身的有效期，到期后卡密将失效">
            <span class="settings-info-label">卡密有效期</span>
            <span class="settings-info-value">{{ licenseExpiresText }}</span>
          </div>
          <div class="settings-info-item" title="短效执行 Token 到期时间，到期需联网续约后继续使用">
            <span class="settings-info-label">下次续约</span>
            <span class="settings-info-value">{{ leaseExpiresText }}</span>
          </div>
          <div class="settings-info-item">
            <span class="settings-info-label">最近校验</span>
            <span class="settings-info-value">{{ licenseVerifiedText }}</span>
          </div>
          <div class="settings-info-item">
            <span class="settings-info-label">卡密</span>
            <span class="settings-info-value settings-info-value--mono">{{ appStore.licenseKey || "未保存" }}</span>
          </div>
        </div>

        <div
          data-testid="settings-license-actions"
          class="settings-action-card settings-license-actions flex flex-col gap-2"
        >
          <input
            v-model.trim="licenseKey"
            class="field-input settings-license-field min-h-10 w-full min-w-0"
            placeholder="输入卡密"
            aria-label="卡密"
          />
          <div class="settings-action-buttons-grid settings-action-buttons-grid--1x2 shrink-0">
            <button
              :disabled="activateLoading"
              class="action-btn action-btn-primary min-h-10 cursor-pointer"
              @click="handleActivate"
            >
              {{ activateLoading ? "激活中..." : "立即激活" }}
            </button>
            <button
              :disabled="verifyLoading"
              class="action-btn action-btn-secondary min-h-10 cursor-pointer"
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
        class="surface-panel settings-section-card settings-section-card--about flex flex-col p-5 lg:p-6"
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
            <div class="min-w-0 contents">
              <div v-if="updateCheck.hasUpdateAvailable" class="flex min-w-0 flex-wrap items-baseline gap-x-2 gap-y-1">
                <button
                  type="button"
                  data-testid="settings-update-download"
                  class="settings-info-value settings-info-value--mono min-w-0 cursor-pointer truncate text-left font-semibold text-emerald-800 underline decoration-emerald-600/45 underline-offset-2 hover:text-emerald-900"
                  :title="
                    [
                      `当前 v${APP_VERSION || appStore.appVersion}`,
                      updateCheck.isSnoozed ? `顶部提示已暂停至 ${updateCheck.snoozeUntilText}` : '',
                      '打开下载页',
                    ]
                      .filter(Boolean)
                      .join(' · ')
                  "
                  @click="updateCheck.openDownloadUrl()"
                >
                  有新版本 v{{ updateCheck.latestInfo?.version }}
                </button>
                <button
                  v-if="updateCheck.isSnoozed"
                  type="button"
                  class="shrink-0 cursor-pointer text-[11px] text-slate-500 underline decoration-slate-400/60 underline-offset-2 hover:text-slate-700"
                  title="恢复顶部更新提示"
                  @click="updateCheck.clearSnooze()"
                >
                  恢复
                </button>
              </div>
              <span v-else class="settings-info-value settings-info-value--mono">v{{ APP_VERSION || appStore.appVersion }}</span>
              <p v-if="updateCheck.downloadActionError" class="mt-0.5 truncate text-[11px] text-red-600" :title="updateCheck.downloadActionError">
                {{ updateCheck.downloadActionError }}
              </p>
              <p v-if="updateCheck.lastError && !updateCheck.hasUpdateAvailable" class="mt-0.5 flex flex-wrap items-center gap-x-1.5 text-[11px] text-amber-800">
                <span class="min-w-0 break-all">{{ updateCheck.lastError }}</span>
                <button type="button" class="shrink-0 cursor-pointer underline" @click="updateCheck.refresh()">重试</button>
              </p>
            </div>
          </div>
          <div class="settings-info-item">
            <span class="settings-info-label">作者微信</span>
            <span class="settings-info-value settings-info-value--mono">{{ AUTHOR_WECHAT }}</span>
          </div>
          <div class="settings-info-item" title="从本次启动到现在的累计运行时长">
            <span class="settings-info-label">会话时长</span>
            <span class="settings-info-value">{{ uptimeText }}</span>
          </div>
          <div class="settings-info-item" title="系统当前时间（每 30 秒刷新）">
            <span class="settings-info-label">当前时间</span>
            <span class="settings-info-value settings-info-value--mono">{{ clockText }}</span>
          </div>
        </div>
      </article>
      </div>
    </section>
  </div>
</template>
