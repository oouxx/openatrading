import {
  useState,
  useCallback,
  useEffect,
  useId,
  useMemo,
  useRef,
  type ComponentProps,
} from "react"
import ReactMarkdown from "react-markdown"
import remarkGfm from "remark-gfm"
import rehypeRaw from "rehype-raw"
import rehypeSanitize from "rehype-sanitize"
import type { Components } from "react-markdown"
import { Check, Copy } from "lucide-react"
import { Button } from "@/components/ui/button"
import { useUiStore } from "@/stores/ui"

type HighlightPlugin = (typeof import("rehype-highlight"))["default"]
type RehypePlugins = NonNullable<ComponentProps<typeof ReactMarkdown>["rehypePlugins"]>

let highlightPluginPromise: Promise<HighlightPlugin> | null = null

/** True when the message has a fenced block with a non-Mermaid language. */
function hasHighlightableCode(content: string): boolean {
  return /(?:```|~~~)(?!mermaid\b)[\w.+#-]+/.test(content)
}

/** Load the syntax-highlighting plugin only when a message actually has code. */
function loadHighlightPlugin(): Promise<HighlightPlugin> {
  if (!highlightPluginPromise) {
    highlightPluginPromise = import("rehype-highlight")
      .then(({ default: plugin }) => plugin)
      .catch((error) => {
        highlightPluginPromise = null
        throw error
      })
  }
  return highlightPluginPromise
}

// ── Code Block Component ──────────────────────────────

function CodeBlock({ className, children, ...props }: React.ComponentPropsWithoutRef<"code">) {
  const [copied, setCopied] = useState(false)
  // rehype-highlight adds `language-*` and `hljs` classes; pick only the
  // language class so the header label never shows the stray `hljs` token.
  const language =
    className
      ?.split(/\s+/)
      .find((c) => c.startsWith("language-"))
      ?.slice("language-".length) ?? ""

  const handleCopy = useCallback(() => {
    const text = String(children).replace(/\n$/, "")
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    })
  }, [children])

  return (
    <div className="bg-hover border-hairline group my-2 overflow-hidden rounded-lg border">
      {/* Header bar */}
      <div className="border-hairline flex items-center justify-between border-b px-4 py-2">
        <span className="text-tertiary font-number text-caption">{language || "code"}</span>
        <Button
          variant="ghost"
          size="xs"
          onClick={handleCopy}
          // 触摸设备没有 hover：按钮必须常驻，否则永远不可见。
          className="hover:text-secondary text-tertiary can-hover:opacity-0 can-hover:group-hover:opacity-100 can-hover:focus-visible:opacity-100 px-2 transition-opacity"
        >
          {copied ? (
            <>
              <Check className="text-success size-3" />
              <span className="text-success">已复制</span>
            </>
          ) : (
            <>
              <Copy className="size-3" />
              <span>复制</span>
            </>
          )}
        </Button>
      </div>
      {/* Code content */}
      <pre className="font-number text-caption overflow-x-auto p-3">
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
    <code className="bg-hover text-primary font-number text-caption rounded px-2 py-0.5" {...props}>
      {children}
    </code>
  )
}

// ── Mermaid Diagram Component ─────────────────────────
// Renders ```mermaid blocks as diagrams. During streaming the raw source is
// shown (the diagram would be incomplete); once the block is complete the
// diagram is rendered. Falls back to the code block on parse errors.

function MermaidBlock({ code, isStreaming }: { code: string; isStreaming?: boolean }) {
  const theme = useUiStore((s) => s.theme)
  const containerRef = useRef<HTMLDivElement | null>(null)
  const [failedCode, setFailedCode] = useState<string | null>(null)
  const id = useId().replace(/[^a-zA-Z0-9]/g, "")
  const failed = failedCode === code

  useEffect(() => {
    if (isStreaming) return
    let cancelled = false
    ;(async () => {
      try {
        const { default: mermaid } = await import("mermaid")
        mermaid.initialize({
          startOnLoad: false,
          securityLevel: "strict",
          theme: theme === "dark" ? "dark" : "default",
        })
        const { svg } = await mermaid.render(`mmd-${id}`, code)
        if (!cancelled && containerRef.current) {
          containerRef.current.innerHTML = svg
        }
      } catch {
        if (!cancelled) setFailedCode(code)
      }
    })()
    return () => {
      cancelled = true
    }
  }, [code, isStreaming, theme, id])

  if (isStreaming || failed) {
    return <CodeBlock className="language-mermaid">{code}</CodeBlock>
  }
  return <div ref={containerRef} className="my-2 overflow-x-auto" />
}

// ── Markdown Components ──────────────────────────────

const getMdComponents = (isStreaming: boolean): Components => ({
  code: ({ className, children, ...props }) => {
    const isInline = !className
    if (isInline) {
      return <InlineCode {...props}>{children}</InlineCode>
    }
    if (className?.includes("language-mermaid")) {
      return <MermaidBlock code={String(children).replace(/\n$/, "")} isStreaming={isStreaming} />
    }
    return (
      <CodeBlock className={className} {...props}>
        {children}
      </CodeBlock>
    )
  },
  pre: ({ children }) => <>{children}</>,
  table: ({ children }) => (
    <div className="my-2 overflow-x-auto">
      <table className="font-number text-caption w-full border-collapse">{children}</table>
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
  h1: ({ children }) => <h1 className="text-h1 text-primary mt-5 mb-3">{children}</h1>,
  h2: ({ children }) => <h2 className="text-h2 text-primary mt-4 mb-2">{children}</h2>,
  h3: ({ children }) => <h3 className="text-h2 text-primary mt-3 mb-1.5">{children}</h3>,
  p: ({ children }) => <p className="text-body my-2 leading-[1.7]">{children}</p>,
  blockquote: ({ children }) => (
    <blockquote className="border-brand/30 text-secondary text-body my-2 border-l-2 pl-3 italic">
      {children}
    </blockquote>
  ),
})

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
  const [highlightPlugin, setHighlightPlugin] = useState<HighlightPlugin | null>(null)

  // Highlighting pulls in highlight.js/lowlight (~200 kB). Defer that chunk
  // until the message contains a fenced code block; plain chat replies never
  // pay for it.
  useEffect(() => {
    if (highlightPlugin || !hasHighlightableCode(content)) return
    let cancelled = false
    loadHighlightPlugin()
      .then((plugin) => {
        if (!cancelled) setHighlightPlugin(() => plugin)
      })
      .catch((error) => console.warn("[markdown] 代码高亮插件加载失败", error))
    return () => {
      cancelled = true
    }
  }, [content, highlightPlugin])

  const rehypePlugins = useMemo<RehypePlugins>(
    () => [rehypeRaw, rehypeSanitize, ...(highlightPlugin ? [highlightPlugin] : [])],
    [highlightPlugin]
  )

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
        rehypePlugins={rehypePlugins}
        components={getMdComponents(isStreaming)}
      >
        {content}
      </ReactMarkdown>
      {isStreaming && <StreamingIndicator />}
    </div>
  )
}
