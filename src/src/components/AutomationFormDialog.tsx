import { useMemo, useState } from "react"
import {
  ChevronDown,
  ClipboardCheck,
  Newspaper,
  PenLine,
  Radar,
  Repeat,
  type LucideIcon,
} from "lucide-react"
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
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible"
import { useCreateAutomation, useUpdateAutomation } from "@/hooks/query/useAutomations"
import { useThreads } from "@/hooks/useChatThreads"
import {
  describeSchedule,
  describeScope,
  isWeekdays,
  tradingHoursCron,
  tradingHoursMinutes,
  type Automation,
  type NewAutomationBody,
  type Schedule,
} from "@/types/automation"

/**
 * 新建/编辑自动化任务。
 *
 * 新建路径的目标是「不填也能创建」：默认选中一个常用模板，名称/指令/频率
 * 都已填好，点「创建」就能跑；想改成自己的需求，只动指令和频率两个字段。
 *
 * 低频决策（运行范围/工作目录/只读/超时）收进「高级设置」，折叠时用一行摘要
 * 交代当前取值——用户不展开也知道任务会怎么跑，不必逐项读一遍。
 */

/** 运行频率 —— 覆盖日常场景，cron 只留给确实要写表达式的人 */
type Frequency = "weekdays" | "trading_hours" | "daily" | "weekly" | "every_minutes" | "cron"

const FREQUENCY_OPTIONS: { value: Frequency; label: string }[] = [
  { value: "weekdays", label: "每个工作日（周一至周五）" },
  { value: "trading_hours", label: "交易时段内每 N 分钟" },
  { value: "daily", label: "每天" },
  { value: "weekly", label: "每周（指定星期几）" },
  { value: "every_minutes", label: "全天每 N 分钟" },
  { value: "cron", label: "自定义 cron 表达式" },
]

/** 每 N 分钟的常用间隔，省得用户手打数字 */
const MINUTE_PRESETS = [15, 30, 60, 120]
/** 间隔输入框被清空时的预览占位值（提交前 validateForm 会挡掉非法值） */
const DEFAULT_EVERY_MINUTES = 30
const DEFAULT_TIME = "09:00"
/** 后端默认单次运行超时（秒），见 core::automation::DEFAULT_RUN_TIMEOUT_SECS */
const DEFAULT_TIMEOUT_SECS = 600
const MAX_TIMEOUT_SECS = 86_400

const WEEKDAY_LABELS = ["一", "二", "三", "四", "五", "六", "日"]

interface TaskTemplate {
  id: string
  title: string
  desc: string
  icon: LucideIcon
  name: string
  prompt: string
  schedule: Schedule
  read_only: boolean
}

/**
 * 常用模板。A 股日常巡检是模拟盘用户最需要的三类任务，
 * 模板直接给出可用的指令（自包含、写清「什么算有发现」），监控类默认只读。
 */
const TASK_TEMPLATES: TaskTemplate[] = [
  {
    id: "premarket",
    title: "每日盘前简报",
    desc: "汇总隔夜消息与今日关注点",
    icon: Newspaper,
    name: "每日盘前简报",
    prompt:
      "汇总隔夜到今晨的财经快讯与市场情绪，输出盘前简报：① 3 条最重要的消息；② 今天值得关注的板块；③ 一句话风险提示。没有重要消息时回复「无重要消息」即可。",
    schedule: { kind: "weekly", days: [1, 2, 3, 4, 5], hour: 8, minute: 30 },
    read_only: true,
  },
  {
    id: "intraday",
    title: "盘中异动监控",
    desc: "交易时段定时巡检异动",
    icon: Radar,
    name: "盘中异动监控",
    prompt:
      "检查最近这段时间的涨停/炸板、板块异动与财经快讯；只在出现明显异动时输出标的、原因与关键数据，没有异常则不输出任何内容。",
    // 只在交易时段跑（Quartz 周字段 2-6 = 周一~周五），否则盘后一夜要白跑几十次
    schedule: { kind: "cron", expr: tradingHoursCron(30) },
    read_only: true,
  },
  {
    id: "close",
    title: "收盘复盘",
    desc: "收盘后复盘当天行情",
    icon: ClipboardCheck,
    name: "收盘复盘",
    prompt:
      "复盘今天的行情：涨停家数、炸板率、情绪温度、最强板块与龙头；按「数据—原因—明日关注」输出，控制在 300 字以内。",
    schedule: { kind: "weekly", days: [1, 2, 3, 4, 5], hour: 15, minute: 30 },
    read_only: true,
  },
  {
    id: "custom",
    title: "自定义任务",
    desc: "自己写指令，从空白开始",
    icon: PenLine,
    name: "",
    prompt: "",
    schedule: { kind: "daily", hour: 9, minute: 0 },
    // 自定义任务可能真要写文件/跑命令，不默认替用户关掉写权限
    read_only: false,
  },
]

