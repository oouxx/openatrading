import { useMemo, useState } from "react"
import { ChevronRight } from "lucide-react"
import { Collapsible } from "radix-ui"
import ActivityTimeline, { type ActivityItem } from "./ActivityTimeline"
import { summarizeTool } from "@/lib/tool-summary"

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

/** 这组里最后一次工具动作：多步时用它当摘要（全是思考就退回"思考过程"）。 */
function lastActionLabel(items: ActivityItem[]): string {
  for (let i = items.length - 1; i >= 0; i--) {
    if (items[i].type === "tool") return itemLabel(items[i])
  }
  return "思考过程"
}

/**
 * 一轮回复里的全部中间步骤（思考 + 工具调用）收成**一枚小胶囊**。
 *
 * 只在乎正文的阅读场景下，N 个卡片会把正文挤到视线之外。形态照 Claude：一枚贴着
 * 左边缘的圆角小标签（一行字 + 右箭头），点开才是完整时间线。
 *
 * 收起态**只说做了什么**：进行中是手头这一步，结束后是这组里最后一次工具动作。
 * 步数、成功/失败、运行中这些状态一律不往外放——它们是过程细节，展开后每一步自己
 * 会讲（`ToolCallItem` 的 StatusTag：对勾 / 运行中 / 失败）。外面只剩一句摘要是
 * 最强的"正文优先"。
 */
export default function ActivityGroup({ items, isStreaming, sessionId }: ActivityGroupProps) {
  const [manualOpen, setManualOpen] = useState<boolean | null>(null)

  const liveItem = useMemo(() => {
    for (let i = items.length - 1; i >= 0; i--) {
      if (isLive(items[i])) return items[i]
    }
    return null
  }, [items])

  if (items.length === 0) return null

  const open = manualOpen ?? false

  // 收起态唯一的文字：手头这一步，或这一组最后一次动作
  const label = liveItem
    ? liveItem.type === "thinking"
      ? "思考中"
      : itemLabel(liveItem)
    : lastActionLabel(items)

  return (
    <Collapsible.Root open={open} onOpenChange={(next) => setManualOpen(next)} className="my-2">
      <Collapsible.Trigger className="group border-hairline hover:bg-hover inline-flex max-w-full cursor-pointer items-center gap-1.5 rounded-lg border px-2.5 py-1 text-left transition-colors">
        <span className="text-caption text-secondary min-w-0 truncate">{label}</span>
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
