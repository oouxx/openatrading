import { useMemo, useState } from "react"
import { AlertTriangle, Archive, Bell, Plus, X } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import AutomationFormDialog from "@/components/AutomationFormDialog"
import { AutomationRow } from "@/components/AutomationRow"
import { AutomationRunRow } from "@/components/AutomationRunRow"
import {
  useArchiveRun,
  useAutomationRuns,
  useAutomations,
  useDeleteAutomation,
  useMarkRunRead,
  useTriggerAutomation,
  useUpdateAutomation,
} from "@/hooks/query/useAutomations"
import type { Automation, NextRuns, RunQuery } from "@/types/automation"

/** Stable fallback so useMemo dependencies don't change on every render. */
const EMPTY_AUTOMATIONS: Automation[] = []

/** 后端错误 → 用户能理解的一句话（web 端只带状态码，Tauri 端带原始文本） */
function actionErrorMessage(err: unknown): string {
  const raw = err instanceof Error ? err.message : String(err)
  if (raw === "404" || raw.includes("not found")) return "任务或运行记录已不存在，列表已刷新"
  if (/^5\d{2}$/.test(raw)) return "服务端处理失败，请稍后重试"
  if (/^\d{3}$/.test(raw)) return `请求被拒绝（HTTP ${raw}）`
  return raw
}