interface FormState {
  name: string
  prompt: string
  frequency: Frequency
  /** "HH:MM" —— 工作日/每天/每周共用 */
  time: string
  /** ISO 周序号：1=周一 … 7=周日 */
  weeklyDays: number[]
  /** 每 N 分钟（字符串，输入框被清空时不留 NaN） */
  minutes: string
  cronExpr: string
  scopeKind: "new_thread" | "existing_thread"
  sessionId: string
  cwd: string
  read_only: boolean
  /** 空串 = 用后端默认超时 */
  timeoutSecs: string
}

const pad2 = (n: number) => String(n).padStart(2, "0")

/** "HH:MM" → [hour, minute]；解析不出来时退回 09:00（部分环境原生时间框会给空值） */
function parseTime(value: string): [number, number] {
  const m = /^(\d{1,2}):(\d{1,2})$/.exec(value.trim())
  if (!m) return [9, 0]
  const hour = Number(m[1])
  const minute = Number(m[2])
  if (hour > 23 || minute > 59) return [9, 0]
  return [hour, minute]
}

/** 间隔分钟数：输入框可能暂时为空，先退回默认值（提交前 validateForm 会挡掉非法输入） */
function parseMinutes(value: string): number {
  const n = Number.parseInt(value, 10)
  return Number.isFinite(n) && n > 0 ? n : DEFAULT_EVERY_MINUTES
}

/** schedule → 频率相关字段（编辑已有任务时用） */
function frequencyFields(
  s: Schedule
): Pick<FormState, "frequency" | "time" | "weeklyDays" | "minutes" | "cronExpr"> {
  const rest = { time: DEFAULT_TIME, weeklyDays: [1, 2, 3, 4, 5], minutes: String(DEFAULT_EVERY_MINUTES), cronExpr: "" }
  switch (s.kind) {
    case "cron": {
      const tradingMinutes = tradingHoursMinutes(s.expr)
      if (tradingMinutes !== null) {
        return { frequency: "trading_hours", ...rest, minutes: String(tradingMinutes) }
      }
      return { frequency: "cron", ...rest, cronExpr: s.expr }
    }
    case "every_minutes":
      return { frequency: "every_minutes", ...rest, minutes: String(s.minutes) }
    case "daily":
      return { frequency: "daily", ...rest, time: `${pad2(s.hour)}:${pad2(s.minute)}` }
    case "weekly":
      return {
        frequency: isWeekdays(s.days) ? "weekdays" : "weekly",
        ...rest,
        time: `${pad2(s.hour)}:${pad2(s.minute)}`,
        weeklyDays: s.days,
      }
  }
}

function formFromTemplate(t: TaskTemplate): FormState {
  return {
    name: t.name,
    prompt: t.prompt,
    read_only: t.read_only,
    scopeKind: "new_thread",
    sessionId: "",
    cwd: "",
    timeoutSecs: "",
    ...frequencyFields(t.schedule),
  }
}

