import { create } from 'zustand'

const isTauri = '__TAURI_INTERNALS__' in window

export function getApiBaseUrl(): string {
  if (isTauri) return ''
  return localStorage.getItem('trading-agent-api-url') || __API_BASE_URL__
}

async function fetchJson<T>(path: string, init?: RequestInit): Promise<T> {
  if (isTauri) {
    const { invoke } = await import('@tauri-apps/api/core')
    if (init?.method === 'POST' || init?.method === 'PUT' || init?.method === 'DELETE') {
      let body: Record<string, unknown> = {}
      if (init.body) {
        try { body = JSON.parse(String(init.body)) } catch { /* use empty */ }
      }
      return invoke('api_post', { path, body }) as Promise<T>
    }
    return invoke('api_fetch', { path }) as Promise<T>
  }
  const base = getApiBaseUrl()
  const headers: Record<string, string> = {}
  if (init?.body) headers['Content-Type'] = 'application/json'
  const resp = await fetch(`${base}${path}`, { ...init, headers: { ...headers, ...(init?.headers as Record<string, string>) } })
  if (!resp.ok) throw new Error(`${resp.status}`)
  return resp.json()
}

export async function apiFetch(path: string, init?: RequestInit): Promise<Response> {
  if (isTauri) {
    const { invoke } = await import('@tauri-apps/api/core')
    if (init?.method === 'POST' || init?.method === 'PUT' || init?.method === 'DELETE') {
      const body = init.body ? JSON.parse(String(init.body)) : {}
      const data = await invoke('api_post', { path, body })
      return new Response(JSON.stringify(data), { headers: { 'content-type': 'application/json' } })
    }
    const data = await invoke('api_fetch', { path })
    return new Response(JSON.stringify(data), { headers: { 'content-type': 'application/json' } })
  }
  const base = getApiBaseUrl()
  return fetch(`${base}${path}`, init)
}

export interface ChatStreamCallbacks {
  onData: (text: string) => void
  onDone: () => void
  onError: (err: string) => void
}

export async function apiChatStream(
  body: { message: string; role: string; history: Array<{ role: string; content: string }> },
  callbacks: ChatStreamCallbacks,
): Promise<void> {
  if (isTauri) {
    const { invoke, Channel } = await import('@tauri-apps/api/core')
    const channel = new Channel<string>()
    let chunkCount = 0
    channel.onmessage = (text) => {
      chunkCount++
      if (chunkCount <= 3 || chunkCount % 20 === 0) {
        console.log(`[chat_stream] chunk #${chunkCount}:`, text.slice(0, 80))
      }
      callbacks.onData(text.replace(/\\n/g, '\n'))
    }
    console.log('[chat_stream] invoking api_chat_stream, body=', body)
    try {
      await invoke('api_chat_stream', { body, channel })
      console.log(`[chat_stream] invoke resolved, total chunks=${chunkCount}`)
      callbacks.onDone()
    } catch (e: unknown) {
      console.error('[chat_stream] invoke rejected:', e)
      callbacks.onError(String(e))
    }
    return
  }
  const base = getApiBaseUrl()
  const resp = await fetch(`${base}/api/ai/chat`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  })
  if (!resp.ok) {
    callbacks.onError(`HTTP ${resp.status}`)
    return
  }
  const reader = resp.body?.getReader()
  if (!reader) {
    callbacks.onError('无法读取响应流')
    return
  }
  const decoder = new TextDecoder()
  let buffer = ''
  while (true) {
    const { done, value } = await reader.read()
    if (done) break
    buffer += decoder.decode(value, { stream: true })
    const lines = buffer.split('\n')
    buffer = lines.pop() || ''
    for (const line of lines) {
      if (line.startsWith('data: ')) {
        const data = line.slice(6)
        if (data === '[DONE]') continue
        if (data.startsWith('[ERROR]')) {
          callbacks.onError(data)
        } else {
          callbacks.onData(data.replace(/\\n/g, '\n'))
        }
      }
    }
  }
  callbacks.onDone()
}

function todayStr(): string {
  return new Date().toISOString().split('T')[0]
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
      }
    }
  }>
}

