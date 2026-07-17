import ReactMarkdown from "react-markdown"
import remarkGfm from "remark-gfm"
import rehypeRaw from "rehype-raw"
import type { Components } from "react-markdown"

const mdComponents: Components = {
  code: ({ className, children, ...props }) => {
    const isInline = !className
    if (isInline) {
      return (
        <code className="bg-bg-hover text-ink rounded px-1 py-0.5 font-mono text-xs" {...props}>
          {children}
        </code>
      )
    }
    return (
      <pre className="rounded-card border-hairline bg-bg-hover text-ink-muted my-2 overflow-x-auto border p-3 font-mono text-xs leading-relaxed">
        <code className={className} {...props}>
          {children}
        </code>
      </pre>
    )
  },
  table: ({ children }) => (
    <div className="my-2 overflow-x-auto">
      <table className="w-full border-collapse font-mono text-xs">{children}</table>
    </div>
  ),
  th: ({ children }) => (
    <th className="border-hairline bg-bg-hover text-ink-muted border-b px-3 py-2 text-left font-medium">
      {children}
    </th>
  ),
  td: ({ children }) => (
    <td className="border-hairline text-ink-muted border-b px-3 py-2">{children}</td>
  ),
  a: ({ children, href }) => (
    <a href={href} className="text-ai hover:underline" target="_blank" rel="noreferrer">
      {children}
    </a>
  ),
  ul: ({ children }) => <ul className="my-1 list-disc space-y-1 pl-5">{children}</ul>,
  ol: ({ children }) => <ol className="my-1 list-decimal space-y-1 pl-5">{children}</ol>,
  hr: () => <hr className="border-hairline my-3" />,
}

interface TextBlockProps {
  content: string
  isStreaming?: boolean
}

export default function TextBlock({ content, isStreaming }: TextBlockProps) {
  if (!content && isStreaming) {
    return (
      <div className="flex gap-1 py-1">
        <span className="bg-ink-muted size-1.5 animate-pulse rounded-full" />
        <span
          className="bg-ink-muted size-1.5 animate-pulse rounded-full"
          style={{ animationDelay: "0.2s" }}
        />
        <span
          className="bg-ink-muted size-1.5 animate-pulse rounded-full"
          style={{ animationDelay: "0.4s" }}
        />
      </div>
    )
  }

  if (!content) return null

  return (
    <div className="prose-custom text-sm leading-relaxed">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[rehypeRaw]}
        components={mdComponents}
      >
        {content}
      </ReactMarkdown>
      {isStreaming && (
        <span className="text-ai ml-0.5 inline-block h-4 w-1.5 animate-pulse align-middle">
          |
        </span>
      )}
    </div>
  )
}
