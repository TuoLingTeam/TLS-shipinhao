// @vitest-environment jsdom

import { mount } from "@vue/test-utils";
import { describe, expect, it, vi } from "vitest";
import AppLayout from "./AppLayout.vue";

vi.mock("../license/useLicense", () => ({
  useLicense: () => ({
    refreshStoredLicenseStatus: vi.fn(),
  }),
}));

describe("AppLayout", () => {
  it("uses a fixed viewport-height shell so only the content area scrolls", () => {
    const wrapper = mount(AppLayout, {
      global: {
        stubs: {
          AppSidebar: true,
          AppHeader: true,
          RouterView: true,
        },
      },
    });

    expect(wrapper.classes()).toContain("h-dvh");
    expect(wrapper.classes()).not.toContain("min-h-dvh");
  });
});
