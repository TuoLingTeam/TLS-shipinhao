<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { AUTHOR_WECHAT, APP_NAME, APP_VERSION, APP_NAME_EN } from "../shared/brand";
import { useLicense } from "../license/useLicense";
import { useAppStore } from "../app.store";
import { formatDateTime } from "../shared/format";
import { LICENSE_STATE_LABELS } from "../license/license.types";
import { useCookieHealthStore } from "../shared/cookieHealth";

const appStore = useAppStore();
const cookieHealth = useCookieHealthStore();
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

const currentStateText = computed(() => LICENSE_STATE_LABELS[appStore.licenseState] ?? LICENSE_STATE_LABELS.unknown);
const stateTone = computed(() => (appStore.isLicensed ? "bg-brand-soft text-brand-deep" : "bg-amber-50 text-amber-700"));
const cookieStateText = computed(() => {
  switch (cookieHealth.status) {
    case "healthy":
      return "Cookie 正常";
    case "unhealthy":
      return "Cookie 失效";
    case "unconfigured":
      return "Cookie 未配置";
    default:
      return "Cookie 待探测";
  }
});
const cookieStateTone = computed(() => {
  switch (cookieHealth.status) {
    case "healthy":
      return "bg-brand-soft text-brand-deep";
    case "unhealthy":
      return "bg-red-50 text-red-700";
    case "unconfigured":
      return "bg-amber-50 text-amber-700";
    default:
      return "bg-slate-100 text-slate-500";
  }
});

