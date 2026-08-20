import { Sparkles } from "lucide-react"
import ThinkingBlock from "./ThinkingBlock"
import { ToolCallItem, ToolIcon, type ToolCall } from "./ToolCallList"

// ── Types ────────────────────────────────────────────────

export type ActivityItem =
  | { type: "thinking"; id: string; content: string; streaming?: boolean }
  | { type: "tool"; id: string; toolCall: ToolCall }

interface ActivityTimelineProps {
  items: ActivityItem[]
  isStreaming?: boolean
  sessionId?: string | null
}

/**
 * Activity timeline (Claude.ai 样式)：把一轮回复中的思考（thinking）与
 * 工具调用（tool call）按发生顺序串在一条垂直时间线上，每个节点是一个
 * 圆点图标（星形=思考 / 工具图标=调用），右侧为折叠卡内容。
 * 文字回复不在此列——由 TurnRenderer 在时间线之后渲染。
 */
export default function ActivityTimeline({
  items,
  isStreaming,
  sessionId,
}: ActivityTimelineProps) {
  if (items.length === 0) return null

  return (
    <div className="relative my-2">
      {/* 垂直时间轴 */}
      <div className="absolute top-3 bottom-3 left-[11px] w-px bg-hairline" />
      <div className="flex flex-col">
        {items.map((item, i) => (
          <div key={item.id ?? i} className="relative flex gap-2.5 pb-2.5 last:pb-0">
            {/* 时间线节点 */}
            <div className="bg-surface border-hairline relative z-10 mt-0.5 flex size-[22px] shrink-0 items-center justify-center rounded-full border">
              {item.type === "thinking" ? (
                <Sparkles className="size-3 text-warning" />
              ) : (
                <ToolIcon name={item.toolCall.name} className="size-3" />
              )}
            </div>
            <div className="min-w-0 flex-1">
              {item.type === "thinking" ? (
                <ThinkingBlock content={item.content} isStreaming={item.streaming} />
              ) : (
                <ToolCallItem
                  toolCall={item.toolCall}
                  isStreaming={isStreaming}
                  sessionId={sessionId}
                  showIcon={false}
                />
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
