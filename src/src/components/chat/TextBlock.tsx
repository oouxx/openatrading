import { useState, useCallback } from "react"
import ReactMarkdown from "react-markdown"
import remarkGfm from "remark-gfm"
import rehypeRaw from "rehype-raw"
import rehypeHighlight from "rehype-highlight"
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
    <div className="bg-hover border-hairline group rounded-lg my-2 overflow-hidden border">
      {/* Header bar */}
      <div className="border-hairline flex items-center justify-between border-b px-3 py-2">
        <span className="text-secondary/60 font-mono text-caption tracking-wider uppercase">
          {language || "code"}
        </span>
        <button
          onClick={handleCopy}
          className="hover:text-secondary text-secondary/50 flex items-center gap-1 rounded px-2 py-0.5 text-caption opacity-0 transition-colors group-hover:opacity-100"
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
      <pre className="overflow-x-auto p-3 font-mono text-caption">
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
      className="bg-hover text-primary rounded px-2 py-0.5 font-mono text-caption"
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
      <table className="w-full border-collapse font-mono text-caption">{children}</table>
    </div>
  ),
  th: ({ children }) => (
    <th className="bg-hover border-hairline text-secondary border-b px-3 py-2 text-left font-medium">
      {children}
    </th>
  ),
  td: ({ children }) => (
    <td className="border-hairline text-secondary border-b px-3 py-2">{children}</td>
  ),
  a: ({ children, href }) => (
    <a href={href} className="text-brand hover:underline" target="_blank" rel="noreferrer">
      {children}
    </a>
  ),
  ul: ({ children }) => <ul className="my-1 list-disc space-y-1 pl-5">{children}</ul>,
  ol: ({ children }) => <ol className="my-1 list-decimal space-y-1 pl-5">{children}</ol>,
  hr: () => <hr className="border-hairline my-3" />,
  h1: ({ children }) => <h1 className="text-h1 mt-5 mb-3 text-primary">{children}</h1>,
  h2: ({ children }) => <h2 className="text-h2 mt-4 mb-2 text-primary">{children}</h2>,
  h3: ({ children }) => <h3 className="text-h2 mt-3 mb-1.5 text-primary">{children}</h3>,
  p: ({ children }) => <p className="text-body my-2 leading-relaxed">{children}</p>,
  blockquote: ({ children }) => (
    <blockquote className="text-body border-brand/30 text-secondary my-2 border-l-2 pl-3 italic">
      {children}
    </blockquote>
  ),
}

// ── Streaming Indicator ────────────────────────────────
// A subtle pulsing cursor shown at the end of streaming text (Claude-style).

function StreamingIndicator() {
  return (
    <span className="relative ml-0.5 inline-block h-[1em] w-[2px] align-text-bottom">
      <span
        className="absolute inset-y-0 left-0 w-full animate-pulse rounded-sm bg-current opacity-70"
        style={{ animationDuration: "1s" }}
      />
    </span>
  )
}

// ── Pulsating Dots ────────────────────────────────────
// Claude-style loading indicator: three dots that pulse in/out with opacity.

function PulsatingDots() {
  return (
    <div className="flex items-center gap-1 py-1">
      {[0, 1, 2].map((i) => (
        <span
          key={i}
          className="bg-ink-muted inline-block size-1.5 rounded-full"
          style={{
            animation: "pulsating-dot 1.4s ease-in-out infinite",
            animationDelay: `${i * 0.2}s`,
          }}
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
    return <PulsatingDots />
  }

  if (!content) return null

  // Always render with ReactMarkdown, even during streaming (Claude-style
  // progressive markdown rendering). The markdown parser handles partial
  // content gracefully — incomplete constructs simply don't close until the
  // final delta arrives.
  return (
    <div className="prose-custom text-body">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[rehypeRaw, rehypeHighlight]}
        components={mdComponents}
      >
        {content}
      </ReactMarkdown>
      {isStreaming && <StreamingIndicator />}
    </div>
  )
}
