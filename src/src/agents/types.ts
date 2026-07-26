// ── Transcript rendering data model (docs/agentTransport.md §5) ────────────
//
// The frontend renders a conversation as a list of *turns*, each turn a list
// of *blocks*. This model is shared 1:1 between the WebSocket (Web) and Tauri
// IPC (desktop) transports; only the transport layer differs.

/** A single renderable unit inside a turn. */
export type TranscriptBlock =
  | { type: "text"; id: string; content: string; streaming: boolean }
  | { type: "thinking"; id: string; content: string; streaming: boolean }
  | {
      type: "tool_call"
      id: string
      name: string
      /** Raw tool input — string for completed calls, accumulated deltas while streaming. */
      input: string
      status: "streaming" | "running" | "done" | "error"
      /** Tool result text once available. */
      result?: string
      /** Partial result streamed during execution. */
      partialResult?: string
      /** Preview info for truncated tool results (§15). */
      output?: { preview: string; truncated: boolean; fullLength: number }
    }
  | {
      type: "system_notice"
      id: string
      kind: "compaction" | "retry" | "turn" | "notice"
      detail: string
      isError?: boolean
      /** Transient lifecycle status for compaction/retry indicators. */
      status?: "active" | "done" | "error"
      attempt?: number
      maxAttempts?: number
      delayMs?: number
    }
  | {
      type: "permission_request"
      id: string
      toolName: string
      toolInput: unknown
      resolved: boolean
    }

export interface TranscriptTurn {
  id: string
  role: "user" | "assistant"
  /** `in_progress` while the agent is producing this turn, `complete` once settled. */
  status: "in_progress" | "complete"
  blocks: TranscriptBlock[]
  /** Set when the assistant turn ended in an error. */
  isError?: boolean
}

// ── Wire event envelope (matches backend `TranscriptEvent` serde tags) ──────
//
// Backend serializes all events as `TranscriptEvent::Session(SessionEventPayload)`:
//   { "kind": "session", "type": "...", ... }
// The `type` field discriminates all variants: AgentStart, MessageUpdate,
// ToolExecutionStart, TurnStart, AgentSettled, CompactionStart, etc.
// `assistant_message_event` is a nested `#[serde(tag = "type")]` enum with
// snake_case variants: text_delta / text_end / thinking_delta / thinking_end /
// toolcall_delta / toolcall_end / done / error.

export interface AssistantMessageEvent {
  type: string
  delta?: string
  content?: string
  toolCall?: { id: string; name: string; arguments: unknown }
  error?: { errorMessage?: string | null }
  message?: { errorMessage?: string | null }
}

/** Unified session event payload — matches `SessionEventPayload` (snake_case).
 *
 * All events from the backend use `kind: "session"` with a `type` field that
 * discriminates the variant: AgentStart, MessageUpdate, ToolExecutionStart,
 * TurnStart, AgentSettled, CompactionStart, etc. */
export interface SessionEvent {
  kind: "session"
  type: string
  /** Assistant message event (nested serde enum) — present for MessageUpdate. */
  assistant_message_event?: AssistantMessageEvent
  /** Tool execution fields — present for ToolExecutionStart/Update/End. */
  tool_call_id?: string
  tool_name?: string
  args?: unknown
  /** Streamed partial result — may arrive as a JSON value (object/array). */
  partial_result?: unknown
  /** Final tool result — may arrive as a JSON value (object/array). */
  result?: unknown
  is_error?: boolean
  /** Tool results from turn_end. */
  tool_results?: ToolResultPayload[]
  /** Retry / lifecycle fields. */
  will_retry?: boolean
  reason?: string
  aborted?: boolean
  error_message?: string | null
  attempt?: number
  max_attempts?: number
  delay_ms?: number
  success?: boolean
  final_error?: string | null
  /** Notice fields. */
  text?: string
  /** Permission request fields (§11). */
  id?: string
  tool_input?: unknown
}

export type TranscriptEvent = SessionEvent | { kind: string; [k: string]: unknown }

/** Client → server commands (matches backend `ClientCommand`). */
export type ClientCommand =
  | { type: "prompt"; text: string }
  | { type: "interrupt" }
  | { type: "ping" }
  | { type: "permission_response"; id: string; approved: boolean }

// ── History message shape (GET /api/ai/sessions/:id/messages) ─────────────

/** Tool result preview payload with truncation info (§15). */
export interface ToolResultPayload {
  preview: string
  truncated: boolean
  fullLength: number
}

/** Structured content block returned in history messages (finding #10). */
export type HistoryBlock =
  | { type: "thinking"; text: string }
  | { type: "tool_use"; id: string; name: string; input: unknown }
  | { type: "text"; text: string }

export interface HistoryMessage {
  role: string
  content: string
  created_at?: string
  /** Structured blocks for assistant turns (thinking / tool_use / text). */
  blocks?: HistoryBlock[]
}
