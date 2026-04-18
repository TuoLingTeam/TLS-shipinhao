import { createRouter, createWebHistory } from "vue-router";
import { routes } from "./routeRecords";

const router = createRouter({
  history: createWebHistory(),
  routes,
});

export default router;
