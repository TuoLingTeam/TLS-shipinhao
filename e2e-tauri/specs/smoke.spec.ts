/** Tauri 真壳冒烟：验证应用能启动并挂载 Vue。 */
import { browser, expect, $ } from "@wdio/globals";

const PRODUCT_TITLE_REGEX = /驼铃|视频小店|TLS-shipinhao/;

describe("@tauri-smoke 应用骨架", () => {
  it("Vue 挂载到 #app 并渲染出子节点", async () => {
    const app = await $("#app");
    await app.waitForExist({ timeout: 30_000 });
    await expect(app).toBeExisting();

    const firstChild = await app.$("> *");
    await firstChild.waitForExist({ timeout: 10_000 });
    await expect(firstChild).toBeExisting();
  });

  it("窗口标题最终包含产品名", async () => {
    await browser.waitUntil(
      async () => PRODUCT_TITLE_REGEX.test(await browser.getTitle()),
      {
        timeout: 15_000,
        timeoutMsg: "title 未在 15s 内匹配产品名（驼铃|视频小店|TLS-shipinhao）",
      },
    );
  });
});
