import { createRouter, createWebHistory } from "vue-router";
import PublicMeetingsPage from "@/views/PublicMeetingsPage.vue";
import AdminLoginPage from "@/views/AdminLoginPage.vue";
import AdminDashboardPage from "@/views/AdminDashboardPage.vue";

const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: "/", component: PublicMeetingsPage },
    { path: "/admin/login", component: AdminLoginPage },
    { path: "/admin", component: AdminDashboardPage },
  ],
});

export default router;
