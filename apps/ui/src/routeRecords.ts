import type { RouteRecordRaw } from "vue-router";

export const routes: readonly RouteRecordRaw[] = [
  {
    path: "/",
    name: "dashboard",
    component: () => import("./dashboard/DashboardView.vue"),
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
