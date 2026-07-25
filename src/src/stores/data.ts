import { useSettingsStore } from "@/stores/settingsStore"

const isTauri = "__TAURI_INTERNALS__" in window

export function getApiBaseUrl(): string {
  if (isTauri) return ""
  return useSettingsStore.getState().apiUrl || __API_BASE_URL__
}

export async function fetchJson<T>(path: string, init?: RequestInit): Promise<T> {
  if (isTauri) {
    const { invoke } = await import("@tauri-apps/api/core")
    if (init?.method === "POST" || init?.method === "PUT" || init?.method === "DELETE") {
      let body: Record<string, unknown> = {}
      if (init.body) {
        try {
          body = JSON.parse(String(init.body))
        } catch {
          /* use empty */
        }
      }
      return invoke("api_post", { path, body }) as Promise<T>
    }
    return invoke("api_fetch", { path }) as Promise<T>
  }
  const base = getApiBaseUrl()
  const headers: Record<string, string> = {}
  if (init?.body) headers["Content-Type"] = "application/json"
  const resp = await fetch(`${base}${path}`, {
    ...init,
    headers: { ...headers, ...(init?.headers as Record<string, string>) },
  })
  if (!resp.ok) throw new Error(`${resp.status}`)
  if (resp.status === 204) return undefined as T
  return resp.json()
}

export interface ChatStreamCallbacks {
  onText: (text: string) => void
  onReasoning?: (text: string) => void
  onToolCall?: (tool: { id: string; name: string; args: string }) => void
  onToolDelta?: (tool: { id: string; args: string }) => void
  onToolResult?: (tool: { id: string; name: string; result: string }) => void
  onToolExecUpdate?: (tool: { id: string; name: string; args: string; partial_result: string }) => void
  onTurnStart?: () => void
  onTurnEnd?: (data: { tool_results: string[] }) => void
  onRetry?: () => void
  onCompactionStart?: (reason: string) => void
  onCompactionEnd?: (data: { reason: string; aborted: boolean; willRetry: boolean; error: string | null }) => void
  onRetryStart?: (data: { attempt: number; maxAttempts: number; delayMs: number; error: string }) => void
  onRetryEnd?: (data: { success: boolean; attempt: number; finalError: string | null }) => void
  onSettled?: () => void
  onDone: () => void
  onError: (err: string) => void
}

export interface AgentStreamBody {
  message: string
  session_id: string
}

// ── Transcript event model (unified Web/Tauri transport) ───────────────
//
// The backend serializes `TranscriptEvent` as:
//   { "kind": "core", ...AgentEvent }          — text/tool/thinking deltas
//   { "kind": "session", "type": "...", ... }  — compaction/retry/turn/notice
// We treat the events as loosely-typed payloads and only read the fields we
// need; unknown variants are silently ignored (schema forward-compat).

interface CoreEvent {
  type: string
  // MessageUpdate payload
  assistant_message_event?: {
    type: string
    delta?: string
    content?: string
    toolCall?: { id: string; name: string; arguments: unknown }
    error?: { errorMessage?: string | null }
    message?: { errorMessage?: string | null }
  }
  // ToolExecution* payloads
  tool_call_id?: string
  tool_name?: string
  args?: string
  partial_result?: string
  result?: string
  is_error?: boolean
  // TurnEnd
  tool_results?: Array<{ content?: Array<{ text?: string }> }>
}

interface SessionEvent {
  type: string
  will_retry?: boolean
  reason?: string
  aborted?: boolean
  error_message?: string | null
  attempt?: number
  max_attempts?: number
  delay_ms?: number
  success?: boolean
  final_error?: string | null
  tool_results?: string[]
  text?: string
  is_error?: boolean
}

interface TranscriptEvent {
  kind: "core" | "session"
  type?: string
  [k: string]: unknown
}

/// Map a `TranscriptEvent` to the legacy `ChatStreamCallbacks`.
///
/// Returns `true` when the event is terminal (the transport should close and
/// emit `onDone`). Non-terminal events return `false`.
function applyEventToCallbacks(
  event: TranscriptEvent,
  cb: ChatStreamCallbacks,
): boolean {
  if (event.kind === "core") {
    return applyCoreEvent(event as CoreEvent & TranscriptEvent, cb)
  }
  if (event.kind === "session") {
    return applySessionEvent(event as SessionEvent & TranscriptEvent, cb)
  }
  return false
}

