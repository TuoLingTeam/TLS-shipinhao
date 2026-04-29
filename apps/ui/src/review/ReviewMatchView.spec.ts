// @vitest-environment jsdom

import { mount } from "@vue/test-utils";
import { createPinia, setActivePinia, type Pinia } from "pinia";
import { createMemoryHistory, createRouter } from "vue-router";
import { beforeEach, describe, expect, it } from "vitest";
import ReviewMatchView from "./ReviewMatchView.vue";
import { routes } from "../routeRecords";
import { useReviewStore } from "./store";

describe("ReviewMatchView", () => {
  let pinia: Pinia;

  beforeEach(() => {
    pinia = createPinia();
    setActivePinia(pinia);

    const reviewStore = useReviewStore();
    reviewStore.setLastQuery({
      days: 7,
      time_window: {
        start_at: "2026-04-11T00:00:00Z",
        end_at: "2026-04-18T23:59:59Z",
      },
    });
  });

  it("lays out preset select and two fetch buttons as three equal-width controls", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [...routes],
    });
    router.push("/review");
    await router.isReady();

    const wrapper = mount(ReviewMatchView, {
      global: {
        plugins: [router, pinia],
        stubs: {
          LoadingState: true,
          EmptyState: true,
          ReviewMatchStrategyBadge: true,
        },
      },
    });

    const row = wrapper.get('[data-testid="review-config-actions"]');
    expect(row.findAll("select").length).toBe(1);
    expect(row.findAll("button").length).toBe(2);
    expect(wrapper.get('[data-testid="review-range-preset"]').attributes("aria-label")).toBe("选择日期范围");
    expect(wrapper.text()).not.toContain("REVIEW MATCH");
  });

  it("uses date range plus separate fetch actions for bad reviews and quality refunds", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [...routes],
    });
    router.push("/review");
    await router.isReady();

    const wrapper = mount(ReviewMatchView, {
      global: {
        plugins: [router, pinia],
        stubs: {
          LoadingState: true,
          EmptyState: true,
          ReviewMatchStrategyBadge: true,
        },
      },
    });

    const shell = wrapper.get('[data-testid="review-control-shell"]');
    expect(shell.classes()).toContain("hero-panel");
    expect(shell.classes()).toContain("review-config-panel");
    const preset = wrapper.get('[data-testid="review-range-preset"]');
    expect(preset.element.tagName).toBe("SELECT");
    expect(preset.findAll("option").length).toBe(4);
    expect(wrapper.get('[data-testid="review-fetch-bad"]').text()).toContain("获取差评");
    expect(wrapper.get('[data-testid="review-fetch-quality"]').text()).toContain("获取品退");
  });

  it("does not render the evaluation id column in bad review results", async () => {
    const reviewStore = useReviewStore();
    reviewStore.setResults([
      {
        order_id: "o-1",
        buyer_nickname: "测试买家",
        evaluation_content: "用户觉得不够好。",
        product_id: "p-1",
        sku_id: "s-1",
        sku_name: "默认规格",
        product_name: "测试商品",
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

    const router = createRouter({
      history: createMemoryHistory(),
      routes: [...routes],
    });
    router.push("/review");
    await router.isReady();

    const wrapper = mount(ReviewMatchView, {
      global: {
        plugins: [router, pinia],
        stubs: {
          LoadingState: true,
          EmptyState: true,
          ReviewMatchStrategyBadge: true,
        },
      },
    });

    expect(wrapper.text()).not.toContain("评价ID");
    expect(wrapper.text()).toContain("测试买家");
    expect(wrapper.text()).toContain("用户觉得不够好。");
  });
});
