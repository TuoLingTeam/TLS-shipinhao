/**
 * Tauri 真壳冒烟：仅锁住「应用能启动 + Vue 挂载到 #app + 标题包含产品名」。
 *
 * 业务断言（订单匹配、授权状态机、批量发货）继续由 vitest 单测覆盖；
 * 此 spec 的职责是端到端验证「构建产物 + WebView + IPC 链路」整体不挂。
 */
import { browser, expect, $ } from "@wdio/globals";

describe("@tauri-smoke 应用骨架", () => {
  it("Vue 挂载到 #app", async () => {
    const app = await $("#app");
    await app.waitForExist({ timeout: 30_000 });
    await expect(app).toBeExisting();

    const firstChild = await app.$("> *");
    await expect(firstChild).toBeExisting();
  });

  it("窗口标题含产品名", async () => {
    const title = await browser.getTitle();
    expect(title).toMatch(/驼铃|视频小店|TLS-shipinhao/);
  });
});