export interface RoleEntry {
  name: string
  display_name: string
  strategy: string
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

// ── Store ──────────────────────────────────────────────

interface DataState {
  overview: { data: OverviewData | null; loading: boolean; error: string }
  limitUp: { data: Stock[]; loading: boolean; error: string }
  fieldItems: { data: FieldPlate[]; loading: boolean; error: string; date: string }
  plateAbnormal: { data: unknown[]; loading: boolean; error: string }
  timeline: { data: unknown[]; loading: boolean; error: string }
  superStock: { data: Stock[]; loading: boolean; error: string }
  dapanAbnormal: { data: unknown[]; loading: boolean; error: string }
  sentiment: { data: unknown; loading: boolean; error: string }
  roles: { data: RoleEntry[]; loading: boolean; error: string }
  rolePositions: Record<string, { data: RolePosition[]; loading: boolean; error: string }>
  roleLogs: Record<string, { data: RoleLog[]; loading: boolean; error: string }>
  rolePnlHistory: Record<string, { data: PnlPoint[]; loading: boolean; error: string }>
  roleHistory: Record<string, { data: ClosedPosition[]; loading: boolean; error: string }>
  indexQuotes: { data: IndexQuote[]; loading: boolean; error: string }
  sentimentHistory: { data: SentimentHistoryDay[]; loading: boolean; error: string }