export default function AutomationsView() {
  const automationsQuery = useAutomations()
  const automations = automationsQuery.data?.automations ?? EMPTY_AUTOMATIONS
  const nextRuns: NextRuns = automationsQuery.data?.next_runs ?? {}
  const [runsFilter, setRunsFilter] = useState<RunQuery>({ limit: 50 })
  const runsQuery = useAutomationRuns(runsFilter)
  const runs = runsQuery.data ?? []

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
  const filtered = Boolean(runsFilter.unread || runsFilter.active)

  // 所有写操作共用一个错误位：动作失败必须可见，否则用户看到的就是「点了没反应」。
  const failed = [
    updateMutation.error,
    deleteMutation.error,
    triggerMutation.error,
    markReadMutation.error,
    archiveMutation.error,
  ].find(Boolean)
  const dismissError = () => {
    updateMutation.reset()
    deleteMutation.reset()
    triggerMutation.reset()
    markReadMutation.reset()
    archiveMutation.reset()
  }

  return (
    <div className="mx-auto flex w-full max-w-3xl flex-col gap-6 px-4 py-6">
      <header className="flex items-center justify-between gap-3">
        <h1 className="text-h1 text-primary font-semibold">自动化</h1>
        <Button size="sm" onClick={() => setDialogTarget({ automation: null })}>
          <Plus className="size-4" />
          新建
        </Button>
      </header>

      {failed && (
        <div
          role="alert"
          className="border-danger/25 bg-danger-bg flex items-start gap-2 rounded-lg border px-3 py-2"
        >
          <AlertTriangle className="text-danger mt-0.5 size-3.5 shrink-0" />
          <p className="text-danger text-caption flex-1 leading-5">{actionErrorMessage(failed)}</p>
          <Button
            type="button"
            variant="ghost"
            size="icon-xs"
            onClick={dismissError}
            aria-label="关闭错误提示"
            className="text-danger/70 hover:text-danger -my-0.5"
          >
            <X />
          </Button>
        </div>
      )}

      {/* ── 定时任务 ── */}
      <section className="flex flex-col gap-3">
        <h2 className="text-h2 text-primary flex items-baseline gap-2 font-medium">
          定时任务
          {/* 加载中/失败时不显示数字：0 会被读成「一个任务都没有」 */}
          {!automationsQuery.isPending && !automationsQuery.isError && automations.length > 0 && (
            <span className="text-tertiary text-caption font-normal">{automations.length}</span>
          )}
        </h2>

        {automationsQuery.isPending ? (
          <div className="flex flex-col gap-2">
            {Array.from({ length: 2 }).map((_, i) => (
              <Skeleton key={i} className="h-[84px] w-full rounded-lg" />
            ))}
          </div>
        ) : automationsQuery.isError ? (
          <div className="bg-surface border-hairline flex flex-col items-center gap-2 rounded-lg border px-6 py-10 text-center">
            <p className="text-secondary text-caption">无法加载自动化任务</p>
            <Button size="xs" variant="outline" onClick={() => automationsQuery.refetch()}>
              重试
            </Button>
          </div>
        ) : automations.length === 0 ? (
          <div className="border-hairline text-secondary text-body rounded-lg border border-dashed p-8 text-center">
            还没有自动化任务。点击「新建」让 agent 按计划在后台巡检行情、生成简报。
          </div>
        ) : (
          <div className="flex flex-col gap-2">
            {automations.map((a) => (
              <AutomationRow
                key={a.id}
                automation={a}
                nextRun={nextRuns[a.id]}
                togglePending={updateMutation.isPending && updateMutation.variables?.id === a.id}
                triggerPending={triggerMutation.isPending && triggerMutation.variables === a.id}
                deletePending={deleteMutation.isPending && deleteMutation.variables === a.id}
                onToggle={(enabled) => updateMutation.mutate({ id: a.id, patch: { enabled } })}
                onTrigger={() => triggerMutation.mutate(a.id)}
                onEdit={() => setDialogTarget({ automation: a })}
                onDelete={() => deleteMutation.mutate(a.id)}
              />
            ))}
          </div>
        )}
      </section>

      {/* ── 运行记录（未读 / 归档 triage）── */}
      <section className="flex flex-col gap-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <h2 className="text-h2 text-primary flex items-baseline gap-2 font-medium">
            运行记录
            {unreadCount > 0 && (
              <span className="bg-brand/15 text-brand text-caption rounded-full px-2 py-0.5 leading-none">
                {unreadCount} 未读
              </span>
            )}
          </h2>
          <div className="flex items-center gap-1">
            <Button
              size="sm"
              variant={runsFilter.unread ? "secondary" : "ghost"}
              aria-pressed={Boolean(runsFilter.unread)}
              onClick={() => setRunsFilter((f) => ({ ...f, unread: !f.unread }))}
              className={runsFilter.unread ? "text-primary" : "text-secondary"}
            >
              <Bell className="size-3.5" />
              未读
            </Button>
            <Button
              size="sm"
              variant={runsFilter.active ? "secondary" : "ghost"}
              aria-pressed={Boolean(runsFilter.active)}
              onClick={() => setRunsFilter((f) => ({ ...f, active: !f.active }))}
              className={runsFilter.active ? "text-primary" : "text-secondary"}
            >
              <Archive className="size-3.5" />
              未归档
            </Button>
          </div>
        </div>

        {runsQuery.isPending ? (
          <div className="flex flex-col gap-2">
            {Array.from({ length: 3 }).map((_, i) => (
              <Skeleton key={i} className="h-11 w-full rounded-lg" />
            ))}
          </div>
        ) : runsQuery.isError ? (
          <div className="bg-surface border-hairline flex flex-col items-center gap-2 rounded-lg border px-6 py-10 text-center">
            <p className="text-secondary text-caption">无法加载运行记录</p>
            <Button size="xs" variant="outline" onClick={() => runsQuery.refetch()}>
              重试
            </Button>
          </div>
        ) : runs.length === 0 ? (
          <div className="border-hairline text-secondary text-body rounded-lg border border-dashed p-8 text-center">
            {filtered ? (
              <>
                没有符合筛选条件的记录。
                <Button
                  size="xs"
                  variant="ghost"
                  className="text-brand ml-1"
                  onClick={() => setRunsFilter({ limit: 50 })}
                >
                  清除筛选
                </Button>
              </>
            ) : (
              "暂无运行记录。任务到点自动运行后，有发现的结果会出现在这里。"
            )}
          </div>
        ) : (
          <div className="flex flex-col gap-2">
            {runs.map((r) => (
              <AutomationRunRow
                key={r.id}
                run={r}
                automationName={automationById.get(r.automation_id)?.name}
                expanded={expandedRun === r.id}
                onToggleExpand={() => setExpandedRun(expandedRun === r.id ? null : r.id)}
                onMarkRead={() => markReadMutation.mutate(r.id)}
                readPending={markReadMutation.isPending && markReadMutation.variables === r.id}
                onArchive={() => archiveMutation.mutate({ runId: r.id, archived: !r.archived })}
                archivePending={
                  archiveMutation.isPending && archiveMutation.variables?.runId === r.id
                }
              />
            ))}
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
