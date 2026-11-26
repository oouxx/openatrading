import { useMemo, useState } from "react"
import { Check, ChevronDown, CircleAlert, Loader2 } from "lucide-react"
import { Collapsible } from "radix-ui"
import ActivityTimeline, { type ActivityItem } from "./ActivityTimeline"
import { summarizeTool } from "@/lib/tool-summary"
import { cn } from "@/lib/utils"

interface ActivityGroupProps {
  items: ActivityItem[]
  isStreaming?: boolean
  sessionId?: string | null
}

/** 该步是否还在进行中（思考流式 / 工具执行）。 */
function isLive(item: ActivityItem): boolean {
  if (item.type === "thinking") return !!item.streaming
  const s = item.toolCall.status
  return s === "streaming" || s === "running"
}

/** 单步的动作说明，收起态那一行文字就用它。 */
function itemLabel(item: ActivityItem): string {
  if (item.type === "thinking") return "思考过程"
  return summarizeTool(item.toolCall.name, item.toolCall.args) || item.toolCall.name || "工具调用"
}

/**
 * 一轮回复里的全部中间步骤（思考 + 工具调用）合并成**一行**。
 *
 * 只在乎正文的阅读场景下，N 个卡片会把正文挤到视线之外；Claude 的做法是
 * 把整段过程收起来，只有正文是默认展开的。收起态这一行必须自己讲清楚两件
 * 事，否则用户没有理由点开：
 * - 正在进行时：正在做哪一步（图标在转 + 具体动作）
 * - 结束后：一共几步；有失败就显式标出来（失败不能只靠颜色之外的信息隐藏）
 */
export default function ActivityGroup({ items, isStreaming, sessionId }: ActivityGroupProps) {
  const [manualOpen, setManualOpen] = useState<boolean | null>(null)

  const { liveItem, failures } = useMemo(() => {
    let live: ActivityItem | null = null
    let failed = 0
    for (let i = items.length - 1; i >= 0; i--) {
      const it = items[i]
      if (!live && isLive(it)) live = it
      if (it.type === "tool" && it.toolCall.status === "error") failed++
    }
    return { liveItem: live, failures: failed }
  }, [items])

  if (items.length === 0) return null

  // 一轮还没结束就不给对勾——那等于宣告“做完了”。
  const inProgress = !!isStreaming
  const open = manualOpen ?? false

  const label = liveItem
    ? liveItem.type === "thinking"
      ? "思考中"
      : itemLabel(liveItem)
    : inProgress
      ? "处理中"
      : items.length === 1
        ? itemLabel(items[0])
        : `执行了 ${items.length} 步`

  const tone = inProgress
    ? liveItem?.type === "tool" && liveItem.toolCall.status === "running"
      ? "text-warning"
      : "text-brand"
    : failures > 0
      ? "text-danger"
      : "text-tertiary"

  return (
    <Collapsible.Root open={open} onOpenChange={(next) => setManualOpen(next)} className="my-1.5">
      <Collapsible.Trigger className="group hover:bg-hover/50 flex w-full cursor-pointer items-center gap-2 rounded-md px-1 py-1 text-left transition-colors">
        {inProgress ? (
          <Loader2 className={cn("size-3.5 shrink-0 animate-spin", tone)} />
        ) : failures > 0 ? (
          <CircleAlert className={cn("size-3.5 shrink-0", tone)} />
        ) : (
          <Check className={cn("size-3.5 shrink-0", tone)} />
        )}
        <span
          className={cn(
            "text-body min-w-0 flex-1 truncate",
            inProgress
              ? "text-primary font-medium"
              : failures > 0
                ? "text-danger"
                : "text-secondary"
          )}
          title={label}
        >
          {label}
        </span>
        {!inProgress && failures > 0 && (
          <span className="text-danger text-caption shrink-0">· {failures} 个失败</span>
        )}
        <ChevronDown className="text-tertiary size-3.5 shrink-0 transition-transform duration-200 group-data-[state=open]:rotate-180" />
      </Collapsible.Trigger>
      <Collapsible.Content className="reasoning-content overflow-hidden">
        <div className="pt-1 pl-1">
          <ActivityTimeline
            items={items}
            isStreaming={isStreaming}
            sessionId={sessionId}
            className="my-0"
          />
        </div>
      </Collapsible.Content>
    </Collapsible.Root>
  )
}
