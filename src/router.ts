import { createRouter, createWebHistory } from "vue-router";
import { routes, handleHotUpdate } from "vue-router/auto-routes";
import { useConnections } from "./lib/connectionStore";

export const router = createRouter({
  history: createWebHistory(),
  routes: [{ path: "/", redirect: "/tables" }, ...routes],
});

// The tables page is the home page and copes with having no connection, but the
// other pages need a reachable one, so send the user back there until then.
router.beforeEach((to) => {
  const { canAccess } = useConnections();
  if (!canAccess.value && to.path !== "/tables") {
    return "/tables";
  }
});

// This will update routes at runtime without reloading the page
if (import.meta.hot) {
  handleHotUpdate(router);
}
