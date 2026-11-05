// ── Per-session transport registry ───────────────────────────────────────
//
// Desktop used to keep a single transport: starting a prompt in another
// session disposed the previous one, silently killing the background stream.
// This registry keeps one transport per session and only releases the session
// that reached its terminal event (or the whole registry on unmount).

export interface Disposable {
  dispose(): void
}

export class TransportRegistry<T extends Disposable> {
  private transports = new Map<string, T>()

  get(sessionId: string): T | undefined {
    return this.transports.get(sessionId)
  }

  /** Return the session's live transport, creating it on first use. Creating
   *  B never touches A: background streams must keep their listeners. */
  getOrCreate(sessionId: string, create: () => T): T {
    const existing = this.transports.get(sessionId)
    if (existing) return existing
    const created = create()
    this.transports.set(sessionId, created)
    return created
  }

  /** Drop and dispose one session's transport. */
  release(sessionId: string): void {
    const transport = this.transports.get(sessionId)
    if (!transport) return
    this.transports.delete(sessionId)
    transport.dispose()
  }

  /** Tear down every transport (component unmount). */
  disposeAll(): void {
    for (const transport of this.transports.values()) transport.dispose()
    this.transports.clear()
  }
}
