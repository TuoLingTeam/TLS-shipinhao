// @vitest-environment jsdom

import { mount } from "@vue/test-utils";
import { createPinia, setActivePinia, type Pinia } from "pinia";
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

    if (command === "set_cookie") {
      return { success: true, biz_magic: "mock", cookie_path: "/tmp/cookie.txt" };
    }

    return {};
  }),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

describe("SettingsView", () => {
  let pinia: Pinia;

  beforeEach(() => {
    pinia = createPinia();
    setActivePinia(pinia);
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
        plugins: [router, pinia],
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
    expect(wrapper.get('[data-testid="settings-cookie-actions"]').classes()).toContain("settings-cookie-flows");
    expect(wrapper.find('[data-testid="settings-cookie-textarea"]').exists()).toBe(true);
    expect(wrapper.get('[data-testid="settings-cookie-save-manual"]').text()).toContain("保存手动 Cookie");
    expect(wrapper.get('[data-testid="settings-cookie-path"]').text()).toContain("/tmp/cookie.txt");
    expect(wrapper.get('[data-testid="settings-about-meta"]').classes()).toContain("settings-info-grid");
    // 方式一：打开登录页 / 选择保存路径（1x2） + 方式二：清除 Cookie / 保存手动 Cookie（1x2）
    const actionsHost = wrapper.get('[data-testid="settings-cookie-actions"]');
    expect(actionsHost.find(".settings-action-buttons-grid--1x2").exists()).toBe(true);
    expect(actionsHost.findAll("button").length).toBe(4);
    expect(actionsHost.text()).toContain("打开登录页");
    expect(actionsHost.text()).toContain("选择保存路径");
    expect(actionsHost.text()).toContain("清除 Cookie");
    expect(actionsHost.text()).toContain("保存手动 Cookie");
    // 授权信息走单列 grid
    expect(wrapper.find(".settings-info-grid--single").exists()).toBe(true);
    expect(wrapper.text()).toContain("授权信息");
    expect(wrapper.text()).toContain("Cookie 配置");
    expect(wrapper.text()).toContain("方式一");
    expect(wrapper.text()).toContain("方式二");
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
    // 简化内容：三处冗余 description 与「快捷操作」小标题已移除
    expect(wrapper.text()).not.toContain("只有自动链路拿不到完整内容时");
    expect(wrapper.text()).not.toContain("激活卡密、刷新状态、管理到期时间");
    expect(wrapper.text()).not.toContain("版本与联系方式");
    expect(wrapper.text()).not.toContain("快捷操作");
    expect(wrapper.text()).not.toContain("选择保存目录");
    // 已配置（mock 返回 configured=true & has_biz_magic=true）下不显示状态 chip
    expect(wrapper.find('[data-testid="settings-section-cookie"] .subsystem-chipbar').exists()).toBe(false);
    // 已不再使用 sidebar 嵌套包裹层，保证 DOM 扁平
    expect(wrapper.find('[data-testid="settings-sidebar"]').exists()).toBe(false);
    expect(wrapper.find('.settings-sidebar-stack').exists()).toBe(false);
    // 禁止出现独立的 step 编号样式（如 "01 授权 / 02 Cookie / 03 应用"），
    // 但允许数字嵌在其他字串（例如 AUTHOR_WECHAT = "TLS-801"、时钟 HH:mm）。
    // 历史用的 /\b0[123]\b/ 会误匹配 01/02/03:XX 时钟格式——在凌晨跑测试会挂；
    // 改为精确断言不应出现的 step + 字样组合，更稳。
    expect(wrapper.text()).not.toContain("01 授权");
    expect(wrapper.text()).not.toContain("02 Cookie");
    expect(wrapper.text()).not.toContain("03 应用");
  });
});
