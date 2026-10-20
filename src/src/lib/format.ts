/**
 * Format a yuan / share amount with Chinese compact units.
 *
 * The same helper is used for turnover, market cap and volume so tables do not
 * drift into different precision or unit conventions.
 */
export function formatCompactNumber(value: number | null | undefined): string {
  if (value == null || !Number.isFinite(value) || value === 0) return "-"
  const abs = Math.abs(value)
  if (abs >= 1e12) return `${(value / 1e12).toFixed(2)}万亿`
  if (abs >= 1e8) return `${(value / 1e8).toFixed(2)}亿`
  if (abs >= 1e4) return `${(value / 1e4).toFixed(2)}万`
  return value.toFixed(0)
}

/** Format a decimal ratio (`0.0523`) as a signed percentage (`+5.23%`). */
export function formatRatioPct(value: number): string {
  if (!Number.isFinite(value)) return "-"
  return `${value >= 0 ? "+" : ""}${(value * 100).toFixed(2)}%`
}

/**
 * Format a value that is already a percentage (`5.23` → `+5.23%`).
 * Do not multiply again — backend `*_pct` fields already use percent units.
 */
export function formatPct(value: number): string {
  if (!Number.isFinite(value)) return "-"
  return `${value >= 0 ? "+" : ""}${value.toFixed(2)}%`
}

/** Shared rise/fall colour mapping. Zero and missing values stay neutral. */
export function pctClass(value: number | null | undefined): string {
  if (value == null || value === 0) return "text-primary"
  return value > 0 ? "text-up" : "text-down"
}
