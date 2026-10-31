// ── Multi-session WebSocket agent transport hook ─────────────────────────
//
// Manages multiple WebSocket connections simultaneously, one per active
// session. Each connection independently processes events into its own turn
// list. Connections are created/removed as the sessionIds array changes.

import { useCallback, useEffect, useMemo, useRef } from "react"
import { appendUserTurn, applyEvent, historyToTurns } from "./reducer"
import type {
  HistoryMessage,
  PromptFile,
  PromptImage,
  TranscriptEvent,
  TranscriptTurn,
} from "./types"
import { getApiBaseUrl } from "@/stores/data"

export enum ReadyState {
  UNINSTANTIATED = -1,
  CONNECTING = 0,
  OPEN = 1,
  CLOSING = 2,
  CLOSED = 3,
}

function wsBaseUrl(): string {
  const base = getApiBaseUrl()
  if (base) {
    return base.replace(/^http:/, "ws:").replace(/^https:/, "wss:")
  }
  return `${window.location.protocol === "https:" ? "wss" : "ws"}://${window.location.host}`
}

// ── Internal connection state ────────────────────────────────────────────

interface ConnectionState {
  ws: WebSocket | null
  turns: TranscriptTurn[]
  readyState: ReadyState
  reconnectCount: number
  reconnectTimer: ReturnType<typeof setTimeout> | null
  readyResolve: (() => void) | null
  /** Highest event `seq` received from the server. 0 = cold start (no dedup).
   *  Persisted across reconnects (the same `conn` is reused) so the WS URL
   *  carries `lastSeq` on reconnect and the server replays the gap. */
  lastSeq: number
  /** Monotonic revision of `turns`; stale REST history snapshots must not
   *  overwrite optimistic user turns or newer live events. */
  turnRevision: number
  /** Latest history request; older responses are ignored even when the turn
   *  revision did not change between overlapping requests. */
  historyRequestSeq: number
}

function createConnectionState(): ConnectionState {
  return {
    ws: null,
    turns: [],
    readyState: ReadyState.UNINSTANTIATED,
    reconnectCount: 0,
    reconnectTimer: null,
    readyResolve: null,
    lastSeq: 0,
    turnRevision: 0,
    historyRequestSeq: 0,
  }
}

function teardownConnection(conn: ConnectionState) {
  if (conn.ws) {
    conn.ws.onopen = null
    conn.ws.onclose = null
    conn.ws.onmessage = null
    conn.ws.onerror = null
    if (conn.ws.readyState === WebSocket.OPEN || conn.ws.readyState === WebSocket.CONNECTING) {
      conn.ws.close()
    }
    conn.ws = null
  }
  if (conn.reconnectTimer) {
    clearTimeout(conn.reconnectTimer)
    conn.reconnectTimer = null
  }
  // Resolve any pending waitForReady promise so callers don't hang forever.
  conn.readyResolve?.()
  conn.readyResolve = null
  conn.readyState = ReadyState.CLOSED
}

// ── Hook ─────────────────────────────────────────────────────────────────

export interface UseMultiAgentWebSocketOptions {
  /** Called when any session's turns are updated */
  onTurnsUpdate?: (sessionId: string, turns: TranscriptTurn[]) => void
  /** Called when a terminal event (agent_end / notice) is received for a session */
  onTerminal?: (sessionId: string) => void
}

