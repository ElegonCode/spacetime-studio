import { computed, ref, watch } from "vue";

export type ThemePreference = "system" | "light" | "dark";

const storageKey = "spacetime-studio.theme";
const savedPreference = localStorage.getItem(storageKey);
export const themePreference = ref<ThemePreference>(
  savedPreference === "light" || savedPreference === "dark" ? savedPreference : "system",
);
const systemPrefersDark = ref(window.matchMedia("(prefers-color-scheme: dark)").matches);

export const resolvedTheme = computed(() =>
  themePreference.value === "system"
    ? systemPrefersDark.value ? "dark" : "light"
    : themePreference.value,
);

watch(themePreference, (preference) => {
  localStorage.setItem(storageKey, preference);
}, { immediate: true, flush: "sync" });

watch(resolvedTheme, (theme) => {
  document.documentElement.classList.toggle("dark", theme === "dark");
  document.documentElement.style.colorScheme = theme;
}, { immediate: true, flush: "sync" });

window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", (event) => {
  systemPrefersDark.value = event.matches;
});
