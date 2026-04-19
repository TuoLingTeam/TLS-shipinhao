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

const licenseActionHint = computed(() =>
  appStore.isLicensed ? "当前授权可继续使用；若刚续费，建议手动刷新同步服务端状态。" : "尚未激活，建议先输入卡密完成授权。",
);

type CookieTone = "success" | "warn" | "error" | "idle";

const cookieHealthTone = computed<CookieTone>(() => {
  switch (cookieHealth.status) {
    case "healthy":
      return "success";
    case "unhealthy":
      return "error";
    case "unconfigured":
      return "warn";
    default:
      return "idle";
  }
});

const cookieHealthLabel = computed(() => {
  switch (cookieHealth.status) {
    case "healthy":
      return "Cookie 可用";
    case "unhealthy":
      return "Cookie 已失效";
    case "unconfigured":
      return "Cookie 尚未配置";
    default:
      return "Cookie 待探测";
  }
});

const cookieHealthHint = computed(
  () =>
    cookieHealth.error ||
    cookieHealth.snapshot.hint ||
    (cookieHealth.status === "healthy"
      ? "Cookie 已通过探测，可直接进入评价匹配与批量发货。"
      : "建议完成登录并执行一次自动提取，再回仪表盘核对状态。"),
);

const cookieHealthCheckedAt = computed(() =>
  cookieHealth.snapshot.last_checked_at ? formatDateTime(cookieHealth.snapshot.last_checked_at) : "尚未探测",
);

