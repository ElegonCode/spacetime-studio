<script setup lang="ts">
import type { TableColumn } from "@nuxt/ui";
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { useToast } from "@nuxt/ui/composables";
import {
  executeSql,
  exportRows,
  getSchema,
  getSelectedConnectionId,
  type ExportFormat,
  type SqlStatementResult,
  type TableSummary,
} from "../lib/spacetime";
import { formatCell } from "../lib/cells";
import {
  clearPageRefreshHandler,
  setPageRefreshHandler,
  type PageRefreshReason,
} from "../lib/pageActions";
import { dataTableUi, selectedCellUi } from "../lib/tableUi";
import { useConnections } from "../lib/connectionStore";
import SqlCodeEditor from "../components/SqlCodeEditor.vue";

const HISTORY_KEY = "spacetime-studio:sql-history";
const QUERY_TABS_KEY = "spacetime-studio:query-tabs";
const SQL_EDITOR_SPLIT_KEY = "spacetime-studio:sql-editor-split";
const SQL_EDITOR_SPLIT_MIN = 0.2;
const SQL_EDITOR_SPLIT_MAX = 0.8;
const HISTORY_LIMIT = 25;

function loadSqlEditorSplit() {
  try {
    const split = Number(localStorage.getItem(SQL_EDITOR_SPLIT_KEY));
    return Number.isFinite(split) && split > 0
      ? Math.max(SQL_EDITOR_SPLIT_MIN, Math.min(SQL_EDITOR_SPLIT_MAX, split))
      : 0.5;
  } catch {
    return 0.5;
  }
}

const { selectedConnection, selectedId } = useConnections();
const toast = useToast();

type QueryTab = { id: number; name: string; query: string; pinned: boolean; results: SqlStatementResult[]; ranAt: string; activeResult: string };

type SavedQueryTabState = { tabs: Array<{ name: string; query: string; pinned: boolean }>; activeIndex: number };

function loadQueryTabState(connectionId: string | null): SavedQueryTabState {
  const fallback = (): SavedQueryTabState => ({ tabs: [{ name: "Query 1", query: "SELECT * FROM ", pinned: false }], activeIndex: 0 });
  try {
    const stored = JSON.parse(localStorage.getItem(QUERY_TABS_KEY) ?? "null");
    if (stored && typeof stored === "object" && stored.byConnection && typeof stored.byConnection === "object") {
      const state = connectionId ? stored.byConnection[connectionId] : null;
      if (!state || !Array.isArray(state.tabs)) return fallback();
      return normalizeQueryTabState(state);
    }
    // Migrate the previous single-connection format to the currently selected connection.
    if (stored && Array.isArray(stored.tabs)) {
      const state = normalizeQueryTabState(stored);
      if (connectionId) localStorage.setItem(QUERY_TABS_KEY, JSON.stringify({ byConnection: { [connectionId]: state } }));
      return state;
    }
    return fallback();
  } catch {
    return fallback();
  }
}

function normalizeQueryTabState(stored: { tabs: unknown[]; activeIndex?: unknown }): SavedQueryTabState {
  const tabs = stored.tabs.filter((tab: unknown): tab is { name: string; query: string; pinned?: unknown } =>
          !!tab && typeof tab === "object" &&
          typeof (tab as { name?: unknown }).name === "string" &&
          typeof (tab as { query?: unknown }).query === "string")
        .map((tab) => ({ ...tab, pinned: Boolean((tab as { pinned?: unknown }).pinned) }));
  return {
    tabs: tabs.length ? tabs : [{ name: "Query 1", query: "SELECT * FROM ", pinned: false }],
    activeIndex: tabs.length && Number.isInteger(stored.activeIndex)
      ? Math.max(0, Math.min(stored.activeIndex as number, tabs.length - 1))
      : 0,
  };
}

function saveQueryTabState(connectionId: string | null, state: SavedQueryTabState) {
  if (!connectionId) return;
  try {
    const stored = JSON.parse(localStorage.getItem(QUERY_TABS_KEY) ?? "null");
    const byConnection = stored && typeof stored === "object" && stored.byConnection && typeof stored.byConnection === "object"
      ? stored.byConnection
      : {};
    localStorage.setItem(QUERY_TABS_KEY, JSON.stringify({ byConnection: { ...byConnection, [connectionId]: state } }));
  } catch {
    // Tab restoration is a convenience; the current session remains usable.
  }
}