async function refreshCookieHealth() {
  try {
    await cookieHealth.refreshSilently();
  } catch {
    // 忽略刷新异常，页面上已有错误态文案。
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

onMounted(() => {
  void Promise.all([loadCookieStatus(), refreshCookieHealth()]);
});
</script>

<template>
  <div class="space-y-4">
    <section
      data-testid="settings-panels"
      class="subsystem-panel-grid grid grid-cols-1 gap-4 xl:grid-cols-3"
    >
      <article class="surface-panel p-4 lg:p-5">
        <div class="flex items-start justify-between gap-3">
          <div>
            <div class="text-[11px] font-semibold uppercase tracking-[0.22em] text-slate-400">License</div>
            <h3 class="mt-2 text-xl font-semibold tracking-tight text-slate-900">授权与激活</h3>
            <p class="mt-1 text-[13px] leading-5 text-slate-500">输入卡密、激活并刷新当前授权状态。</p>
          </div>
          <div class="rounded-full px-2.5 py-1 text-xs font-semibold" :class="stateTone">
            {{ currentStateText }}
          </div>
        </div>

        <div class="subsystem-summary-strip mt-4 sm:grid-cols-3 text-sm">
          <div class="subsystem-summary-card">
            <div class="subsystem-summary-label">当前状态</div>
            <div class="subsystem-summary-value">{{ currentStateText }}</div>
          </div>
          <div class="subsystem-summary-card">
            <div class="subsystem-summary-label">到期时间</div>
            <div class="subsystem-summary-value text-sm font-medium text-slate-700">{{ formatDateTime(appStore.licenseExpiresAt) }}</div>
          </div>
          <div class="subsystem-summary-card">
            <div class="subsystem-summary-label">最近校验</div>
            <div class="subsystem-summary-value text-sm font-medium text-slate-700">{{ formatDateTime(appStore.lastVerifiedAt) }}</div>
          </div>
        </div>

        <div class="mt-4 space-y-3">
          <div>
            <label class="field-label">卡密</label>
            <input v-model.trim="licenseKey" class="field-input" placeholder="输入卡密" />
          </div>
          <div class="grid gap-2.5 sm:grid-cols-2">
            <button :disabled="activateLoading" class="action-btn action-btn-primary w-full" @click="handleActivate">
              {{ activateLoading ? "激活中..." : "立即激活" }}
            </button>
            <button :disabled="verifyLoading" class="action-btn action-btn-secondary w-full" @click="handleRefresh">
              {{ verifyLoading ? "刷新中..." : "刷新状态" }}
            </button>
          </div>
          <div class="rounded-[16px] bg-slate-50 px-3 py-2.5">
            <div class="text-[11px] text-slate-400">已保存卡密</div>
            <div class="mt-1 break-all font-mono text-xs leading-5 text-slate-700">{{ appStore.licenseKey || "未保存" }}</div>
          </div>
          <div v-if="licenseMessage" class="soft-alert" :class="licenseMessageType === 'success' ? 'success' : 'error'">
            {{ licenseMessage }}
          </div>
        </div>
      </article>

      <article class="surface-panel p-4 lg:p-5">
        <div class="flex items-start justify-between gap-3">
          <div>
            <div class="text-[11px] font-semibold uppercase tracking-[0.22em] text-slate-400">Cookie</div>
            <h3 class="mt-2 text-xl font-semibold tracking-tight text-slate-900">Cookie 配置</h3>
            <p class="mt-1 text-[13px] leading-5 text-slate-500">登录、提取、覆盖和保存路径都集中在这里。</p>
          </div>
          <span class="rounded-full px-2.5 py-1 text-xs font-semibold" :class="cookieStateTone">
            {{ cookieStateText }}
          </span>
        </div>

        <div class="subsystem-chipbar mt-4 text-xs">
          <span class="subsystem-chip">
            {{ cookieConfigured ? "已配置" : "未配置" }}
          </span>
          <span class="subsystem-chip">
            {{ hasBizMagic ? "已识别 biz_magic" : "未识别 biz_magic" }}
          </span>
        </div>

        <div class="mt-4 space-y-3">
          <div>
            <label class="field-label">手动覆盖 Cookie</label>
            <textarea
              v-model.trim="cookieHeader"
              rows="4"
              class="field-textarea font-mono text-sm"
              placeholder="粘贴完整的 Cookie 字符串..."
            />
          </div>

          <div class="grid gap-2.5 sm:grid-cols-2">
            <button class="action-btn action-btn-primary w-full" @click="handleSave">保存 Cookie</button>
            <button class="action-btn action-btn-secondary w-full" :disabled="pickDirLoading" @click="handlePickSaveDir">
              {{ pickDirLoading ? "选择中..." : "选择保存目录" }}
            </button>
            <button class="action-btn action-btn-secondary w-full" :disabled="loginLoading" @click="handleOpenLogin">
              {{ loginLoading ? "打开登录页中..." : "打开登录页" }}
            </button>
            <button class="action-btn action-btn-primary w-full" :disabled="extractLoading" @click="handleExtractCookie">
              {{ extractLoading ? "提取中..." : "自动提取 Cookie" }}
            </button>
          </div>

          <div class="rounded-[16px] bg-slate-50 px-3 py-2.5">
            <div class="text-[11px] text-slate-400">当前保存位置</div>
            <div class="mt-1 break-all font-mono text-xs leading-5 text-slate-600">{{ cookiePath || "未设置" }}</div>
          </div>

          <div v-if="saved" class="text-sm font-semibold text-brand">已保存</div>
          <p v-if="loadError" class="text-sm text-amber-600">{{ loadError }}</p>
          <p v-if="saveError" class="text-sm text-red-600">{{ saveError }}</p>
        </div>
      </article>

      <article class="surface-panel p-4 lg:p-5">
        <div>
          <div class="text-[11px] font-semibold uppercase tracking-[0.22em] text-slate-400">About</div>
          <h3 class="mt-2 text-xl font-semibold tracking-tight text-slate-900">应用信息</h3>
          <p class="mt-1 text-[13px] leading-5 text-slate-500">保留核心品牌与版本信息，避免重复说明。</p>
        </div>

        <div class="mt-4 grid gap-2.5">
          <div class="rounded-[16px] bg-slate-50 px-3 py-2.5">
            <div class="text-[11px] text-slate-400">应用名称</div>
            <div class="mt-1 text-base font-semibold text-slate-900">{{ APP_NAME }}</div>
          </div>
          <div class="rounded-[16px] bg-slate-50 px-3 py-2.5">
            <div class="text-[11px] text-slate-400">英文代号</div>
            <div class="mt-1 text-base font-semibold text-slate-900">{{ APP_NAME_EN }}</div>
          </div>
          <div class="grid gap-2.5 sm:grid-cols-2">
            <div class="rounded-[16px] bg-slate-50 px-3 py-2.5">
              <div class="text-[11px] text-slate-400">版本</div>
              <div class="mt-1 text-base font-semibold text-slate-900">v{{ APP_VERSION || appStore.appVersion }}</div>
            </div>
            <div class="rounded-[16px] bg-slate-50 px-3 py-2.5">
              <div class="text-[11px] text-slate-400">作者微信</div>
              <div class="mt-1 font-mono text-sm text-slate-700">{{ AUTHOR_WECHAT }}</div>
            </div>
          </div>
          <div class="rounded-[16px] border border-brand-tint bg-brand-soft/50 px-3 py-2.5 text-[13px] leading-5 text-brand-deep">
            建议先完成授权与 Cookie，再去订单管理同步最近 30 天缓存。
          </div>
        </div>
      </article>
    </section>
  </div>
</template>
