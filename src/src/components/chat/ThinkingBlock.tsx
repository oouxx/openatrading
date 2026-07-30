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
}

export default function ThinkingBlock({ content, isStreaming }: ThinkingBlockProps) {
  const [userOpen, setUserOpen] = useState(false)

  const handleOpenChange = useCallback(
    (open: boolean) => {
      setUserOpen(open)
    },
    []
  )

  // Derive open state: auto-expand when streaming, otherwise respect user toggle
  const open = isStreaming || userOpen

  if (!content) return null

  return (
    <ReasoningRoot open={open} onOpenChange={handleOpenChange}>
      <ReasoningTrigger active={isStreaming} />
      <ReasoningContent>
        <ReasoningText>
          {content}
        </ReasoningText>
      </ReasoningContent>
    </ReasoningRoot>
  )
}
