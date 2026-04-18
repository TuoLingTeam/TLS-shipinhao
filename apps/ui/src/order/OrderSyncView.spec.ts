// @vitest-environment jsdom

import { mount } from "@vue/test-utils";
import { createPinia, setActivePinia, type Pinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import OrderSyncView from "./OrderSyncView.vue";
import { useAppStore } from "../app.store";
import { useOrderStore } from "./order.store";

vi.mock("../order/useOrder", () => ({
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
        receiver_name: "alice",
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

    expect(wrapper.text()).toContain("同步最近 30 天缓存");
    expect(wrapper.text()).toContain("本地订单检索");
    expect(wrapper.text()).not.toContain("缓存统计");
    expect(wrapper.text()).not.toContain("本地检索状态");
    expect(wrapper.get('[data-testid="order-chipbar"]').classes()).toContain("subsystem-chipbar");
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
    expect(wrapper.get('[data-testid="order-sync-shell"]').classes()).toContain("order-sync-shell");
    expect(wrapper.get('[data-testid="order-stats-grid"]').classes()).toContain("order-stats-grid");
    expect(wrapper.findAll('.order-stat-card')).toHaveLength(3);
    expect(wrapper.get('[data-testid="order-table-shell"]').classes()).toContain("order-table-shell");
  });
});