  fetchOverview: () => Promise<void>
  fetchLimitUp: () => Promise<void>
  fetchFieldItems: (date?: string) => Promise<void>
  fetchPlateAbnormal: () => Promise<void>
  fetchTimeline: () => Promise<void>
  fetchSuperStock: () => Promise<void>
  fetchDapanAbnormal: () => Promise<void>
  fetchSentiment: () => Promise<void>
  fetchRoles: () => Promise<void>
  fetchRolePositions: (name: string) => Promise<void>
  fetchRoleLogs: (name: string) => Promise<void>
  fetchRolePnlHistory: (name: string, days?: number) => Promise<void>
  fetchRoleHistory: (name: string, limit?: number) => Promise<void>
  fetchIndexQuotes: () => Promise<void>
  fetchSentimentHistory: (days?: number) => Promise<void>
  createRole: (name: string, displayName: string, strategy: string) => Promise<void>
  updateRoleStrategy: (name: string, strategy: string) => Promise<void>
  deleteRole: (name: string) => Promise<void>
  resetRole: (name: string) => Promise<void>
  refreshRole: (name: string) => Promise<void>
}

export const useDataStore = create<DataState>((set, get) => ({
  overview: { data: null, loading: false, error: '' },
  limitUp: { data: [], loading: false, error: '' },
  fieldItems: { data: [], loading: false, error: '', date: todayStr() },
  plateAbnormal: { data: [], loading: false, error: '' },
  timeline: { data: [], loading: false, error: '' },
  superStock: { data: [], loading: false, error: '' },
  dapanAbnormal: { data: [], loading: false, error: '' },
  sentiment: { data: null, loading: false, error: '' },
  roles: { data: [], loading: false, error: '' },
  rolePositions: {},
  roleLogs: {},
  rolePnlHistory: {},
  roleHistory: {},
  indexQuotes: { data: [], loading: false, error: '' },
  sentimentHistory: { data: [], loading: false, error: '' },

  fetchOverview: async () => {
    set((s) => ({ overview: { ...s.overview, loading: true, error: '' } }))
    try {
      const data = await fetchJson<OverviewData>('/api/overview')
      set({ overview: { data, loading: false, error: '' } })
    } catch (e: unknown) {
      set((s) => ({ overview: { ...s.overview, loading: false, error: (e as Error).message || String(e) } }))
    }
  },

  fetchLimitUp: async () => {
    set((s) => ({ limitUp: { ...s.limitUp, loading: true, error: '' } }))
    try {
      const data = await fetchJson<Stock[]>('/api/limit-up')
      set({ limitUp: { data, loading: false, error: '' } })
    } catch (e: unknown) {
      set((s) => ({ limitUp: { ...s.limitUp, loading: false, error: (e as Error).message || String(e) } }))
    }
  },

  fetchFieldItems: async (date?: string) => {
    set((s) => ({ fieldItems: { ...s.fieldItems, loading: true, error: '' } }))
    const d = date || get().fieldItems.date
    try {
      const data = await fetchJson<FieldPlate[]>(`/api/field-items?date=${d}`)
      set({ fieldItems: { data, loading: false, error: '', date: d } })
    } catch (e: unknown) {
      set((s) => ({ fieldItems: { ...s.fieldItems, loading: false, error: (e as Error).message || String(e) } }))
    }
  },

  fetchPlateAbnormal: async () => {
    set((s) => ({ plateAbnormal: { ...s.plateAbnormal, loading: true, error: '' } }))
    try {
      const data = await fetchJson<unknown[]>('/api/plate-abnormal')
      set({ plateAbnormal: { data, loading: false, error: '' } })
    } catch (e: unknown) {
      set((s) => ({ plateAbnormal: { ...s.plateAbnormal, loading: false, error: (e as Error).message || String(e) } }))
    }
  },

  fetchTimeline: async () => {
    set((s) => ({ timeline: { ...s.timeline, loading: true, error: '' } }))
    try {
      const data = await fetchJson<unknown[]>('/api/timeline')
      set({ timeline: { data, loading: false, error: '' } })
    } catch (e: unknown) {
      set((s) => ({ timeline: { ...s.timeline, loading: false, error: (e as Error).message || String(e) } }))
    }
  },

  fetchSuperStock: async () => {
    set((s) => ({ superStock: { ...s.superStock, loading: true, error: '' } }))
    try {
      const data = await fetchJson<Stock[]>('/api/super-stock')
      set({ superStock: { data, loading: false, error: '' } })
    } catch (e: unknown) {
      set((s) => ({ superStock: { ...s.superStock, loading: false, error: (e as Error).message || String(e) } }))
    }
  },

  fetchDapanAbnormal: async () => {
    set((s) => ({ dapanAbnormal: { ...s.dapanAbnormal, loading: true, error: '' } }))
    try {
      const data = await fetchJson<unknown[]>('/api/dapan-abnormal')
      set({ dapanAbnormal: { data, loading: false, error: '' } })
    } catch (e: unknown) {
      set((s) => ({ dapanAbnormal: { ...s.dapanAbnormal, loading: false, error: (e as Error).message || String(e) } }))
    }
  },

  fetchSentiment: async () => {
    set((s) => ({ sentiment: { ...s.sentiment, loading: true, error: '' } }))
    try {
      const data = await fetchJson<unknown>('/api/sentiment')
      set({ sentiment: { data, loading: false, error: '' } })
    } catch (e: unknown) {
      set((s) => ({ sentiment: { ...s.sentiment, loading: false, error: (e as Error).message || String(e) } }))
    }
  },

  fetchRoles: async () => {
    set((s) => ({ roles: { ...s.roles, loading: true, error: '' } }))
    try {
      const data = await fetchJson<RoleEntry[]>('/api/roles')
      set({ roles: { data, loading: false, error: '' } })
    } catch (e: unknown) {
      set((s) => ({ roles: { ...s.roles, loading: false, error: (e as Error).message || String(e) } }))
    }
  },

  fetchRolePositions: async (name: string) => {
    set((s) => ({
      rolePositions: {
        ...s.rolePositions,
        [name]: { data: s.rolePositions[name]?.data || [], loading: true, error: '' },
      },
    }))
    try {
      const data = await fetchJson<RolePosition[]>(`/api/roles/${name}/positions`)
      set((s) => ({
        rolePositions: { ...s.rolePositions, [name]: { data, loading: false, error: '' } },
      }))
    } catch (e: unknown) {
      set((s) => ({
        rolePositions: {
          ...s.rolePositions,
          [name]: { ...s.rolePositions[name], loading: false, error: (e as Error).message || String(e) },
        },
      }))
    }
  },

  fetchRoleLogs: async (name: string) => {
    set((s) => ({
      roleLogs: {
        ...s.roleLogs,
        [name]: { data: s.roleLogs[name]?.data || [], loading: true, error: '' },
      },
    }))
    try {
      const data = await fetchJson<RoleLog[]>(`/api/roles/${name}/logs`)
      set((s) => ({
        roleLogs: { ...s.roleLogs, [name]: { data, loading: false, error: '' } },
      }))
    } catch (e: unknown) {
      set((s) => ({
        roleLogs: {
          ...s.roleLogs,
          [name]: { ...s.roleLogs[name], loading: false, error: (e as Error).message || String(e) },
        },
      }))
    }
  },

  fetchRolePnlHistory: async (name: string, days = 30) => {
    set((s) => ({
      rolePnlHistory: {
        ...s.rolePnlHistory,
        [name]: { data: s.rolePnlHistory[name]?.data || [], loading: true, error: '' },
      },
    }))
    try {
      const res = await fetchJson<PnlHistoryResponse>(`/api/roles/${name}/pnl-history?days=${days}`)
      set((s) => ({
        rolePnlHistory: { ...s.rolePnlHistory, [name]: { data: res.points, loading: false, error: '' } },
      }))
    } catch (e: unknown) {
      set((s) => ({
        rolePnlHistory: {
          ...s.rolePnlHistory,
          [name]: { ...s.rolePnlHistory[name], loading: false, error: (e as Error).message || String(e) },
        },
      }))
    }
  },

  fetchRoleHistory: async (name: string, limit = 100) => {
    set((s) => ({
      roleHistory: {
        ...s.roleHistory,
        [name]: { data: s.roleHistory[name]?.data || [], loading: true, error: '' },
      },
    }))
    try {
      const data = await fetchJson<ClosedPosition[]>(`/api/roles/${name}/history?limit=${limit}`)
      set((s) => ({
        roleHistory: { ...s.roleHistory, [name]: { data, loading: false, error: '' } },
      }))
    } catch (e: unknown) {
      set((s) => ({
        roleHistory: {
          ...s.roleHistory,
          [name]: { ...s.roleHistory[name], loading: false, error: (e as Error).message || String(e) },
        },
      }))
    }
  },

  fetchIndexQuotes: async () => {
    set((s) => ({ indexQuotes: { ...s.indexQuotes, loading: true, error: '' } }))
    try {
      const data = await fetchJson<IndexQuote[]>('/api/index-quotes')
      set({ indexQuotes: { data, loading: false, error: '' } })
    } catch (e: unknown) {
      set((s) => ({ indexQuotes: { ...s.indexQuotes, loading: false, error: (e as Error).message || String(e) } }))
    }
  },

  fetchSentimentHistory: async (days = 10) => {
    set((s) => ({ sentimentHistory: { ...s.sentimentHistory, loading: true, error: '' } }))
    try {
      const data = await fetchJson<SentimentHistoryDay[]>(`/api/sentiment-history?days=${days}`)
      set({ sentimentHistory: { data, loading: false, error: '' } })
    } catch (e: unknown) {
      set((s) => ({ sentimentHistory: { ...s.sentimentHistory, loading: false, error: (e as Error).message || String(e) } }))
    }
  },

  createRole: async (name: string, displayName: string, strategy: string) => {
    await fetchJson('/api/roles', {
      method: 'POST',
      body: JSON.stringify({ name, display_name: displayName, strategy }),
    })
  },

  updateRoleStrategy: async (name: string, strategy: string) => {
    await fetchJson(`/api/roles/${name}/strategy`, {
      method: 'PUT',
      body: JSON.stringify({ strategy }),
    })
  },

  deleteRole: async (name: string) => {
    await fetchJson(`/api/roles/${name}`, { method: 'DELETE' })
  },

  resetRole: async (name: string) => {
    await fetchJson(`/api/roles/${name}/reset`, { method: 'POST' })
  },

  refreshRole: async (name: string) => {
    const state = get()
    await state.fetchRolePositions(name)
    await state.fetchRoleLogs(name)
    await state.fetchRolePnlHistory(name)
  },
}))