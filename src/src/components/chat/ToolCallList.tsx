import { useMemo, useState } from "react"
import {
  Check,
  ChevronDown,
  ChevronRight,
  CircleAlert,
  FileEdit,
  Loader2,
  Terminal,
  Wrench,
} from "lucide-react"
import { Button } from "@/components/ui/button"
import { fetchJson } from "@/stores/data"
import { summarizeTool } from "@/lib/tool-summary"

// ── Types ────────────────────────────────────────────────

export interface ToolCall {
  id: string
  name: string
  args?: string
  result?: string
  partialResult?: string
  status?: "streaming" | "running" | "success" | "error"
  output?: { preview: string; truncated: boolean; fullLength: number }
}

interface ToolCallListProps {
  toolCalls: ToolCall[]
  isStreaming?: boolean
  sessionId?: string | null
}

/** 结果超过这个字符数才做客户端截断（低于此值直接全量渲染，减少一次点击）。 */
const CLIENT_TRUNCATE_AT = 4000

/** Coerce any tool result / partial-result payload to a renderable string. */
function asText(v: unknown): string {
  if (v == null) return ""
  if (typeof v === "string") return v
  if (typeof v === "number" || typeof v === "boolean") return String(v)
  try {
    return JSON.stringify(v)
  } catch {
    return String(v)
  }
}

function parseArgsObject(args?: string): Record<string, unknown> | null {
  if (!args) return null
  try {
    const parsed = JSON.parse(args)
    return parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : null
  } catch {
    return null
  }
}

/** pick 第一个存在的字符串字段，用于把 args 渲染成一行可读摘要 */
function pickString(obj: Record<string, unknown> | null, keys: string[]): string | null {
  if (!obj) return null
  for (const k of keys) {
    const v = obj[k]
    if (typeof v === "string" && v.trim()) return v
  }
  return null
}

// ── Main components ───────────────────────────────────────

/**
 * 单条工具调用卡片：一行给「做了什么 + 现在什么状态」，点开才看细节。
 *
 * 信息层级约定（见 CLAUDE.md UI/UX 原则）：
 * - 收起态只放 3 件事：动作摘要、结果摘要（几行 / +a −b）、异常状态
 * - 原始工具名（`bash` / `read_file`）只在展开区出现一次；动作摘要里已经
 *   包含了它的中文语义，重复显示等于同一事实说两遍
 * - 成功是默认预期，不给文字、只用对勾；运行中与失败必须带文字，
 *   不允许只靠颜色区分
 */
