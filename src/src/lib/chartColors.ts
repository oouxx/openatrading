/**
 * Chart color resolver — reads CSS custom properties at runtime
 * so canvas/chart libraries get concrete hex values that match the active theme.
 */
function resolveVar(name: string, fallback: string): string {
  if (typeof window === "undefined") return fallback
  const val = getComputedStyle(document.documentElement)
    .getPropertyValue(name)
    .trim()
  // oklch() values can't be used directly by canvas — convert via a temporary element
  if (val.startsWith("oklch") || val.startsWith("var(")) return fallback
  return val || fallback
}

export const chartColors = {
  up: () => resolveVar("--color-up", "#FF5C5C"),
  down: () => resolveVar("--color-down", "#1D9E75"),
  brand: () => resolveVar("--color-brand", "#D97757"),
  warning: () => resolveVar("--color-warning", "#F5A623"),
  primary: () => resolveVar("--color-primary", "#FFFFFF"),
  tertiary: () => resolveVar("--color-tertiary", "#888780"),
  // MA line colors — chart-specific palette, not theme tokens
  ma5: "#F5A623",
  ma10: "#4A90D9",
  ma20: "#D0021B",
  ma30: "#7ED321",
}
