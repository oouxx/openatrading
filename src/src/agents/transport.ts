// ── Agent transport abstraction (docs/agentTransport.md §4) ───────────────
//
// Web and Tauri share the same `AgentTransport` interface; only the concrete
// implementation differs. The frontend reducer/UI never branches on transport
// kind — they only call `onEvent` / `sendPrompt` / `interrupt`.

import { getApiBaseUrl } from "@/stores/data"
import type { ClientCommand, TranscriptEvent } from "./types"

export interface AgentTransport {
  /** Register an event listener. Returns an unsubscribe function. */
  onEvent(cb: (event: TranscriptEvent) => void): () => void
  /** Send a user prompt (or slash command as raw text) to the agent. */
  sendPrompt(text: string): Promise<void>
  /** Interrupt the running agent turn. */
  interrupt(): Promise<void>
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

// ── WebSocket transport (Web) ─────────────────────────────────────────────

export class WebSocketTransport implements AgentTransport {
  private ws: WebSocket
  private listeners = new Set<(event: TranscriptEvent) => void>()
  private readyPromise: Promise<void>
  private disposed = false

  constructor(sessionId: string) {
    const url = `${wsBaseUrl()}/api/ai/sessions/${sessionId}/ws`
    this.ws = new WebSocket(url)

    this.readyPromise = new Promise<void>((resolve, reject) => {
      const onOpen = () => {
        this.ws.removeEventListener("open", onOpen)
        this.ws.removeEventListener("error", onErr)
        resolve()
      }
      const onErr = () => {
        this.ws.removeEventListener("open", onOpen)
        this.ws.removeEventListener("error", onErr)
        reject(new Error("websocket 连接失败"))
      }
      this.ws.addEventListener("open", onOpen)
      this.ws.addEventListener("error", onErr)
    })

    this.ws.onmessage = (ev) => {
      let event: TranscriptEvent
      try {
        event = JSON.parse(typeof ev.data === "string" ? ev.data : "")
      } catch {
        return
      }
      for (const cb of this.listeners) cb(event)
    }
    this.ws.onclose = () => {
      // Surface a synthetic terminal event so the reducer can finalize.
      if (this.disposed) return
      for (const cb of this.listeners) {
        cb({ kind: "session", type: "agent_end", will_retry: false })
      }
    }
  }

  onEvent(cb: (event: TranscriptEvent) => void): () => void {
    this.listeners.add(cb)
    return () => this.listeners.delete(cb)
  }

  async sendPrompt(text: string): Promise<void> {
    await this.readyPromise
    sendCmd(this.ws, { type: "prompt", text })
  }

  async interrupt(): Promise<void> {
    await this.readyPromise
    sendCmd(this.ws, { type: "interrupt" })
  }

  async ready(): Promise<void> {
    return this.readyPromise
  }

  dispose(): void {
    this.disposed = true
    this.listeners.clear()
    try {
      this.ws.close()
    } catch {
      /* ignore */
    }
  }
}

// ── Tauri IPC transport (desktop) ─────────────────────────────────────────

export class TauriTransport implements AgentTransport {
  private listeners = new Set<(event: TranscriptEvent) => void>()
  private unlisten: (() => void) | null = null
  private readyPromise: Promise<void>
  private invokeFn: typeof import("@tauri-apps/api/core").invoke | null = null

  constructor(private sessionId: string) {
    this.readyPromise = this.init()
  }

  private async init(): Promise<void> {
    const { invoke } = await import("@tauri-apps/api/core")
    const { listen } = await import("@tauri-apps/api/event")
    this.invokeFn = invoke
    this.unlisten = await listen<{ session_id: string; event: TranscriptEvent }>(
      "agent-event",
      (e) => {
        if (e.payload.session_id !== this.sessionId) return
        for (const cb of this.listeners) cb(e.payload.event)
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
export function createTransport(sessionId: string): AgentTransport {
  return isTauri ? new TauriTransport(sessionId) : new WebSocketTransport(sessionId)
}
