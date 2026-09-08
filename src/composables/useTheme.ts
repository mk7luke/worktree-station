import { ref, watch } from "vue";
import type { Settings } from "@/lib/types";

type Theme = Settings["theme"];

const theme = ref<Theme>("system");
const systemDark = window.matchMedia("(prefers-color-scheme: dark)");

function apply() {
  const dark = theme.value === "dark" || (theme.value === "system" && systemDark.matches);
  document.documentElement.dataset.theme = dark ? "dark" : "light";
}

systemDark.addEventListener("change", apply);
watch(theme, apply, { immediate: true });

export function useTheme() {
  return {
    theme,
    setTheme: (next: Theme) => {
      theme.value = next;
    },
  };
}
