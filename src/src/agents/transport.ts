// ── Agent transport abstraction (docs/agentTransport.md §4, §9) ────────────
//
// Web and Tauri share the same `AgentTransport` interface; only the concrete
// implementation differs. The frontend reducer/UI never branches on transport
// kind — they only call `onEvent` / `sendPrompt` / `interrupt`.
//
// ## Web path
// The Web path uses the `useMultiAgentWebSocket` React hook (wrapping native
// WebSocket per session) instead of a class-based transport.
// See `useMultiAgentWebSocket.ts`.
//
// ## Tauri path (desktop)
// `TauriTransport` uses Tauri IPC (`invoke` / `listen`) to communicate with the
// Rust backend. It implements the same `AgentTransport` interface.

import type { PromptFile, PromptImage, TranscriptEvent } from "./types"

export interface AgentTransport {
  /** Register an event listener. Returns an unsubscribe function. */
  onEvent(cb: (event: TranscriptEvent) => void): () => void
  /** Send a user prompt (or slash command as raw text) to the agent. */
  sendPrompt(text: string, images?: PromptImage[], files?: PromptFile[]): Promise<void>
  /** Interrupt the running agent turn. */
  interrupt(): Promise<void>
  /** Respond to a permission request (§11). */
  respondToPermission(id: string, approved: boolean): Promise<void>
  /** Tear down the connection / listeners. */
  dispose(): void
  /** Resolves once the transport is ready to receive `sendPrompt`. */
  ready(): Promise<void>
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
    this.unlisten = await listen<{
      session_id: string
      event: { schema_version: number; seq: number | null; kind: string; [k: string]: unknown }
    }>("agent-event", (e) => {
      if (e.payload.session_id !== this.sessionId) return
      const envelope = e.payload.event
      // The event field is the EventEnvelope with flattened TranscriptEvent.
      // Strip the envelope fields to get the inner event for the reducer.
      const { schema_version: _schema_version, seq: _seq, ...innerEvent } = envelope
      for (const cb of this.listeners) cb(innerEvent as TranscriptEvent)
    })
  }

  onEvent(cb: (event: TranscriptEvent) => void): () => void {
    this.listeners.add(cb)
    return () => this.listeners.delete(cb)
  }

  async sendPrompt(text: string, images?: PromptImage[], files?: PromptFile[]): Promise<void> {
    await this.readyPromise
    if (!this.invokeFn) throw new Error("transport not ready")
    await this.invokeFn("send_prompt", {
      sessionId: this.sessionId,
      text,
      images: images ?? [],
      files: files ?? [],
    })
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
