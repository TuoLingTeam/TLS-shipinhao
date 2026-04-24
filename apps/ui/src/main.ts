import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import router from "./router";
import { useAppStore } from "./app.store";
import "./styles/main.css";

const app = createApp(App);
const pinia = createPinia();
app.use(pinia);
app.use(router);
app.mount("#app");

// 运行时从 Tauri 读真实版本号，覆盖 Vite 编译期注入的 package.json 快照。
// 这样 apps/ui/package.json 和 apps/desktop/tauri.conf.json.version 不小心
// 不同步时，UI 上显示的仍是 Tauri 二进制内的真实版本，与窗口标题保持一致。
useAppStore(pinia).initAppVersion();
