<script setup lang="ts">
import { computed, nextTick, onMounted, onBeforeUnmount, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import { AUTHOR_WECHAT } from "../shared/brand";
import { useLicense } from "../license/useLicense";
import { useAppStore } from "../app.store";
import { formatDateTime } from "../shared/format";
import { LICENSE_STATE_LABELS } from "../license/types";
import { useCookieHealthStore } from "../shared/cookieHealth";
import { useRuntimeClock } from "../shared/useRuntimeClock";
import { useStoreContextStore } from "../shared/storeContext";
import { useUpdateCheckStore } from "../shared/updateCheck";
import { toErrorMessage } from "../shared/toErrorMessage";
import { useNotification } from "../shared/useNotification";
import { isSettingsSection } from "../layout/navigation";
import type { SettingsSectionId } from "../layout/navigation";

/** 登录窗口打开后，前端轮询读取登录态的间隔（ms） */
const COOKIE_POLL_INTERVAL_MS = 1500;
/** 轮询最长持续时间（ms），到期后自动停止以避免无谓占用 */
const COOKIE_POLL_TIMEOUT_MS = 5 * 60 * 1000;

const appStore = useAppStore();
const cookieHealth = useCookieHealthStore();
const storeContext = useStoreContextStore();
const updateCheck = useUpdateCheckStore();
const route = useRoute();
const { activateLicense, activateLoading } = useLicense();
const { clockText, uptimeText } = useRuntimeClock();
const { show: showToast } = useNotification();

/** 最近一次成功保存来源：`auto`=登录窗口轮询；`manual`=手动粘贴后保存 */
const saveNotice = ref<null | "auto" | "manual">(null);
const saveError = ref<string | null>(null);
const loadError = ref<string | null>(null);
const loginLoading = ref(false);
const manualCookie = ref("");
const manualSaveLoading = ref(false);

function handleClearManualCookie() {
  manualCookie.value = "";
  saveError.value = null;
}

const licenseKey = ref("");

const activeSection = computed<SettingsSectionId>(() => {
  const raw = Array.isArray(route.query.section) ? route.query.section[0] : route.query.section;
  return isSettingsSection(raw) ? raw : "cookie";
});

const currentStateText = computed(() => LICENSE_STATE_LABELS[appStore.licenseState] ?? "未知状态");
const cookiePathText = computed(() => storeContext.cookiePath || "当前店铺尚未写入 Cookie 文件");
const activeStoreNameText = computed(() => storeContext.activeStore?.store_name ?? "尚未识别店铺");
const activeStoreIdText = computed(() => storeContext.activeStore?.store_id ?? "未生成");
const storeSelectorDisabled = computed(
  () => storeContext.busy || loginLoading.value || manualSaveLoading.value,
);
const storeCountText = computed(() =>
  storeContext.stores.length > 0 ? `已识别 ${storeContext.stores.length} 家店铺` : "暂无已识别店铺",
);
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
    await storeContext.refresh();
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

async function tryExtractCookieOnce() {
  try {
    const previousStoreId = storeContext.activeStore?.store_id ?? null;
    await invoke<{
      success: boolean;
      biz_magic: string | null;
      cookie_header: string;
      cookie_path: string;
      store: { store_id: string; store_name: string };
    }>("extract_cookie_from_login");
    await storeContext.refreshAfterCookieUpdate(previousStoreId);
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
    const previousStoreId = storeContext.activeStore?.store_id ?? null;
    await invoke("set_cookie", { cookie_header: raw });
    await storeContext.refreshAfterCookieUpdate(previousStoreId);
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

async function handleRefreshStoreStatus() {
  saveError.value = null;
  await Promise.all([loadCookieStatus(), refreshCookieHealth(), storeContext.refreshOrderCacheStatus()]);
}

async function handleSelectStore(event: Event) {
  const target = event.target as HTMLSelectElement | null;
  const nextStoreId = target?.value?.trim() ?? "";
  if (!target || !nextStoreId || nextStoreId === storeContext.activeStoreId) {
    return;
  }
  saveError.value = null;
  try {
    const result = await storeContext.selectStore(nextStoreId);
    if (!result) {
      target.value = storeContext.activeStoreId;
    }
  } catch (e) {
    target.value = storeContext.activeStoreId;
    saveError.value = toErrorMessage(e);
  }
}

async function handleActivate() {
  // 空输入直接红色 toast，避免用户点了没反馈以为按钮坏掉。
  // licenseKey 已用 v-model.trim 绑定，不需要再 trim 一次。
  if (!licenseKey.value) {
    showToast("请先在上方输入卡密", "error");
    return;
  }
  // useLicense.activateLicense 在 invoke 失败时会兜底返回带 message 的 payload，
  // 因此 result 不可能为 null；message 兜底文案覆盖服务端 / 网络异常两条路径。
  const result = await activateLicense(licenseKey.value);
  const message =
    result.message?.trim() ||
    (result.success
      ? "激活成功，授权信息已更新"
      : "激活失败，请确认卡密未被使用 / 未被吊销，或检查网络后重试");
  showToast(message, result.success ? "success" : "error");
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
  <div class="settings-view-shell flex min-w-0 flex-col">
    <section
      data-testid="settings-panels"
      class="settings-layout"
    >
      <article
        id="settings-section-cookie"
        data-testid="settings-section-cookie"
        class="surface-panel settings-section-card settings-section-card--cookie flex min-h-0 flex-col p-5 lg:p-6"
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
          <div v-if="!storeContext.cookieConfigured || !storeContext.hasBizMagic" class="subsystem-chipbar">
            <span v-if="!storeContext.cookieConfigured" class="subsystem-chip subsystem-chip--warn">未配置</span>
            <span v-if="!storeContext.hasBizMagic" class="subsystem-chip subsystem-chip--warn">待识别 biz_magic</span>
          </div>
        </header>

        <div class="settings-cookie-body flex min-h-0 flex-1 flex-col">
          <div class="settings-info-item settings-store-combo shrink-0">
            <div class="settings-store-combo-head">
              <span class="settings-info-label">当前店铺</span>
              <span class="settings-badge" :class="storeContext.hasStores ? 'is-positive' : 'is-muted'">
                {{ storeCountText }}
              </span>
            </div>
            <label data-testid="settings-active-store" class="field-affix field-affix--leading">
              <svg class="field-affix-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="M4 7.5h16" />
                <path d="M5 7.5 6.6 4.8A1.5 1.5 0 0 1 7.9 4h8.2a1.5 1.5 0 0 1 1.29.8L19 7.5" />
                <rect x="4" y="7.5" width="16" height="12.5" rx="2.5" />
                <path d="M8 12h8" />
                <path d="M8 15.5h5" />
              </svg>
              <select
                data-testid="settings-store-selector"
                aria-label="选择当前店铺"
                class="field-input field-input--with-leading-icon min-h-[38px] w-full min-w-0"
                :value="storeContext.activeStoreId"
                :disabled="storeSelectorDisabled || !storeContext.hasStores"
                @change="handleSelectStore"
              >
                <option value="" disabled>
                  {{ storeContext.hasStores ? "选择当前店铺" : "暂无已识别店铺" }}
                </option>
                <option
                  v-for="store in storeContext.stores"
                  :key="store.store_id"
                  :value="store.store_id"
                  :data-testid="store.store_id === storeContext.activeStoreId ? 'settings-active-store-id' : undefined"
                >
                  {{ store.store_name }}（{{ store.store_id }}）
                </option>
              </select>
            </label>
            <p class="settings-cookie-subpanel-hint">
              切换后会自动切到对应 Cookie 与订单缓存；订单 / 查评 / 发货页面会清空旧店铺内存态。
            </p>
          </div>

          <div data-testid="settings-cookie-actions" class="settings-cookie-flows flex min-h-0 flex-1 flex-col">
            <div class="settings-cookie-subpanel settings-cookie-subpanel--auto shrink-0">
              <p class="settings-cookie-subpanel-title">方式一 · 浏览器登录（推荐）</p>
              <p class="settings-cookie-subpanel-hint">在弹出窗口完成登录后，应用会自动识别店铺并把 Cookie 写入对应店铺目录。</p>
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
                  :disabled="storeContext.loading"
                  @click="handleRefreshStoreStatus"
                >
                  {{ storeContext.loading ? "刷新中..." : "刷新店铺状态" }}
                </button>
              </div>
              <div data-testid="settings-cookie-path" class="settings-cookie-path-box shrink-0">
                <div class="settings-cookie-path-label">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="h-3.5 w-3.5">
                    <path d="M3 7.5A1.5 1.5 0 0 1 4.5 6h4.2l1.5 1.8h9.3a1.5 1.5 0 0 1 1.5 1.5v7.2a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 16.5Z" />
                  </svg>
                  当前 Cookie 文件
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
                    :disabled="manualSaveLoading || storeContext.busy"
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

      <div class="settings-right-column flex min-w-0 flex-col gap-app">
      <article
        id="settings-section-license"
        data-testid="settings-section-license"
        class="surface-panel settings-section-card settings-section-card--license flex flex-col p-4 lg:p-5"
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
          <button
            :disabled="activateLoading"
            class="action-btn action-btn-primary min-h-10 w-full cursor-pointer"
            @click="handleActivate"
          >
            {{ activateLoading ? "激活中..." : "立即激活" }}
          </button>
        </div>

        <div class="settings-info-grid settings-info-grid--single">
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
                      `当前 v${appStore.appVersion}`,
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
              <span v-else class="settings-info-value settings-info-value--mono">v{{ appStore.appVersion }}</span>
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
