import { createRouter, createWebHistory } from "vue-router";

const router = createRouter({
  history: createWebHistory(),
  routes: [
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
      name: "license",
      component: () => import("./license/LicenseView.vue"),
    },
    {
      path: "/settings",
      name: "settings",
      component: () => import("./settings/SettingsView.vue"),
    },
  ],
});

export default router;