function formFromAutomation(a: Automation): FormState {
  return {
    name: a.name,
    prompt: a.prompt,
    read_only: a.read_only,
    scopeKind: a.scope.type,
    sessionId: a.scope.type === "existing_thread" ? a.scope.session_id : "",
    cwd: a.cwd,
    timeoutSecs: a.timeout_secs === null ? "" : String(a.timeout_secs),
    ...frequencyFields(a.schedule),
  }
}

function buildSchedule(f: FormState): Schedule {
  const [hour, minute] = parseTime(f.time)
  switch (f.frequency) {
    case "weekdays":
      return { kind: "weekly", days: [1, 2, 3, 4, 5], hour, minute }
    case "daily":
      return { kind: "daily", hour, minute }
    case "weekly":
      return { kind: "weekly", days: [...f.weeklyDays].sort((a, b) => a - b), hour, minute }
    case "trading_hours":
      return { kind: "cron", expr: tradingHoursCron(parseMinutes(f.minutes)) }
    case "every_minutes":
      return { kind: "every_minutes", minutes: parseMinutes(f.minutes) }
    case "cron":
      return { kind: "cron", expr: f.cronExpr.trim() }
  }
}

/** 本地校验 —— 错误信息直接说要改哪里（后端校验信息是英文的，不适合直接抛给用户） */
function validateForm(f: FormState): string | null {
  if (!f.name.trim()) return "请填写任务名称"
  if (!f.prompt.trim()) return "请填写运行指令"
  if (f.frequency === "weekly" && f.weeklyDays.length === 0) return "请至少选择一个星期几"
  if (f.frequency === "every_minutes" || f.frequency === "trading_hours") {
    const n = Number.parseInt(f.minutes, 10)
    if (!Number.isFinite(n) || n < 1 || n > 1440) return "间隔需在 1–1440 分钟之间"
  }
  if (f.frequency === "cron") {
    const fields = f.cronExpr.trim().split(/\s+/).filter(Boolean).length
    if (fields < 5 || fields > 7) return "cron 需要 5–7 个字段（秒 分 时 日 月 周）"
  }
  if (f.timeoutSecs !== "") {
    const t = Number.parseInt(f.timeoutSecs, 10)
    if (!Number.isFinite(t) || t < 1 || t > MAX_TIMEOUT_SECS) {
      return `超时需在 1–${MAX_TIMEOUT_SECS} 秒之间`
    }
  }
  if (f.scopeKind === "existing_thread" && !f.sessionId.trim()) return "复用会话需要填写会话 ID"
  return null
}

/** 高级设置是否偏离默认值 —— 编辑老任务时据此决定要不要默认展开 */
function hasCustomAdvanced(f: FormState): boolean {
  return f.scopeKind !== "new_thread" || f.cwd.trim() !== "" || f.read_only || f.timeoutSecs !== ""
}

