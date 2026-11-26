import { useMemo, useState } from "react"
import { ChevronRight, CircleAlert, Loader2 } from "lucide-react"
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
 * 一轮回复里的全部中间步骤（思考 + 工具调用）收成**一枚小胶囊**。
 *
 * 只在乎正文的阅读场景下，N 个卡片会把正文挤到视线之外。形态照 Claude：一枚贴着
 * 左边缘的圆角小标签（一行字 + 右箭头），点开才是完整时间线。它自己讲清楚两件事，
 * 否则用户没有理由点开：
 * - 正在进行时：正在做哪一步（转圈 + 具体动作）
 * - 结束后：一共几步；有失败就整枚标红并写出来（不能只靠"少了个对勾"暗示）
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

  const base = liveItem
    ? liveItem.type === "thinking"
      ? "思考中"
      : itemLabel(liveItem)
    : inProgress
      ? "处理中"
      : items.length === 1
        ? itemLabel(items[0])
        : `执行了 ${items.length} 步`

  // 失败要说出来而不是靠颜色：直接写进这一行文字里，胶囊不必再多一个角标
  const label = !inProgress && failures > 0 ? `${base} · ${failures} 个失败` : base

  const tone = inProgress
    ? liveItem?.type === "tool" && liveItem.toolCall.status === "running"
      ? "text-warning"
      : "text-brand"
    : failures > 0
      ? "text-danger"
      : "text-tertiary"

  return (
    <Collapsible.Root open={open} onOpenChange={(next) => setManualOpen(next)} className="my-2">
      <Collapsible.Trigger
        className={cn(
          "group border-hairline hover:bg-hover inline-flex max-w-full cursor-pointer items-center gap-1.5 rounded-lg border px-2.5 py-1 text-left transition-colors",
          failures > 0 && !inProgress ? "border-danger/30" : ""
        )}
      >
        {inProgress ? (
          <Loader2 className={cn("size-3 shrink-0 animate-spin", tone)} />
        ) : failures > 0 ? (
          <CircleAlert className={cn("size-3 shrink-0", tone)} />
        ) : null}
        <span
          className={cn(
            "text-caption min-w-0 truncate",
            inProgress ? "text-primary" : failures > 0 ? "text-danger" : "text-secondary"
          )}
        >
          {label}
        </span>
        <ChevronRight className="text-tertiary size-3 shrink-0 transition-transform duration-200 group-data-[state=open]:rotate-90" />
      </Collapsible.Trigger>
      <Collapsible.Content className="reasoning-content overflow-hidden">
        <div className="pt-2 pl-1">
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
