import { createRouter, createWebHistory } from "vue-router";
import { routes, handleHotUpdate } from "vue-router/auto-routes";
import { useConnections } from "./lib/connectionStore";

export const router = createRouter({
  history: createWebHistory(),
  routes: [{ path: "/", redirect: "/tables" }, ...routes],
});

// Tables and settings work without a reachable database; database pages send
// the user back to tables until a connection is available.
router.beforeEach((to) => {
  const { canAccess } = useConnections();
  if (!canAccess.value && !["/tables", "/settings"].includes(to.path)) {
    return "/tables";
  }
});

// This will update routes at runtime without reloading the page
if (import.meta.hot) {
  handleHotUpdate(router);
}
