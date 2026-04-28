import { expect, test } from "@playwright/test";
import { IPC_MOCK_BOOT } from "./ipc-mock-boot";

test.describe("@e2e 授权流程（IPC mock）", () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(IPC_MOCK_BOOT);
  });

  test("设置页模拟激活后状态与提示正确", async ({ page }) => {
    await page.goto("/settings?section=license");
    await page.getByLabel("卡密").fill("TEST-KEY-001");
    await page.getByRole("button", { name: "立即激活" }).click();
    await expect(page.getByText("已激活").first()).toBeVisible();
    await expect(page.getByText("e2e 模拟激活成功")).toBeVisible();
    await expect(page.getByText("TEST-KEY-001").first()).toBeVisible();
  });
});
