export const overviewKeys = {
  all: ["overview"] as const,
};

export const limitUpKeys = {
  all: ["limit-up"] as const,
};

export const limitDownKeys = {
  all: ["limit-down"] as const,
};

export const fieldItemsKeys = {
  all: ["field-items"] as const,
  byDate: (date: string) => ["field-items", date] as const,
};

export const plateAbnormalKeys = {
  all: ["plate-abnormal"] as const,
};

export const timelineKeys = {
  all: ["timeline"] as const,
};

export const superStockKeys = {
  all: ["super-stock"] as const,
};

export const dapanAbnormalKeys = {
  all: ["dapan-abnormal"] as const,
};

export const sentimentKeys = {
  all: ["sentiment"] as const,
};

export const indexQuotesKeys = {
  all: ["index-quotes"] as const,
};

export const sentimentHistoryKeys = {
  all: ["sentiment-history"] as const,
  byDays: (days: number) => ["sentiment-history", days] as const,
};

export const yesterdayLimitUpKeys = {
  all: ["yesterday-limit-up"] as const,
};

export const dragonTigerKeys = {
  all: ["dragon-tiger"] as const,
};

export const replayKeys = {
  all: ["replay"] as const,
  events: () => [...replayKeys.all, "events"] as const,
};

export const telegraphKeys = {
  all: ["telegraph"] as const,
};

export const aiRolesKeys = {
  all: ["ai", "roles"] as const,
};

export const realtimeKeys = {
  bySymbol: (symbol: string) => ["realtime", symbol] as const,
};

export const platesByStockKeys = {
  bySymbol: (symbol: string) => ["plates-by-stock", symbol] as const,
};

export const roleKeys = {
  all: ["roles"] as const,
  list: () => [...roleKeys.all, "list"] as const,
  detail: (name: string) => [...roleKeys.all, "detail", name] as const,
  positions: (name: string) =>
    [...roleKeys.all, "positions", name] as const,
  logs: (name: string) => [...roleKeys.all, "logs", name] as const,
  pnlHistory: (name: string, days?: number) =>
    [...roleKeys.all, "pnl-history", name, days] as const,
  history: (name: string, limit?: number) =>
    [...roleKeys.all, "history", name, limit] as const,
};
