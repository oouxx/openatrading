// ── Pure transcript reducer (docs/agentTransport.md §5, §10.2, §16.1) ──────
//
// `applyEvent` is a pure function: (turns, event) -> turns. Both the Web and
// Tauri transports feed events through it, so the rendering logic is shared.
// Unknown event variants are silently ignored (forward-compat with future
// pi-rs enum variants).

import type {
  CoreEvent,
  HistoryMessage,
  SessionEvent,
  TranscriptBlock,
  TranscriptEvent,
  TranscriptTurn,
} from "./types"

let blockSeq = 0
const newBlockId = () => `blk-${++blockSeq}`
let turnSeq = 0
const newTurnId = () => `turn-${++turnSeq}`

/** Append a text/thinking delta to the last block of that type, creating one if needed. */
function appendDelta(
  turns: TranscriptTurn[],
  blockType: "text" | "thinking",
  delta: string,
): TranscriptTurn[] {
  if (!delta) return turns
  const last = turns[turns.length - 1]
  if (!last || last.status === "complete") {
    // No in-progress turn — TurnStart missed or not yet arrived. Auto-create a
    // fallback assistant turn (§10.2 defensive ordering).
    return [
      ...turns,
      {
        id: newTurnId(),
        role: "assistant",
        status: "in_progress",
        blocks: [{ type: blockType, id: newBlockId(), content: delta, streaming: true }],
      },
    ]
  }
  const blocks = last.blocks
  // Find the last block of the requested type that is still streaming.
  let idx = -1
  for (let i = blocks.length - 1; i >= 0; i--) {
    const b = blocks[i]
    if (b.type === blockType) {
      idx = i
      break
    }
    // A different block after the last matching one means the matching block
    // is finalized; start a fresh one instead.
    break
  }
  if (idx >= 0 && blocks[idx].type === blockType && (blocks[idx] as { streaming: boolean }).streaming) {
    const block = blocks[idx] as Extract<TranscriptBlock, { type: "text" | "thinking" }>
    const updated: TranscriptBlock = { ...block, content: block.content + delta }
    const nextBlocks = blocks.slice()
    nextBlocks[idx] = updated
    return replaceLast(turns, { ...last, blocks: nextBlocks })
  }
  // No active matching block — create a new one.
  return replaceLast(turns, {
    ...last,
    blocks: [
      ...last.blocks,
      { type: blockType, id: newBlockId(), content: delta, streaming: true },
    ],
  })
}

/** Replace text/thinking block content with the final assembled content. */
function finalizeBlock(
  turns: TranscriptTurn[],
  blockType: "text" | "thinking",
  content: string,
): TranscriptTurn[] {
  const last = turns[turns.length - 1]
  if (!last || last.status === "complete") return turns
  const blocks = last.blocks
  let idx = -1
  for (let i = blocks.length - 1; i >= 0; i--) {
    if (blocks[i].type === blockType) {
      idx = i
      break
    }
    break
  }
  if (idx >= 0 && blocks[idx].type === blockType) {
    const block = blocks[idx] as Extract<TranscriptBlock, { type: "text" | "thinking" }>
    const nextBlocks = blocks.slice()
    nextBlocks[idx] = { ...block, content: content || block.content, streaming: false }
    return replaceLast(turns, { ...last, blocks: nextBlocks })
  }
  if (!content) return turns
  return replaceLast(turns, {
    ...last,
    blocks: [
      ...last.blocks,
      { type: blockType, id: newBlockId(), content, streaming: false },
    ],
  })
}