const savedQueryTabState = loadQueryTabState(selectedId.value);
let nextTabId = savedQueryTabState.tabs.length + 1;
const queryTabs = ref<QueryTab[]>(savedQueryTabState.tabs.map((tab, index) => ({
  id: index + 1,
  name: tab.name,
  query: tab.query,
  pinned: tab.pinned,
  results: [],
  ranAt: "",
  activeResult: "0",
})));
queryTabs.value = [
  ...queryTabs.value.filter((tab) => tab.pinned),
  ...queryTabs.value.filter((tab) => !tab.pinned),
];
const activeTabId = ref(savedQueryTabState.activeIndex + 1);
const queryTabsBar = ref<HTMLElement | null>(null);
const draggedQueryTab = ref<{ id: number; pointerId: number; startX: number; startY: number; grabX: number; grabY: number; dragging: boolean } | null>(null);
const queryDragPreview = ref<HTMLElement | null>(null);
let queryTabReorderFrame = 0;
let latestQueryTabPointerEvent: PointerEvent | null = null;
let suppressQueryTabClick = false;
let suppressQueryTabClickTimer: ReturnType<typeof setTimeout> | undefined;
const hasMoreQueryTabsRight = ref(false);
const hasMoreQueryTabsLeft = ref(false);
let queryTabsResizeObserver: ResizeObserver | undefined;
const queryTabItems = computed(() => queryTabs.value.map((tab) => ({
  label: tab.name,
  value: tab.id,
  icon: "i-lucide-square-terminal",
  pinned: tab.pinned,
  ui: { trigger: draggedQueryTab.value?.id === tab.id && draggedQueryTab.value.dragging ? "pointer-events-none opacity-35" : "" },
})));
const draggedQueryTabLabel = computed(() => queryTabs.value.find((tab) => tab.id === draggedQueryTab.value?.id)?.name ?? "");
const activeTab = computed(() => queryTabs.value.find((tab) => tab.id === activeTabId.value) ?? queryTabs.value[0] ?? { id: 0, name: "", query: "", pinned: false, results: [], ranAt: "", activeResult: "0" });
const query = computed({ get: () => activeTab.value.query, set: (value: string) => { activeTab.value.query = value; } });
const running = ref(false);
const results = computed(() => activeTab.value.results);
const ranAt = computed(() => activeTab.value.ranAt);
const activeResult = computed({ get: () => activeTab.value.activeResult, set: (value: string) => { activeTab.value.activeResult = value; } });
const history = ref<string[]>(loadHistory());
const schemaTables = ref<TableSummary[]>([]);
const sqlResultsViewport = ref<HTMLElement | null>(null);
const sqlResultsScrollbar = ref<HTMLElement | null>(null);
const sqlEditorLayout = ref<HTMLElement | null>(null);
const sqlEditorSplit = ref(loadSqlEditorSplit());
const sqlEditorGridRows = computed(() => `${sqlEditorSplit.value}fr 1px ${1 - sqlEditorSplit.value}fr`);
const isSqlEditorResizing = ref(false);
let sqlEditorResize: { pointerId: number; startY: number; startSplit: number } | null = null;
const sqlResultsScrollWidth = ref(0);
const sqlResultsClientWidth = ref(0);
const sqlResultsScrollLeft = ref(0);
let sqlResultsScrollbarDrag: { pointerId: number; startX: number; startScrollLeft: number } | null = null;
let sqlResultsResizeObserver: ResizeObserver | undefined;

function updateSqlEditorGlow(event: PointerEvent) {
  const resizer = event.currentTarget as HTMLElement;
  const layout = sqlEditorLayout.value;
  if (!layout) return;
  const rect = layout.getBoundingClientRect();
  const x = Math.max(0, Math.min(rect.width, event.clientX - rect.left));
  resizer.style.setProperty("--resize-x", `${x}px`);
}

function startSqlEditorResize(event: PointerEvent) {
  if (event.button !== 0 || !sqlEditorLayout.value) return;
  updateSqlEditorGlow(event);
  sqlEditorResize = { pointerId: event.pointerId, startY: event.clientY, startSplit: sqlEditorSplit.value };
  isSqlEditorResizing.value = true;
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  event.preventDefault();
}

function moveSqlEditorResize(event: PointerEvent) {
  updateSqlEditorGlow(event);
  const drag = sqlEditorResize;
  const layout = sqlEditorLayout.value;
  if (!drag || drag.pointerId !== event.pointerId || !layout) return;
  const height = layout.getBoundingClientRect().height;
  if (!height) return;
  sqlEditorSplit.value = Math.max(
    SQL_EDITOR_SPLIT_MIN,
    Math.min(SQL_EDITOR_SPLIT_MAX, drag.startSplit + (event.clientY - drag.startY) / height),
  );
}

function endSqlEditorResize(event: PointerEvent) {
  if (sqlEditorResize?.pointerId !== event.pointerId) return;
  sqlEditorResize = null;
  isSqlEditorResizing.value = false;
  try {
    localStorage.setItem(SQL_EDITOR_SPLIT_KEY, String(sqlEditorSplit.value));
  } catch {
    // Remembering the split is a convenience; resizing still works this session.
  }
}

function adjustSqlEditorSplit(event: KeyboardEvent) {
  const split = event.key === "Home"
    ? SQL_EDITOR_SPLIT_MIN
    : event.key === "End"
      ? SQL_EDITOR_SPLIT_MAX
      : event.key === "ArrowUp"
        ? sqlEditorSplit.value + 0.02
        : event.key === "ArrowDown"
          ? sqlEditorSplit.value - 0.02
          : null;
  if (split === null) return;
  event.preventDefault();
  sqlEditorSplit.value = Math.max(SQL_EDITOR_SPLIT_MIN, Math.min(SQL_EDITOR_SPLIT_MAX, split));
  try {
    localStorage.setItem(SQL_EDITOR_SPLIT_KEY, String(sqlEditorSplit.value));
  } catch {
    // Remembering the split is a convenience; resizing still works this session.
  }
}
let sqlResultsMeasureFrame = 0;
let sqlResultsWheelFrame = 0;
let pendingSqlResultsWheelDelta = 0;
const contextQueryTabId = ref<number | null>(null);
const renameQueryTabOpen = ref(false);
const renameQueryTabDraft = ref("");

const readOnly = computed(() => selectedConnection.value?.readOnly ?? false);
const sqlHelp = computed(() =>
  `${readOnly.value ? "Read-only connections allow SELECT, SHOW, and EXPLAIN." : "INSERT, UPDATE, and DELETE run immediately."} SpacetimeDB SQL has a limited dialect. Use Ctrl+Space for supported keywords, tables, and columns. Separate statements with semicolons.`,
);

function loadHistory(): string[] {
  try {
    const stored = JSON.parse(localStorage.getItem(HISTORY_KEY) ?? "[]");
    return Array.isArray(stored) ? stored.filter((item) => typeof item === "string") : [];
  } catch {
    return [];
  }
}

