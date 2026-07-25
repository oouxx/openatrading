// ── Agent transport abstraction (docs/agentTransport.md §4, §9) ────────────
//
// Web and Tauri share the same `AgentTransport` interface; only the concrete
// implementation differs. The frontend reducer/UI never branches on transport
// kind — they only call `onEvent` / `sendPrompt` / `interrupt`.
//
// ## Reconnection (§9)
//
// `WebSocketTransport` implements automatic reconnection with exponential
// backoff, heartbeat pings, and seq-based event replay. On disconnect it
// waits, then reconnects with `?last_seq=...` to recover missed events. If
// the server signals `resync_required`, the caller's `onResync` callback is
// invoked to fall back to full history reload.

import { getApiBaseUrl } from "@/stores/data"
import type { ClientCommand, TranscriptEvent } from "./types"

export interface AgentTransport {
  /** Register an event listener. Returns an unsubscribe function. */
  onEvent(cb: (event: TranscriptEvent) => void): () => void
  /** Send a user prompt (or slash command as raw text) to the agent. */
  sendPrompt(text: string): Promise<void>
  /** Interrupt the running agent turn. */
  interrupt(): Promise<void>
  /** Respond to a permission request (§11). */
  respondToPermission(id: string, approved: boolean): Promise<void>
  /** Tear down the connection / listeners. */
  dispose(): void
  /** Resolves once the transport is ready to receive `sendPrompt`. */
  ready(): Promise<void>
}

const isTauri = "__TAURI_INTERNALS__" in window

function wsBaseUrl(): string {
  const base = getApiBaseUrl()
  if (base) {
    return base.replace(/^http:/, "ws:").replace(/^https:/, "wss:")
  }
  return `${window.location.protocol === "https:" ? "wss" : "ws"}://${window.location.host}`
}

function sendCmd(ws: WebSocket, cmd: ClientCommand): void {
  ws.send(JSON.stringify(cmd))
}

// ── WebSocket transport with reconnection (Web, §9) ────────────────────────

export class WebSocketTransport implements AgentTransport {
  private ws: WebSocket | null = null
  private listeners = new Set<(event: TranscriptEvent) => void>()
  private readyResolve: (() => void) | null = null
  private readyReject: ((err: Error) => void) | null = null
  private readyPromise: Promise<void>
  private disposed = false
  private lastSeq: number | undefined
  private retryDelay = 1000
  private readonly maxRetryDelay = 30_000
  private heartbeatTimer: ReturnType<typeof setInterval> | null = null
  private retryTimer: ReturnType<typeof setTimeout> | null = null

  /** Heartbeat ping interval for the WebSocket keep-alive (finding #17). */
  private static readonly HEARTBEAT_INTERVAL_MS = 15_000

  constructor(
    private sessionId: string,
    private onResync?: () => void,
  ) {
    this.readyPromise = this.newReadyPromise()
    this.connect()
  }

  /** Create a fresh ready promise + resolvers. Called on every connect(). */
  private newReadyPromise(): Promise<void> {
    return new Promise<void>((resolve, reject) => {
      this.readyResolve = resolve
      this.readyReject = reject
    })
  }

  private connect(): void {
    if (this.disposed) return

    // Re-create the ready promise on every (re)connection so `sendPrompt`
    // awaits a genuinely open socket instead of a stale resolved promise
    // from a previous connection (finding #3).
    this.readyPromise = this.newReadyPromise()

    const url = new URL(`${wsBaseUrl()}/api/ai/sessions/${this.sessionId}/ws`)
    if (this.lastSeq !== undefined) {
      url.searchParams.set("last_seq", String(this.lastSeq))
    }

    this.ws = new WebSocket(url)

    this.ws.onopen = () => {
      this.retryDelay = 1000 // Reset backoff on successful connection
      this.readyResolve?.()
      this.readyResolve = null
      this.readyReject = null

      // Start heartbeat ping (finding #17).
      this.heartbeatTimer = setInterval(() => {
        try {
          this.ws?.send(JSON.stringify({ type: "ping" }))
        } catch {
          // Connection may be closed; timer will be cleared on close
        }
      }, WebSocketTransport.HEARTBEAT_INTERVAL_MS)
    }

    this.ws.onmessage = (ev) => {
      let parsed: Record<string, unknown>
      try {
        parsed = JSON.parse(typeof ev.data === "string" ? ev.data : "")
      } catch {
        return
      }

      // Handle resync signal (§9.1)
      if (parsed.type === "resync_required") {
        this.onResync?.()
        return
      }

      // Track seq for reconnection replay
      if (typeof parsed.seq === "number") {
        this.lastSeq = parsed.seq as number
      }

      const event = parsed as TranscriptEvent
      for (const cb of this.listeners) cb(event)
    }

    this.ws.onclose = () => {
      this.clearHeartbeat()
      if (this.disposed) return

      // Do NOT emit a synthetic `agent_end` here. The reducer treats
      // `agent_end` as a terminal signal and would mark the in-progress turn
      // complete, so a transient disconnect during streaming would split a
      // single assistant message across two turns once reconnected. The
      // server-side buffer replays missed events on reconnect (§9), and the
      // authoritative `agent_end` is delivered from the agent itself. We only
      // emit a terminal event on explicit `dispose()` (finding #4).

      // Reject any pending ready promise so `sendPrompt` doesn't proceed on a
      // closed socket; a fresh promise is created in the next `connect()`.
      this.readyReject?.(new Error("websocket closed"))
      this.readyReject = null
      this.readyResolve = null

      // Schedule reconnection with exponential backoff (§9.2).
      this.retryTimer = setTimeout(() => {
        this.connect()
      }, this.retryDelay)
      this.retryDelay = Math.min(this.retryDelay * 2, this.maxRetryDelay)
    }

    this.ws.onerror = () => {
      // onclose will fire after onerror, so reconnection is handled there
    }
  }

