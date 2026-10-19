/** 自动化任务(Codex Scheduled Tasks 复刻)——与 Rust 端 serde 输出一一对应 */

export type Schedule =
  | { kind: "cron"; expr: string }
  | { kind: "every_minutes"; minutes: number }
  | { kind: "daily"; hour: number; minute: number }
  /** days 为 ISO 周序号:1=周一 … 7=周日 */
  | { kind: "weekly"; days: number[]; hour: number; minute: number }

export type Scope =
  | { type: "new_thread" }
  | { type: "existing_thread"; session_id: string }

export interface Automation {
  id: string
  name: string
  prompt: string
  schedule: Schedule
  scope: Scope
  cwd: string
  enabled: boolean
  read_only: boolean
  timeout_secs: number | null
  created_at: string
  updated_at: string
}

/** automation_id → 下次运行时间(本地时区),未调度的任务不出现 */
export type NextRuns = Record<string, string>

export type RunStatus = "running" | "success" | "failed"

export interface AutomationRun {
  id: number
  automation_id: string
  started_at: string
  finished_at: string | null
  status: RunStatus
  output: string | null
  error: string | null
  unread: boolean
  archived: boolean
}

export interface NewAutomationBody {
  name: string
  prompt: string
  schedule: Schedule
  scope: Scope
  cwd: string
  read_only: boolean
  timeout_secs: number | null
}

export interface AutomationPatchBody {
  name?: string
  prompt?: string
  schedule?: Schedule
  scope?: Scope
  cwd?: string
  enabled?: boolean
  read_only?: boolean
  timeout_secs?: number | null
}

export interface RunQuery {
  automation_id?: string
  unread?: boolean
  active?: boolean
  limit?: number
}

/** ISO 周序号是否正好是周一~周五——任务创建页的「每工作日」预设就用这个写法 */
export function isWeekdays(days: number[]): boolean {
  const sorted = [...days].sort((a, b) => a - b)
  return sorted.length === 5 && sorted.every((d, i) => d === i + 1)
}

/** 「交易时段内每 N 分钟」用的 cron 表达式。
 *
 * 周字段 `2-6` 是 Quartz 语义下的周一~周五（1 = 周日，与后端 `Schedule::to_cron`
 * 的周序号映射一致），`9-15` 点覆盖全天交易时段（含飏中休市，监控任务无副作用）。
 */
export function tradingHoursCron(minutes: number): string {
  return `0 */${minutes} 9-15 * * 2-6`
}

const TRADING_HOURS_CRON_RE = /^0 \*\/(\d+) 9-15 \* \* 2-6$/

/** 识别 tradingHoursCron 生成的表达式，返回间隔分钟数；不是该形态返回 null */
export function tradingHoursMinutes(expr: string): number | null {
  const m = TRADING_HOURS_CRON_RE.exec(expr.trim())
  if (!m) return null
  const n = Number(m[1])
  return Number.isFinite(n) && n > 0 ? n : null
}

/** 调度计划 → 中文摘要 */
export function describeSchedule(s: Schedule): string {
  switch (s.kind) {
    case "cron": {
      const tradingMinutes = tradingHoursMinutes(s.expr)
      if (tradingMinutes !== null) return `交易时段每 ${tradingMinutes} 分钟`
      return `cron: ${s.expr}`
    }
    case "every_minutes":
      return `每 ${s.minutes} 分钟`
    case "daily":
      return `每天 ${String(s.hour).padStart(2, "0")}:${String(s.minute).padStart(2, "0")}`
    case "weekly": {
      const hhmm = `${String(s.hour).padStart(2, "0")}:${String(s.minute).padStart(2, "0")}`
      // 周一~周五是最常见的取值，缩写一句比「每周一、二、三、四、五」好读
      if (isWeekdays(s.days)) return `每工作日 ${hhmm}`
      const names = ["", "一", "二", "三", "四", "五", "六", "日"]
      const days = s.days.map((d) => names[d] ?? "?").join("、")
      return `每周${days} ${hhmm}`
    }
  }
}

/** 运行范围 → 中文摘要 */
export function describeScope(s: Scope): string {
  return s.type === "new_thread"
    ? "独立会话"
    : `复用会话 ${s.session_id.slice(0, 8)}…`
}