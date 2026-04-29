import { describe, expect, it } from "vitest";
import { navGroups, pageMetaMap, settingsSections } from "./navigation";

describe("navigation", () => {
  it("keeps authorization inside settings instead of a standalone sidebar entry", () => {
    const labels = navGroups.flatMap((group) => group.items.map((item) => item.label));

    expect(labels).toContain("软件设置");
    expect(labels).not.toContain("授权管理");
  });

  it("keeps order management before review management in the business flow", () => {
    const workspace = navGroups.find((group) => group.id === "workspace");

    expect(workspace?.items.map((item) => item.label)).toEqual([
      "仪表盘",
      "订单管理",
      "评价管理",
      "发货管理",
    ]);
  });

  it("exposes settings metadata for the merged control center", () => {
    expect(pageMetaMap.settings.title).toBe("设置中心");
    expect(settingsSections.map((section) => section.id)).toContain("license");
  });
});
