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
            <span className="text-ai relative ml-0.5 inline-block h-3 w-1.5 align-middle">
              <span
                className="absolute inset-y-0 left-0 w-0.5 animate-pulse rounded-full bg-current"
                style={{ animationDuration: "0.8s" }}
              />
            </span>
          )}
        </ReasoningText>
      </ReasoningContent>
    </ReasoningRoot>
  )
}
