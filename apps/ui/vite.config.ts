import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath, URL } from "node:url";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [vue(), tailwindcss()],
  clearScreen: false,
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
