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

  it("keeps three top-level columns but stacks each panel vertically to avoid crowding", async () => {
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

    expect(wrapper.get('[data-testid="settings-panels"]').classes()).toContain("xl:grid-cols-3");
    expect(wrapper.get('[data-testid="settings-panels"]').classes()).toContain("subsystem-panel-grid");
    expect(wrapper.get('[data-testid="settings-license-summary"]').classes()).toContain("grid-cols-1");
    expect(wrapper.get('[data-testid="settings-license-actions"]').classes()).toContain("grid-cols-1");
    expect(wrapper.get('[data-testid="settings-cookie-actions"]').classes()).toContain("grid-cols-1");
    expect(wrapper.get('[data-testid="settings-about-meta"]').classes()).toContain("grid-cols-1");
    expect(wrapper.text()).toContain("授权与激活");
    expect(wrapper.text()).toContain("Cookie 配置");
    expect(wrapper.text()).toContain("应用信息");
    expect(wrapper.text()).not.toContain("当前授权快照");
    expect(wrapper.text()).not.toContain("推荐顺序");
    expect(wrapper.text()).not.toContain("环境检查");
  });
});
