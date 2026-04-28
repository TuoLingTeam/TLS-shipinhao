/**
 * Tauri 真壳冒烟：仅锁住「应用能启动 + Vue 挂载到 #app + 标题包含产品名」。
 *
 * 业务断言（订单匹配、授权状态机、批量发货）继续由 vitest 单测覆盖；
 * 此 spec 的职责是端到端验证「构建产物 + WebView + IPC 链路」整体不挂。
 */
import { browser, expect, $ } from "@wdio/globals";

const PRODUCT_TITLE_REGEX = /驼铃|视频小店|TLS-shipinhao/;

describe("@tauri-smoke 应用骨架", () => {
  it("Vue 挂载到 #app 并渲染出子节点", async () => {
    const app = await $("#app");
    await app.waitForExist({ timeout: 30_000 });
    await expect(app).toBeExisting();

    // Vue 挂载后会向 #app 注入子节点；等待最长 10s，避免冷启动慢时假阴。
    const firstChild = await app.$("> *");
    await firstChild.waitForExist({ timeout: 10_000 });
    await expect(firstChild).toBeExisting();
  });

  it("窗口标题最终包含产品名", async () => {
    // 不直接 getTitle 一次：Vue 路由 / brand 常量在挂载后才把 document.title
    // 同步过来，冷启动前若直接读 title 会拿到空串/默认值，造成假阴。改 waitUntil。
    await browser.waitUntil(
      async () => PRODUCT_TITLE_REGEX.test(await browser.getTitle()),
      {
        timeout: 15_000,
        timeoutMsg: "title 未在 15s 内匹配产品名（驼铃|视频小店|TLS-shipinhao）",
      },
    );
  });
});
