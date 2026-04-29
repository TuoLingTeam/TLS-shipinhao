// @vitest-environment jsdom

import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia, type Pinia } from "pinia";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import DashboardView from "./DashboardView.vue";
import { useAppStore } from "../app.store";
import { useDeliveryStore } from "../delivery/store";
import { useOrderStore } from "../order/store";
import { useReviewStore } from "../review/store";

const { executeMock, loadCacheCountsMock, loadCacheStatusMock, refreshSilentlyMock } = vi.hoisted(() => ({
  executeMock: vi.fn(async () => ({
    name: "TLS",
    name_en: "TLS",
    version: "5.1.0",
    author_wechat: "zxr",
    window_title: "TLS",
    runtime: "tauri",
  })),
  loadCacheCountsMock: vi.fn(async () => undefined),
  loadCacheStatusMock: vi.fn(async () => undefined),
  refreshSilentlyMock: vi.fn(async () => undefined),
}));

vi.mock("../shared/useTauriInvoke", () => ({
  useTauriInvoke: () => ({
    execute: executeMock,
  }),
}));

vi.mock("../order/useOrder", () => ({
  useOrder: () => ({
    loadCacheCounts: loadCacheCountsMock,
    loadCacheStatus: loadCacheStatusMock,
  }),
}));

vi.mock("../shared/cookieHealth", () => ({
  useCookieHealthStore: () => ({
    status: "healthy",
    snapshot: {
      last_checked_at: "2026-04-18T10:00:00Z",
      hint: "Cookie 可用",
    },
    refreshSilently: refreshSilentlyMock,
  }),
}));

describe("DashboardView", () => {
  let pinia: Pinia;

  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 3, 29, 18, 20, 0));
    pinia = createPinia();
    setActivePinia(pinia);
    executeMock.mockClear();
    loadCacheCountsMock.mockClear();
    loadCacheStatusMock.mockClear();
    refreshSilentlyMock.mockClear();

    const appStore = useAppStore();
    appStore.setLicenseInfo({
      license_state: "active",
      license_key: "abc123",
      license_expires_at: "2026-05-18T10:00:00Z",
      last_verified_at: "2026-04-18T10:00:00Z",
    });

    const orderStore = useOrderStore();
    orderStore.cacheStatus = {
      cached_order_count: 36,
      today_count: 3,
      yesterday_count: 7,
      last_7_days_count: 18,
      last_30_days_count: 36,
      today_latest_order_at: "2026-04-29T10:18:25Z",
      last_sync_at: "2026-04-18T10:00:00Z",
      coverage_start: "2026-03-19T00:00:00Z",
      coverage_end: "2026-04-18T23:59:59Z",
      coverage_complete: true,
      missing_segment_count: 0,
    };
    const reviewStore = useReviewStore();
    reviewStore.setResults([
      {
        evaluation_id: "r-1",
        order_id: "o-1",
        buyer_nickname: "alice",
        evaluation_content: "很好",
        product_id: "p-1",
        sku_id: "s-1",
        sku_name: "默认",
        product_name: "商品",
        matched: true,
        source: "receiver_and_time_window",
        strategy: "exact_match",
        replyable: true,
        reply_deadline: null,
        confidence_score: 100,
        quality_refund_info: null,
        match_reasons: [],
        candidate_count: 1,
        top_score: 100,
      },
    ]);

    const deliveryStore = useDeliveryStore();
    deliveryStore.batchProgress = {
      totalCount: 8,
      successCount: 8,
      failureCount: 0,
      processedCount: 8,
      fatalError: null,
      stopped: false,
      running: false,
      cancelRequested: false,
      steps: [],
    };
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("uses a single-row metrics strip on desktop with enlarged metric tiles", async () => {
    const wrapper = mount(DashboardView, {
      global: {
        plugins: [pinia],
        stubs: {
          RouterLink: { template: "<a><slot /></a>" },
          AppNavIcon: true,
        },
      },
    });

    await flushPromises();

    const metrics = wrapper.get('[data-testid="dashboard-metrics"]');

    expect(metrics.classes()).toContain("xl:grid-cols-4");
    expect(wrapper.findAll('[data-testid="dashboard-metric-tile"]')).toHaveLength(4);
    expect(wrapper.findAll(".dashboard-metric-tile")).toHaveLength(4);
    expect(wrapper.text()).toContain("今天缓存");
    expect(wrapper.text()).toContain("昨天缓存");
    expect(wrapper.text()).toContain("近 7 天缓存");
    expect(wrapper.text()).toContain("近 30 天缓存");
    expect(wrapper.text()).toContain("3");
    expect(wrapper.text()).toContain("7");
    expect(wrapper.text()).toContain("18");
    expect(wrapper.text()).toContain("36");
    expect(wrapper.text()).toContain("4.29 00:00:00-18:18:25");
    expect(wrapper.text()).toContain("4.28 00:00:00-23:59:59");
    expect(wrapper.text()).toContain("4.22 00:00:00-4.28 23:59:59");
    expect(wrapper.text()).toContain("3.30 00:00:00-4.28 23:59:59");
    expect(wrapper.get('[data-testid="dashboard-shortcuts"]').classes()).toContain("subsystem-summary-strip");
    expect(wrapper.findAll(".quick-link-compact")).toHaveLength(4);
    expect(wrapper.find(".subsystem-chipbar").exists()).toBe(false);
    expect(wrapper.text()).not.toContain("TLS 工作台");
    // NOTE: 底部元数据卡片为：版本 / 作者微信 / 查看教程 / 当前时间。
    expect(wrapper.text()).not.toContain("运行时");
    expect(wrapper.text()).not.toContain("提醒");
    expect(wrapper.findAll('[data-testid="dashboard-metric-tile"]')[0]?.text()).not.toContain("建议续费");
  });
});
