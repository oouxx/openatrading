import { useMemo, useState } from "react"
import { Archive, Bell, BellOff, Pencil, Play, Plus, RotateCw, Trash2, Zap } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Switch } from "@/components/ui/switch"
import { Textarea } from "@/components/ui/textarea"
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import {
  useArchiveRun,
  useAutomationRuns,
  useAutomations,
  useCreateAutomation,
  useDeleteAutomation,
  useMarkRunRead,
  useTriggerAutomation,
  useUpdateAutomation,
} from "@/hooks/query/useAutomations"
import {
  describeSchedule,
  describeScope,
  type Automation,
  type NextRuns,
  type NewAutomationBody,
  type RunQuery,
  type Schedule,
  type Scope,
} from "@/types/automation"

interface FormState {
  id: string | null // null = 新建
  name: string
  prompt: string
  cwd: string
  scheduleKind: ScheduleKind
  cronExpr: string
  minutes: number
  hour: number
  minute: number
  weeklyDays: number[]
  scopeKind: "new_thread" | "existing_thread"
  sessionId: string
  read_only: boolean
  timeout_secs: string // 空串 = 默认
}

const EMPTY_FORM: FormState = {
  id: null,
  name: "",
  prompt: "",
  cwd: "",
  scheduleKind: "daily",
  cronExpr: "0 0 9 * * *",
  minutes: 30,
  hour: 9,
  minute: 0,
  weeklyDays: [1, 2, 3, 4, 5],
  scopeKind: "new_thread",
  sessionId: "",
  read_only: false,
  timeout_secs: "",
}
type ScheduleKind = "daily" | "weekly" | "every_minutes" | "cron"

function formFromSchedule(s: Schedule): Pick<FormState, "scheduleKind" | "cronExpr" | "minutes" | "hour" | "minute" | "weeklyDays"> {
  switch (s.kind) {
    case "cron":
      return { scheduleKind: "cron", cronExpr: s.expr, minutes: EMPTY_FORM.minutes, hour: EMPTY_FORM.hour, minute: EMPTY_FORM.minute, weeklyDays: EMPTY_FORM.weeklyDays }
    case "every_minutes":
      return { scheduleKind: "every_minutes", cronExpr: EMPTY_FORM.cronExpr, minutes: s.minutes, hour: EMPTY_FORM.hour, minute: EMPTY_FORM.minute, weeklyDays: EMPTY_FORM.weeklyDays }
    case "daily":
      return { scheduleKind: "daily", cronExpr: EMPTY_FORM.cronExpr, minutes: EMPTY_FORM.minutes, hour: s.hour, minute: s.minute, weeklyDays: EMPTY_FORM.weeklyDays }
    case "weekly":
      return { scheduleKind: "weekly", cronExpr: EMPTY_FORM.cronExpr, minutes: EMPTY_FORM.minutes, hour: s.hour, minute: s.minute, weeklyDays: s.days }
  }
}

function buildSchedule(f: FormState): Schedule {
  switch (f.scheduleKind) {
    case "cron":
      return { kind: "cron", expr: f.cronExpr.trim() }
    case "every_minutes":
      return { kind: "every_minutes", minutes: f.minutes }
    case "daily":
      return { kind: "daily", hour: f.hour, minute: f.minute }
    case "weekly":
      return { kind: "weekly", days: f.weeklyDays, hour: f.hour, minute: f.minute }
  }
}

function buildScope(f: FormState): Scope {
  return f.scopeKind === "new_thread"
    ? { type: "new_thread" }
    : { type: "existing_thread", session_id: f.sessionId.trim() }
}

function statusBadge(run: { status: string; output: string | null }): string {
  if (run.status === "running") return "运行中"
  if (run.status === "failed") return "失败"
  return run.output && run.output.trim() ? "有发现" : "无产出"
}

function formatTime(value: string): string {
  return value.replace("T", " ").slice(0, 19)
}

