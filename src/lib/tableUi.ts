// Columns pinned to the right edge (via UTable's `column-pinning`) get a near
// opaque surface, a divider and a soft shadow so scrolled content reads as
// sliding underneath them.
const pinnedRight =
  "data-[pinned=right]:border-l data-[pinned=right]:bg-default/95 data-[pinned=right]:shadow-[-12px_0_18px_-18px_rgba(0,0,0,0.9)]";

// Shared UTable slot classes that restyle Nuxt UI's defaults into the compact
// data-grid look used across the app. Borders sit on the cells (separate
// border model) rather than the rows so they travel with the sticky header.
export const dataTableUi = {
  base: "border-separate border-spacing-0",
  thead: "bg-default/60",
  tbody: "divide-y-0 [&>tr]:data-[selectable=true]:hover:bg-default/50",
  th: `border-b border-default px-3 py-2 text-xs font-medium text-muted ${pinnedRight}`,
  td: `whitespace-normal border-b border-default/60 px-3 py-2 text-default ${pinnedRight}`,
  separator: "hidden",
  empty: "px-3 py-6 text-left text-muted",
};

// A cell receives keyboard focus when clicked. Keep the selection treatment
// on the inner cell content so it works for both default and custom slots.
export const selectedCellUi =
  "relative -mx-3 -my-2 min-h-9 px-3 py-2 outline-none focus:bg-success/10 focus:text-success focus:shadow-[inset_0_0_0_1px_var(--ui-color-success-500)] focus-within:bg-success/10 focus-within:text-success focus-within:shadow-[inset_0_0_0_1px_var(--ui-color-success-500)]";
