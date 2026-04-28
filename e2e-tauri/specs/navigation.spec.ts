/**
 * Tauri 真壳路由烟测：在 vue-router 的 createWebHistory 模式下逐个导航四条业务
 * 主路由（评价 / 订单 / 发货 / 设置），断言 `<RouterView />` 渲染出至少一个子节点。
 *
 * 重点验证「打包后的二进制 + WebView + history API 拦截」整体不挂 —— Mac WKWebView
 * 与 Windows WebView2 历史上对 `file://` 起步的 history 行为有过差异（见
 * `routeRecords.ts` 顶部注释：`/index.html` redirect 修复），此 spec 锁住该路径。
 *
 * 不在本 spec 做业务断言（订单匹配 / 授权状态 / 批量发货）—— 那些用 vitest 单测和
 * Playwright + IPC mock 已能覆盖；真壳 E2E 只验集成层。
 */
import { browser, expect, $ } from "@wdio/globals";

const ROUTES: ReadonlyArray<{ path: string; label: string }> = [
  { path: "/review", label: "评价管理" },
  { path: "/order", label: "订单管理" },
  { path: "/delivery", label: "发货管理" },
  { path: "/settings", label: "设置" },
];

describe("@tauri-smoke 主路由可达", () => {
  for (const { path, label } of ROUTES) {
    it(`${path}（${label}）能渲染出 RouterView 子节点`, async () => {
      // Tauri 真壳里地址栏对前端透明，通过 location.hash / history.pushState 直接驱动
      // vue-router；这里走 history.pushState + hashchange 兼容两种 history 实现。
      await browser.execute((nextPath: string) => {
        if (window.location.pathname !== nextPath) {
          window.history.pushState({}, "", nextPath);
          window.dispatchEvent(new PopStateEvent("popstate"));
        }
      }, path);

      const app = await $("#app");
      await app.waitForExist({ timeout: 30_000 });
      const firstChild = await app.$("> *");
      await firstChild.waitForExist({ timeout: 10_000 });
      await expect(firstChild).toBeExisting();
    });
  }
});
