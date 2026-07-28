import { useState, useCallback, useEffect } from "react"
import {
  ReasoningRoot,
  ReasoningTrigger,
  ReasoningContent,
  ReasoningText,
} from "@/components/ui/reasoning"

interface ThinkingBlockProps {
  content: string
  isStreaming?: boolean
}

export default function ThinkingBlock({ content, isStreaming }: ThinkingBlockProps) {
  const [userOpen, setUserOpen] = useState(false)

  // Auto-expand when streaming starts (Claude-style), but user can still
  // toggle closed during streaming.
  useEffect(() => {
    if (isStreaming) {
      setUserOpen(true)
    }
  }, [isStreaming])

  const handleOpenChange = useCallback(
    (open: boolean) => {
      setUserOpen(open)
    },
    []
  )

  if (!content) return null

  return (
    <ReasoningRoot open={userOpen} onOpenChange={handleOpenChange}>
      <ReasoningTrigger active={isStreaming} />
      <ReasoningContent>
        <ReasoningText>
          {content}
        </ReasoningText>
      </ReasoningContent>
    </ReasoningRoot>
  )
}