function remember(sql: string) {
  history.value = [sql, ...history.value.filter((item) => item !== sql)].slice(0, HISTORY_LIMIT);
  try {
    localStorage.setItem(HISTORY_KEY, JSON.stringify(history.value));
  } catch {
    // History is a convenience; losing it is fine.
  }
}

const historyItems = computed(() =>
  history.value.map((sql) => ({
    label: sql.length > 80 ? `${sql.slice(0, 80)}…` : sql,
    onSelect: () => {
      if (!queryTabs.value.length) addQueryTab();
      query.value = sql;
    },
  })),
);

const resultTabs = computed(() =>
  results.value.map((result, index) => ({
    label: results.value.length > 1 ? `Statement ${index + 1}` : "Result",
    value: String(index),
    badge: result.rows.length,
  })),
);

const current = computed<SqlStatementResult | null>(
  () => results.value[Number(activeResult.value)] ?? null,
);
const sqlResultsThumbWidth = computed(() => {
  if (!sqlResultsScrollWidth.value || !sqlResultsClientWidth.value) return 100;
  return Math.max(8, (sqlResultsClientWidth.value / sqlResultsScrollWidth.value) * 100);
});
const sqlResultsThumbOffset = computed(() => {
  const maxScroll = sqlResultsScrollWidth.value - sqlResultsClientWidth.value;
  if (maxScroll <= 0) return 0;
  return (sqlResultsScrollLeft.value / maxScroll) * (sqlResultsClientWidth.value - sqlResultsClientWidth.value * sqlResultsThumbWidth.value / 100);
});

function measureSqlResultsScroll() {
  const viewport = sqlResultsViewport.value;
  if (!viewport) {
    sqlResultsScrollWidth.value = 0;
    sqlResultsClientWidth.value = 0;
    sqlResultsScrollLeft.value = 0;
    return;
  }
  sqlResultsScrollWidth.value = viewport.scrollWidth;
  sqlResultsClientWidth.value = viewport.clientWidth;
  sqlResultsScrollLeft.value = viewport.scrollLeft;
}

function scheduleSqlResultsScrollMeasure() {
  if (sqlResultsMeasureFrame) return;
  sqlResultsMeasureFrame = requestAnimationFrame(() => {
    sqlResultsMeasureFrame = 0;
    measureSqlResultsScroll();
  });
}

function scrollSqlResultsWithWheel(event: WheelEvent) {
  if (!event.shiftKey && event.deltaX === 0) return;
  const viewport = sqlResultsViewport.value;
  if (!viewport || viewport.scrollWidth <= viewport.clientWidth) return;
  const delta = event.deltaX || event.deltaY;
  if (!delta) return;
  event.preventDefault();
  pendingSqlResultsWheelDelta += delta;
  if (sqlResultsWheelFrame) return;
  sqlResultsWheelFrame = requestAnimationFrame(() => {
    sqlResultsWheelFrame = 0;
    const currentViewport = sqlResultsViewport.value;
    if (currentViewport) currentViewport.scrollLeft += pendingSqlResultsWheelDelta;
    pendingSqlResultsWheelDelta = 0;
  });
}

function startSqlResultsScrollbarDrag(event: PointerEvent) {
  if (event.button !== 0) return;
  const viewport = sqlResultsViewport.value;
  const scrollbar = sqlResultsScrollbar.value;
  if (!viewport || !scrollbar) return;
  const thumb = event.target instanceof Element && event.target.closest<HTMLElement>("[data-results-scroll-thumb]");
  if (!thumb) {
    const rect = scrollbar.getBoundingClientRect();
    const ratio = Math.max(0, Math.min(1, (event.clientX - rect.left) / rect.width));
    viewport.scrollLeft = ratio * (viewport.scrollWidth - viewport.clientWidth);
    measureSqlResultsScroll();
    return;
  }
  sqlResultsScrollbarDrag = {
    pointerId: event.pointerId,
    startX: event.clientX,
    startScrollLeft: viewport.scrollLeft,
  };
  event.preventDefault();
}

function moveSqlResultsScrollbarDrag(event: PointerEvent) {
  const drag = sqlResultsScrollbarDrag;
  const viewport = sqlResultsViewport.value;
  const scrollbar = sqlResultsScrollbar.value;
  if (!drag || drag.pointerId !== event.pointerId || !viewport || !scrollbar) return;
  const thumbWidth = sqlResultsClientWidth.value * sqlResultsThumbWidth.value / 100;
  const scrollRange = viewport.scrollWidth - viewport.clientWidth;
  const trackRange = scrollbar.clientWidth - thumbWidth;
  if (trackRange <= 0) return;
  viewport.scrollLeft = drag.startScrollLeft + (event.clientX - drag.startX) * scrollRange / trackRange;
  scheduleSqlResultsScrollMeasure();
}

function endSqlResultsScrollbarDrag(event: PointerEvent) {
  if (sqlResultsScrollbarDrag?.pointerId === event.pointerId) sqlResultsScrollbarDrag = null;
}

watch(current, async () => {
  await nextTick();
  scheduleSqlResultsScrollMeasure();
});

watch(sqlResultsViewport, async (viewport) => {
  sqlResultsResizeObserver?.disconnect();
  if (viewport && sqlResultsResizeObserver) {
    sqlResultsResizeObserver.observe(viewport);
    const table = viewport.querySelector<HTMLElement>('[data-slot="base"]');
    if (table) sqlResultsResizeObserver.observe(table);
  }
  await nextTick();
  scheduleSqlResultsScrollMeasure();
}, { flush: "post" });

function addQueryTab() {
  const id = nextTabId++;
  queryTabs.value.push({ id, name: `Query ${id}`, query: "SELECT * FROM ", pinned: false, results: [], ranAt: "", activeResult: "0" });
  activeTabId.value = id;
}