export function useMultiAgentWebSocket(
  sessionIds: string[],
  options: UseMultiAgentWebSocketOptions = {}
) {
  const connectionsRef = useRef<Map<string, ConnectionState>>(new Map())
  const optionsRef = useRef(options)
  const mountedRef = useRef(true)

  // Keep options ref current
  useEffect(() => {
    optionsRef.current = options
  }, [options])

  // Track previous sessionIds to detect actual changes.
  const prevSessionIdsRef = useRef<string[]>([])

  // ── History loading ────────────────────────────────────────────────────

  const fetchHistory = useCallback(async (sid: string, conn: ConnectionState) => {
    try {
      const requestSeq = ++conn.historyRequestSeq
      const revisionAtRequest = conn.turnRevision
      const base = getApiBaseUrl()
      const res = await fetch(`${base}/api/ai/sessions/${sid}/messages`)
      if (!res.ok) return
      const msgs: HistoryMessage[] = await res.json()
      if (!msgs || msgs.length === 0) return
      if (requestSeq !== conn.historyRequestSeq || conn.turnRevision !== revisionAtRequest) {
        return
      }
      // Never overwrite a turn list that has an in-progress (streaming) turn —
      // live data always takes precedence over a stale history snapshot.
      // This matters when switching to a session that is still streaming in
      // the background: the live turns are authoritative until the turn ends.
      if (conn.turns.some((t) => t.status === "in_progress")) return
      const turns = historyToTurns(msgs)
      conn.turns = turns
      conn.turnRevision++
      // Only notify if the connection is still alive and hasn't been
      // replaced by a newer connection.
      if (mountedRef.current) {
        optionsRef.current.onTurnsUpdate?.(sid, [...turns])
      }
    } catch {
      // History loading is best-effort; live events will still arrive.
    }
  }, [])

  // `connectSession` reconnects itself from inside `onclose`, so keep a ref to
  // the latest stable callback instead of putting it in its own dependency list.
  const connectSessionRef = useRef<(sid: string, conn: ConnectionState) => void>(() => {})

  const connectSession = useCallback(
    (sid: string, conn: ConnectionState) => {
      if (!mountedRef.current) return

      const lastSeqParam = conn.lastSeq > 0 ? `&lastSeq=${conn.lastSeq}` : ""
      const url = `${wsBaseUrl()}/api/ai/sessions/ws?id=${sid}${lastSeqParam}`
      conn.readyState = ReadyState.CONNECTING

      const ws = new WebSocket(url)
      conn.ws = ws

      ws.onopen = () => {
        if (!mountedRef.current || ws !== conn.ws) return
        conn.readyState = ReadyState.OPEN
        conn.reconnectCount = 0
        conn.readyResolve?.()
        conn.readyResolve = null
        // Cold start (lastSeq===0): load full history via REST, then accept all
        // live events (dedup off). Reconnect (lastSeq>0): the server replays
        // the gap from its ring buffer, so skip fetchHistory and dedup by seq.
        if (conn.lastSeq === 0) {
          fetchHistory(sid, conn)
        }
      }

      ws.onclose = () => {
        if (!mountedRef.current || ws !== conn.ws) return
        conn.readyState = ReadyState.CLOSED

        // Auto-reconnect with exponential backoff
        const maxAttempts = 20
        if (conn.reconnectCount < maxAttempts) {
          const delay = Math.min(1000 * Math.pow(1.5, conn.reconnectCount), 30000)
          conn.reconnectCount++
          // Clear any previous timer before setting a new one (prevents duplicate
          // reconnects when onclose fires multiple times).
          if (conn.reconnectTimer) clearTimeout(conn.reconnectTimer)
          conn.reconnectTimer = setTimeout(() => {
            // Check ws !== conn.ws: if the connection was torn down and recreated
            // during the delay, this stale timer must not start a new connection.
            if (mountedRef.current && ws === conn.ws) connectSessionRef.current(sid, conn)
          }, delay)
        }
      }

      ws.onerror = () => {
        // onclose will fire after this
      }

      ws.onmessage = (event: MessageEvent) => {
        if (!mountedRef.current || ws !== conn.ws) return
        try {
          const parsed = JSON.parse(event.data) as Record<string, unknown>
          // Resync signal: gap too large to replay — reset and reload full
          // history. Advance lastSeq past this signal and bail.
          if (parsed.kind === "session" && parsed.type === "resync_required") {
            conn.turns = []
            conn.turnRevision++
            optionsRef.current.onTurnsUpdate?.(sid, [])
            fetchHistory(sid, conn)
            const seq = Number(parsed.seq)
            if (Number.isFinite(seq) && seq > 0) conn.lastSeq = seq
            return
          }
          // Dedup by seq: drop events already seen (overlap of replay + live,
          // or a duplicate after reconnect). seq=0 (out-of-band, e.g. session
          // create-failure) is always accepted — it bypasses dedup.
          const seq = Number(parsed.seq)
          if (Number.isFinite(seq) && seq > 0) {
            if (seq <= conn.lastSeq) return
            conn.lastSeq = seq
          }
          const prev = conn.turns
          const { turns: newTurns, terminal } = applyEvent(prev, parsed as TranscriptEvent)
          conn.turns = newTurns
          conn.turnRevision++
          optionsRef.current.onTurnsUpdate?.(sid, [...newTurns])
          if (terminal) {
            optionsRef.current.onTerminal?.(sid)
          }
        } catch {
          // Ignore parse errors for malformed messages
        }
      }
    },
    [fetchHistory]
  )

  useEffect(() => {
    connectSessionRef.current = connectSession
  }, [connectSession])

  // Sync connections when sessionIds actually change.
  //
  // IMPORTANT: This effect has NO cleanup function. The cleanup would run on
  // every dependency change and tear down ALL connections before the new
  // effect runs, even when the quick check below decides nothing changed.
  // That caused mass disconnects of all sessions on every re-render where
  // the sessionIds array reference changed but contents were identical.
  //
  // Unmount-only teardown is handled by a separate useEffect([]) below.
  useEffect(() => {
    mountedRef.current = true
    const prev = prevSessionIdsRef.current
    prevSessionIdsRef.current = sessionIds

    const connections = connectionsRef.current
    const desired = new Set(sessionIds)
    const previous = new Set(prev)

    // Quick check: if nothing changed, skip entirely.
    if (desired.size === previous.size) {
      let same = true
      for (const sid of desired) {
        if (!previous.has(sid)) {
          same = false
          break
        }
      }
      if (same) return
    }

    // Disconnect sessions no longer in the list
    for (const [sid, conn] of connections) {
      if (!desired.has(sid)) {
        teardownConnection(conn)
        connections.delete(sid)
      }
    }

    // Connect new sessions
    for (const sid of desired) {
      if (!connections.has(sid)) {
        const conn = createConnectionState()
        connections.set(sid, conn)
        connectSession(sid, conn)
      }
    }
  }, [sessionIds, connectSession])

  // Unmount-only cleanup: tear down all connections when the component
  // unmounts. This is a separate effect with [] deps so it only runs once
  // on mount and cleans up on unmount — never on intermediate re-renders.
  useEffect(() => {
    // Capture the map once so the cleanup closes over the same instance even
    // if the ref is reassigned during teardown.
    const connections = connectionsRef.current
    return () => {
      mountedRef.current = false
      for (const conn of connections.values()) {
        teardownConnection(conn)
      }
      connections.clear()
    }
  }, [])

  // ── Connection lifecycle helpers ──────────────────────────────────────

  // ── Public API ────────────────────────────────────────────────────────

  // Create the connection on demand if it's missing (e.g. sendPrompt raced
  // ahead of the sessionIds effect, or the connection was torn down). Makes
  // the send path self-healing instead of throwing "Session not found".
  const ensureConnection = useCallback(
    (sid: string) => {
      let conn = connectionsRef.current.get(sid)
      if (!conn) {
        conn = createConnectionState()
        connectionsRef.current.set(sid, conn)
        connectSession(sid, conn)
      }
      return conn
    },
    [connectSession]
  )

  const waitForReady = useCallback(async (sid: string): Promise<void> => {
    // Poll until the connection exists (may not be created yet if the
    // caller just triggered a state update that will cause a re-render
    // and subsequent connection creation via the useEffect).
    let conn = connectionsRef.current.get(sid)
    if (!conn) {
      for (let i = 0; i < 50; i++) {
        await new Promise((r) => setTimeout(r, 50))
        conn = connectionsRef.current.get(sid)
        if (conn) break
      }
      if (!conn) {
        throw new Error(`Session ${sid} not found`)
      }
    }
    if (conn.readyState === ReadyState.OPEN) return
    return new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => {
        if (conn.readyResolve === resolve) conn.readyResolve = null
        reject(new Error(`Session ${sid} 连接超时`))
      }, 10000)
      conn.readyResolve = () => {
        clearTimeout(timer)
        resolve()
      }
    })
  }, [])

  const sendPrompt = useCallback(
    async (
      sid: string,
      text: string,
      images?: PromptImage[],
      files?: PromptFile[]
    ): Promise<void> => {
      ensureConnection(sid)
      await waitForReady(sid)
      const conn = connectionsRef.current.get(sid)
      // socket 在 ready 之后又被关掉时不能静默返回：调用方会以为消息发出去了，
      // 输入框已清空、聊天记录里也插了一条「已发送」，而服务端什么都没收到。
      if (!conn?.ws || conn.ws.readyState !== WebSocket.OPEN) {
        throw new Error("连接已断开")
      }
      conn.ws.send(
        JSON.stringify({ type: "prompt", text, images: images ?? [], files: files ?? [] })
      )
      // The connection owns the transcript used by live events. Append the
      // optimistic user turn here so the first assistant delta cannot replace
      // a user message that only existed in React component state.
      conn.turns = appendUserTurn(conn.turns, text, images ?? [], files ?? [])
      conn.turnRevision++
      optionsRef.current.onTurnsUpdate?.(sid, [...conn.turns])
    },
    [ensureConnection, waitForReady]
  )

  const interrupt = useCallback(
    async (sid: string): Promise<void> => {
      ensureConnection(sid)
      await waitForReady(sid)
      const conn = connectionsRef.current.get(sid)
      if (conn?.ws && conn.ws.readyState === WebSocket.OPEN) {
        conn.ws.send(JSON.stringify({ type: "interrupt" }))
      }
    },
    [ensureConnection, waitForReady]
  )

  const respondToPermission = useCallback(
    async (sid: string, id: string, approved: boolean): Promise<void> => {
      ensureConnection(sid)
      await waitForReady(sid)
      const conn = connectionsRef.current.get(sid)
      if (conn?.ws && conn.ws.readyState === WebSocket.OPEN) {
        conn.ws.send(JSON.stringify({ type: "permission_response", id, approved }))
      }
    },
    [ensureConnection, waitForReady]
  )

  const getTurns = useCallback((sid: string): TranscriptTurn[] => {
    return connectionsRef.current.get(sid)?.turns ?? []
  }, [])

  const isReady = useCallback((sid: string): boolean => {
    return connectionsRef.current.get(sid)?.readyState === ReadyState.OPEN
  }, [])

  /** Explicitly load history messages for a session from the REST API.
   *  Useful when switching to an already-connected session whose history
   *  was loaded only once during WebSocket onopen. */
  const loadHistory = useCallback(
    async (sid: string): Promise<void> => {
      const conn = connectionsRef.current.get(sid)
      if (!conn) return
      await fetchHistory(sid, conn)
    },
    [fetchHistory]
  )

  return useMemo(
    () => ({
      sendPrompt,
      interrupt,
      respondToPermission,
      getTurns,
      isReady,
      loadHistory,
    }),
    [sendPrompt, interrupt, respondToPermission, getTurns, isReady, loadHistory]
  )
}
