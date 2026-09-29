import { createRouter, createWebHistory } from "vue-router";
import { ref } from "vue";
import { routes, handleHotUpdate } from "vue-router/auto-routes";
import { useConnections } from "./lib/connectionStore";

export const pageNavigationLoadingPath = ref<string | null>(null);
const DEFAULT_PAGE = "/tables";

export const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: "/", redirect: DEFAULT_PAGE },
    ...routes,
    { path: "/:pathMatch(.*)*", redirect: DEFAULT_PAGE },
  ],
});

// Tables and settings work without a reachable database; database pages send
// the user back to tables until a connection is available.
router.beforeEach((to) => {
  const { canAccess } = useConnections();
  if (!canAccess.value && !["/tables", "/settings"].includes(to.path)) {
    pageNavigationLoadingPath.value = router.currentRoute.value.path === "/tables"
      ? null
      : "/tables";
    return "/tables";
  }

  pageNavigationLoadingPath.value = to.path === router.currentRoute.value.path
    ? null
    : to.path;
});

router.afterEach(() => {
  pageNavigationLoadingPath.value = null;
});

router.onError(() => {
  pageNavigationLoadingPath.value = null;
});

// This will update routes at runtime without reloading the page
if (import.meta.hot) {
  handleHotUpdate(router);
}