function closeQueryTab(id: number) {
  const target = queryTabs.value.find((tab) => tab.id === id);
  if (!target || queryTabs.value.length <= 1 || target.pinned || running.value && id === activeTabId.value) return;
  const index = queryTabs.value.findIndex((tab) => tab.id === id);
  queryTabs.value = queryTabs.value.filter((tab) => tab.id !== id);
  if (activeTabId.value === id) activeTabId.value = queryTabs.value[Math.max(0, index - 1)]?.id ?? 0;
}

function togglePinnedQueryTab(id: number) {
  const tab = queryTabs.value.find((item) => item.id === id);
  if (!tab) return;
  tab.pinned = !tab.pinned;
  queryTabs.value = [
    ...queryTabs.value.filter((item) => item.pinned),
    ...queryTabs.value.filter((item) => !item.pinned),
  ];
}

function scrollQueryTabsHorizontally(event: WheelEvent) {
  const list = queryTabsBar.value?.querySelector<HTMLElement>('[data-slot="list"]');
  if (!list || list.scrollWidth <= list.clientWidth) return;
  const delta = Math.abs(event.deltaX) > Math.abs(event.deltaY) ? event.deltaX : event.deltaY;
  if (!delta) return;
  event.preventDefault();
  list.scrollLeft += delta;
}

function startQueryTabPointerDrag(event: PointerEvent) {
  if (!(event.target instanceof Element)) return;
  if (event.button === 2) {
    if (event.target.closest('[role="tab"]')) event.stopPropagation();
    return;
  }
  if (event.button !== 0 || event.target.closest('[role="button"]')) return;
  const trigger = event.target.closest<HTMLElement>('[role="tab"]');
  const list = queryTabsBar.value?.querySelector<HTMLElement>('[data-slot="list"]');
  if (!trigger || !list) return;
  const index = Array.from(list.querySelectorAll('[role="tab"]')).indexOf(trigger);
  const tab = queryTabs.value[index];
  if (!tab) return;
  const rect = trigger.getBoundingClientRect();
  draggedQueryTab.value = { id: tab.id, pointerId: event.pointerId, startX: event.clientX, startY: event.clientY, grabX: event.clientX - rect.left, grabY: event.clientY - rect.top, dragging: false };
}

function moveQueryTabPointerDrag(event: PointerEvent) {
  const drag = draggedQueryTab.value;
  if (!drag || drag.pointerId !== event.pointerId) return;
  if (!drag.dragging && Math.hypot(event.clientX - drag.startX, event.clientY - drag.startY) < 6) return;
  drag.dragging = true;
  positionQueryDragPreview(drag, event.clientX, event.clientY);
  event.preventDefault();
  latestQueryTabPointerEvent = event;
  if (!queryTabReorderFrame) {
    queryTabReorderFrame = requestAnimationFrame(() => {
      queryTabReorderFrame = 0;
      const latestEvent = latestQueryTabPointerEvent;
      const activeDrag = draggedQueryTab.value;
      latestQueryTabPointerEvent = null;
      if (!latestEvent || !activeDrag?.dragging) return;
      const list = queryTabsBar.value?.querySelector<HTMLElement>('[data-slot="list"]');
      if (!list) return;
      const listRect = list.getBoundingClientRect();
      if (latestEvent.clientX < listRect.left + 24) list.scrollLeft -= 12;
      else if (latestEvent.clientX > listRect.right - 24) list.scrollLeft += 12;
      reorderQueryTabAtPointer(latestEvent, activeDrag);
    });
  }
}

function positionQueryDragPreview(drag: NonNullable<typeof draggedQueryTab.value>, x: number, y: number) {
  const position = () => {
    if (draggedQueryTab.value !== drag || !queryDragPreview.value) return;
    queryDragPreview.value.style.transform = `translate3d(${x - drag.grabX}px, ${y - drag.grabY}px, 0)`;
  };
  if (queryDragPreview.value) position();
  else void nextTick(position);
}

function reorderQueryTabAtPointer(event: PointerEvent, drag: NonNullable<typeof draggedQueryTab.value>) {
  const list = queryTabsBar.value?.querySelector<HTMLElement>('[data-slot="list"]');
  if (!list) return;
  const hit = document.elementFromPoint(event.clientX, event.clientY);
  const trigger = hit instanceof Element ? hit.closest<HTMLElement>('[role="tab"]') : null;
  if (!trigger) return;
  const targetIndex = Array.from(list.querySelectorAll('[role="tab"]')).indexOf(trigger);
  const target = queryTabs.value[targetIndex];
  const dragged = queryTabs.value.find((tab) => tab.id === drag.id);
  if (!target || !dragged || target.id === dragged.id) return;
  const pinned = queryTabs.value.filter((tab) => tab.pinned);
  const unpinned = queryTabs.value.filter((tab) => !tab.pinned);
  const sourceGroup = dragged.pinned ? pinned : unpinned;
  const sourceIndex = sourceGroup.findIndex((tab) => tab.id === dragged.id);
  if (sourceIndex < 0) return;
  sourceGroup.splice(sourceIndex, 1);

  let insertAt: number;
  if (dragged.pinned === target.pinned) {
    const targetPosition = sourceGroup.findIndex((tab) => tab.id === target.id);
    const targetRect = trigger.getBoundingClientRect();
    insertAt = targetPosition + (event.clientX > targetRect.left + targetRect.width / 2 ? 1 : 0);
  } else {
    // Keep pinned and unpinned tabs in their own groups when dragging across
    // the boundary between them.
    insertAt = dragged.pinned ? sourceGroup.length : 0;
  }
  sourceGroup.splice(Math.max(0, insertAt), 0, dragged);
  const reordered = [...pinned, ...unpinned];
  if (reordered.some((tab, index) => tab.id !== queryTabs.value[index]?.id)) {
    const previousRects = new Map(Array.from(list.querySelectorAll<HTMLElement>('[role="tab"]')).map((tab, index) => [queryTabs.value[index]?.id, tab.getBoundingClientRect()]));
    queryTabs.value = reordered;
    void nextTick(() => {
      const updatedList = queryTabsBar.value?.querySelector<HTMLElement>('[data-slot="list"]');
      updatedList?.querySelectorAll<HTMLElement>('[role="tab"]').forEach((tab, index) => {
        const previous = previousRects.get(queryTabs.value[index]?.id);
        if (!previous) return;
        const current = tab.getBoundingClientRect();
        const x = previous.left - current.left;
        const y = previous.top - current.top;
        if (Math.abs(x) + Math.abs(y) < 1) return;
        tab.getAnimations().forEach((animation) => animation.cancel());
        tab.animate([{ transform: `translate(${x}px, ${y}px)` }, { transform: "translate(0, 0)" }], { duration: 130, easing: "ease-out" });
      });
    });
  }
}