function appendToolCallDelta(turns: TranscriptTurn[], delta: string): TranscriptTurn[] {
  if (!delta) return turns
  const last = turns[turns.length - 1]
  if (!last || last.status === "complete") return turns
  const blocks = last.blocks
  let idx = -1
  for (let i = blocks.length - 1; i >= 0; i--) {
    if (blocks[i].type === "tool_call") {
      idx = i
      break
    }
    break
  }
  if (idx >= 0 && blocks[idx].type === "tool_call") {
    const tc = blocks[idx] as Extract<TranscriptBlock, { type: "tool_call" }>
    const nextBlocks = blocks.slice()
    nextBlocks[idx] = { ...tc, input: tc.input + delta, status: "streaming" }
    return replaceLast(turns, { ...last, blocks: nextBlocks })
  }
  return replaceLast(turns, {
    ...last,
    blocks: [
      ...last.blocks,
      { type: "tool_call", id: newBlockId(), name: "", input: delta, status: "streaming" },
    ],
  })
}

function finalizeToolCall(
  turns: TranscriptTurn[],
  toolCall: { id: string; name: string; arguments: unknown },
): TranscriptTurn[] {
  const last = turns[turns.length - 1]
  if (!last || last.status === "complete") return turns
  const blocks = last.blocks
  const idx = blocks.findIndex(
    (b) => b.type === "tool_call" && (b as Extract<TranscriptBlock, { type: "tool_call" }>).id === toolCall.id,
  )
  const args =
    typeof toolCall.arguments === "string"
      ? toolCall.arguments
      : JSON.stringify(toolCall.arguments ?? "")
  if (idx >= 0) {
    const tc = blocks[idx] as Extract<TranscriptBlock, { type: "tool_call" }>
    const nextBlocks = blocks.slice()
    nextBlocks[idx] = { ...tc, id: toolCall.id, name: toolCall.name || tc.name, input: args || tc.input, status: "running" }
    return replaceLast(turns, { ...last, blocks: nextBlocks })
  }
  return replaceLast(turns, {
    ...last,
    blocks: [
      ...last.blocks,
      { type: "tool_call", id: toolCall.id, name: toolCall.name, input: args, status: "running" },
    ],
  })
}

function updateToolCall(
  turns: TranscriptTurn[],
  id: string,
  patch: Partial<Extract<TranscriptBlock, { type: "tool_call" }>>,
): TranscriptTurn[] {
  const last = turns[turns.length - 1]
  if (!last || last.status === "complete") return turns
  const blocks = last.blocks
  const idx = blocks.findIndex(
    (b) => b.type === "tool_call" && (b as Extract<TranscriptBlock, { type: "tool_call" }>).id === id,
  )
  if (idx < 0) return turns
  const tc = blocks[idx] as Extract<TranscriptBlock, { type: "tool_call" }>
  const nextBlocks = blocks.slice()
  nextBlocks[idx] = { ...tc, ...patch }
  return replaceLast(turns, { ...last, blocks: nextBlocks })
}

function replaceLast(turns: TranscriptTurn[], turn: TranscriptTurn): TranscriptTurn[] {
  const next = turns.slice()
  next[next.length - 1] = turn
  return next
}

function startTurn(turns: TranscriptTurn[]): TranscriptTurn[] {
  return [
    ...turns,
    { id: newTurnId(), role: "assistant", status: "in_progress", blocks: [] },
  ]
}

/** Mark all streaming blocks + the turn itself as complete. */
function markAllComplete(turns: TranscriptTurn[]): TranscriptTurn[] {
  const last = turns[turns.length - 1]
  if (!last || last.status === "complete") return turns
  const blocks = last.blocks.map((b) => {
    if (b.type === "text" || b.type === "thinking") return { ...b, streaming: false }
    if (b.type === "tool_call" && (b.status === "streaming" || b.status === "running"))
      return { ...b, status: "done" as const }
    return b
  })
  return replaceLast(turns, { ...last, status: "complete", blocks })
}

