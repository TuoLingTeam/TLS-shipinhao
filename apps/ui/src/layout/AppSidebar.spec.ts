// @vitest-environment jsdom

import { mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createMemoryHistory, createRouter } from "vue-router";
import { beforeEach, describe, expect, it } from "vitest";
import AppSidebar from "./AppSidebar.vue";
import { routes } from "../routeRecords";

describe("AppSidebar", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it("uses compact brand and navigation blocks without redundant system summary", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [...routes],
    });
    router.push("/settings");
    await router.isReady();

    const wrapper = mount(AppSidebar, {
      global: {
        plugins: [router, createPinia()],
      },
    });

    expect(wrapper.text()).not.toContain("系统状态");
    expect(wrapper.text()).not.toContain("版本");
    expect(wrapper.text()).not.toContain("代号");
  });
});
