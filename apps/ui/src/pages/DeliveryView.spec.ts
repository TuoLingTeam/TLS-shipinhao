// @vitest-environment jsdom

import { mount } from "@vue/test-utils";
import { createPinia, setActivePinia, type Pinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import DeliveryView from "./DeliveryView.vue";
import { useAppStore } from "@/stores/app";
import { useDeliveryStore } from "@/stores/delivery";

vi.mock("@/services/delivery", () => ({
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
          retryable: false,
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

  it("does not offer retry for confirmed-receipt delivery failures", async () => {
    const store = useDeliveryStore();
    store.batchProgress = {
      totalCount: 1,
      successCount: 0,
      failureCount: 1,
      processedCount: 1,
      fatalError: null,
      stopped: false,
      running: false,
      cancelRequested: false,
      steps: [
        {
          index: 1,
          orderId: "3735894545537057280",
          trackingNumber: "SF5199163576268",
          status: "failed",
          retryable: false,
          oldWaybill: null,
          errorMessage: "更新物流信息失败：订单已确认收货，不支持修改物流",
        },
      ],
    };

    const wrapper = mount(DeliveryView, {
      global: {
        plugins: [pinia],
        stubs: {
          ConfirmDialog: true,
        },
      },
    });

    const buttonTexts = wrapper.findAll("button").map((button) => button.text());
    expect(buttonTexts).not.toContain("重试 1 条");

    const failedTab = wrapper.findAll("button").find((button) => button.text().includes("失败条目"));
    expect(failedTab).toBeTruthy();
    await failedTab!.trigger("click");
    expect(wrapper.text()).toContain("不可重试");
  });
});