export function ToolCallItem({
  toolCall: tc,
  isStreaming,
  sessionId,
  showIcon = true,
}: {
  toolCall: ToolCall
  isStreaming?: boolean
  sessionId?: string | null
  showIcon?: boolean
}) {
  // 失败默认展开（用户必然要看原因），但保留用户手动折叠的权利：
  // null = 未手动干预，跟随状态推导
  const [manualExpanded, setManualExpanded] = useState<boolean | null>(null)
  const status = tc.status ?? (isStreaming ? "streaming" : "success")
  const isRunning = status === "streaming" || status === "running"

  const argsObj = useMemo(() => parseArgsObject(tc.args), [tc.args])
  const edit = useMemo(() => extractEdit(argsObj), [argsObj])
  const body = useMemo(() => extractBody(argsObj), [argsObj])
  const resultText = asText(tc.result ?? "")
  const liveText = asText(tc.partialResult ?? "")
  const hasResult = !!tc.result || !!tc.partialResult
  const summary = summarizeTool(tc.name, tc.args)
  const expandable = (!!tc.args && tc.args.length > 0) || hasResult
  const meta = toolMeta(edit, resultText)

  const expanded = manualExpanded ?? status === "error" // 失败默认展开

  const row = (
    <>
      {showIcon && (
        <span className="bg-hover flex size-5 shrink-0 items-center justify-center rounded-md">
          <ToolIcon name={tc.name} className="size-3" />
        </span>
      )}
      <span className="text-primary text-body min-w-0 flex-1 truncate font-medium" title={summary}>
        {summary}
      </span>
      {meta && (
        <span className="text-tertiary font-number text-caption hidden shrink-0 sm:inline">
          {meta}
        </span>
      )}
      <StatusTag status={status} />
      {expandable && (
        <span className="text-tertiary shrink-0">
          {expanded ? <ChevronDown className="size-3.5" /> : <ChevronRight className="size-3.5" />}
        </span>
      )}
    </>
  )

  const rowClass =
    "hover:bg-hover/60 h-auto w-full justify-start gap-2.5 rounded-none px-3 py-2 text-left font-normal whitespace-normal"

  return (
    <div
      className={`rounded-lg border transition-colors ${
        status === "error" ? "border-danger/30 bg-danger/10" : "border-hairline bg-hover/50"
      }`}
    >
      {expandable ? (
        <Button
          variant="ghost"
          className={rowClass}
          aria-expanded={expanded}
          onClick={() => setManualExpanded(!expanded)}
        >
          {row}
        </Button>
      ) : (
        <div className="flex items-center gap-2.5 px-3 py-2">{row}</div>
      )}

      {expandable && expanded && (
        <div className="border-hairline space-y-2.5 border-t px-3 py-2.5">
          {tc.args && tc.args.length > 0 && (
            <Section title="参数">
              <ToolArgsContent
                name={tc.name}
                args={tc.args}
                parsed={argsObj}
                edit={edit}
                body={body}
              />
            </Section>
          )}
          {hasResult && (
            <Section title="结果">
              <ToolResultContent
                isError={status === "error"}
                content={isRunning && liveText ? liveText : resultText}
                toolCallId={tc.id}
                sessionId={sessionId}
                output={tc.output}
              />
            </Section>
          )}
          <div className="text-tertiary font-number text-caption">{tc.name}</div>
        </div>
      )}
    </div>
  )
}

/** Stacked tool-call list (several invocations of one turn, standalone use). */
export default function ToolCallList({ toolCalls, isStreaming, sessionId }: ToolCallListProps) {
  if (!toolCalls || toolCalls.length === 0) return null
  return (
    <div className="my-2 flex flex-col gap-1.5">
      {toolCalls
        .filter((tc) => tc.name)
        .map((tc, i) => (
          <ToolCallItem
            key={tc.id || i}
            toolCall={tc}
            isStreaming={isStreaming}
            sessionId={sessionId}
          />
        ))}
    </div>
  )
}

// ── Tool icon by name ─────────────────────────────────

export function ToolIcon({ name, className }: { name: string; className?: string }) {
  const cls = className ?? "size-3.5"
  if (
    name.startsWith("edit") ||
    name === "file_edit" ||
    name === "write_file" ||
    name === "create_file"
  ) {
    return <FileEdit className={`${cls} text-warning shrink-0`} />
  }
  if (name.startsWith("bash") || name === "execute_command" || name === "run") {
    return <Terminal className={`${cls} text-info shrink-0`} />
  }
  return <Wrench className={`${cls} text-secondary shrink-0`} />
}

// ── 结果摘要（收起态那一小段数字）───────────────────────

/**
 * 结果摘要：只在能给出「扫一眼就懂」的量化信息时返回。
 * 宁可什么都不显示，也不显示无信息量的字（例如「完成」）。
 */
function toolMeta(edit: ReturnType<typeof extractEdit>, resultText: string): string | null {
  if (edit) {
    const removed = edit.oldText ? edit.oldText.split("\n").length : 0
    const added = edit.newText ? edit.newText.split("\n").length : 0
    return `+${added} −${removed}`
  }
  const lines = resultText ? resultText.replace(/\n+$/, "").split("\n").length : 0
  if (lines > 1) return `${lines} 行`
  return null
}

/**
 * 单个长文本字段（`content` / 多行 `command`）→ 原文 + 路径头。
 * 走 JSON 会把换行显示成 `\n`，比原文难读得多。
 */
