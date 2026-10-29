/**
 * Format a yuan / share amount with Chinese compact units.
 *
 * The same helper is used for turnover, market cap and volume so tables do not
 * drift into different precision or unit conventions.
 */
export function formatCompactNumber(value: number | null | undefined): string {
  if (value == null || !Number.isFinite(value) || value === 0) return "—"
  const abs = Math.abs(value)
  if (abs >= 1e12) return `${(value / 1e12).toFixed(2)}万亿`
  if (abs >= 1e8) return `${(value / 1e8).toFixed(2)}亿`
  if (abs >= 1e4) return `${(value / 1e4).toFixed(2)}万`
  return value.toFixed(0)
}

/** Format a decimal ratio (`0.0523`) as a signed percentage (`+5.23%`). */
export function formatRatioPct(value: number): string {
  if (!Number.isFinite(value)) return "—"
  return `${value >= 0 ? "+" : ""}${(value * 100).toFixed(2)}%`
}

/**
 * Format a value that is already a percentage (`5.23` → `+5.23%`).
 * Do not multiply again — backend `*_pct` fields already use percent units.
 */
export function formatPct(value: number): string {
  if (!Number.isFinite(value)) return "—"
  return `${value >= 0 ? "+" : ""}${value.toFixed(2)}%`
}

/** Shared rise/fall colour mapping. Zero and missing values stay neutral. */
export function pctClass(value: number | null | undefined): string {
  if (value == null || value === 0) return "text-primary"
  return value > 0 ? "text-up" : "text-down"
}

const pad2 = (n: number) => String(n).padStart(2, "0")

/**
 * 运行记录时间戳 → 一眼能定位的相对时间。
 *
 * 列表里的秒没有意义，而纯 `MM-DD HH:MM` 又需要心算「这是多久之前」。按区间
 * 给三种写法：刚刚 / N 分钟前 / 当天 HH:MM / 昨天 HH:MM / MM-DD HH:MM。
 *
 * 后端写的是 SQLite `datetime('now','localtime')` 的 `YYYY-MM-DD HH:MM:SS`（无
 * 时区后缀）。Safari/WKWebView 不接受空格分隔的日期，必须换成 ISO 的 `T` 再解析；
 * 不带时区后缀的 ISO 串按本地时间解释，因此这里不做时区换算。
 */
export function formatRelativeTime(iso: string, now: Date = new Date()): string {
  const at = new Date(iso.includes("T") ? iso : iso.replace(" ", "T"))
  if (Number.isNaN(at.getTime())) return iso.replace("T", " ").slice(0, 16)

  const hhmm = `${pad2(at.getHours())}:${pad2(at.getMinutes())}`
  const elapsed = now.getTime() - at.getTime()
  if (elapsed >= 0 && elapsed < 60_000) return "刚刚"
  if (elapsed >= 0 && elapsed < 3_600_000) return `${Math.floor(elapsed / 60_000)} 分钟前`

  const todayStart = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime()
  if (at.getTime() >= todayStart) return hhmm
  if (at.getTime() >= todayStart - 86_400_000) return `昨天 ${hhmm}`
  return `${pad2(at.getMonth() + 1)}-${pad2(at.getDate())} ${hhmm}`
}
