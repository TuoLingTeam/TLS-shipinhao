// @vitest-environment jsdom

import { mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createMemoryHistory, createRouter } from "vue-router";
import { beforeEach, describe, expect, it, vi } from "vitest";
import SettingsView from "./SettingsView.vue";
import { routes } from "../routeRecords";

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(async (command: string) => {
    if (command === "get_cookie_status") {
      return {
        configured: true,
        has_biz_magic: true,
        cookie_path: "/tmp/cookie.txt",
      };
    }

    if (command === "get_cookie_health") {
      return {
        healthy: true,
        configured: true,
        has_biz_magic: true,
        last_checked_at: "2026-04-18T10:00:00Z",
        hint: "Cookie 可用",
      };
    }

    return {};
  }),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

describe("SettingsView", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    invokeMock.mockClear();
    Element.prototype.scrollIntoView = vi.fn();
  });

  it("keeps the first screen compact by removing duplicated status summary cards", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [...routes],
    });
    router.push("/settings?section=cookie");
    await router.isReady();

    const wrapper = mount(SettingsView, {
      global: {
        plugins: [router, createPinia()],
      },
    });

    await Promise.resolve();

    expect(wrapper.text()).not.toContain("把授权与设置收口到一个面板里");
    expect(wrapper.text()).not.toContain("授权状态");
    expect(wrapper.text()).not.toContain("Cookie 健康");
  });
});