function appendSystemNotice(
  turns: TranscriptTurn[],
  kind: "compaction" | "retry" | "turn" | "notice",
  detail: string,
  extra: Partial<Extract<TranscriptBlock, { type: "system_notice" }>> = {},
): TranscriptTurn[] {
  const last = turns[turns.length - 1]
  if (!last || last.status === "complete") {
    return [
      ...turns,
      {
        id: newTurnId(),
        role: "assistant",
        status: "in_progress",
        blocks: [
          { type: "system_notice", id: newBlockId(), kind, detail, status: "active", ...extra },
        ],
      },
    ]
  }
  return replaceLast(turns, {
    ...last,
    blocks: [
      ...last.blocks,
      { type: "system_notice", id: newBlockId(), kind, detail, status: "active", ...extra },
    ],
  })
}

function updateSystemNotice(
  turns: TranscriptTurn[],
  kind: "compaction" | "retry",
  patch: Partial<Extract<TranscriptBlock, { type: "system_notice" }>>,
): TranscriptTurn[] {
  const last = turns[turns.length - 1]
  if (!last) return turns
  const blocks = last.blocks
  // Find the last system_notice of this kind.
  let idx = -1
  for (let i = blocks.length - 1; i >= 0; i--) {
    const b = blocks[i]
    if (b.type === "system_notice" && (b as Extract<TranscriptBlock, { type: "system_notice" }>).kind === kind) {
      idx = i
      break
    }
  }
  if (idx < 0) return turns
  const notice = blocks[idx] as Extract<TranscriptBlock, { type: "system_notice" }>
  const nextBlocks = blocks.slice()
  nextBlocks[idx] = { ...notice, ...patch }
  if (last.status === "complete") return turns
  return replaceLast(turns, { ...last, blocks: nextBlocks })
}

// ── Core (AgentEvent) handling ────────────────────────────────────────────

function applyCoreEvent(turns: TranscriptTurn[], e: CoreEvent): TranscriptTurn[] {
  switch (e.type) {
    case "MessageUpdate": {
      const sub = e.assistant_message_event
      if (!sub) return turns
      switch (sub.type) {
        case "text_delta":
          return appendDelta(turns, "text", sub.delta ?? "")
        case "text_end":
          return finalizeBlock(turns, "text", sub.content ?? "")
        case "thinking_delta":
          return appendDelta(turns, "thinking", sub.delta ?? "")
        case "thinking_end":
          return finalizeBlock(turns, "thinking", sub.content ?? "")
        case "toolcall_delta":
          return appendToolCallDelta(turns, sub.delta ?? "")
        case "toolcall_end":
          if (sub.toolCall) return finalizeToolCall(turns, sub.toolCall)
          return turns
        case "error": {
          const msg = sub.error?.errorMessage ?? "unknown error"
          return appendSystemNotice(turns, "notice", msg, { isError: true, status: "error" })
        }
        case "done": {
          // Trailing assistant error (if any) is surfaced via the message field.
          const err = sub.message?.errorMessage
          if (err) return appendSystemNotice(turns, "notice", err, { isError: true, status: "error" })
          return turns
        }
        default:
          return turns
      }
    }
    case "TurnStart":
    case "TurnEnd":
      // Core also emits TurnStart/TurnEnd, but the session-level turn_start /
      // turn_end are authoritative (they carry extracted tool_results). Ignore
      // the core variants to avoid duplicate turn blocks (§5).
      return turns
    case "ToolExecutionStart":
      // Tool call card already created on toolcall_end; nothing to do here.
      return turns
    case "ToolExecutionUpdate":
      return updateToolCall(turns, e.tool_call_id ?? "", {
        partialResult: e.partial_result ?? "",
        status: "running",
      })
    case "ToolExecutionEnd":
      return updateToolCall(turns, e.tool_call_id ?? "", {
        result: e.result ?? "",
        status: e.is_error ? "error" : "done",
      })
    case "AgentEnd":
      // Terminal signal is taken from the session-level AgentEnd (which knows
      // about retries); ignore the core one to avoid premature completion.
      return turns
    default:
      return turns
  }
}

function startTurnIfIdle(turns: TranscriptTurn[]): TranscriptTurn[] {
  const last = turns[turns.length - 1]
  if (!last || last.status === "complete") return startTurn(turns)
  return turns
}

