import { useState } from "react"
import { Loader2, Pencil, Play, Trash2 } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { Switch } from "@/components/ui/switch"
import { describeSchedule, describeScope, type Automation } from "@/types/automation"

/**
 * 一条定时任务。
 *
 * 三行各有分工：名称 + 下次运行、频率 + （非默认时）运行范围、指令原文。
 * 指令默认单行截断——它是「这个任务到底做什么」的唯一真实描述，但完整文本属
 * 编辑弹窗；行内保留 `title` 让悬停能看到全貌。
 */
export function AutomationRow({
  automation: a,
  nextRun,
  onToggle,
  onTrigger,
  onEdit,
  onDelete,
  togglePending,
  triggerPending,
  deletePending,
}: {
  automation: Automation
  nextRun?: string
  onToggle: (enabled: boolean) => void
  onTrigger: () => void
  onEdit: () => void
  onDelete: () => void
  togglePending?: boolean
  triggerPending?: boolean
  deletePending?: boolean
}) {
  const [confirmOpen, setConfirmOpen] = useState(false)

  return (
    <div className="bg-surface border-hairline flex items-start gap-3 rounded-lg border p-3.5">
      <Switch
        checked={a.enabled}
        disabled={togglePending}
        onCheckedChange={onToggle}
        aria-label={`${a.enabled ? "停用" : "启用"} ${a.name}`}
        title={a.enabled ? "停用" : "启用"}
        className="mt-0.5 shrink-0"
      />

      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="text-primary text-body min-w-0 truncate font-medium">{a.name}</span>
          {a.read_only && (
            <span className="border-hairline text-tertiary text-caption shrink-0 rounded border px-1.5 py-0.5 leading-none">
              只读
            </span>
          )}
          {nextRun && (
            <span className="text-tertiary text-caption ml-auto shrink-0">下次 {nextRun}</span>
          )}
        </div>
        <p className="text-secondary text-caption mt-0.5 truncate">
          {describeSchedule(a.schedule)}
          {/* 「独立会话」是绝大多数任务的默认值，逐行重复只会稀释真正的差异 */}
          {a.scope.type === "existing_thread" && ` · ${describeScope(a.scope)}`}
        </p>
        <p className="text-tertiary text-caption mt-0.5 truncate" title={a.prompt}>
          {a.prompt}
        </p>
      </div>

      <div className="flex shrink-0 items-center gap-0.5">
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          className="text-secondary hover:text-primary"
          onClick={onTrigger}
          disabled={triggerPending}
          aria-label={`立即运行 ${a.name}`}
          title="立即运行"
        >
          {triggerPending ? (
            <Loader2 className="size-3.5 animate-spin" />
          ) : (
            <Play className="size-3.5" />
          )}
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          className="text-secondary hover:text-primary"
          onClick={onEdit}
          aria-label={`编辑 ${a.name}`}
          title="编辑"
        >
          <Pencil className="size-3.5" />
        </Button>

        <Popover open={confirmOpen} onOpenChange={setConfirmOpen}>
          <PopoverTrigger asChild>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              className="text-secondary hover:text-down"
              disabled={deletePending}
              aria-label={`删除 ${a.name}`}
              title="删除"
            >
              <Trash2 className="size-3.5" />
            </Button>
          </PopoverTrigger>
          <PopoverContent align="end" className="w-64 p-3">
            <p className="text-primary text-body font-medium">删除「{a.name}」？</p>
            <p className="text-secondary text-caption mt-1">
              任务不再调度，已有的运行记录会保留；此操作不可撤销。
            </p>
            <div className="mt-3 flex justify-end gap-2">
              <Button type="button" variant="ghost" size="xs" onClick={() => setConfirmOpen(false)}>
                取消
              </Button>
              <Button
                type="button"
                variant="destructive"
                size="xs"
                disabled={deletePending}
                onClick={() => {
                  setConfirmOpen(false)
                  onDelete()
                }}
              >
                {deletePending ? "删除中…" : "删除"}
              </Button>
            </div>
          </PopoverContent>
        </Popover>
      </div>
    </div>
  )
}
