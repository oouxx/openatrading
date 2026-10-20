import { useMemo, useState } from "react"
import { Archive, Bell, BellOff, Pencil, Play, Plus, RotateCw, Trash2, Zap } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Switch } from "@/components/ui/switch"
import AutomationFormDialog from "@/components/AutomationFormDialog"
import {
  useArchiveRun,
  useAutomationRuns,
  useAutomations,
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
  type RunQuery,
} from "@/types/automation"

function statusBadge(run: { status: string; output: string | null }): string {
  if (run.status === "running") return "运行中"
  if (run.status === "failed") return "失败"
  return run.output && run.output.trim() ? "有发现" : "无产出"
}

function formatTime(value: string): string {
  return value.replace("T", " ").slice(0, 19)
}

/** Stable fallback so useMemo dependencies don't change on every render. */
const EMPTY_AUTOMATIONS: Automation[] = []

export default function AutomationsView() {
  const { data, isLoading } = useAutomations()
  const automations = data?.automations ?? EMPTY_AUTOMATIONS
  const nextRuns: NextRuns = data?.next_runs ?? {}
  const [runsFilter, setRunsFilter] = useState<RunQuery>({ limit: 50 })
  const { data: runs = [] } = useAutomationRuns(runsFilter)
  const updateMutation = useUpdateAutomation()
  const deleteMutation = useDeleteAutomation()
  const triggerMutation = useTriggerAutomation()
  const markReadMutation = useMarkRunRead()
  const archiveMutation = useArchiveRun()

  /** 弹窗目标：null = 关闭；`automation` 为 null 表示新建 */
  const [dialogTarget, setDialogTarget] = useState<{ automation: Automation | null } | null>(null)
  const [expandedRun, setExpandedRun] = useState<number | null>(null)

  const automationById = useMemo(() => {
    const map = new Map<string, Automation>()
    for (const a of automations) map.set(a.id, a)
    return map
  }, [automations])

  const unreadCount = runs.filter((r) => r.unread).length

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-6 px-4 py-6">
      {/* ── 自动化任务列表 ── */}
      <section>
        <div className="mb-3 flex items-center justify-between">
          <h2 className="text-lg font-semibold">自动化任务</h2>
          <Button size="sm" onClick={() => setDialogTarget({ automation: null })}>
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
            还没有自动化任务。点击「新建」让 agent 按计划在后台巡检行情、生成简报。
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
                  onCheckedChange={(v) =>
                    updateMutation.mutate({ id: a.id, patch: { enabled: v } })
                  }
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
                      <span className="text-secondary ml-auto text-xs">下次 {nextRuns[a.id]}</span>
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
                  onClick={() => setDialogTarget({ automation: a })}
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
                  <Button
                    type="button"
                    variant="ghost"
                    className="h-auto w-full justify-start gap-2 px-0 text-left font-normal hover:bg-transparent"
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
                  </Button>
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

      {dialogTarget && (
        <AutomationFormDialog
          automation={dialogTarget.automation}
          onClose={() => setDialogTarget(null)}
        />
      )}
    </div>
  )
}