function endQueryTabPointerDrag(event: PointerEvent) {
  const drag = draggedQueryTab.value;
  if (!drag || drag.pointerId !== event.pointerId) return;
  if (drag.dragging) {
    if (queryTabReorderFrame) cancelAnimationFrame(queryTabReorderFrame);
    queryTabReorderFrame = 0;
    latestQueryTabPointerEvent = null;
    positionQueryDragPreview(drag, event.clientX, event.clientY);
    reorderQueryTabAtPointer(event, drag);
    suppressQueryTabClick = true;
    if (suppressQueryTabClickTimer) clearTimeout(suppressQueryTabClickTimer);
    suppressQueryTabClickTimer = setTimeout(() => { suppressQueryTabClick = false; }, 300);
  }
  draggedQueryTab.value = null;
}

function suppressQueryTabClickAfterDrag(event: MouseEvent) {
  if (!suppressQueryTabClick) return;
  suppressQueryTabClick = false;
  if (suppressQueryTabClickTimer) clearTimeout(suppressQueryTabClickTimer);
  event.preventDefault();
  event.stopPropagation();
}

function activateQueryTabClose(id: number) {
  const tab = queryTabs.value.find((item) => item.id === id);
  if (tab?.pinned) togglePinnedQueryTab(id);
  else closeQueryTab(id);
}

function setContextQueryTab(event: MouseEvent) {
  contextQueryTabId.value = null;
  if (!(event.target instanceof Element)) return;
  const trigger = event.target.closest<HTMLElement>('[role="tab"]');
  const list = queryTabsBar.value?.querySelector<HTMLElement>('[data-slot="list"]');
  if (!trigger || !list) return;
  const index = Array.from(list.querySelectorAll('[role="tab"]')).indexOf(trigger);
  contextQueryTabId.value = queryTabs.value[index]?.id ?? null;
}

function renameContextQueryTab() {
  const tab = queryTabs.value.find((item) => item.id === contextQueryTabId.value);
  const name = renameQueryTabDraft.value.trim();
  if (tab && name) tab.name = name;
  renameQueryTabOpen.value = false;
}

const queryContextMenuItems = computed(() => {
  const tab = queryTabs.value.find((item) => item.id === contextQueryTabId.value);
  if (!tab) return [];
  return [
    { label: "Rename tab", icon: "i-lucide-pencil", onSelect: () => { renameQueryTabDraft.value = tab.name; renameQueryTabOpen.value = true; } },
    { type: "separator" as const },
    { label: tab.pinned ? "Unpin tab" : "Pin tab", icon: tab.pinned ? "i-lucide-pin-off" : "i-lucide-pin", onSelect: () => togglePinnedQueryTab(tab.id) },
  ];
});

function closeQueryTabFromMiddleClick(event: MouseEvent) {
  if (event.button !== 1 || !(event.target instanceof Element)) return;
  const trigger = event.target.closest<HTMLElement>('[role="tab"]');
  const list = queryTabsBar.value?.querySelector<HTMLElement>('[data-slot="list"]');
  if (!trigger || !list) return;
  const index = Array.from(list.querySelectorAll('[role="tab"]')).indexOf(trigger);
  const tab = queryTabs.value[index];
  if (!tab || queryTabs.value.length <= 1 || tab.pinned || running.value && tab.id === activeTabId.value) return;
  event.preventDefault();
  closeQueryTab(tab.id);
}

async function measureQueryTabOverflow() {
  await nextTick();
  const list = queryTabsBar.value?.querySelector<HTMLElement>('[data-slot="list"]');
  hasMoreQueryTabsLeft.value = Boolean(list && list.scrollLeft > 1);
  hasMoreQueryTabsRight.value = Boolean(list && list.scrollLeft + list.clientWidth < list.scrollWidth - 1);
}

watch(queryTabItems, () => {
  if (!draggedQueryTab.value?.dragging) void measureQueryTabOverflow();
});
watch(
  () => draggedQueryTab.value?.dragging ? null : JSON.stringify({
      tabs: queryTabs.value.map(({ name, query, pinned }) => ({ name, query, pinned })),
      activeIndex: queryTabs.value.findIndex((tab) => tab.id === activeTabId.value),
    }),
  (state) => {
    if (state === null) return;
    try {
      saveQueryTabState(selectedId.value, JSON.parse(state) as SavedQueryTabState);
    } catch {
      // Tab restoration is a convenience; the current session remains usable.
    }
  },
);