// ── Session (AgentSessionEvent) handling ──────────────────────────────────

/** @returns `true` when the event is terminal (agent turn fully ended, no retry). */
export function applySessionEvent(turns: TranscriptTurn[], e: SessionEvent): {
  turns: TranscriptTurn[]
  terminal: boolean
} {
  switch (e.type) {
    case "turn_start":
      return { turns: startTurnIfIdle(turns), terminal: false }
    case "turn_end": {
      const results = e.tool_results ?? []
      let next = turns
      if (results.length > 0) {
        next = appendSystemNotice(next, "turn", results.join("\n---\n"))
      }
      return { turns: next, terminal: false }
    }
    case "agent_settled":
      return { turns: markAllComplete(turns), terminal: false }
    case "agent_end":
      if (e.will_retry) {
        // Finalize current assistant turn as error; a new turn will start on retry.
        const last = turns[turns.length - 1]
        let next = turns
        if (last && last.status === "in_progress") {
          next = replaceLast(turns, { ...last, status: "complete", isError: true })
        }
        return { turns: next, terminal: false }
      }
      return { turns: markAllComplete(turns), terminal: true }
    case "compaction_start":
      return {
        turns: appendSystemNotice(turns, "compaction", e.reason ?? "", { status: "active" }),
        terminal: false,
      }
    case "compaction_end":
      return {
        turns: updateSystemNotice(turns, "compaction", {
          status: e.error_message ? "error" : "done",
          detail: e.reason ?? "",
          isError: !!e.error_message,
        }),
        terminal: false,
      }
    case "auto_retry_start":
      return {
        turns: appendSystemNotice(turns, "retry", e.error_message ?? "", {
          status: "active",
          attempt: e.attempt,
          maxAttempts: e.max_attempts,
          delayMs: e.delay_ms,
        }),
        terminal: false,
      }
    case "auto_retry_end":
      return {
        turns: updateSystemNotice(turns, "retry", {
          status: e.success ? "done" : "error",
          detail: e.final_error ?? "",
          isError: !e.success,
        }),
        terminal: false,
      }
    case "notice": {
      // Slash command result / error. Terminal — no AgentEnd follows.
      let next = turns
      if (e.is_error) {
        if (e.text) next = appendSystemNotice(next, "notice", e.text, { isError: true, status: "error" })
      } else {
        if (e.text) next = appendSystemNotice(next, "notice", e.text, { status: "done" })
      }
      next = markAllComplete(next)
      return { turns: next, terminal: true }
    }
    case "queue_update":
    case "session_info_changed":
    case "other":
    default:
      return { turns, terminal: false }
  }
}

// ── Public reducer ────────────────────────────────────────────────────────

/** @returns `{ turns, terminal }` — `terminal` signals the transport can close. */
export function applyEvent(
  turns: TranscriptTurn[],
  event: TranscriptEvent,
): { turns: TranscriptTurn[]; terminal: boolean } {
  if (event.kind === "core") {
    return { turns: applyCoreEvent(turns, event as CoreEvent), terminal: false }
  }
  if (event.kind === "session") {
    return applySessionEvent(turns, event as SessionEvent)
  }
  // Unknown event kind — log but never break rendering (§16.1).
  // eslint-disable-next-line no-console
  console.warn("unknown event kind, ignored:", event)
  return { turns, terminal: false }
}

// ── History → turns (one-shot snapshot, §7.2) ─────────────────────────────

/** Convert persisted history messages into completed turns. */
export function historyToTurns(messages: HistoryMessage[]): TranscriptTurn[] {
  return messages.map((m) => ({
    id: newTurnId(),
    role: m.role === "user" ? "user" : "assistant",
    status: "complete" as const,
    blocks: m.content
      ? [{ type: "text" as const, id: newBlockId(), content: m.content, streaming: false }]
      : [],
  }))
}