function applyCoreEvent(e: CoreEvent & TranscriptEvent, cb: ChatStreamCallbacks): boolean {
  switch (e.type) {
    case "MessageUpdate": {
      const sub = e.assistant_message_event
      if (!sub) return false
      switch (sub.type) {
        case "text_delta":
          if (sub.delta) cb.onText(sub.delta)
          break
        case "text_end":
          if (sub.content) cb.onText(sub.content)
          break
        case "thinking_delta":
          if (sub.delta) cb.onReasoning?.(sub.delta)
          break
        case "thinking_end":
          if (sub.content) cb.onReasoning?.(sub.content)
          break
        case "toolcall_delta":
          if (sub.delta) cb.onToolDelta?.({ id: "", args: sub.delta })
          break
        case "toolcall_end":
          if (sub.toolCall) {
            cb.onToolCall?.({
              id: sub.toolCall.id,
              name: sub.toolCall.name,
              args: typeof sub.toolCall.arguments === "string"
                ? sub.toolCall.arguments
                : JSON.stringify(sub.toolCall.arguments ?? ""),
            })
          }
          break
        case "error": {
          const msg = sub.error?.errorMessage ?? "unknown error"
          cb.onError(msg ?? "unknown error")
          break
        }
        case "done": {
          // Assistant message finalized; surface trailing assistant error if any.
          const err = sub.message?.errorMessage
          if (err) cb.onError(err)
          break
        }
        default:
          break
      }
      return false
    }
    case "TurnStart":
      cb.onTurnStart?.()
      return false
    case "TurnEnd": {
      const results = (e.tool_results ?? [])
        .map((m) => (m.content ?? []).map((b) => b.text ?? "").join("\n"))
        .filter((t) => t.length > 0)
      cb.onTurnEnd?.({ tool_results: results })
      return false
    }
    case "ToolExecutionStart":
      // no UI update needed — tool call card is created on toolcall_end
      return false
    case "ToolExecutionUpdate":
      cb.onToolExecUpdate?.({
        id: e.tool_call_id ?? "",
        name: e.tool_name ?? "",
        args: e.args ?? "",
        partial_result: e.partial_result ?? "",
      })
      return false
    case "ToolExecutionEnd":
      cb.onToolResult?.({
        id: e.tool_call_id ?? "",
        name: e.tool_name ?? "",
        result: e.result ?? "",
      })
      return false
    case "AgentEnd":
      // Terminal signal is taken from the session-level AgentEnd (which knows
      // about retries); ignore the core one to avoid premature close.
      return false
    default:
      return false
  }
}

function applySessionEvent(e: SessionEvent & TranscriptEvent, cb: ChatStreamCallbacks): boolean {
  switch (e.type) {
    case "turn_start":
      cb.onTurnStart?.()
      return false
    case "turn_end":
      cb.onTurnEnd?.({ tool_results: e.tool_results ?? [] })
      return false
    case "agent_settled":
      cb.onSettled?.()
      return false
    case "agent_end":
      if (e.will_retry) {
        cb.onRetry?.()
        return false
      }
      return true
    case "compaction_start":
      cb.onCompactionStart?.(e.reason ?? "")
      return false
    case "compaction_end":
      cb.onCompactionEnd?.({
        reason: e.reason ?? "",
        aborted: e.aborted ?? false,
        willRetry: e.will_retry ?? false,
        error: e.error_message ?? null,
      })
      return false
    case "auto_retry_start":
      cb.onRetryStart?.({
        attempt: e.attempt ?? 0,
        maxAttempts: e.max_attempts ?? 0,
        delayMs: e.delay_ms ?? 0,
        error: e.error_message ?? "",
      })
      return false
    case "auto_retry_end":
      cb.onRetryEnd?.({
        success: e.success ?? false,
        attempt: e.attempt ?? 0,
        finalError: e.final_error ?? null,
      })
      return false
    case "notice": {
      // Slash command result / error. This is terminal — no AgentEnd follows.
      if (e.is_error) {
        if (e.text) cb.onError(e.text)
      } else {
        if (e.text) cb.onText(e.text)
        cb.onSettled?.()
      }
      return true
    }
    default:
      return false
  }
}

/// Stream an agent prompt over the unified transport (WebSocket on Web,
/// Tauri IPC on desktop). `callbacks` mirror the legacy SSE callback shape so
/// the existing ChatView reducer can be reused unchanged.
export async function agentStream(
  body: AgentStreamBody,
  callbacks: ChatStreamCallbacks,
): Promise<void> {
  if (isTauri) {
    return agentStreamTauri(body, callbacks)
  }
  return agentStreamWs(body, callbacks)
}

