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
  if (!content) return null

  return (
    <ReasoningRoot defaultOpen={isStreaming}>
      <ReasoningTrigger active={isStreaming} />
      <ReasoningContent>
        <ReasoningText>
          {content}
          {isStreaming && (
            <span className="text-ai ml-0.5 inline-block h-3 w-1 animate-pulse align-middle">
              |
            </span>
          )}
        </ReasoningText>
      </ReasoningContent>
    </ReasoningRoot>
  )
}