  private clearHeartbeat(): void {
    if (this.heartbeatTimer !== null) {
      clearInterval(this.heartbeatTimer)
      this.heartbeatTimer = null
    }
  }

  onEvent(cb: (event: TranscriptEvent) => void): () => void {
    this.listeners.add(cb)
    return () => this.listeners.delete(cb)
  }

  async sendPrompt(text: string): Promise<void> {
    await this.ready()
    if (!this.ws) throw new Error("transport not connected")
    sendCmd(this.ws, { type: "prompt", text })
  }

  async interrupt(): Promise<void> {
    await this.ready()
    if (!this.ws) throw new Error("transport not connected")
    sendCmd(this.ws, { type: "interrupt" })
  }

  async respondToPermission(id: string, approved: boolean): Promise<void> {
    await this.ready()
    if (!this.ws) throw new Error("transport not connected")
    sendCmd(this.ws, { type: "permission_response", id, approved })
  }

  async ready(): Promise<void> {
    return this.readyPromise
  }

  dispose(): void {
    this.disposed = true
    // Surface a synthetic terminal event so the reducer finalizes the
    // in-progress turn when the transport is torn down intentionally (finding
    // #4): this is the only place a synthetic `agent_end` is emitted.
    for (const cb of this.listeners) {
      cb({ kind: "session", type: "agent_end", will_retry: false } as TranscriptEvent)
    }
    this.listeners.clear()
    this.clearHeartbeat()
    if (this.retryTimer !== null) {
      clearTimeout(this.retryTimer)
      this.retryTimer = null
    }
    try {
      this.ws?.close()
    } catch {
      /* ignore */
    }
    this.ws = null
  }
}

// ── Tauri IPC transport (desktop) ─────────────────────────────────────────

export class TauriTransport implements AgentTransport {
  private listeners = new Set<(event: TranscriptEvent) => void>()
  private unlisten: (() => void) | null = null
  private readyPromise: Promise<void>
  private invokeFn: typeof import("@tauri-apps/api/core").invoke | null = null
  private lastSeq: number | undefined

  constructor(private sessionId: string) {
    this.readyPromise = this.init()
  }

  private async init(): Promise<void> {
    const { invoke } = await import("@tauri-apps/api/core")
    const { listen } = await import("@tauri-apps/api/event")
    this.invokeFn = invoke
    this.unlisten = await listen<{ session_id: string; event: { schema_version: number; seq: number | null; kind: string; [k: string]: unknown } }>(
      "agent-event",
      (e) => {
        if (e.payload.session_id !== this.sessionId) return
        const envelope = e.payload.event
        // Track seq for reconnection replay
        if (typeof envelope.seq === "number") {
          this.lastSeq = envelope.seq
        }
        // The event field is the EventEnvelope with flattened TranscriptEvent.
        // Strip the envelope fields to get the inner event for the reducer.
        const { schema_version, seq, ...innerEvent } = envelope
        for (const cb of this.listeners) cb(innerEvent as TranscriptEvent)
      },
    )
  }

  onEvent(cb: (event: TranscriptEvent) => void): () => void {
    this.listeners.add(cb)
    return () => this.listeners.delete(cb)
  }

  async sendPrompt(text: string): Promise<void> {
    await this.readyPromise
    if (!this.invokeFn) throw new Error("transport not ready")
    await this.invokeFn("send_prompt", { sessionId: this.sessionId, text })
  }

  async interrupt(): Promise<void> {
    await this.readyPromise
    if (!this.invokeFn) throw new Error("transport not ready")
    await this.invokeFn("interrupt_session", { sessionId: this.sessionId })
  }

  async respondToPermission(id: string, approved: boolean): Promise<void> {
    await this.readyPromise
    if (!this.invokeFn) throw new Error("transport not ready")
    await this.invokeFn("respond_permission", { sessionId: this.sessionId, id, approved })
  }

  async ready(): Promise<void> {
    return this.readyPromise
  }

  dispose(): void {
    this.listeners.clear()
    this.unlisten?.()
    this.unlisten = null
  }
}

/** Create the transport appropriate for the current environment. */
export function createTransport(
  sessionId: string,
  onResync?: () => void,
): AgentTransport {
  return isTauri
    ? new TauriTransport(sessionId)
    : new WebSocketTransport(sessionId, onResync)
}
