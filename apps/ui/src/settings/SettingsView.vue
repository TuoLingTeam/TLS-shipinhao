<script setup lang="ts">
import { nextTick, onMounted, ref, computed, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import { AUTHOR_WECHAT, APP_NAME, APP_VERSION, APP_NAME_EN } from "../shared/brand";
import { useLicense } from "../license/useLicense";
import { useAppStore } from "../app.store";
import { formatDateTime } from "../shared/format";
import { LICENSE_STATE_LABELS } from "../license/license.types";
import { useCookieHealthStore } from "../shared/cookieHealth";
import { isSettingsSection, settingsSections, type SettingsSectionId } from "../layout/navigation";

const route = useRoute();
const router = useRouter();
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
const activeSection = ref<SettingsSectionId>("cookie");

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

function normalizeSection(value: unknown): SettingsSectionId {
  return isSettingsSection(value) ? value : "cookie";
}

async function scrollToSection(section: SettingsSectionId, behavior: ScrollBehavior = "smooth") {
  await nextTick();
  document.getElementById(`settings-section-${section}`)?.scrollIntoView({ behavior, block: "start" });
}

async function syncSectionFromRoute() {
  const nextSection = normalizeSection(route.query.section);
  activeSection.value = nextSection;
  await scrollToSection(nextSection, "auto");
}

function jumpToSection(section: SettingsSectionId) {
  activeSection.value = section;
  if (route.query.section === section) {
    void scrollToSection(section);
    return;
  }
  void router.replace({ name: "settings", query: { section } });
}

watch(
  () => route.query.section,
  () => {
    void syncSectionFromRoute();
  },
  { immediate: true },
);

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
  <div class="space-y-5">
    <section class="surface-panel px-3 py-3 lg:px-4 lg:py-4">
      <div class="flex flex-wrap gap-2">
        <button
          v-for="section in settingsSections"
          :key="section.id"
          type="button"
          class="inline-flex cursor-pointer items-center gap-2 rounded-[14px] border px-3 py-2 text-left transition"
          :class="activeSection === section.id ? 'border-brand-tint bg-brand-soft/50 text-slate-900 shadow-sm' : 'border-transparent bg-transparent text-slate-500 hover:border-brand-tint/70 hover:bg-brand-soft/30 hover:text-slate-700'"
          @click="jumpToSection(section.id)"
        >
          <span class="text-sm font-semibold">{{ section.label }}</span>
          <span class="text-[11px] text-slate-400">{{ section.description }}</span>
        </button>
      </div>
    </section>

    <section id="settings-section-license" class="grid grid-cols-1 gap-4 xl:grid-cols-[1.15fr_0.95fr]">
      <div class="surface-panel p-4 lg:p-5">
        <div class="flex items-start justify-between gap-3">
          <div>
            <div class="text-[11px] font-semibold uppercase tracking-[0.22em] text-slate-400">License</div>
            <h3 class="mt-2 text-xl font-semibold tracking-tight text-slate-900">授权与激活</h3>
            <p class="mt-1 text-[13px] leading-5 text-slate-500">输入卡密后即可激活或刷新状态。</p>
          </div>
          <div class="rounded-full px-2.5 py-1 text-xs font-semibold" :class="stateTone">
            {{ currentStateText }}
          </div>
        </div>

        <div class="mt-5 space-y-3">
          <div>
            <label class="field-label">卡密</label>
            <input v-model.trim="licenseKey" class="field-input" placeholder="输入卡密" />
          </div>
          <div class="grid gap-3 sm:grid-cols-[minmax(0,1fr)_180px]">
            <button :disabled="activateLoading" class="action-btn action-btn-primary w-full" @click="handleActivate">
              {{ activateLoading ? "激活中..." : "立即激活" }}
            </button>
            <button :disabled="verifyLoading" class="action-btn action-btn-secondary w-full" @click="handleRefresh">
              {{ verifyLoading ? "刷新中..." : "刷新状态" }}
            </button>
          </div>
          <div v-if="licenseMessage" class="soft-alert" :class="licenseMessageType === 'success' ? 'success' : 'error'">
            {{ licenseMessage }}
          </div>
        </div>
      </div>

      <div class="surface-panel p-4 lg:p-5">
        <div class="flex items-center justify-between gap-3">
          <div>
            <div class="text-[11px] font-semibold uppercase tracking-[0.22em] text-slate-400">Snapshot</div>
            <h3 class="mt-2 text-xl font-semibold tracking-tight text-slate-900">当前授权快照</h3>
          </div>
          <span class="rounded-full px-2.5 py-1 text-xs font-semibold" :class="stateTone">{{ currentStateText }}</span>
        </div>

        <div class="mt-5 grid grid-cols-1 gap-3 text-sm">
          <div class="rounded-[16px] bg-slate-50 px-3.5 py-3">
            <div class="text-xs text-slate-400">版本</div>
            <div class="mt-1 text-base font-semibold text-slate-900">{{ appStore.appVersion }}</div>
          </div>
          <div class="rounded-[16px] bg-slate-50 px-3.5 py-3">
            <div class="text-xs text-slate-400">卡密</div>
            <div class="mt-1 break-all font-mono text-xs leading-6 text-slate-700">{{ appStore.licenseKey || "未保存" }}</div>
          </div>
          <div class="grid gap-3 sm:grid-cols-2">
            <div class="rounded-[16px] bg-slate-50 px-3.5 py-3">
              <div class="text-xs text-slate-400">到期时间</div>
              <div class="mt-1 text-sm font-medium text-slate-700">{{ formatDateTime(appStore.licenseExpiresAt) }}</div>
            </div>
            <div class="rounded-[16px] bg-slate-50 px-3.5 py-3">
              <div class="text-xs text-slate-400">校验时间</div>
              <div class="mt-1 text-sm font-medium text-slate-700">{{ formatDateTime(appStore.lastVerifiedAt) }}</div>
            </div>
          </div>
        </div>
      </div>
    </section>

    <section id="settings-section-cookie" class="grid grid-cols-1 gap-4 xl:grid-cols-[1.2fr_0.8fr]">
      <div class="surface-panel p-4 lg:p-5">
        <div class="flex items-start justify-between gap-3">
          <div>
            <div class="text-[11px] font-semibold uppercase tracking-[0.22em] text-slate-400">Cookie</div>
            <h3 class="mt-2 text-xl font-semibold tracking-tight text-slate-900">Cookie 配置</h3>
            <p class="mt-1 text-[13px] leading-5 text-slate-500">登录、提取与保存都集中在这里。</p>
          </div>
          <span class="rounded-full px-2.5 py-1 text-xs font-semibold" :class="cookieStateTone">
            {{ cookieStateText }}
          </span>
        </div>

        <div class="mt-5 grid gap-4 lg:grid-cols-[minmax(0,1fr)_300px]">
          <div class="space-y-3">
            <div>
              <label class="field-label">手动覆盖 Cookie</label>
              <textarea
                v-model.trim="cookieHeader"
                rows="5"
                class="field-textarea font-mono text-sm"
                placeholder="粘贴完整的 Cookie 字符串..."
              />
            </div>
            <div class="flex flex-wrap items-center gap-3">
              <button class="action-btn action-btn-primary" @click="handleSave">保存 Cookie</button>
              <span v-if="saved" class="text-sm font-semibold text-brand">已保存</span>
              <span v-if="hasBizMagic" class="text-sm text-slate-500">已识别 biz_magic</span>
            </div>
          </div>

          <div class="rounded-[18px] border border-slate-200/80 bg-slate-50/90 p-3.5">
            <div class="text-sm font-semibold text-slate-800">当前保存位置</div>
            <div class="mt-2 break-all font-mono text-xs leading-6 text-slate-500">{{ cookiePath || "未设置" }}</div>
            <div class="mt-3 flex flex-col gap-2.5">
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
          </div>
        </div>

        <div class="mt-4 space-y-2">
          <p v-if="loadError" class="text-sm text-amber-600">{{ loadError }}</p>
          <p v-if="saveError" class="text-sm text-red-600">{{ saveError }}</p>
        </div>
      </div>

      <div class="surface-panel p-4 lg:p-5">
        <div class="text-[11px] font-semibold uppercase tracking-[0.22em] text-slate-400">Workflow</div>
        <h3 class="mt-2 text-xl font-semibold tracking-tight text-slate-900">推荐顺序</h3>
        <div class="mt-4 space-y-2.5">
          <div class="rounded-[16px] border border-slate-200/80 bg-slate-50 px-3.5 py-3">
            <div class="text-sm font-semibold text-slate-800">1. 登录</div>
            <div class="mt-1 text-[13px] leading-5 text-slate-500">打开登录页完成登录。</div>
          </div>
          <div class="rounded-[16px] border border-slate-200/80 bg-slate-50 px-3.5 py-3">
            <div class="text-sm font-semibold text-slate-800">2. 自动提取</div>
            <div class="mt-1 text-[13px] leading-5 text-slate-500">优先带回 Cookie 与 biz_magic。</div>
          </div>
          <div class="rounded-[16px] border border-slate-200/80 bg-slate-50 px-3.5 py-3">
            <div class="text-sm font-semibold text-slate-800">3. 手动覆盖</div>
            <div class="mt-1 text-[13px] leading-5 text-slate-500">异常时再手动粘贴保存。</div>
          </div>
        </div>
      </div>
    </section>

    <section id="settings-section-about" class="grid grid-cols-1 gap-4 xl:grid-cols-[0.95fr_1.05fr]">
      <div class="surface-panel p-4 lg:p-5">
        <div class="text-[11px] font-semibold uppercase tracking-[0.22em] text-slate-400">About</div>
        <h3 class="mt-2 text-xl font-semibold tracking-tight text-slate-900">应用信息</h3>
        <div class="mt-5 grid gap-3">
          <div class="rounded-[16px] bg-slate-50 px-3.5 py-3">
            <div class="text-xs text-slate-400">应用名称</div>
            <div class="mt-1 text-base font-semibold text-slate-900">{{ APP_NAME }}</div>
          </div>
          <div class="rounded-[16px] bg-slate-50 px-3.5 py-3">
            <div class="text-xs text-slate-400">英文代号</div>
            <div class="mt-1 text-base font-semibold text-slate-900">{{ APP_NAME_EN }}</div>
          </div>
          <div class="rounded-[16px] bg-slate-50 px-3.5 py-3">
            <div class="text-xs text-slate-400">版本</div>
            <div class="mt-1 text-base font-semibold text-slate-900">v{{ APP_VERSION }}</div>
          </div>
          <div class="rounded-[16px] bg-slate-50 px-3.5 py-3">
            <div class="text-xs text-slate-400">作者微信</div>
            <div class="mt-1 font-mono text-sm text-slate-700">{{ AUTHOR_WECHAT }}</div>
          </div>
        </div>
      </div>

      <div class="surface-panel p-4 lg:p-5">
        <div class="text-[11px] font-semibold uppercase tracking-[0.22em] text-slate-400">Checklist</div>
        <h3 class="mt-2 text-xl font-semibold tracking-tight text-slate-900">环境检查</h3>
        <div class="mt-5 space-y-2.5">
          <div class="rounded-[16px] border border-brand-tint bg-brand-soft/60 px-3.5 py-3 text-[13px] leading-5 text-brand-deep">
            先确认授权与 Cookie，再去订单管理同步最近 30 天缓存。
          </div>
          <div class="rounded-[16px] border border-slate-200/80 bg-slate-50 px-3.5 py-3 text-[13px] leading-5 text-slate-600">
            顶部若提示 Cookie 失效，请先重新登录并提取。
          </div>
          <div class="rounded-[16px] border border-slate-200/80 bg-slate-50 px-3.5 py-3 text-[13px] leading-5 text-slate-600">
            授权临近到期时，直接在本页刷新即可确认最新租约。
          </div>
        </div>
      </div>
    </section>
  </div>
</template>
