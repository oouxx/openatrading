/**
 * Chart color resolver — reads CSS custom properties at runtime
 * so canvas/chart libraries get concrete hex values that match the active theme.
 *
 * Theme tokens are authored in oklch(). Neither canvas nor lightweight-charts
 * can parse oklch reliably (LWC does its own color parsing and only accepts
 * hex/rgb), and the browser does NOT normalize oklch in getComputedStyle
 * (Chrome/Safari/Firefox preserve `oklch(...)` verbatim), so we convert
 * numerically to sRGB here. Falls back only when the variable is missing.
 */

// ── OKLCH → sRGB (CSS Color 4) ────────────────────────────────────────────
// oklch → OKLab → linear LMS → linear sRGB → gamma-encoded 8-bit channels.

function oklchToRgbBytes(l: number, c: number, hDeg: number): [number, number, number] {
  const hRad = (hDeg * Math.PI) / 180
  const a = c * Math.cos(hRad)
  const b = c * Math.sin(hRad)

  // OKLab → LMS (Björn Ottosson's reference matrices)
  const l_ = l + 0.3963377774 * a + 0.2158037573 * b
  const m_ = l - 0.1055613458 * a - 0.0638541728 * b
  const s_ = l - 0.0894841775 * a - 1.291485548 * b
  const L = l_ * l_ * l_
  const M = m_ * m_ * m_
  const S = s_ * s_ * s_

  // LMS → linear sRGB
  const r = 4.0767416621 * L - 3.3077115913 * M + 0.2309699292 * S
  const g = -1.2684380046 * L + 2.6097574011 * M - 0.3413193965 * S
  const bl = -0.0041960863 * L - 0.7034186147 * M + 1.707614701 * S

  // Linear sRGB → gamma-encoded 8-bit, clamped to the sRGB gamut.
  const enc = (v: number): number => {
    if (!Number.isFinite(v)) return 0
    v = v <= 0.0031308 ? 12.92 * v : 1.055 * Math.pow(v, 1 / 2.4) - 0.055
    return Math.round(Math.min(1, Math.max(0, v)) * 255)
  }
  return [enc(r), enc(g), enc(bl)]
}

const hex2 = (n: number) => n.toString(16).padStart(2, "0")

/** Parse a subset of CSS `<color>`: `oklch(L C H[/A])`, `#rgb/#rrggbb`, `rgb()/rgba()`.
 *  Returns `{ r, g, b, alpha }` (0-255 channels, alpha 0..1) or `null`. */
function parseColor(v: string): { r: number; g: number; b: number; alpha: number } | null {
  const s = v.trim().toLowerCase()
  let m =
    /^oklch\(\s*([\d.]+%?)\s+([\d.]+%?)\s+(-?[\d.]+(?:deg)?)(?:\s*\/\s*([\d.]+%?))?\s*\)$/.exec(s)
  if (m) {
    const l = m[1].endsWith("%") ? parseFloat(m[1]) / 100 : parseFloat(m[1])
    const c = m[2].endsWith("%") ? (parseFloat(m[2]) / 100) * 0.4 : parseFloat(m[2])
    const h = parseFloat(m[3])
    const alpha = m[4] ? (m[4].endsWith("%") ? parseFloat(m[4]) / 100 : parseFloat(m[4])) : 1
    if (![l, c, h].every(Number.isFinite)) return null
    const [r, g, b] = oklchToRgbBytes(l, c, h)
    return { r, g, b, alpha: Number.isFinite(alpha) ? alpha : 1 }
  }
  m = /^#([0-9a-f]{6})$/.exec(s)
  if (m) {
    const n = parseInt(m[1], 16)
    return { r: (n >> 16) & 255, g: (n >> 8) & 255, b: n & 255, alpha: 1 }
  }
  m = /^rgba?\(\s*([\d.]+)[\s,]+([\d.]+)[\s,]+([\d.]+)(?:[\s,/]+([\d.]+%?))?\s*\)$/.exec(s)
  if (m) {
    return {
      r: Math.round(parseFloat(m[1])),
      g: Math.round(parseFloat(m[2])),
      b: Math.round(parseFloat(m[3])),
      alpha: m[4] ? (m[4].endsWith("%") ? parseFloat(m[4]) / 100 : parseFloat(m[4])) : 1,
    }
  }
  return null
}

/** Read a CSS custom property and resolve it to a canvas-usable color string.
 *  oklch() tokens are converted to `#rrggbb` (or `rgba(r,g,b,a)` when the
 *  token itself carries alpha); anything already canvas-friendly is returned
 *  as-is. Falls back only when the variable is missing/unparseable. */
function resolveVar(name: string, fallback: string): string {
  if (typeof window === "undefined") return fallback
  const val = getComputedStyle(document.documentElement).getPropertyValue(name).trim()
  if (!val) return fallback
  const parsed = parseColor(val)
  if (!parsed) return val.startsWith("#") || val.startsWith("rgb") ? val : fallback
  const { r, g, b, alpha } = parsed
  return alpha < 1 ? `rgba(${r},${g},${b},${alpha})` : `#${hex2(r)}${hex2(g)}${hex2(b)}`
}

export const chartColors = {
  up: () => resolveVar("--color-up", "#FF5C5C"),
  down: () => resolveVar("--color-down", "#1D9E75"),
  brand: () => resolveVar("--color-brand", "#D97757"),
  warning: () => resolveVar("--color-warning", "#F5A623"),
  primary: () => resolveVar("--color-primary", "#FFFFFF"),
  tertiary: () => resolveVar("--color-tertiary", "#888780"),
  // MA line colors — chart-specific palette resolved per theme. Kept away from
  // up/down (red/green) and brand so the lines can't be misread as price color.
  ma5: () => resolveVar("--color-chart-ma5", "#6E6B63"),
  ma10: () => resolveVar("--color-chart-ma10", "#875814"),
  ma20: () => resolveVar("--color-chart-ma20", "#3E668F"),
  ma30: () => resolveVar("--color-chart-ma30", "#804F7C"),
}

/** Convert a color to rgba() with the given alpha. Accepts `#rrggbb`,
 *  `rgb()/rgba()` strings and (via parse) `oklch()`; passes through anything
 *  else unchanged so invalid input never silently becomes opaque. */
export function withAlpha(color: string, alpha: number): string {
  const parsed = parseColor(color)
  if (!parsed) return color
  return `rgba(${parsed.r},${parsed.g},${parsed.b},${alpha})`
}
