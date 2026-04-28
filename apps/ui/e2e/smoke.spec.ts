import { test, expect } from "@playwright/test";

// 冒烟测试：vite preview 起一个仅前端的实例。Tauri invoke 在浏览器环境里
// 会落到 useTauriInvoke 的错误分支，不影响首屏 layout/router 渲染本身。
//
// 这一组用例只验证「构建产物能起来 + 主要路由能渲染 + 关键文案存在」。
// 不应用业务断言（订单匹配、授权状态机、批量发货）作为 e2e 入口；那些
// 用 vitest 单测覆盖更稳更快。

test.describe("@smoke 应用骨架", () => {
  test("首页可加载且 title 含产品名", async ({ page }) => {
    await page.goto("/");
    await expect(page).toHaveTitle(/驼铃|视频小店|TLS-shipinhao/);
  });

  test("根容器 #app 存在并被 Vue 挂载", async ({ page }) => {
    await page.goto("/");
    const app = page.locator("#app");
    await expect(app).toBeAttached();
    // Vue 挂载后会向 #app 注入子节点（layout、侧边栏等）。这里仅断言
    // 容器有任意子节点存在，不绑定具体组件 DOM 结构，便于后续重构。
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
    // SPA 在 vite preview 下若未配置 historyApiFallback，会返回 200
    // （根 index.html 的兜底）；只要不出现 404 即可。
    for (const status of responses) {
      expect(status, "settings 路径不应 404").not.toBe(404);
    }
  });
});