export default function AutomationsView() {
  const { data, isLoading } = useAutomations()
  const automations = data?.automations ?? []
  const nextRuns: NextRuns = data?.next_runs ?? {}
  const [runsFilter, setRunsFilter] = useState<RunQuery>({ limit: 50 })
  const { data: runs = [] } = useAutomationRuns(runsFilter)
  const createMutation = useCreateAutomation()
  const updateMutation = useUpdateAutomation()
  const deleteMutation = useDeleteAutomation()
  const triggerMutation = useTriggerAutomation()
  const markReadMutation = useMarkRunRead()
  const archiveMutation = useArchiveRun()

  const [form, setForm] = useState<FormState | null>(null)
  const [expandedRun, setExpandedRun] = useState<number | null>(null)
  const [formError, setFormError] = useState("")

  const automationById = useMemo(() => {
    const map = new Map<string, Automation>()
    for (const a of automations) map.set(a.id, a)
    return map
  }, [automations])

  const unreadCount = runs.filter((r) => r.unread).length

  const openCreate = () => {
    setFormError("")
    setForm({ ...EMPTY_FORM })
  }

  const openEdit = (a: Automation) => {
    setFormError("")
    setForm({
      id: a.id,
      name: a.name,
      prompt: a.prompt,
      cwd: a.cwd,
      scopeKind: a.scope.type,
      sessionId: a.scope.type === "existing_thread" ? a.scope.session_id : "",
      read_only: a.read_only,
      timeout_secs: a.timeout_secs === null ? "" : String(a.timeout_secs),
      ...formFromSchedule(a.schedule),
    })
  }

  const submitForm = async () => {
    if (!form) return
    setFormError("")
    const body: NewAutomationBody = {
      name: form.name.trim(),
      prompt: form.prompt,
      schedule: buildSchedule(form),
      scope: buildScope(form),
      cwd: form.cwd.trim() || ".",
      read_only: form.read_only,
      timeout_secs: form.timeout_secs === "" ? null : Number(form.timeout_secs),
    }
    try {
      if (form.id === null) {
        await createMutation.mutateAsync(body)
      } else {
        await updateMutation.mutateAsync({ id: form.id, patch: body })
      }
      setForm(null)
    } catch (err) {
      setFormError(String(err))
    }
  }

  const pending = createMutation.isPending || updateMutation.isPending

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-6 px-4 py-6">
      {/* ── 自动化任务列表 ── */}
      <section>
        <div className="mb-3 flex items-center justify-between">
          <h2 className="text-lg font-semibold">自动化任务</h2>
          <Button size="sm" onClick={openCreate}>
            <Plus className="size-4" />
            新建
          </Button>
        </div>

        {isLoading ? (
          <div className="text-secondary flex items-center gap-2 py-8 text-sm">
            <RotateCw className="size-4 animate-spin" />
            加载中…
          </div>
        ) : automations.length === 0 ? (
          <div className="border-hairline text-secondary rounded-lg border border-dashed p-8 text-center text-sm">
            还没有自动化任务。点击「新建」让 Codex 按计划在后台替你巡检项目、生成简报。
          </div>
        ) : (
          <div className="flex flex-col gap-2">
            {automations.map((a) => (
              <div
                key={a.id}
                className="border-hairline flex items-center gap-3 rounded-lg border px-3 py-2.5"
              >
                <Switch
                  checked={a.enabled}
                  onCheckedChange={(v) => updateMutation.mutate({ id: a.id, patch: { enabled: v } })}
                />
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2 text-sm font-medium">
                    {a.name}
                    {a.read_only && (
                      <span className="border-hairline text-secondary rounded border px-1.5 py-0.5 text-xs">
                        只读
                      </span>
                    )}
                    <span className="text-secondary text-xs font-normal">
                      {describeSchedule(a.schedule)} · {describeScope(a.scope)}
                    </span>
                    {nextRuns[a.id] && (
                      <span className="text-secondary ml-auto text-xs">
                        下次 {nextRuns[a.id]}
                      </span>
                    )}
                  </div>
                  <div className="text-secondary truncate text-xs">{a.prompt}</div>
                </div>
                <Button
                  size="icon"
                  variant="ghost"
                  title="立即运行"
                  onClick={() => triggerMutation.mutate(a.id)}
                  disabled={triggerMutation.isPending}
                >
                  <Play className="size-4" />
                </Button>
                <Button
                  size="icon"
                  variant="ghost"
                  title="编辑"
                  onClick={() => openEdit(a)}
                >
                  <Pencil className="size-4" />
                </Button>
                <Button
                  size="icon"
                  variant="ghost"
                  title="删除"
                  onClick={() => deleteMutation.mutate(a.id)}
                  disabled={deleteMutation.isPending}
                >
                  <Trash2 className="size-4" />
                </Button>
              </div>
            ))}
          </div>
        )}
      </section>

      {/* ── 收件箱(Triage)── */}
      <section>
        <div className="mb-3 flex items-center justify-between">
          <h2 className="flex items-center gap-2 text-lg font-semibold">
            运行收件箱
            {unreadCount > 0 && (
              <span className="bg-destructive text-destructive-foreground rounded-full px-2 py-0.5 text-xs text-white">
                {unreadCount} 未读
              </span>
            )}
          </h2>
          <div className="text-secondary flex items-center gap-1 text-xs">
            <Button
              size="sm"
              variant={runsFilter.unread ? "default" : "ghost"}
              onClick={() => setRunsFilter((f) => ({ ...f, unread: !f.unread }))}
            >
              <Bell className="size-3.5" />
              仅未读
            </Button>
            <Button
              size="sm"
              variant={runsFilter.active ? "default" : "ghost"}
              onClick={() => setRunsFilter((f) => ({ ...f, active: !f.active }))}
            >
              <BellOff className="size-3.5" />
              隐藏归档
            </Button>
          </div>
        </div>

        {runs.length === 0 ? (
          <div className="border-hairline text-secondary rounded-lg border border-dashed p-8 text-center text-sm">
            暂无运行记录。任务到点自动运行后,有发现的结果会出现在这里。
          </div>
        ) : (
          <div className="flex flex-col gap-2">
            {runs.map((r) => {
              const auto = automationById.get(r.automation_id)
              const expanded = expandedRun === r.id
              const detail = r.status === "failed" ? (r.error ?? "") : (r.output ?? "")
              return (
                <div key={r.id} className="border-hairline rounded-lg border px-3 py-2.5">
                  <button
                    type="button"
                    className="flex w-full items-center gap-2 text-left"
                    onClick={() => {
                      setExpandedRun(expanded ? null : r.id)
                      if (r.unread) markReadMutation.mutate(r.id)
                    }}
                  >
                    <span
                      className={`size-2 shrink-0 rounded-full ${
                        r.unread ? "bg-primary" : "bg-transparent ring-1 ring-gray-400/40"
                      }`}
                    />
                    <span
                      className={`text-sm ${
                        r.status === "failed" ? "text-destructive" : "font-medium"
                      }`}
                    >
                      {auto?.name ?? r.automation_id.slice(0, 8)}
                    </span>
                    <span className="text-secondary text-xs">{statusBadge(r)}</span>
                    <span className="text-secondary ml-auto text-xs">
                      {formatTime(r.started_at)}
                    </span>
                  </button>
                  {expanded && detail && (
                    <div className="bg-surface mt-2 rounded-md p-2 text-xs whitespace-pre-wrap">
                      {detail}
                    </div>
                  )}
                  <div className="mt-1.5 flex items-center justify-end gap-1">
                    {r.unread && (
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() => markReadMutation.mutate(r.id)}
                      >
                        <Zap className="size-3.5" />
                        标记已读
                      </Button>
                    )}
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={() => archiveMutation.mutate({ runId: r.id, archived: !r.archived })}
                    >
                      <Archive className="size-3.5" />
                      {r.archived ? "取消归档" : "归档"}
                    </Button>
                  </div>
                </div>
              )
            })}
          </div>
        )}
      </section>

      {/* ── 新建/编辑表单 ── */}
      <Dialog open={form !== null} onOpenChange={(open) => !open && setForm(null)}>
        <DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-lg">
          <DialogHeader>
            <DialogTitle>{form?.id === null ? "新建自动化" : "编辑自动化"}</DialogTitle>
          </DialogHeader>
          {form && (
            <div className="flex flex-col gap-3">
              <div className="flex flex-col gap-1.5">
                <label className="text-sm font-medium">名称</label>
                <Input
                  value={form.name}
                  onChange={(e) => setForm({ ...form, name: e.target.value })}
                  placeholder="每日项目简报"
                />
              </div>
              <div className="flex flex-col gap-1.5">
                <label className="text-sm font-medium">提示词(每次运行注入)</label>
                <Textarea
                  value={form.prompt}
                  onChange={(e) => setForm({ ...form, prompt: e.target.value })}
                  rows={4}
                  placeholder="查看最近 24 小时的提交,按主题分组写一份简短简报…"
                />
              </div>
              <div className="flex flex-col gap-1.5">
                <label className="text-sm font-medium">调度计划</label>
                <Select
                  value={form.scheduleKind}
                  onValueChange={(v) =>
                    setForm({ ...form, scheduleKind: v as FormState["scheduleKind"] })
                  }
                >
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="daily">每天固定时间</SelectItem>
                    <SelectItem value="weekly">每周固定时间</SelectItem>
                    <SelectItem value="every_minutes">每 N 分钟</SelectItem>
                    <SelectItem value="cron">Cron 表达式</SelectItem>
                  </SelectContent>
                </Select>
                {form.scheduleKind === "daily" && (
                  <div className="flex items-center gap-2">
                    <Input
                      type="number"
                      min={0}
                      max={23}
                      value={form.hour}
                      onChange={(e) => setForm({ ...form, hour: Number(e.target.value) })}
                      className="w-20"
                    />
                    <span className="text-secondary text-sm">:</span>
                    <Input
                      type="number"
                      min={0}
                      max={59}
                      value={form.minute}
                      onChange={(e) => setForm({ ...form, minute: Number(e.target.value) })}
                      className="w-20"
                    />
                  </div>
                )}
                {form.scheduleKind === "weekly" && (
                  <div className="flex flex-col gap-1.5">
                    <div className="flex gap-1">
                      {["一", "二", "三", "四", "五", "六", "日"].map((name, idx) => {
                        const isoDay = idx + 1
                        const active = form.weeklyDays.includes(isoDay)
                        return (
                          <Button
                            key={isoDay}
                            size="sm"
                            variant={active ? "default" : "ghost"}
                            onClick={() =>
                              setForm((f) => {
                                if (!f) return f
                                const days = active
                                  ? f.weeklyDays.filter((d) => d !== isoDay)
                                  : [...f.weeklyDays, isoDay]
                                return { ...f, weeklyDays: days }
                              })
                            }
                          >
                            {name}
                          </Button>
                        )
                      })}
                    </div>
                    <div className="flex items-center gap-2">
                      <Input
                        type="number"
                        min={0}
                        max={23}
                        value={form.hour}
                        onChange={(e) => setForm({ ...form, hour: Number(e.target.value) })}
                        className="w-20"
                      />
                      <span className="text-secondary text-sm">:</span>
                      <Input
                        type="number"
                        min={0}
                        max={59}
                        value={form.minute}
                        onChange={(e) => setForm({ ...form, minute: Number(e.target.value) })}
                        className="w-20"
                      />
                    </div>
                  </div>
                )}
                {form.scheduleKind === "every_minutes" && (
                  <div className="flex items-center gap-2">
                    <span className="text-secondary text-sm">每</span>
                    <Input
                      type="number"
                      min={1}
                      max={1440}
                      value={form.minutes}
                      onChange={(e) => setForm({ ...form, minutes: Number(e.target.value) })}
                      className="w-24"
                    />
                    <span className="text-secondary text-sm">分钟</span>
                  </div>
                )}
                {form.scheduleKind === "cron" && (
                  <Input
                    value={form.cronExpr}
                    onChange={(e) => setForm({ ...form, cronExpr: e.target.value })}
                    placeholder="0 30 9 * * *（秒 分 时 日 月 周,1=周日）"
                  />
                )}
              </div>
              <div className="flex flex-col gap-1.5">
                <label className="text-sm font-medium">运行范围</label>
                <Select
                  value={form.scopeKind}
                  onValueChange={(v) =>
                    setForm({ ...form, scopeKind: v as FormState["scopeKind"] })
                  }
                >
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="new_thread">独立会话(每次全新运行)</SelectItem>
                    <SelectItem value="existing_thread">复用会话(保留上下文)</SelectItem>
                  </SelectContent>
                </Select>
                {form.scopeKind === "existing_thread" && (
                  <Input
                    value={form.sessionId}
                    onChange={(e) => setForm({ ...form, sessionId: e.target.value })}
                    placeholder="要唤醒的会话 ID"
                  />
                )}
              </div>
              <div className="flex flex-col gap-1.5">
                <label className="text-sm font-medium">工作目录</label>
                <Input
                  value={form.cwd}
                  onChange={(e) => setForm({ ...form, cwd: e.target.value })}
                  placeholder="项目路径(独立会话时使用)"
                />
              </div>
              <div className="border-hairline flex items-center justify-between rounded-lg border px-3 py-2">
                <div>
                  <div className="text-sm font-medium">只读模式</div>
                  <div className="text-secondary text-xs">
                    禁用 bash/edit/write 和交易决策(推荐巡检类任务开启)
                  </div>
                </div>
                <Switch
                  checked={form.read_only}
                  onCheckedChange={(v) => setForm({ ...form, read_only: v })}
                />
              </div>
              <div className="flex items-center gap-2">
                <label className="text-secondary text-sm whitespace-nowrap">超时(秒)</label>
                <Input
                  type="number"
                  min={1}
                  max={86400}
                  value={form.timeout_secs}
                  onChange={(e) => setForm({ ...form, timeout_secs: e.target.value })}
                  placeholder="默认 600"
                  className="w-28"
                />
              </div>
              {formError && <div className="text-destructive text-xs">{formError}</div>}
            </div>
          )}
          <DialogFooter>
            <Button variant="ghost" onClick={() => setForm(null)}>
              取消
            </Button>
            <Button onClick={() => void submitForm()} disabled={pending}>
              {form?.id === null ? "创建" : "保存"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}