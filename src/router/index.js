import { createRouter, createWebHistory } from "vue-router";
import LoginPage from "../views/LoginPage.vue";
import WorkspacePage from "../views/WorkspacePage.vue";

function hasLoginState() {
  try {
    const userInfo = JSON.parse(localStorage.getItem("userInfo") || "null");
    return Boolean((userInfo?.tenantId || userInfo?.renterId) && userInfo?.userId);
  } catch {
    return false;
  }
}

const routes = [
  {
    path: "/",
    redirect: () => (hasLoginState() ? "/home" : "/login"),
  },
  {
    path: "/login",
    name: "login",
    component: LoginPage,
    meta: { guestOnly: true },
  },
  {
    path: "/home",
    name: "home",
    component: WorkspacePage,
    meta: { requiresAuth: true },
  },
  {
    path: "/:pathMatch(.*)*",
    redirect: () => (hasLoginState() ? "/home" : "/login"),
  },
];

const router = createRouter({
  history: createWebHistory(),
  routes,
});

router.beforeEach((to) => {
  const loggedIn = hasLoginState();

  if (to.meta.requiresAuth && !loggedIn) {
    return { name: "login", query: { redirect: to.fullPath } };
  }

  if (to.meta.guestOnly && loggedIn) {
    return { name: "home" };
  }

  return true;
});

export default router;
