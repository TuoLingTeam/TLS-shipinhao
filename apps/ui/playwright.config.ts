import { defineConfig, devices } from "@playwright/test";

// E2E 仅作前端冒烟。Tauri 桌面壳的真实端到端（webdriver / tauri-driver）
// 工作量更大，本骨架先把「vite preview 起得来 + 主要路由能渲染 + 主要资源
// 不报 404」三条粗粒度路径锁住，后续按需扩展。
//
// 故意不在 CI 默认开启：CI 跑 Playwright 需要 xvfb/headed 配置 + 浏览器二进制
// 缓存，不在「零行为变更」边界内。本地手动跑：
//   pnpm exec playwright install chromium   # 仅首次
//   pnpm test:e2e
export default defineConfig({
  testDir: "./e2e",
  fullyParallel: false,
  reporter: [["list"]],
  use: {
    baseURL: "http://localhost:4173",
    trace: "retain-on-failure",
    headless: true,
  },
  // 让 Playwright 自己管 vite preview 生命周期；不要复用现有 dev server
  // 以防开发期端口冲突。
  webServer: {
    command: "pnpm vite preview --host --port 4173",
    url: "http://localhost:4173",
    reuseExistingServer: false,
    timeout: 60_000,
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
});
