export const overviewKeys = {
  all: ["overview"] as const,
}

export const limitUpKeys = {
  all: ["limit-up"] as const,
}

export const fieldItemsKeys = {
  all: ["field-items"] as const,
  byDate: (date: string) => ["field-items", date] as const,
}

export const plateAbnormalKeys = {
  all: ["plate-abnormal"] as const,
}

export const timelineKeys = {
  all: ["timeline"] as const,
}

export const superStockKeys = {
  all: ["super-stock"] as const,
}

export const dapanAbnormalKeys = {
  all: ["dapan-abnormal"] as const,
}

export const sentimentKeys = {
  all: ["sentiment"] as const,
}

export const indexQuotesKeys = {
  all: ["index-quotes"] as const,
}

export const sentimentHistoryKeys = {
  all: ["sentiment-history"] as const,
  byDays: (days: number) => ["sentiment-history", days] as const,
}

export const yesterdayLimitUpKeys = {
  all: ["yesterday-limit-up"] as const,
}

export const dragonTigerKeys = {
  detail: (page: number, pageSize: number) => ["dragon-tiger", "detail", page, pageSize] as const,
  stockSeat: (code: string, date: string, flag: string) =>
    ["dragon-tiger", "stock-seat", code, date, flag] as const,
}

export const replayKeys = {
  all: ["replay"] as const,
  events: (minPct?: number) => [...replayKeys.all, "events", minPct ?? 2] as const,
}

export const telegraphKeys = {
  all: ["telegraph"] as const,
}

export const aiRolesKeys = {
  all: ["ai", "roles"] as const,
}

export const realtimeKeys = {
  bySymbol: (symbol: string) => ["realtime", symbol] as const,
}

export const platesByStockKeys = {
  bySymbol: (symbol: string) => ["plates-by-stock", symbol] as const,
}

/**
 * 尾部带可选参数（limit / days）的 key，失效时必须用对应的 `*Scope(name)` 前缀，
 * 不能复用 `roleKeys.xxx(name)`：省略参数会留下一个 `undefined` 尾元素，
 * react-query 的前缀匹配逐元素比较，`undefined` 不等于实际值，查询不会被失效。
 */
const roleTradesScope = (name: string) => ["roles", "trades", name] as const
const roleHistoryScope = (name: string) => ["roles", "history", name] as const
const rolePnlHistoryScope = (name: string) => ["roles", "pnl-history", name] as const

export const roleKeys = {
  all: ["roles"] as const,
  list: () => [...roleKeys.all, "list"] as const,
  detail: (name: string) => [...roleKeys.all, "detail", name] as const,
  positions: (name: string) => [...roleKeys.all, "positions", name] as const,
  account: (name: string) => [...roleKeys.all, "account", name] as const,
  logs: (name: string) => [...roleKeys.all, "logs", name] as const,
  trades: (name: string, limit: number) => [...roleTradesScope(name), limit] as const,
  tradesScope: roleTradesScope,
  pnlHistory: (name: string, days: number) => [...rolePnlHistoryScope(name), days] as const,
  pnlHistoryScope: rolePnlHistoryScope,
  history: (name: string, limit: number) => [...roleHistoryScope(name), limit] as const,
  historyScope: roleHistoryScope,
}

export const watchlistKeys = {
  all: ["watchlist"] as const,
}

export const mcpKeys = {
  all: ["mcp-servers"] as const,
}

/** pi-rs 可选模型列表（`/api/agent/models`）。 */
export const agentModelsKeys = {
  all: ["agent-models"] as const,
}

/** provider 凭据状态（`/api/agent/auth`）。 */
export const agentAuthKeys = {
  all: ["agent-auth"] as const,
}
