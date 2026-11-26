import { useState, useCallback } from "react"
import {
  ReasoningRoot,
  ReasoningTrigger,
  ReasoningContent,
  ReasoningText,
} from "@/components/ui/reasoning"

interface ThinkingBlockProps {
  content: string
  isStreaming?: boolean
  /** 时间线节点已经带了星形图标，卡片里再画一次就是重复。 */
  showIcon?: boolean
}

export default function ThinkingBlock({
  content,
  isStreaming,
  showIcon = true,
}: ThinkingBlockProps) {
  // 默认行为是派生的（流式中自动展开、结束后自动收起），但一旦用户手动点过，
  // 就以用户为准 —— 否则“流式中点收起”会被 isStreaming 立刻顶回去。
  const [manualOpen, setManualOpen] = useState<boolean | null>(null)

  const handleOpenChange = useCallback((open: boolean) => {
    setManualOpen(open)
  }, [])

  const open = manualOpen ?? !!isStreaming

  if (!content) return null

  return (
    <ReasoningRoot open={open} onOpenChange={handleOpenChange}>
      <ReasoningTrigger active={isStreaming} showIcon={showIcon} />
      <ReasoningContent>
        <ReasoningText>{content}</ReasoningText>
      </ReasoningContent>
    </ReasoningRoot>
  )
}
