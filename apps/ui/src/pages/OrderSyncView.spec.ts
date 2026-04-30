// @vitest-environment jsdom

import { mount } from "@vue/test-utils";
import { createPinia, setActivePinia, type Pinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import OrderSyncView from "./OrderSyncView.vue";
import { useAppStore } from "@/stores/app";
import { useOrderStore } from "@/stores/order";

vi.mock("@/services/order", () => ({
  useOrder: () => ({
    syncRecentCache: vi.fn(async () => undefined),
    loadRecentCache: vi.fn(async () => undefined),
    loadCacheStatus: vi.fn(async () => undefined),
  }),
}));

describe("OrderSyncView", () => {
  let pinia: Pinia;

  beforeEach(() => {
    pinia = createPinia();
    setActivePinia(pinia);

    const appStore = useAppStore();
    appStore.setLicenseState("active");

    const orderStore = useOrderStore();
    orderStore.cacheStatus = {
      cached_order_count: 2,
      today_count: 0,
      yesterday_count: 1,
      last_7_days_count: 2,
      last_30_days_count: 2,
      today_latest_order_at: null,
      last_sync_at: "2026-04-18T10:00:00Z",
      coverage_start: "2026-03-19T00:00:00Z",
      coverage_end: "2026-04-18T23:59:59Z",
      coverage_complete: true,
      missing_segment_count: 0,
    };
    orderStore.cachedOrders = [
      {
        order_id: "o-1",
        buyer_name: "alice",
        amount_cent: 1299,
        created_at: "2026-04-18T10:00:00Z",
        updated_at: "2026-04-18T10:00:00Z",
      },
    ];
  });

  it("keeps only the core order components and removes redundant side panels", () => {
    const wrapper = mount(OrderSyncView, {
      global: {
        plugins: [pinia],
        stubs: {
          LoadingState: true,
          EmptyState: true,
        },
      },
    });

    expect(wrapper.get("#order-local-search-input").attributes("placeholder")).toContain("订单号");
    expect(wrapper.text()).not.toContain("缓存统计");
    expect(wrapper.text()).not.toContain("本地检索状态");
    expect(wrapper.text()).not.toContain("ORDER CACHE");
    expect(wrapper.text()).not.toContain("同步最近 30 天缓存");
  });


  it("uses compact shells for order controls, stats, and table density", () => {
    const wrapper = mount(OrderSyncView, {
      global: {
        plugins: [pinia],
        stubs: {
          LoadingState: true,
          EmptyState: true,
        },
      },
    });

    expect(wrapper.get('[data-testid="order-hero-shell"]').classes()).toContain("order-hero-shell");
    expect(wrapper.get('[data-testid="order-table-shell"]').classes()).toContain("order-table-shell");
    expect(wrapper.text()).not.toContain("仅维护缓存，不额外展示冗余面板");
    expect(wrapper.findComponent({ name: "OrderSearchBar" }).exists()).toBe(true);
    expect(wrapper.text()).toContain("同步缓存");
  });

  it("renders a single compact progress card while syncing cache", () => {
    const orderStore = useOrderStore();
    orderStore.loading = true;
    orderStore.syncProgress = 78;
    orderStore.syncPhase = "refresh_light_cache";
    orderStore.syncMessage = "近 30 天（不含今天）富缓存已更新，正在刷新订单列表视图…";

    const wrapper = mount(OrderSyncView, {
      global: {
        plugins: [pinia],
        stubs: {
          EmptyState: true,
        },
      },
    });

    expect(wrapper.get('[data-testid="order-sync-progress"]').classes()).toContain("order-progress-shell");
    expect(wrapper.get('[data-testid="order-sync-progress"]').classes()).toContain("flex-1");
    expect(wrapper.get('[data-testid="order-sync-progress"]').classes()).not.toContain("shrink-0");
    expect(wrapper.text()).toContain("同步订单缓存");
    expect(wrapper.text()).toContain("78%");
    expect(wrapper.text()).not.toContain("正在加载");
  });
});