const cookieHealthCardClass = computed(() => `settings-cookie-health--${cookieHealthTone.value}`);

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
  <div class="settings-view-shell flex h-full min-h-0 flex-col gap-app">
    <section
      data-testid="settings-panels"
      class="subsystem-panel-grid settings-layout"
    >
      <aside
        data-testid="settings-sidebar"
        class="surface-panel settings-sidebar p-4 lg:p-5"
      >
        <div class="settings-sidebar-stack">
          <article
            id="settings-section-license"
            data-testid="settings-sidebar-license"
            class="settings-sidebar-card"
            :class="{ 'is-active': activeSection === 'license' }"
          >
            <div class="settings-sidebar-card-head">
              <div class="flex min-w-0 items-start gap-2.5">
                <span class="settings-card-badge settings-card-badge--license" aria-hidden="true">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="h-[15px] w-[15px]">
                    <path d="M12 3 19 6v5c0 4.4-2.83 8.45-7 9.75C7.83 19.45 5 15.4 5 11V6z" />
                    <path d="m9.4 11.8 1.7 1.7 3.5-3.8" />
                  </svg>
                </span>
                <div class="min-w-0">
                  <div class="settings-sidebar-card-title">授权信息</div>
                  <p class="settings-sidebar-card-copy">激活卡密、刷新状态、管理到期时间。</p>
                </div>
              </div>
              <span class="settings-badge" :class="appStore.isLicensed ? 'is-positive' : 'is-warning'">{{ currentStateText }}</span>
            </div>

            <div class="settings-info-grid">
              <div class="settings-info-item">
                <span class="settings-info-label">当前状态</span>
                <span class="settings-info-value">{{ currentStateText }}</span>
              </div>
              <div class="settings-info-item">
                <span class="settings-info-label">到期时间</span>
                <span class="settings-info-value">{{ licenseExpiresText }}</span>
              </div>
              <div class="settings-info-item">
                <span class="settings-info-label">最近校验</span>
                <span class="settings-info-value">{{ licenseVerifiedText }}</span>
              </div>
              <div class="settings-info-item settings-info-item--wide">
                <span class="settings-info-label">已保存卡密</span>
                <span class="settings-info-value settings-info-value--mono">{{ appStore.licenseKey || "未保存" }}</span>
              </div>
            </div>

            <p class="settings-sidebar-copy settings-sidebar-copy--tight">{{ licenseActionHint }}</p>

            <div
              data-testid="settings-license-actions"
              class="settings-sidebar-actions settings-action-card"
            >
              <input
                v-model.trim="licenseKey"
                class="field-input settings-license-field min-h-10 w-full min-w-0"
                placeholder="输入卡密"
                aria-label="卡密"
              />
              <div class="settings-sidebar-action-row">
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
            data-testid="settings-sidebar-about"
            class="settings-sidebar-card"
            :class="{ 'is-active': activeSection === 'about' }"
          >
            <div class="settings-sidebar-card-head">
              <div class="flex min-w-0 items-start gap-2.5">
                <span class="settings-card-badge settings-card-badge--about" aria-hidden="true">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="h-[15px] w-[15px]">
                    <circle cx="12" cy="12" r="9" />
                    <path d="M12 8.5v4" />
                    <path d="M12 15.5h.01" />
                  </svg>
                </span>
                <div class="min-w-0">
                  <div class="settings-sidebar-card-title">应用信息</div>
                  <p class="settings-sidebar-card-copy">版本、联系方式与使用建议。</p>
                </div>
              </div>
            </div>

            <div data-testid="settings-about-meta" class="settings-sidebar-row-list">
              <div class="settings-sidebar-row">
                <span class="settings-sidebar-row-label">版本</span>
                <span class="settings-sidebar-row-value font-mono">v{{ APP_VERSION || appStore.appVersion }}</span>
              </div>
              <div class="settings-sidebar-row">
                <span class="settings-sidebar-row-label">作者微信</span>
                <span class="settings-sidebar-row-value font-mono">{{ AUTHOR_WECHAT }}</span>
              </div>
              <div class="settings-sidebar-row settings-sidebar-row--stacked">
                <span class="settings-sidebar-row-label">建议节奏</span>
                <span class="settings-sidebar-row-value">先授权，再 Cookie，最后回仪表盘核对整体状态。</span>
              </div>
            </div>
          </article>
        </div>
      </aside>

      <div class="settings-workspace">
        <article
          id="settings-section-cookie"
          data-testid="settings-section-cookie"
          class="surface-panel settings-section-card p-4 lg:p-5"
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
                <p class="settings-section-copy">优先「登录 → 自动提取」；只有自动链路拿不到完整内容时，才回到左侧手动覆盖。</p>
              </div>
            </div>
            <div class="subsystem-chipbar">
              <span class="subsystem-chip" :class="cookieConfigured ? '' : 'subsystem-chip--warn'">
                {{ cookieConfigured ? "已配置" : "未配置" }}
              </span>
              <span class="subsystem-chip" :class="hasBizMagic ? '' : 'subsystem-chip--warn'">
                {{ hasBizMagic ? "已识别 biz_magic" : "待识别 biz_magic" }}
              </span>
            </div>
          </header>

          <div class="settings-cookie-grid">
            <div class="settings-field-card settings-cookie-main">
              <label class="field-label">手动覆盖 Cookie</label>
              <textarea
                data-testid="settings-cookie-textarea"
                v-model.trim="cookieHeader"
                class="field-textarea settings-cookie-textarea font-mono text-sm"
                placeholder="粘贴完整的 Cookie 字符串..."
              />
              <div class="settings-field-footer">
                <span v-if="saved" class="settings-inline-note is-success">Cookie 已保存，可继续回仪表盘核对状态。</span>
                <span v-else class="settings-inline-note">自动提取成功后会回填到这里，方便继续检查或微调。</span>
              </div>

              <div data-testid="settings-cookie-path" class="settings-cookie-path-box">
                <div class="settings-cookie-path-label">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="h-3.5 w-3.5">
                    <path d="M3 7.5A1.5 1.5 0 0 1 4.5 6h4.2l1.5 1.8h9.3a1.5 1.5 0 0 1 1.5 1.5v7.2a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 16.5Z" />
                  </svg>
                  当前保存位置
                </div>
                <div class="settings-cookie-path-value font-mono">{{ cookiePathText }}</div>
              </div>
            </div>

            <div data-testid="settings-cookie-side" class="settings-side-stack">
              <div class="settings-callout settings-callout--compact">
                <div class="settings-callout-title">推荐顺序</div>
                <p class="settings-callout-copy">
                  登录 → 自动提取 →（失败时）手动覆盖。左侧编辑区聚焦内容，右侧按钮形成操作工作台。
                </p>
              </div>

              <div
                data-testid="settings-cookie-actions"
                class="settings-action-card"
              >
                <div class="settings-action-title">快捷操作</div>
                <div class="settings-action-buttons-grid">
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

              <div class="settings-cookie-health" :class="cookieHealthCardClass">
                <div class="settings-cookie-health-head">
                  <span class="status-dot" :class="cookieHealthTone !== 'idle' ? cookieHealthTone : ''"></span>
                  <span class="settings-cookie-health-label">{{ cookieHealthLabel }}</span>
                  <span class="settings-cookie-health-meta">{{ cookieHealthCheckedAt }}</span>
                </div>
                <p class="settings-cookie-health-hint">{{ cookieHealthHint }}</p>
                <div class="settings-cookie-health-tags">
                  <span class="settings-cookie-health-tag" :class="cookieConfigured ? 'is-positive' : 'is-muted'">
                    {{ cookieConfigured ? "已保存 Cookie" : "未保存 Cookie" }}
                  </span>
                  <span class="settings-cookie-health-tag" :class="hasBizMagic ? 'is-positive' : 'is-muted'">
                    {{ hasBizMagic ? "已识别 biz_magic" : "待识别 biz_magic" }}
                  </span>
                </div>
              </div>
            </div>
          </div>

          <p v-if="loadError" class="text-xs text-amber-600">{{ loadError }}</p>
          <p v-if="saveError" class="text-xs text-red-600">{{ saveError }}</p>
        </article>
      </div>
    </section>
  </div>
</template>