function extractBody(parsed: Record<string, unknown> | null) {
  if (!parsed) return null
  const path = pickString(parsed, ["path", "file_path", "uri"])
  const body = pickString(parsed, [
    "content",
    "text",
    "body",
    "command",
    "cmd",
    "new_string",
    "new_text",
  ])
  if (body == null) return null
  if (!body.includes("\n") && body.length <= 120) return null
  return { path, body }
}

/** 编辑类工具的 args → 前后文本，用于渲染成可读的 diff。 */
function extractEdit(args: Record<string, unknown> | null) {
  if (!args) return null
  const path = pickString(args, ["path", "file_path", "uri"])
  const oldText = pickString(args, ["old_string", "old_text", "oldText", "old_str", "search"])
  const newText = pickString(args, ["new_string", "new_text", "newText", "new_str", "replace"])
  if (oldText == null && newText == null) return null
  return { path, oldText, newText }
}

// ── 参数 / 结果内容 ─────────────────────────────────────

function ToolArgsContent({
  args,
  parsed,
  edit,
  body,
}: {
  name: string
  args: string
  parsed: Record<string, unknown> | null
  edit: ReturnType<typeof extractEdit>
  body: ReturnType<typeof extractBody>
}) {
  if (edit) return <EditDiff {...edit} />

  if (body) {
    return (
      <div>
        {body.path && (
          <div className="text-tertiary font-number text-caption mb-1">{body.path}</div>
        )}
        <pre className="text-secondary font-number text-caption max-h-72 overflow-auto leading-relaxed whitespace-pre-wrap">
          {body.body}
        </pre>
      </div>
    )
  }

  // 少量短字段：直接铺成「键 值」，比让用户读 JSON 花括号快
  const inline = inlineFields(parsed)
  if (inline) {
    return (
      <div className="space-y-0.5">
        {inline.map(({ key, value }) => (
          <div key={key} className="font-number text-caption flex gap-2">
            <span className="text-tertiary w-20 shrink-0 truncate">{key}</span>
            <span className="text-secondary min-w-0 break-all whitespace-pre-wrap">{value}</span>
          </div>
        ))}
      </div>
    )
  }

  return (
    <pre className="text-secondary font-number text-caption max-h-72 overflow-auto leading-relaxed whitespace-pre-wrap">
      {formatArgs(args)}
    </pre>
  )
}

/** ≤3 个字段且值都是短标量时，返回可内联展示的键值对；否则 null。 */
function inlineFields(
  parsed: Record<string, unknown> | null
): { key: string; value: string }[] | null {
  if (!parsed) return null
  const entries = Object.entries(parsed)
  if (entries.length === 0 || entries.length > 3) return null
  const out: { key: string; value: string }[] = []
  for (const [key, raw] of entries) {
    const isScalar = typeof raw === "string" || typeof raw === "number" || typeof raw === "boolean"
    if (!isScalar) return null
    const value = String(raw)
    if (value.length > 200 || value.includes("\n")) return null
    out.push({ key, value })
  }
  return out
}

function EditDiff({
  path,
  oldText,
  newText,
}: {
  path: string | null
  oldText: string | null
  newText: string | null
}) {
  return (
    <div>
      {path && <div className="text-tertiary font-number text-caption mb-1">{path}</div>}
      <pre className="font-number text-caption max-h-72 overflow-auto leading-relaxed whitespace-pre-wrap">
        {oldText?.split("\n").map((line, i) => (
          <div key={`o${i}`} className="text-danger">
            -{line}
          </div>
        ))}
        {newText?.split("\n").map((line, i) => (
          <div key={`n${i}`} className="text-success">
            +{line}
          </div>
        ))}
      </pre>
    </div>
  )
}

