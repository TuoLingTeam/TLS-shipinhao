// @vitest-environment jsdom

import { mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import DeliveryView from "./DeliveryView.vue";
import { useAppStore } from "../app.store";
import { useDeliveryStore } from "./delivery.store";

vi.mock("../delivery/useDelivery", () => ({
  useDelivery: () => ({
    updateDelivery: vi.fn(async () => undefined),
    batchDelivery: vi.fn(async () => undefined),
    cancelBatchDelivery: vi.fn(async () => undefined),
    retryFailedItems: vi.fn(async () => undefined),
    exportFailedCsv: vi.fn(),
  }),
}));

describe("DeliveryView", () => {
  beforeEach(() => {
    setActivePinia(createPinia());

    const appStore = useAppStore();
    appStore.setLicenseState("active");

    const store = useDeliveryStore();
    store.draftOrderId = "order-123";
    store.draftSource = "评价匹配";
    store.batchProgress = {
      totalCount: 3,
      successCount: 2,
      failureCount: 1,
      processedCount: 3,
      fatalError: null,
      stopped: false,
      running: false,
      cancelRequested: false,
      steps: [
        {
          index: 1,
          orderId: "order-123",
          trackingNumber: "JT0001",
          status: "success",
          oldWaybill: null,
          errorMessage: null,
        },
      ],
    };
  });

  it("uses the same compact subsystem shell as the other modules", () => {
    const wrapper = mount(DeliveryView, {
      global: {
        plugins: [createPinia()],
        stubs: {
          EmptyState: true,
          ConfirmDialog: true,
        },
      },
    });

    expect(wrapper.get('[data-testid="delivery-summary-strip"]').classes()).toContain("subsystem-summary-strip");
    expect(wrapper.get('[data-testid="delivery-workspace"]').classes()).toContain("xl:grid-cols-[0.92fr_1.08fr]");
  });
});
