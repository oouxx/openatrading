import { Archive, ArchiveRestore, Check, ChevronDown, XCircle } from "lucide-react"
import { Button } from "@/components/ui/button"
import { formatRelativeTime } from "@/lib/format"
import type { AutomationRun } from "@/types/automation"

/** 运行结果标签：只有「失败」带图标 + 危险色，其余保持中性文字。 */
function RunStatus({ run }: { run: AutomationRun }) {
  if (run.status === "running") return <span className="text-brand text-caption">运行中</span>
  if (run.status === "failed") {
    return (
      <span className="text-danger text-caption flex items-center gap-1">
        <XCircle className="size-3" />
        失败
      </span>
    )
  }
  return (
    <span className="text-tertiary text-caption">
      {run.output && run.output.trim() ? "有发现" : "无产出"}
    </span>
  )
}

/**
 * 一条运行记录。
 *
 * 收起态只回答「哪个任务、结果如何、什么时候」，操作（标记已读 / 归档）收进
 * 展开区——低频动作常驻会把每行撑高一倍，而行高在列表里是最贵的资源。
 * 展开是纯查看，不会顺带把记录标成已读。
 */
export function AutomationRunRow({
  run,
  automationName,
  expanded,
  onToggleExpand,
  onMarkRead,
  onArchive,
  archivePending,
  readPending,
}: {
  run: AutomationRun
  automationName?: string
  expanded: boolean
  onToggleExpand: () => void
  onMarkRead: () => void
  onArchive: () => void
  archivePending?: boolean
  readPending?: boolean
}) {
  const detail = run.status === "failed" ? (run.error ?? "") : (run.output ?? "")

  return (
    <div className="bg-surface border-hairline rounded-lg border px-3.5 py-2.5">
      <button
        type="button"
        onClick={onToggleExpand}
        aria-expanded={expanded}
        className="flex w-full items-center gap-2 text-left"
      >
        <span
          aria-hidden
          className={`size-1.5 shrink-0 rounded-full ${
            run.unread ? "bg-brand" : "ring-hairline bg-transparent ring-1"
          }`}
        />
        <span className="sr-only">{run.unread ? "未读" : "已读"}</span>
        <span className="text-primary text-body min-w-0 truncate font-medium">
          {automationName ?? run.automation_id.slice(0, 8)}
        </span>
        <RunStatus run={run} />
        {run.archived && (
          <span className="border-hairline text-tertiary text-caption shrink-0 rounded border px-1.5 py-0.5 leading-none">
            已归档
          </span>
        )}
        <span className="text-tertiary text-caption ml-auto shrink-0">
          {formatRelativeTime(run.started_at)}
        </span>
        <ChevronDown
          className={`text-tertiary size-3.5 shrink-0 transition-transform ${
            expanded ? "rotate-180" : ""
          }`}
        />
      </button>

      {expanded && (
        <>
          {detail ? (
            <p
              className={`bg-canvas text-caption mt-2 rounded-md p-2.5 leading-5 whitespace-pre-wrap ${
                run.status === "failed" ? "text-danger" : "text-secondary"
              }`}
            >
              {detail}
            </p>
          ) : (
            <p className="text-tertiary text-caption mt-2">
              {run.status === "running" ? "运行中，暂无输出。" : "本次运行没有输出内容。"}
            </p>
          )}
          <div className="mt-2 flex items-center gap-1">
            {run.unread && (
              <Button
                type="button"
                variant="ghost"
                size="xs"
                onClick={onMarkRead}
                disabled={readPending}
              >
                <Check className="size-3" />
                标记已读
              </Button>
            )}
            <Button
              type="button"
              variant="ghost"
              size="xs"
              onClick={onArchive}
              disabled={archivePending}
            >
              {run.archived ? (
                <>
                  <ArchiveRestore className="size-3" />
                  取消归档
                </>
              ) : (
                <>
                  <Archive className="size-3" />
                  归档
                </>
              )}
            </Button>
          </div>
        </>
      )}
    </div>
  )
}
