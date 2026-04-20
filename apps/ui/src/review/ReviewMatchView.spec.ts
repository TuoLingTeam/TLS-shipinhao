// @vitest-environment jsdom

import { mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createMemoryHistory, createRouter } from "vue-router";
import { beforeEach, describe, expect, it } from "vitest";
import ReviewMatchView from "./ReviewMatchView.vue";
import { routes } from "../routeRecords";
import { useReviewStore } from "./review.store";

describe("ReviewMatchView", () => {
  beforeEach(() => {
    setActivePinia(createPinia());

    const reviewStore = useReviewStore();
    reviewStore.setLastQuery({
      days: 7,
      time_window: {
        start_at: "2026-04-11T00:00:00Z",
        end_at: "2026-04-18T23:59:59Z",
      },
    });
  });

  it("compresses the summary area into a single dense desktop row", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [...routes],
    });
    router.push("/review");
    await router.isReady();

    const wrapper = mount(ReviewMatchView, {
      global: {
        plugins: [router, createPinia()],
        stubs: {
          LoadingState: true,
          EmptyState: true,
          ReviewMatchStrategyBadge: true,
        },
      },
    });

    const summary = wrapper.get('[data-testid="review-summary-strip"]');

    expect(summary.classes()).toContain("review-summary-inline");
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
        plugins: [router, createPinia()],
        stubs: {
          LoadingState: true,
          EmptyState: true,
          ReviewMatchStrategyBadge: true,
        },
      },
    });

    expect(wrapper.get('[data-testid="review-control-shell"]').classes()).toContain("review-control-shell");
    expect(wrapper.get('[data-testid="review-filter-grid"]').classes()).toContain("review-config-panel");
    expect(wrapper.text()).toContain("选择日期");
    const preset = wrapper.get('[data-testid="review-range-preset"]');
    expect(preset.element.tagName).toBe("SELECT");
    expect(preset.findAll("option").length).toBe(4);
    expect(wrapper.get('[data-testid="review-fetch-bad"]').text()).toContain("获取差评");
    expect(wrapper.get('[data-testid="review-fetch-quality"]').text()).toContain("获取品退");
  });
});