export default function AutomationFormDialog({
  automation,
  onClose,
}: {
  /** null = 新建 */
  automation: Automation | null
  onClose: () => void
}) {
  const createMutation = useCreateAutomation()
  const updateMutation = useUpdateAutomation()
  const [form, setForm] = useState<FormState>(() =>
    automation ? formFromAutomation(automation) : formFromTemplate(TASK_TEMPLATES[0])
  )
  const [templateId, setTemplateId] = useState<string>(TASK_TEMPLATES[0].id)
  const [showAdvanced, setShowAdvanced] = useState(() => hasCustomAdvanced(form))
  const [error, setError] = useState<string | null>(null)
  // 只有选了「复用会话」才需要会话列表，默认创建路径不额外请求
  const { sessions } = useThreads({ enabled: form.scopeKind === "existing_thread" })

  const update = (patch: Partial<FormState>) => setForm((f) => ({ ...f, ...patch }))

  const applyTemplate = (t: TaskTemplate) => {
    setTemplateId(t.id)
    setError(null)
    // 只覆盖模板负责的字段，高级设置里用户已经改过的东西保持不动
    setForm((f) => ({
      ...f,
      name: t.name,
      prompt: t.prompt,
      read_only: t.read_only,
      ...frequencyFields(t.schedule),
    }))
  }

  const toggleDay = (day: number) =>
    setForm((f) => ({
      ...f,
      weeklyDays: f.weeklyDays.includes(day)
        ? f.weeklyDays.filter((d) => d !== day)
        : [...f.weeklyDays, day].sort((a, b) => a - b),
    }))

  const pending = createMutation.isPending || updateMutation.isPending
  const schedulePreview = describeSchedule(buildSchedule(form))
  /** 复用会话的候选：打开弹窗就拉一次会话列表（与侧边栏同一接口），不再要求用户手抄会话 ID */
  const sessionOptions = useMemo(() => {
    const options = sessions.map((s) => ({
      id: s.id,
      label: s.name?.trim() || s.first_message?.trim().slice(0, 24) || s.id.slice(0, 8),
    }))
    // 老任务的目标会话可能已不在列表里（被删或换机器），保留它，免得一打开就被静默清空
    if (form.sessionId && !options.some((o) => o.id === form.sessionId)) {
      options.unshift({ id: form.sessionId, label: `当前会话 ${form.sessionId.slice(0, 8)}` })
    }
    return options
  }, [sessions, form.sessionId])
  const scope = buildScopeOf(form)
  const advancedSummary = [
    describeScope(scope),
    form.read_only ? "只读" : "可写",
    form.cwd.trim() ? `目录 ${form.cwd.trim()}` : null,
    form.timeoutSecs !== "" ? `超时 ${form.timeoutSecs} 秒` : null,
  ]
    .filter(Boolean)
    .join(" · ")

  const submit = async () => {
    const invalid = validateForm(form)
    if (invalid) {
      setError(invalid)
      return
    }
    setError(null)
    const body: NewAutomationBody = {
      name: form.name.trim(),
      prompt: form.prompt.trim(),
      schedule: buildSchedule(form),
      scope,
      cwd: form.cwd.trim() || ".",
      read_only: form.read_only,
      timeout_secs: form.timeoutSecs === "" ? null : Number.parseInt(form.timeoutSecs, 10),
    }
    try {
      if (automation) {
        await updateMutation.mutateAsync({ id: automation.id, patch: body })
      } else {
        await createMutation.mutateAsync(body)
      }
      onClose()
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    }
  }

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{automation ? "编辑自动化" : "新建自动化"}</DialogTitle>
        </DialogHeader>

        <form
          className="flex flex-col gap-4"
          onSubmit={(e) => {
            e.preventDefault()
            void submit()
          }}
        >
          {!automation && (
            <div className="flex flex-col gap-2">
              <div className="text-secondary text-xs">
                选一个模板就能直接创建，也可以把指令改成自己的需求
              </div>
              <div className="grid grid-cols-2 gap-2">
                {TASK_TEMPLATES.map((t) => {
                  const active = templateId === t.id
                  const Icon = t.icon
                  return (
                    <button
                      key={t.id}
                      type="button"
                      onClick={() => applyTemplate(t)}
                      className={`flex items-start gap-2 rounded-lg border px-3 py-2 text-left transition-colors ${
                        active
                          ? "border-brand bg-brand/5"
                          : "border-hairline hover:bg-hover cursor-pointer"
                      }`}
                    >
                      <Icon
                        className={`mt-0.5 size-4 shrink-0 ${active ? "text-brand" : "text-secondary"}`}
                      />
                      <span className="min-w-0 flex-1">
                        <span className="text-primary block text-sm font-medium">{t.title}</span>
                        <span className="text-secondary block truncate text-xs">{t.desc}</span>
                        <span className="text-tertiary block truncate text-xs">
                          {describeSchedule(t.schedule)}
                          {t.read_only && " · 只读"}
                        </span>
                      </span>
                    </button>
                  )
                })}
              </div>
            </div>
          )}

          <Field label="任务名称">
            <Input
              value={form.name}
              onChange={(e) => update({ name: e.target.value })}
              placeholder="例如：每日盘前简报"
            />
          </Field>

          <Field
            label="运行指令"
            hint="写清做什么、什么算有发现、没有发现时怎么回复"
          >
            <Textarea
              value={form.prompt}
              onChange={(e) => update({ prompt: e.target.value })}
              rows={5}
              placeholder="例如：检查最近 30 分钟的涨停与板块异动，有异常才输出详情。"
            />
          </Field>

          <Field label="运行频率">
            <Select
              value={form.frequency}
              onValueChange={(v) => update({ frequency: v as Frequency })}
            >
              <SelectTrigger className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {FREQUENCY_OPTIONS.map((o) => (
                  <SelectItem key={o.value} value={o.value}>
                    {o.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>

            {(form.frequency === "weekdays" || form.frequency === "daily") && (
              <div className="mt-2 flex items-center gap-2">
                <Input
                  type="time"
                  value={form.time}
                  onChange={(e) => update({ time: e.target.value })}
                  className="w-32"
                />
                <span className="text-secondary text-xs">运行</span>
              </div>
            )}

            {form.frequency === "weekly" && (
              <div className="mt-2 flex flex-col gap-2">
                <div className="flex gap-1">
                  {WEEKDAY_LABELS.map((label, idx) => {
                    const day = idx + 1
                    const active = form.weeklyDays.includes(day)
                    return (
                      <button
                        key={day}
                        type="button"
                        onClick={() => toggleDay(day)}
                        className={`h-7 w-7 cursor-pointer rounded-md border text-xs transition-colors ${
                          active
                            ? "border-brand bg-brand text-brand-foreground"
                            : "border-hairline text-secondary hover:bg-hover"
                        }`}
                      >
                        {label}
                      </button>
                    )
                  })}
                </div>
                <div className="flex items-center gap-2">
                  <Input
                    type="time"
                    value={form.time}
                    onChange={(e) => update({ time: e.target.value })}
                    className="w-32"
                  />
                  <span className="text-secondary text-xs">运行</span>
                </div>
              </div>
            )}

            {(form.frequency === "every_minutes" || form.frequency === "trading_hours") && (
              <div className="mt-2 flex flex-wrap items-center gap-2">
                <span className="text-secondary text-xs">
                  {form.frequency === "trading_hours" ? "交易时段每" : "每"}
                </span>
                <Input
                  type="number"
                  min={1}
                  max={1440}
                  value={form.minutes}
                  onChange={(e) => update({ minutes: e.target.value })}
                  className="w-20"
                />
                <span className="text-secondary text-xs">分钟</span>
                {MINUTE_PRESETS.map((m) => (
                  <button
                    key={m}
                    type="button"
                    onClick={() => update({ minutes: String(m) })}
                    className={`cursor-pointer rounded-md border px-2 py-0.5 text-xs transition-colors ${
                      form.minutes === String(m)
                        ? "border-brand text-brand"
                        : "border-hairline text-secondary hover:bg-hover"
                    }`}
                  >
                    {m} 分
                  </button>
                ))}
              </div>
            )}

            {form.frequency === "cron" && (
              <div className="mt-2 flex flex-col gap-1">
                <Input
                  value={form.cronExpr}
                  onChange={(e) => update({ cronExpr: e.target.value })}
                  placeholder="0 30 8 * * 1-5（秒 分 时 日 月 周）"
                />
                <span className="text-tertiary text-xs">
                  5–7 个字段；周字段 1 = 周日（Quartz 语义）
                </span>
              </div>
            )}

            <div className="text-secondary mt-2 flex items-center gap-1.5 text-xs">
              <Repeat className="size-3.5 shrink-0" />
              {schedulePreview}
            </div>
          </Field>

          <Collapsible open={showAdvanced} onOpenChange={setShowAdvanced}>
            <CollapsibleTrigger asChild>
              <button
                type="button"
                className="border-hairline hover:bg-hover flex w-full cursor-pointer items-center gap-2 rounded-lg border px-3 py-2 text-left transition-colors"
              >
                <span className="text-sm font-medium">高级设置</span>
                <span className="text-secondary min-w-0 flex-1 truncate text-xs">
                  {advancedSummary}
                </span>
                <ChevronDown
                  className={`text-secondary size-4 shrink-0 transition-transform ${
                    showAdvanced ? "rotate-180" : ""
                  }`}
                />
              </button>
            </CollapsibleTrigger>
            <CollapsibleContent className="mt-3 flex flex-col gap-3 px-1">
              <Field label="运行范围">
                <Select
                  value={form.scopeKind}
                  onValueChange={(v) =>
                    update({ scopeKind: v as FormState["scopeKind"] })
                  }
                >
                  <SelectTrigger className="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="new_thread">独立会话（每次全新运行）</SelectItem>
                    <SelectItem value="existing_thread">复用会话（保留上下文）</SelectItem>
                  </SelectContent>
                </Select>
                {form.scopeKind === "existing_thread" && (
                  <div className="mt-1.5 flex flex-col gap-1">
                    <Select
                      value={form.sessionId}
                      onValueChange={(v) => update({ sessionId: v })}
                    >
                      <SelectTrigger className="w-full">
                        <SelectValue
                          placeholder={
                            sessionOptions.length === 0
                              ? "暂无会话，先到对话页开一个"
                              : "选择要唤醒的会话"
                          }
                        />
                      </SelectTrigger>
                      <SelectContent>
                        {sessionOptions.map((o) => (
                          <SelectItem key={o.id} value={o.id}>
                            {o.label}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                    <span className="text-tertiary text-xs">
                      定时把这条指令插进该会话，保留之前的上下文
                    </span>
                  </div>
                )}
              </Field>

              <Field label="工作目录">
                <Input
                  value={form.cwd}
                  onChange={(e) => update({ cwd: e.target.value })}
                  placeholder="留空 = 当前工作区"
                />
              </Field>

              <div className="border-hairline flex items-center justify-between rounded-lg border px-3 py-2">
                <div>
                  <div className="text-sm font-medium">只读模式</div>
                  <div className="text-secondary text-xs">
                    禁用 bash/edit/write（巡检类任务推荐开启）
                  </div>
                </div>
                <Switch
                  checked={form.read_only}
                  onCheckedChange={(v) => update({ read_only: v })}
                />
              </div>

              <Field label="单次运行超时（秒）">
                <Input
                  type="number"
                  min={1}
                  max={MAX_TIMEOUT_SECS}
                  value={form.timeoutSecs}
                  onChange={(e) => update({ timeoutSecs: e.target.value })}
                  placeholder={`默认 ${DEFAULT_TIMEOUT_SECS}`}
                  className="w-28"
                />
              </Field>
            </CollapsibleContent>
          </Collapsible>

          {error && <div className="text-destructive text-xs">{error}</div>}

          <DialogFooter>
            <Button type="button" variant="ghost" onClick={onClose}>
              取消
            </Button>
            <Button type="submit" disabled={pending}>
              {pending ? "保存中…" : automation ? "保存" : "创建"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

/** 表单字段壳：统一 label / 提示 / 控件的间距 */
function Field({
  label,
  hint,
  children,
}: {
  label: string
  hint?: string
  children: React.ReactNode
}) {
  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-baseline justify-between gap-2">
        <label className="text-sm font-medium">{label}</label>
        {hint && <span className="text-tertiary text-xs">{hint}</span>}
      </div>
      {children}
    </div>
  )
}

function buildScopeOf(f: FormState): Automation["scope"] {
  return f.scopeKind === "new_thread"
    ? { type: "new_thread" }
    : { type: "existing_thread", session_id: f.sessionId.trim() }
}
