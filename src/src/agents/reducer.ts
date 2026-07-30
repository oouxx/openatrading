// ── Pure transcript reducer (docs/agentTransport.md §5, §10.2, §16.1) ──────
//
// `applyEvent` is a pure function: (turns, event) -> turns. Both the Web and
// Tauri transports feed events through it, so the rendering logic is shared.
// Unknown event variants are silently ignored (forward-compat with future
// pi-rs enum variants).

import type {
  HistoryBlock,
  HistoryMessage,
  SessionEvent,
  TranscriptBlock,
  TranscriptEvent,
  TranscriptTurn,
} from "./types"

let blockSeq = 0
export const newBlockId = () => `blk-${++blockSeq}`
let turnSeq = 0
export const newTurnId = () => `turn-${++turnSeq}`

/** Coerce a tool result/partial-result value (which may arrive as a string,
 *  number, boolean, object, or array) into a displayable string. Objects and
 *  arrays are JSON-stringified so the render layer never receives a raw
 *  object (which React rejects as a child). */
function asResultString(v: unknown): string {
  if (v === null || v === undefined) return ""
  if (typeof v === "string") return v
  if (typeof v === "number" || typeof v === "boolean") return String(v)
  try {
    return JSON.stringify(v)
  } catch {
    return String(v)
  }
}

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
  // Find the last block of the requested type. Keep scanning backwards past
  // non-matching blocks (e.g. tool_call / thinking interleaved with text) so a
  // delta reaches the right block instead of wrongly spawning a new one
  // (finding #6).
  let idx = -1
  for (let i = blocks.length - 1; i >= 0; i--) {
    const b = blocks[i]
    if (b.type === blockType) {
      idx = i
      break
    }
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
  // Scan backwards past non-matching blocks to find the last block of the
  // requested type (finding #6).
  let idx = -1
  for (let i = blocks.length - 1; i >= 0; i--) {
    if (blocks[i].type === blockType) {
      idx = i
      break
    }
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
  // Scan backwards past non-matching blocks to find the last tool_call
  // block (finding #6).
  let idx = -1
  for (let i = blocks.length - 1; i >= 0; i--) {
    if (blocks[i].type === "tool_call") {
      idx = i
      break
    }
  }
  if (idx >= 0 && blocks[idx].type === "tool_call") {
    const tc = blocks[idx] as Extract<TranscriptBlock, { type: "tool_call" }>
    // Only append to streaming/running blocks; skip completed ones
    if (tc.status === "streaming" || tc.status === "running") {
      const nextBlocks = blocks.slice()
      nextBlocks[idx] = { ...tc, input: tc.input + delta, status: "streaming" }
      return replaceLast(turns, { ...last, blocks: nextBlocks })
    }
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
  // First, try to find a block with the matching toolCall.id
  let idx = blocks.findIndex(
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
  // Not found by id — look for a placeholder block created by appendToolCallDelta
  // (which uses newBlockId() and has an empty name). Update it with the real id.
  idx = blocks.findIndex(
    (b) => b.type === "tool_call" && !(b as Extract<TranscriptBlock, { type: "tool_call" }>).name,
  )
  if (idx >= 0) {
    const tc = blocks[idx] as Extract<TranscriptBlock, { type: "tool_call" }>
    const nextBlocks = blocks.slice()
    nextBlocks[idx] = { ...tc, id: toolCall.id, name: toolCall.name, input: args || tc.input, status: "running" }
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
  // NOTE: `Array.prototype.with` (ES2023) would avoid the O(n) shallow copy
  // here (finding #14), but the project's TS `lib` targets ES2022. Bumping the
  // lib is left as a follow-up; the slice+assign path below is functionally
  // equivalent.
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

function addToolCall(
  turns: TranscriptTurn[],
  id: string,
  name: string,
  input: string,
): TranscriptTurn[] {
  const last = turns[turns.length - 1]
  if (!last || last.status === "complete") {
    // No active turn — start one and add the tool call block
    return [
      ...turns,
      {
        id: newTurnId(),
        role: "assistant",
        status: "in_progress",
        blocks: [{ type: "tool_call", id, name, input, status: "running" }],
      },
    ]
  }
  // Check if a block with the same id already exists (e.g. from toolcall_end).
  // If so, update it instead of adding a duplicate.
  const blocks = last.blocks
  const existing = blocks.findIndex(
    (b) => b.type === "tool_call" && (b as Extract<TranscriptBlock, { type: "tool_call" }>).id === id,
  )
  if (existing >= 0) {
    const tc = blocks[existing] as Extract<TranscriptBlock, { type: "tool_call" }>
    const nextBlocks = blocks.slice()
    nextBlocks[existing] = { ...tc, name: name || tc.name, input: input || tc.input, status: "running" }
    return replaceLast(turns, { ...last, blocks: nextBlocks })
  }
  // Not found by id — look for a placeholder block created by appendToolCallDelta
  // (which uses newBlockId() and has an empty name). Update it with the real id.
  const placeholder = blocks.findIndex(
    (b) => b.type === "tool_call" && !(b as Extract<TranscriptBlock, { type: "tool_call" }>).name,
  )
  if (placeholder >= 0) {
    const tc = blocks[placeholder] as Extract<TranscriptBlock, { type: "tool_call" }>
    const nextBlocks = blocks.slice()
    nextBlocks[placeholder] = { ...tc, id, name, input: input || tc.input, status: "running" }
    return replaceLast(turns, { ...last, blocks: nextBlocks })
  }
  return replaceLast(turns, {
    ...last,
    blocks: [...last.blocks, { type: "tool_call", id, name, input, status: "running" }],
  })
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
    // ── AgentEvent passthrough variants ──
    case "agent_start":
      return { turns: startTurnIfIdle(turns), terminal: false }
    case "agent_end":
      if (e.will_retry) {
        const last = turns[turns.length - 1]
        let next = turns
        if (last && last.status === "in_progress") {
          next = replaceLast(turns, { ...last, status: "complete", isError: true })
        }
        return { turns: next, terminal: false }
      }
      return { turns: markAllComplete(turns), terminal: true }
    case "message_update": {
      const sub = e.assistant_message_event
      if (!sub) return { turns, terminal: false }
      switch (sub.type) {
        case "text_delta":
          return { turns: appendDelta(turns, "text", sub.delta ?? ""), terminal: false }
        case "text_end":
          return { turns: finalizeBlock(turns, "text", sub.content ?? ""), terminal: false }
        case "thinking_delta":
          return { turns: appendDelta(turns, "thinking", sub.delta ?? ""), terminal: false }
        case "thinking_end":
          return { turns: finalizeBlock(turns, "thinking", sub.content ?? ""), terminal: false }
        case "toolcall_delta":
          return { turns: appendToolCallDelta(turns, sub.delta ?? ""), terminal: false }
        case "toolcall_end":
          if (sub.toolCall) return { turns: finalizeToolCall(turns, sub.toolCall), terminal: false }
          return { turns, terminal: false }
        case "error": {
          const msg = sub.error?.errorMessage ?? "unknown error"
          return { turns: appendSystemNotice(turns, "notice", msg, { isError: true, status: "error" }), terminal: false }
        }
        case "done": {
          const err = sub.message?.errorMessage
          if (err) return { turns: appendSystemNotice(turns, "notice", err, { isError: true, status: "error" }), terminal: false }
          if (sub.content) {
            return { turns: finalizeBlock(turns, "text", sub.content), terminal: false }
          }
          return { turns, terminal: false }
        }
        default:
          return { turns, terminal: false }
      }
    }
    case "tool_execution_start":
      return { turns: addToolCall(turns, e.tool_call_id ?? "", e.tool_name ?? "", asResultString(e.args)), terminal: false }
    case "tool_execution_update":
      return { turns: updateToolCall(turns, e.tool_call_id ?? "", {
        partialResult: asResultString(e.partial_result),
        status: "running",
      }), terminal: false }
    case "tool_execution_end":
      return { turns: updateToolCall(turns, e.tool_call_id ?? "", {
        result: asResultString(e.result),
        status: e.is_error ? "error" : "done",
      }), terminal: false }
    case "message_start":
    case "message_end":
      // Message boundaries are handled via message_update sub-types; no
      // separate rendering needed.
      return { turns, terminal: false }

    // ── Session-specific variants ──
    case "turn_start":
      return { turns: startTurnIfIdle(turns), terminal: false }
    case "turn_end": {
      const results = e.tool_results ?? []
      let next = turns
      if (results.length > 0) {
        const text = results
          .map((r) => {
            if (typeof r === "string") return r
            const p = r as { preview?: string; truncated?: boolean; fullLength?: number }
            return p.preview ?? ""
          })
          .join("\n---\n")
        next = appendSystemNotice(next, "turn", text)
      }
      return { turns: next, terminal: false }
    }
    case "agent_settled":
      return { turns: markAllComplete(turns), terminal: false }
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
    case "permission_request": {
      const last = turns[turns.length - 1]
      if (!last || last.status === "complete") {
        return { turns, terminal: false }
      }
      return {
        turns: replaceLast(turns, {
          ...last,
          blocks: [
            ...last.blocks,
            {
              type: "permission_request",
              id: e.id ?? "",
              toolName: e.tool_name ?? "",
              toolInput: e.tool_input ?? null,
              resolved: false,
            },
          ],
        }),
        terminal: false,
      }
    }
    case "notice": {
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

// ── Public reducer ────────────────────────────────────────────────────────// ── Public reducer ────────────────────────────────────────────────────────

/** @returns `{ turns, terminal }` — `terminal` signals the transport can close. */
export function applyEvent(
  turns: TranscriptTurn[],
  event: TranscriptEvent,
): { turns: TranscriptTurn[]; terminal: boolean } {
  if (event.kind === "session") {
    return applySessionEvent(turns, event as SessionEvent)
  }
  // Unknown event kind — log but never break rendering (§16.1).
  // eslint-disable-next-line no-console
  console.warn("unknown event kind, ignored:", event)
  return { turns, terminal: false }
}

// ── History → turns (one-shot snapshot, §7.2) ─────────────────────────────

/** Convert a persisted structured content block into a renderable block. */
function historyBlockToTranscriptBlock(
  block: HistoryBlock,
): TranscriptBlock | null {
  switch (block.type) {
    case "text":
      return {
        type: "text",
        id: newBlockId(),
        content: block.text,
        streaming: false,
      }
    case "thinking":
      return {
        type: "thinking",
        id: newBlockId(),
        content: block.text,
        streaming: false,
      }
    case "tool_use":
      return {
        type: "tool_call",
        id: block.id,
        name: block.name,
        input:
          typeof block.input === "string"
            ? block.input
            : JSON.stringify(block.input ?? ""),
        status: "done",
      }
    default:
      return null
  }
}

/** Convert persisted history messages into completed turns. */
export function historyToTurns(messages: HistoryMessage[]): TranscriptTurn[] {
  return messages.map((m) => {
    // Prefer structured blocks when present (assistant turns carry thinking /
    // tool_use blocks); fall back to the flat text content for user turns
    // (finding #10).
    const blocks: TranscriptBlock[] = m.blocks
      ? m.blocks
          .map(historyBlockToTranscriptBlock)
          .filter((b): b is TranscriptBlock => b !== null)
      : m.content
        ? [{ type: "text", id: newBlockId(), content: m.content, streaming: false }]
        : []
    return {
      id: newTurnId(),
      role: m.role === "user" ? "user" : "assistant",
      status: "complete" as const,
      blocks,
    }
  })
}