function ToolResultContent({
  content,
  isError,
  toolCallId,
  sessionId,
  output,
}: {
  content: string
  isError: boolean
  toolCallId: string
  sessionId?: string | null
  output?: { preview: string; truncated: boolean; fullLength: number }
}) {
  const safeContent = useMemo(() => asText(content), [content])
  return (
    <ResultBlock
      content={safeContent}
      isError={isError}
      toolCallId={toolCallId}
      sessionId={sessionId}
      output={output}
    />
  )
}

/** 结果正文：超长时先给截断 + 展开，服务端截断则按需拉全量。 */
function ResultBlock({
  content,
  isError,
  toolCallId,
  sessionId,
  output,
}: {
  content: string
  isError: boolean
  toolCallId: string
  sessionId?: string | null
  output?: { preview: string; truncated: boolean; fullLength: number }
}) {
  const [fullContent, setFullContent] = useState<string | null>(null)
  const [showFull, setShowFull] = useState(false)
  const [loading, setLoading] = useState(false)

  const serverTruncated = output?.truncated ?? false
  const clientLong = !serverTruncated && content.length > CLIENT_TRUNCATE_AT

  const displayText = asText(
    fullContent ??
      (serverTruncated
        ? (output?.preview ?? content)
        : clientLong && !showFull
          ? content.slice(0, CLIENT_TRUNCATE_AT)
          : content)
  )

  const expandFromServer = async () => {
    if (!sessionId) return
    setLoading(true)
    try {
      const data = await fetchJson<{ content?: string }>(
        `/api/ai/sessions/${sessionId}/tool-results/${toolCallId}`
      )
      setFullContent(data.content ?? content)
    } catch {
      // 拉取失败就继续用已有的部分内容，不弹额外提示
    } finally {
      setLoading(false)
    }
  }

  return (
    <div>
      <pre
        className={`font-number text-caption max-h-96 overflow-y-auto leading-relaxed whitespace-pre-wrap ${
          isError ? "text-danger" : "text-secondary"
        }`}
      >
        {displayText}
        {serverTruncated && !fullContent ? "\n…" : ""}
      </pre>
      {serverTruncated && !fullContent && (
        <Button
          variant="link"
          size="xs"
          className="text-tertiary hover:text-secondary font-number h-auto p-0"
          onClick={expandFromServer}
          disabled={loading}
        >
          {loading ? "加载中…" : `展开全部（共 ${output?.fullLength ?? content.length} 字符）`}
        </Button>
      )}
      {clientLong && !showFull && (
        <Button
          variant="link"
          size="xs"
          className="text-tertiary hover:text-secondary font-number mt-1 h-auto p-0"
          onClick={() => setShowFull(true)}
        >
          展开全部（共 {content.length} 字符）
        </Button>
      )}
    </div>
  )
}

// ── 状态与分组 ─────────────────────────────────────────

/**
 * 状态标记：成功只用对勾（默认预期，不需要文字），
 * 运行中 / 失败必须带文字，不能只靠颜色区分。
 */
function StatusTag({ status }: { status: ToolCall["status"] }) {
  switch (status) {
    case "streaming":
      return (
        <span className="text-brand flex shrink-0 items-center gap-1">
          <Loader2 className="size-3.5 animate-spin" />
          <span className="text-caption">准备中</span>
        </span>
      )
    case "running":
      return (
        <span className="text-warning flex shrink-0 items-center gap-1">
          <Loader2 className="size-3.5 animate-spin" />
          <span className="text-caption">运行中</span>
        </span>
      )
    case "error":
      return (
        <span className="text-danger flex shrink-0 items-center gap-1">
          <CircleAlert className="size-3.5" />
          <span className="text-caption">失败</span>
        </span>
      )
    case "success":
      return <Check className="text-success size-3.5 shrink-0" />
    default:
      return null
  }
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div>
      <div className="text-tertiary font-number text-caption mb-1">{title}</div>
      {children}
    </div>
  )
}

function formatArgs(args: string): string {
  try {
    const parsed = JSON.parse(args)
    return JSON.stringify(parsed, null, 2)
  } catch {
    return args
  }
}
