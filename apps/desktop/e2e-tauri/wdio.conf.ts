/** WebdriverIO 配置：通过 tauri-driver 驱动 Tauri 真壳。 */
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
      "..",
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
