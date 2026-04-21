import type { RouteRecordRaw } from "vue-router";

export const routes: readonly RouteRecordRaw[] = [
  {
    path: "/",
    name: "dashboard",
    component: () => import("./dashboard/DashboardView.vue"),
  },
  // Windows 下 Tauri WebView2（Chromium）加载的入口是 "/index.html"，
  // 而 Mac WKWebView 规范化成 "/"，导致 createWebHistory 匹配不到任何路由
  // → <RouterView /> 渲染为空、仪表盘主体与侧栏子项全部缺失。
  // 用显式 redirect 兜住这两种平台行为差异，保持路由表其余部分不变。
  {
    path: "/index.html",
    redirect: "/",
  },
  {
    path: "/review",
    name: "review",
    component: () => import("./review/ReviewMatchView.vue"),
  },
  {
    path: "/order",
    name: "order",
    component: () => import("./order/OrderSyncView.vue"),
  },
  {
    path: "/delivery",
    name: "delivery",
    component: () => import("./delivery/DeliveryView.vue"),
  },
  {
    path: "/license",
    redirect: {
      name: "settings",
      query: { section: "license" },
    },
  },
  {
    path: "/settings",
    name: "settings",
    component: () => import("./settings/SettingsView.vue"),
  },
];
