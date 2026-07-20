import { useState, useCallback } from "react"
import ReactMarkdown from "react-markdown"
import remarkGfm from "remark-gfm"
import rehypeRaw from "rehype-raw"
import type { Components } from "react-markdown"
import { Check, Copy } from "lucide-react"

// ── Code Block Component ──────────────────────────────

function CodeBlock({ className, children, ...props }: React.ComponentPropsWithoutRef<"code">) {
  const [copied, setCopied] = useState(false)
  const language = className ? className.replace("language-", "") : ""

  const handleCopy = useCallback(() => {
    const text = String(children).replace(/\n$/, "")
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    })
  }, [children])

  return (
    <div className="group rounded-card border-hairline bg-bg-hover my-2 overflow-hidden border">
      {/* Header bar */}
      <div className="border-hairline flex items-center justify-between border-b px-3 py-1.5">
        <span className="text-ink-muted/60 font-mono text-[10px] uppercase tracking-wider">
          {language || "code"}
        </span>
        <button
          onClick={handleCopy}
          className="text-ink-muted/50 hover:text-ink-muted flex items-center gap-1 rounded px-1.5 py-0.5 text-[10px] transition-colors opacity-0 group-hover:opacity-100"
        >
          {copied ? (
            <>
              <Check className="size-3 text-emerald-500" />
              <span className="text-emerald-500">已复制</span>
            </>
          ) : (
            <>
              <Copy className="size-3" />
              <span>复制</span>
            </>
          )}
        </button>
      </div>
      {/* Code content */}
      <pre className="overflow-x-auto p-3 font-mono text-xs leading-relaxed">
        <code className={className} {...props}>
          {children}
        </code>
      </pre>
    </div>
  )
}

// ── Inline Code Component ────────────────────────────

function InlineCode({ children, ...props }: React.ComponentPropsWithoutRef<"code">) {
  return (
    <code
      className="bg-bg-hover text-ink rounded px-1.5 py-0.5 font-mono text-xs"
      {...props}
    >
      {children}
    </code>
  )
}

// ── Markdown Components ──────────────────────────────

const mdComponents: Components = {
  code: ({ className, children, ...props }) => {
    const isInline = !className
    if (isInline) {
      return <InlineCode {...props}>{children}</InlineCode>
    }
    return <CodeBlock className={className} {...props}>{children}</CodeBlock>
  },
  pre: ({ children }) => <>{children}</>,
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
  h1: ({ children }) => <h1 className="text-ink mb-2 mt-4 text-base font-semibold">{children}</h1>,
  h2: ({ children }) => <h2 className="text-ink mb-1.5 mt-3.5 text-sm font-semibold">{children}</h2>,
  h3: ({ children }) => <h3 className="text-ink mb-1 mt-3 text-sm font-medium">{children}</h3>,
  p: ({ children }) => <p className="my-1.5 leading-relaxed">{children}</p>,
  blockquote: ({ children }) => (
    <blockquote className="border-ai/30 text-ink-muted my-2 border-l-2 pl-3 text-xs italic">
      {children}
    </blockquote>
  ),
}

// ── Streaming Cursor ──────────────────────────────────

function StreamingCursor() {
  return (
    <span className="text-ai relative ml-0.5 inline-block h-4 w-2 align-middle">
      <span
        className="absolute inset-y-0 left-0 w-0.5 animate-pulse rounded-full bg-current"
        style={{ animationDuration: "0.8s" }}
      />
    </span>
  )
}

// ── Loading Dots ──────────────────────────────────────

function LoadingDots() {
  return (
    <div className="flex gap-1 py-1">
      {[0, 1, 2].map((i) => (
        <span
          key={i}
          className="bg-ink-muted size-1.5 animate-bounce rounded-full"
          style={{ animationDelay: `${i * 0.15}s`, animationDuration: "0.8s" }}
        />
      ))}
    </div>
  )
}

// ── Main Component ───────────────────────────────────

interface TextBlockProps {
  content: string
  isStreaming?: boolean
}

export default function TextBlock({ content, isStreaming }: TextBlockProps) {
  if (!content && isStreaming) {
    return <LoadingDots />
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
      {isStreaming && <StreamingCursor />}
    </div>
  )
}
