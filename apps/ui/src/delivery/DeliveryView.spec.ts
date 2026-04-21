// @vitest-environment jsdom

import { mount } from "@vue/test-utils";
import { createPinia, setActivePinia, type Pinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import DeliveryView from "./DeliveryView.vue";
import { useAppStore } from "../app.store";
import { useDeliveryStore } from "./store";

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
  let pinia: Pinia;

  beforeEach(() => {
    pinia = createPinia();
    setActivePinia(pinia);

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

  it("renders the two-column batch delivery workspace without the hero summary", () => {
    const wrapper = mount(DeliveryView, {
      global: {
        plugins: [pinia],
        stubs: {
          ConfirmDialog: true,
        },
      },
    });

    expect(wrapper.get('[data-testid="delivery-workspace"]').classes()).toContain("md:grid-cols-2");
    expect(wrapper.find('[data-testid="delivery-summary-strip"]').exists()).toBe(false);
    expect(wrapper.text()).not.toContain("DELIVERY DESK");
    expect(wrapper.text()).not.toContain("单条修正");
    expect(wrapper.text()).not.toContain("发货操作台");
    expect(wrapper.text()).toContain("批量发货");
  });
});
