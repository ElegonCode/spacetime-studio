import { createRouter, createWebHistory } from "vue-router";
import { routes, handleHotUpdate } from "vue-router/auto-routes";
import { useConnections } from "./lib/connectionStore";

export const router = createRouter({
  history: createWebHistory(),
  routes,
});

// Without a reachable connection there is nothing for the other pages to show,
// so keep the user on the connections page until one is confirmed usable.
router.beforeEach((to) => {
  const { canAccess } = useConnections();
  if (!canAccess.value && to.path !== "/") {
    return "/";
  }
});

// This will update routes at runtime without reloading the page
if (import.meta.hot) {
  handleHotUpdate(router);
}
