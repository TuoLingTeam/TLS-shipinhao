import { test, expect } from "@playwright/test";

// Web 冒烟只验证构建产物与主要路由能启动。

test.describe("@smoke 应用骨架", () => {
  test("首页可加载且 title 含产品名", async ({ page }) => {
    await page.goto("/");
    await expect(page).toHaveTitle(/驼铃|视频小店|TLS-shipinhao/);
  });

  test("根容器 #app 存在并被 Vue 挂载", async ({ page }) => {
    await page.goto("/");
    const app = page.locator("#app");
    await expect(app).toBeAttached();
    await expect(app.locator("> *").first()).toBeAttached();
  });

  test("访问 settings 路径不会 404（hash 路由 / 重写后均允许）", async ({ page }) => {
    const responses: number[] = [];
    page.on("response", (resp) => {
      if (resp.url().endsWith("/settings") || resp.url().endsWith("/settings/")) {
        responses.push(resp.status());
      }
    });
    await page.goto("/settings");
    for (const status of responses) {
      expect(status, "settings 路径不应 404").not.toBe(404);
    }
  });
});
