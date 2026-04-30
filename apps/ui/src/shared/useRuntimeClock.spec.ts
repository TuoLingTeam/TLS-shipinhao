// @vitest-environment jsdom

import { mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { defineComponent, h } from "vue";
import { useRuntimeClock } from "./useRuntimeClock";

function mountProbe() {
  const Probe = defineComponent({
    setup() {
      const { clockText, uptimeText } = useRuntimeClock();
      return () =>
        h("div", [
          h("span", { class: "clock" }, clockText.value),
          h("span", { class: "uptime" }, uptimeText.value),
        ]);
    },
  });
  return mount(Probe);
}

describe("useRuntimeClock", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("clockText 以 HH:mm 零填充格式产出（24 小时制）", () => {
    const wrapper = mountProbe();
    expect(wrapper.get(".clock").text()).toMatch(/^\d{2}:\d{2}$/);
    wrapper.unmount();
  });

  it("新挂载组件刚启动时 uptimeText 为「刚刚启动」（< 1 分钟）", () => {
    const wrapper = mountProbe();
    expect(wrapper.get(".uptime").text()).toBe("刚刚启动");
    wrapper.unmount();
  });

  it("sharedNow 每 30 秒 tick 后，clockText 会随 fake system time 更新", async () => {
    const wrapper = mountProbe();
    const before = wrapper.get(".clock").text();

    vi.setSystemTime(new Date(2026, 3, 20, 10, 31, 0));
    vi.advanceTimersByTime(30_000);
    await Promise.resolve();
    await Promise.resolve();

    expect(wrapper.get(".clock").text()).toBe("10:31");
    expect(wrapper.get(".clock").text()).not.toBe(before);
    wrapper.unmount();
  });
});