watch(selectedId, (connectionId, previousConnectionId) => {
  if (previousConnectionId) {
    saveQueryTabState(previousConnectionId, {
      tabs: queryTabs.value.map(({ name, query, pinned }) => ({ name, query, pinned })),
      activeIndex: queryTabs.value.findIndex((tab) => tab.id === activeTabId.value),
    });
  }
  const state = loadQueryTabState(connectionId);
  queryTabs.value = state.tabs.map((tab, index) => ({
    id: index + 1,
    name: tab.name,
    query: tab.query,
    pinned: tab.pinned,
    results: [],
    ranAt: "",
    activeResult: "0",
  }));
  queryTabs.value = [...queryTabs.value.filter((tab) => tab.pinned), ...queryTabs.value.filter((tab) => !tab.pinned)];
  activeTabId.value = queryTabs.value[state.activeIndex]?.id ?? queryTabs.value[0]?.id ?? 0;
  nextTabId = queryTabs.value.length + 1;
}, { flush: "sync" });

async function scrollActiveQueryTabIntoView() {
  await nextTick();
  const list = queryTabsBar.value?.querySelector<HTMLElement>('[data-slot="list"]');
  const active = list?.querySelector<HTMLElement>('[data-state="active"]');
  if (!list || !active) return;
  if (active === list.lastElementChild || active.parentElement === list.lastElementChild) {
    list.scrollTo({ left: list.scrollWidth, behavior: "smooth" });
    return;
  }
  const listRect = list.getBoundingClientRect();
  const activeRect = active.getBoundingClientRect();
  if (activeRect.left < listRect.left) {
    list.scrollBy({ left: activeRect.left - listRect.left, behavior: "smooth" });
  } else if (activeRect.right > listRect.right) {
    list.scrollBy({ left: activeRect.right - listRect.right, behavior: "smooth" });
  }
}

watch(activeTabId, scrollActiveQueryTabIntoView);

// SQL results are positional arrays; key each cell by column index so
// duplicate or empty column names still render.
const tableColumns = computed<TableColumn<unknown[]>[]>(() =>
  (current.value?.columns ?? []).map((column, index) => ({
    id: `c${index}`,
    header: column.name,
    accessorFn: (row) => formatCell(row[index], column.kind),
    meta: {
      class: {
        th: "whitespace-nowrap border-r border-default",
        td: "min-w-40 border-r border-default/60 align-top",
      },
    },
  })),
);

function formatDuration(micros?: number | null) {
  if (micros == null) return "";
  return micros < 1000 ? `${micros} µs` : `${(micros / 1000).toFixed(1)} ms`;
}

async function run() {
  const connectionId = getSelectedConnectionId();
  const sql = query.value.trim();
  if (!connectionId || !sql || running.value) return;

  running.value = true;

  try {
    const response = await executeSql(connectionId, sql);
    activeTab.value.results = response.statements;
    activeResult.value = "0";
    activeTab.value.ranAt = new Date().toLocaleTimeString();
    remember(sql);
  } catch (err) {
    toast.add({
      title: "SQL query failed",
      description: String(err),
      color: "error",
      icon: "i-lucide-circle-alert",
    });
  } finally {
    running.value = false;
  }
}

function handleSqlPageRefresh(reason: PageRefreshReason = "manual") {
  // Schema and per-connection query tabs update from their own connection watchers.
  // Re-running SQL on a connection switch would execute the old editor contents
  // against the newly selected database without the user asking.
  if (reason === "connection-change") return;
  return run();
}

async function exportCurrent(format: ExportFormat) {
  if (!current.value) return;
  try {
    const path = await exportRows("query", format, current.value.columns, current.value.rows);
    if (path) {
      toast.add({ title: "Export saved", description: path, color: "success", icon: "i-lucide-check" });
    }
  } catch (err) {
    toast.add({ title: "Export failed", description: String(err), color: "error" });
  }
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
    event.preventDefault();
    run();
  }
}

onMounted(() => {
  sqlResultsResizeObserver = new ResizeObserver(measureSqlResultsScroll);
  window.addEventListener("pointermove", moveSqlResultsScrollbarDrag);
  window.addEventListener("pointerup", endSqlResultsScrollbarDrag);
  window.addEventListener("pointercancel", endSqlResultsScrollbarDrag);
  window.addEventListener("pointermove", moveQueryTabPointerDrag);
  window.addEventListener("pointerup", endQueryTabPointerDrag);
  window.addEventListener("pointercancel", endQueryTabPointerDrag);
  if (queryTabsBar.value) {
    queryTabsResizeObserver = new ResizeObserver(measureQueryTabOverflow);
    queryTabsResizeObserver.observe(queryTabsBar.value);
  }
  void measureQueryTabOverflow();
  void scrollActiveQueryTabIntoView();
  setPageRefreshHandler(handleSqlPageRefresh);
});
onUnmounted(() => {
  window.removeEventListener("pointermove", moveSqlResultsScrollbarDrag);
  window.removeEventListener("pointerup", endSqlResultsScrollbarDrag);
  window.removeEventListener("pointercancel", endSqlResultsScrollbarDrag);
  sqlResultsResizeObserver?.disconnect();
  if (sqlResultsMeasureFrame) cancelAnimationFrame(sqlResultsMeasureFrame);
  if (sqlResultsWheelFrame) cancelAnimationFrame(sqlResultsWheelFrame);
  if (queryTabReorderFrame) cancelAnimationFrame(queryTabReorderFrame);
  window.removeEventListener("pointermove", moveQueryTabPointerDrag);
  window.removeEventListener("pointerup", endQueryTabPointerDrag);
  window.removeEventListener("pointercancel", endQueryTabPointerDrag);
  if (suppressQueryTabClickTimer) clearTimeout(suppressQueryTabClickTimer);
  queryTabsResizeObserver?.disconnect();
  clearPageRefreshHandler(handleSqlPageRefresh);
});