async function agentStreamWs(
  { message, session_id }: AgentStreamBody,
  cb: ChatStreamCallbacks,
): Promise<void> {
  const base = getApiBaseUrl()
  const wsBase = base
    ? base.replace(/^http:/, "ws:").replace(/^https:/, "wss:")
    : `${window.location.protocol === "https:" ? "wss" : "ws"}://${window.location.host}`
  const url = `${wsBase}/api/ai/sessions/${session_id}/ws`
  const ws = new WebSocket(url)

  let closed = false
  const finish = () => {
    if (closed) return
    closed = true
    cb.onDone()
    try {
      ws.close()
    } catch {
      /* ignore */
    }
  }

  await new Promise<void>((resolve, reject) => {
    const onOpen = () => {
      ws.removeEventListener("open", onOpen)
      ws.removeEventListener("error", onErr)
      resolve()
    }
    const onErr = () => {
      ws.removeEventListener("open", onOpen)
      ws.removeEventListener("error", onErr)
      reject(new Error("websocket 连接失败"))
    }
    ws.addEventListener("open", onOpen)
    ws.addEventListener("error", onErr)
  })

  ws.onmessage = (ev) => {
    let event: TranscriptEvent
    try {
      event = JSON.parse(typeof ev.data === "string" ? ev.data : "")
    } catch {
      return
    }
    if (applyEventToCallbacks(event, cb)) finish()
  }
  ws.onerror = () => {
    if (!closed) cb.onError("websocket 错误")
  }
  ws.onclose = () => finish()

  ws.send(JSON.stringify({ type: "prompt", text: message }))
}

async function agentStreamTauri(
  { message, session_id }: AgentStreamBody,
  cb: ChatStreamCallbacks,
): Promise<void> {
  const { invoke } = await import("@tauri-apps/api/core")
  const { listen } = await import("@tauri-apps/api/event")

  let done = false
  let unlisten: (() => void) | null = null
  const finish = () => {
    if (done) return
    done = true
    unlisten?.()
    cb.onDone()
  }

  unlisten = await listen<{ session_id: string; event: TranscriptEvent }>(
    "agent-event",
    (e) => {
      if (e.payload.session_id !== session_id) return
      if (applyEventToCallbacks(e.payload.event, cb)) finish()
    },
  )

  try {
    await invoke("send_prompt", { sessionId: session_id, text: message })
  } catch (err: unknown) {
    if (!done) {
      done = true
      unlisten?.()
      cb.onError(String(err))
    }
  }
}

// ── Types ──────────────────────────────────────────────

export interface OverviewData {
  date: string
  rise_count: number
  fall_count: number
  limit_up_count: number
  limit_down_count: number
  bomb_rate: number
  risk_level: string
  decision_summary: string
  market_temperature: number
  yesterday_limit_up_avg_pcp: number
}

export interface SentimentHistoryDay {
  date: string
  sentiment: number
  limit_up_count: number
  bomb_rate: number
  rise_ratio: number
}

export interface Stock {
  symbol: string
  stock_chi_name: string
  price: number
  change_percent: number
  limit_up_days: number
  turnover_ratio: number
  total_capital: number
  break_limit_up_times: number
  yesterday_limit_up_days?: number
  surge_reason: {
    stock_reason: string
    related_plates: Array<{ plate_name: string }>
  } | null
}

export interface FieldPlate {
  name: string
  count: number
  reason: string
  list: Array<{
    code: string
    name: string
    article?: {
      action_info?: {
        time?: string
        num?: string
        expound?: string
        shares_range?: number
        day?: number
        edition?: number
        price?: number
      }
    }
  }>
}

export interface RoleEntry {
  name: string
  display_name: string
  strategy: string
  readonly?: boolean
}

export interface RolePosition {
  id: number
  symbol: string
  stock_name: string
  cost_price: number
  current_price: number
  shares: number
  entry_reason: string
  entry_date: string
}

export interface RoleLog {
  date: string
  action: string
  symbol: string
  price: number
  shares: number
  reason: string
}

export interface PnlPoint {
  date: string
  equity: number
  cash: number
  market_value: number
  return_pct: number
}

export interface PnlHistoryResponse {
  points: PnlPoint[]
  initial_capital: number
}

export interface ClosedPosition {
  position_id: number
  symbol: string
  stock_name: string
  entry_date: string
  exit_date: string
  entry_price: number
  exit_price: number
  shares: number
  pnl_pct: number
  entry_reason: string
  exit_reason: string
}

export interface IndexQuote {
  symbol: string
  name: string
  last_px: number
  px_change: number
  px_change_rate: number
  trade_status: string
}
