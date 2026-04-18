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

    expect(summary.classes()).toContain("xl:grid-cols-3");
    expect(summary.classes()).toContain("subsystem-summary-strip");
  });
});
