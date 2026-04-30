/** Tauri 真壳路由烟测：验证主路由能渲染 RouterView。 */
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
