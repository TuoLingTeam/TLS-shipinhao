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
  });

  it("renders three independent flat section cards without nested sidebar wrappers", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [...routes],
    });
    router.push("/settings?section=license");
    await router.isReady();

    const wrapper = mount(SettingsView, {
      global: {
        plugins: [router, createPinia()],
      },
    });

    await Promise.resolve();
    await Promise.resolve();
    await new Promise((resolve) => setTimeout(resolve, 0));

    const panels = wrapper.get('[data-testid="settings-panels"]');
    expect(panels.classes()).toContain("settings-layout");
    expect(wrapper.get('[data-testid="settings-section-cookie"]').classes()).toContain("settings-section-card");
    expect(wrapper.get('[data-testid="settings-section-license"]').classes()).toContain("settings-section-card");
    expect(wrapper.get('[data-testid="settings-section-about"]').classes()).toContain("settings-section-card");
    expect(wrapper.get('[data-testid="settings-license-actions"]').classes()).toContain("settings-action-card");
    expect(wrapper.get('[data-testid="settings-cookie-actions"]').classes()).toContain("settings-action-card");
    expect(wrapper.get('[data-testid="settings-cookie-path"]').text()).toContain("/tmp/cookie.txt");
    expect(wrapper.get('[data-testid="settings-about-meta"]').classes()).toContain("settings-info-grid");
    // Cookie 卡内 4 个按钮使用 2x2 紧凑网格
    expect(
      wrapper.get('[data-testid="settings-cookie-actions"]').find(".settings-action-buttons-grid--2x2").exists(),
    ).toBe(true);
    // 授权信息走单列 grid，按钮放下方
    expect(wrapper.find(".settings-info-grid--single").exists()).toBe(true);
    expect(wrapper.text()).toContain("授权信息");
    expect(wrapper.text()).toContain("Cookie 配置");
    expect(wrapper.text()).toContain("应用信息");
    expect(wrapper.text()).toContain("状态");
    expect(wrapper.text()).toContain("作者微信");
    expect(wrapper.text()).not.toContain("License");
    expect(wrapper.text()).not.toContain("About");
    expect(wrapper.text()).not.toContain("设置导览");
    // 重构后已移除「推荐顺序」callout 与「Cookie 可用」健康速览
    expect(wrapper.text()).not.toContain("推荐顺序");
    expect(wrapper.find(".settings-callout").exists()).toBe(false);
    expect(wrapper.find(".settings-cookie-health").exists()).toBe(false);
    // 已不再使用 sidebar 嵌套包裹层，保证 DOM 扁平
    expect(wrapper.find('[data-testid="settings-sidebar"]').exists()).toBe(false);
    expect(wrapper.find('.settings-sidebar-stack').exists()).toBe(false);
    // 禁止出现独立的 step 编号样式（如 "01 授权 / 02 Cookie / 03 应用"），
    // 但允许数字嵌在其他字串（例如 AUTHOR_WECHAT = "TLS-801"）。
    expect(wrapper.text()).not.toMatch(/\b0[123]\b/);
  });
});
