/**
 * WebdriverIO 配置：连接 `tauri-driver` 桥接平台 WebDriver（Linux 走
 * `webkit2gtk-driver`，Windows 走 `msedgedriver`）驱动 Tauri 真壳。
 *
 * 启动方式：
 * - 本地 Linux：`cargo install tauri-driver` 之后直接 `npm test`，
 *   `onPrepare` 会拉起 `tauri-driver` 子进程，`onComplete` 收尾。
 * - 本地 macOS：**不支持**（Apple WKWebView 没有官方 WebDriver 实现），
 *   请退回 `pnpm test:e2e:web`。
 * - CI：见 `.github/workflows/tauri-e2e.yml`，仅 `workflow_dispatch` 手动触发。
 */
import { spawn, type ChildProcess } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import type { Options } from "@wdio/types";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

let tauriDriver: ChildProcess | undefined;

const APPLICATION_BIN = process.env.TAURI_E2E_BIN
  ? resolve(__dirname, process.env.TAURI_E2E_BIN)
  : resolve(
      __dirname,
      "..",
      "target",
      "release",
      process.platform === "win32" ? "desktop.exe" : "desktop",
    );

export const config: Options.Testrunner = {
  runner: "local",
  hostname: "127.0.0.1",
  port: 4444,
  specs: ["./specs/**/*.spec.ts"],
  exclude: [],
  maxInstances: 1,
  capabilities: [
    {
      "tauri:options": {
        application: APPLICATION_BIN,
      },
      browserName: "wry",
    } as WebdriverIO.Capabilities,
  ],
  logLevel: "info",
  bail: 0,
  waitforTimeout: 10_000,
  connectionRetryTimeout: 60_000,
  connectionRetryCount: 3,
  framework: "mocha",
  reporters: ["spec"],
  mochaOpts: {
    ui: "bdd",
    timeout: 60_000,
  },

  // wdio v9 默认通过 tsx 直接加载 `.ts`，不再需要 v8 时代的 `autoCompileOpts`
  // / `ts-node` 配置；此处保持空白即可。

  onPrepare() {
    if (process.env.SKIP_TAURI_DRIVER === "1") {
      return;
    }
    tauriDriver = spawn("tauri-driver", [], {
      stdio: ["ignore", "inherit", "inherit"],
    });
    tauriDriver.on("error", (err) => {
      console.error("[wdio] failed to spawn tauri-driver:", err);
      process.exit(1);
    });
  },

  onComplete() {
    tauriDriver?.kill();
  },
};