watch(
  () => selectedConnection.value?.id,
  async (connectionId) => {
    schemaTables.value = [];
    if (!connectionId) return;
    try {
      const schema = await getSchema(connectionId);
      if (selectedConnection.value?.id === connectionId) {
        schemaTables.value = schema.tables;
      }
    } catch {
      // Keep the editor usable even if schema introspection is unavailable.
    }
  },
  { immediate: true },
);
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <div ref="queryTabsBar" class="relative flex min-h-12 shrink-0 items-center justify-between gap-2 overflow-hidden border-b border-default bg-elevated/40 px-1" @scroll.capture="measureQueryTabOverflow">
      <UContextMenu v-if="queryTabs.length" :items="queryContextMenuItems" :disabled="contextQueryTabId === null">
      <div class="min-w-0 flex-1 overflow-hidden" @contextmenu.capture="setContextQueryTab" @pointerdown.capture="startQueryTabPointerDrag" @click.capture="suppressQueryTabClickAfterDrag" @wheel="scrollQueryTabsHorizontally">
      <UTabs
        v-if="queryTabs.length"
        v-model="activeTabId"
        :items="queryTabItems"
        :content="false"
        size="lg"
        variant="pill"
        class="min-w-0 overflow-hidden"
        @mousedown.middle.stop.prevent="closeQueryTabFromMiddleClick"
        :ui="{
          root: 'min-w-0 overflow-hidden',
          list: 'tab-scroll-list min-w-0 flex-nowrap gap-1 overflow-x-auto overflow-y-hidden bg-transparent py-1',
          indicator: 'hidden',
          trigger: 'min-h-10 flex-none w-fit cursor-grab rounded-lg border border-transparent px-3 py-2 text-sm font-medium focus-visible:outline-none active:cursor-grabbing data-[state=active]:border-accented data-[state=active]:bg-accented data-[state=active]:font-semibold data-[state=active]:text-highlighted data-[state=active]:shadow-sm',
          label: 'max-w-48 truncate',
        }"
      >
        <template #trailing="{ item }">
          <span
            v-if="queryTabs.length > 1"
            role="button"
            tabindex="0"
            class="ml-1 inline-flex size-6 shrink-0 items-center justify-center rounded-md text-muted transition-colors hover:bg-elevated hover:text-highlighted focus-visible:outline-2 focus-visible:outline-primary"
            :aria-label="item.pinned ? `Unpin ${item.label} tab` : `Close ${item.label} tab`"
            :title="item.pinned ? 'Click to unpin tab' : `Close ${item.label}`"
            :class="running && Number(item.value) === activeTabId && !item.pinned ? 'pointer-events-none opacity-40' : ''"
            @click.stop.prevent="activateQueryTabClose(Number(item.value))"
            @keydown.enter.stop.prevent="activateQueryTabClose(Number(item.value))"
            @keydown.space.stop.prevent="activateQueryTabClose(Number(item.value))"
          >
            <UIcon :name="item.pinned ? 'i-lucide-pin' : 'i-lucide-x'" class="size-4" />
          </span>
        </template>
      </UTabs>
      </div>
      </UContextMenu>
      <div v-if="hasMoreQueryTabsLeft" class="pointer-events-none absolute inset-y-0 left-0 z-10 w-12 bg-gradient-to-r from-elevated via-elevated/95 to-transparent" />
      <div v-if="hasMoreQueryTabsRight" class="pointer-events-none absolute inset-y-0 right-32 z-10 w-12 bg-gradient-to-l from-elevated via-elevated/95 to-transparent" />
      <div class="sticky right-0 z-20 flex shrink-0 items-center px-1">
        <UButton icon="i-lucide-plus" color="neutral" variant="soft" size="md" @click="addQueryTab">
          New query
        </UButton>
      </div>
    </div>
    <div ref="sqlEditorLayout" class="grid min-h-0 min-w-0 flex-1" :style="{ gridTemplateRows: sqlEditorGridRows }">
    <template v-if="queryTabs.length">
    <section class="min-h-0">
      <SqlCodeEditor
        :model-id="activeTabId"
        v-model="query"
        :tables="schemaTables"
        @keydown="onKeydown"
      />
    </section>

    <div
      role="separator"
      aria-label="Resize SQL editor and results"
      aria-orientation="horizontal"
      aria-valuemin="20"
      aria-valuemax="80"
      :aria-valuenow="Math.round(sqlEditorSplit * 100)"
      :data-resizing="isSqlEditorResizing"
      tabindex="0"
      class="pane-resizer pane-resizer--horizontal relative z-10 h-px w-full cursor-row-resize touch-none bg-transparent outline-none"
      @pointerdown="startSqlEditorResize"
      @pointermove="moveSqlEditorResize"
      @pointerup="endSqlEditorResize"
      @pointercancel="endSqlEditorResize"
      @keydown="adjustSqlEditorSplit"
    >
      <span class="pane-resizer-line pointer-events-none absolute inset-x-0 top-0 h-px" />
      <span class="pane-resizer-glow pointer-events-none absolute inset-x-0" />
    </div>

    <section class="flex min-h-0 min-w-0 flex-col">
      <div v-if="results.length" class="group flex min-h-0 flex-1 flex-col">
      <div class="flex shrink-0 flex-wrap items-center justify-between gap-2 border-b border-default p-2">
        <UTabs
          v-if="results.length > 1"
          v-model="activeResult"
          :items="resultTabs"
          :content="false"
          size="xs"
          variant="link"
        />
        <span class="px-1 text-xs text-muted">
          {{ current?.rows.length.toLocaleString() }} row(s)
          <template v-if="current?.durationMicros != null">
            &middot; {{ formatDuration(current.durationMicros) }}
          </template>
          &middot; ran at {{ ranAt }}
        </span>
        <div class="flex gap-1">
          <UButton
            icon="i-lucide-sheet"
            size="xs"
            color="neutral"
            variant="soft"
            :disabled="!current?.columns.length"
            @click="exportCurrent('csv')"
            >CSV</UButton
          >
          <UButton
            icon="i-lucide-braces"
            size="xs"
            color="neutral"
            variant="soft"
            :disabled="!current?.columns.length"
            @click="exportCurrent('json')"
            >JSON</UButton
          >
        </div>
      </div>

      <p v-if="current && !current.columns.length" class="p-3 text-sm text-muted">
        Statement ran successfully and returned no result set.
      </p>
      <div
        v-else
        ref="sqlResultsViewport"
        class="sql-results-scroll min-h-0 min-w-0 flex-1 overflow-x-hidden overflow-y-auto"
        @scroll.passive="scheduleSqlResultsScrollMeasure"
        @wheel="scrollSqlResultsWithWheel"
      >
      <UTable
        :data="current?.rows ?? []"
        :columns="tableColumns"
        sticky="header"
        class="w-max min-w-full"
        :ui="{ ...dataTableUi, base: 'w-max min-w-full border-separate border-spacing-0' }"
      >
        <template
          v-for="(column, index) in current?.columns ?? []"
          :key="`c${index}`"
          #[`c${index}-header`]
        >
          <span class="inline-flex items-center gap-1">
            {{ column.name }}
            <span class="font-normal">({{ column.type }})</span>
          </span>
        </template>
        <template
          v-for="(column, index) in current?.columns ?? []"
          :key="`c${index}-cell`"
          #[`c${index}-cell`]="{ row }"
        >
          <div :tabindex="0" :class="selectedCellUi" class="max-w-md">
            <span
              class="block truncate text-highlighted"
              :title="formatCell(row.original[index], column.kind)"
            >
              {{ formatCell(row.original[index], column.kind) }}
            </span>
          </div>
        </template>
        <template #empty>No rows returned.</template>
      </UTable>
      </div>
      <div
        v-if="sqlResultsScrollWidth > sqlResultsClientWidth"
        ref="sqlResultsScrollbar"
        role="scrollbar"
        aria-label="SQL results horizontal scrollbar"
        aria-orientation="horizontal"
        tabindex="0"
        class="sql-results-horizontal-track mx-2 mb-1 h-[10px] shrink-0 cursor-pointer rounded-full bg-elevated opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100 pointer-events-none group-hover:pointer-events-auto group-focus-within:pointer-events-auto"
        @pointerdown="startSqlResultsScrollbarDrag"
        @keydown.left.prevent="sqlResultsViewport && (sqlResultsViewport.scrollLeft -= 48)"
        @keydown.right.prevent="sqlResultsViewport && (sqlResultsViewport.scrollLeft += 48)"
      >
        <div
          data-results-scroll-thumb
          class="h-full rounded-full bg-accented hover:bg-muted"
          :style="{ width: `${sqlResultsThumbWidth}%`, transform: `translateX(${sqlResultsThumbOffset}px)` }"
        />
      </div>
      </div>

      <div
        v-else
        class="flex min-h-0 flex-1 items-center justify-center text-sm text-muted"
      >
        Run a query to see results here.
      </div>
    </section>
    </template>
    <div v-else class="col-span-full flex items-center justify-center text-sm text-muted">
      No query tabs open. Select New query to create one.
    </div>
    </div>

    <UModal v-model:open="renameQueryTabOpen" title="Rename query tab">
      <template #body>
        <UFormField label="Tab name">
          <UInput v-model="renameQueryTabDraft" class="w-full" autofocus @keydown.enter.prevent="renameContextQueryTab" />
        </UFormField>
      </template>
      <template #footer>
        <div class="flex w-full justify-end gap-2">
          <UButton color="neutral" variant="soft" @click="renameQueryTabOpen = false">Cancel</UButton>
          <UButton :disabled="!renameQueryTabDraft.trim()" @click="renameContextQueryTab">Rename</UButton>
        </div>
      </template>
    </UModal>

    <Teleport to="#page-header-actions">
      <UTooltip :text="sqlHelp">
        <UButton
          icon="i-lucide-circle-help"
          color="neutral"
          variant="ghost"
          aria-label="SQL editor help"
        />
      </UTooltip>
      <UDropdownMenu :items="historyItems" :content="{ align: 'end' }">
        <UButton
          icon="i-lucide-history"
          color="neutral"
          variant="soft"
          :disabled="!history.length"
          aria-label="Query history"
          title="Query history"
        />
      </UDropdownMenu>
      <UButton icon="i-lucide-play" :loading="running" :disabled="!queryTabs.length" @click="run">
        Run
        <UKbd value="ctrl" size="sm" class="ml-1" /><UKbd value="enter" size="sm" />
      </UButton>
    </Teleport>
    <div
      v-if="draggedQueryTab?.dragging"
      ref="queryDragPreview"
      class="pointer-events-none fixed z-50 flex items-center gap-2 rounded-lg border border-primary/40 bg-elevated px-3 py-2 text-sm font-medium text-highlighted shadow-lg opacity-95"
      style="left: 0; top: 0; will-change: transform"
      aria-hidden="true"
    >
      <UIcon name="i-lucide-square-terminal" class="size-4 text-muted" />
      <span class="max-w-48 truncate">{{ draggedQueryTabLabel }}</span>
    </div>
  </div>
</template>

<style>
.sql-results-horizontal-track {
  touch-action: none;
}

.sql-results-horizontal-track [data-results-scroll-thumb] {
  min-width: 1.5rem;
}

.tab-scroll-list {
  scrollbar-width: none;
  -ms-overflow-style: none;
}

.tab-scroll-list::-webkit-scrollbar {
  display: none;
}
</style>
