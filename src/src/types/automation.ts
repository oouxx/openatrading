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

/** 调度计划 → 中文摘要 */
export function describeSchedule(s: Schedule): string {
  switch (s.kind) {
    case "cron":
      return `cron: ${s.expr}`
    case "every_minutes":
      return `每 ${s.minutes} 分钟`
    case "daily":
      return `每天 ${String(s.hour).padStart(2, "0")}:${String(s.minute).padStart(2, "0")}`
    case "weekly": {
      const names = ["", "一", "二", "三", "四", "五", "六", "日"]
      const days = s.days.map((d) => names[d] ?? "?").join("、")
      return `每周${days} ${String(s.hour).padStart(2, "0")}:${String(s.minute).padStart(2, "0")}`
    }
  }
}

/** 运行范围 → 中文摘要 */
export function describeScope(s: Scope): string {
  return s.type === "new_thread"
    ? "独立会话"
    : `复用会话 ${s.session_id.slice(0, 8)}…`
}