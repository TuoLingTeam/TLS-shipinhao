import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath, URL } from "node:url";
import { readFileSync } from "node:fs";

const host = process.env.TAURI_DEV_HOST;
// 从 package.json 读取版本号，编译时静态注入为 __APP_VERSION__ 常量。
// 前端所有 APP_VERSION 来源统一到此，升级只需改 package.json 的 version。
const pkgVersion = (() => {
  const raw = readFileSync(new URL("./package.json", import.meta.url), "utf-8");
  return JSON.parse(raw).version as string;
})();

export default defineConfig({
  plugins: [vue(), tailwindcss()],
  clearScreen: false,
  define: {
    __APP_VERSION__: JSON.stringify(pkgVersion),
  },
  // 与 tsconfig.json 的 compilerOptions.paths["@/*"] 对齐；Vite 6 默认不读 tsconfig paths，
  // 在此显式声明以便 IDE / tsc / Vite 三者一致。新代码可 `import x from "@/shared/x"`，
  // 现存相对路径不在本次批量替换（保持 diff 最小）。
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  server: {
    port: 5173,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 5174 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
});
