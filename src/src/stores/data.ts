import { reactive } from "vue";

// ── Helpers ────────────────────────────────────────────

const isTauri = "__TAURI_INTERNALS__" in window;

export function getApiBaseUrl(): string {
  if (isTauri) return "";
  return localStorage.getItem("trading-agent-api-url") || __API_BASE_URL__;
}

async function fetchJson<T>(path: string, init?: RequestInit): Promise<T> {
  if (isTauri) {
    const { invoke } = await import("@tauri-apps/api/core");
    if (init?.method === "POST" || init?.method === "PUT" || init?.method === "DELETE") {
      const body = init.body ? JSON.parse(String(init.body)) : {};
      return invoke("api_post", { path, body }) as Promise<T>;
    }
    return invoke("api_fetch", { path }) as Promise<T>;
  }
  const base = getApiBaseUrl();
  const resp = await fetch(`${base}${path}`, init);
  if (!resp.ok) throw new Error(`${resp.status}`);
  return resp.json();
}

export async function apiFetch(path: string, init?: RequestInit): Promise<Response> {
  if (isTauri) {
    const { invoke } = await import("@tauri-apps/api/core");
    if (init?.method === "POST" || init?.method === "PUT" || init?.method === "DELETE") {
      const body = init.body ? JSON.parse(String(init.body)) : {};
      const data = await invoke("api_post", { path, body });
      return new Response(JSON.stringify(data), { headers: { "content-type": "application/json" } });
    }
    const data = await invoke("api_fetch", { path });
    return new Response(JSON.stringify(data), { headers: { "content-type": "application/json" } });
  }
  const base = getApiBaseUrl();
  return fetch(`${base}${path}`, init);
}

export interface ChatStreamCallbacks {
  onData: (text: string) => void;
  onDone: () => void;
  onError: (err: string) => void;
}

export async function apiChatStream(
  body: { message: string; role: string; history: Array<{ role: string; content: string }> },
  callbacks: ChatStreamCallbacks,
): Promise<void> {
  if (isTauri) {
    const { invoke, Channel } = await import("@tauri-apps/api/core");
    const channel = new Channel<string>();
    let chunkCount = 0;
    channel.onmessage = (text) => {
      chunkCount++;
      if (chunkCount <= 3 || chunkCount % 20 === 0) {
        console.log(`[chat_stream] chunk #${chunkCount}:`, text.slice(0, 80));
      }
      callbacks.onData(text.replace(/\\n/g, "\n"));
    };
    console.log("[chat_stream] invoking api_chat_stream, body=", body);
    try {
      await invoke("api_chat_stream", { body, channel });
      console.log(`[chat_stream] invoke resolved, total chunks=${chunkCount}`);
      callbacks.onDone();
    } catch (e: any) {
      console.error("[chat_stream] invoke rejected:", e);
      callbacks.onError(String(e));
    }
    return;
  }
  const base = getApiBaseUrl();
  const resp = await fetch(`${base}/api/ai/chat`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!resp.ok) {
    callbacks.onError(`HTTP ${resp.status}`);
    return;
  }
  const reader = resp.body?.getReader();
  if (!reader) {
    callbacks.onError("无法读取响应流");
    return;
  }
  const decoder = new TextDecoder();
  let buffer = "";
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true });
    const lines = buffer.split("\n");
    buffer = lines.pop() || "";
    for (const line of lines) {
      if (line.startsWith("data: ")) {
        const data = line.slice(6);
        if (data === "[DONE]") continue;
        if (data.startsWith("[ERROR]")) {
          callbacks.onError(data);
        } else {
          callbacks.onData(data.replace(/\\n/g, "\n"));
        }
      }
    }
  }
  callbacks.onDone();
}

function todayStr(): string {
  return new Date().toISOString().split("T")[0];
}

// ── Types ──────────────────────────────────────────────

export interface OverviewData {
  date: string;
  rise_count: number;
  fall_count: number;
  limit_up_count: number;
  limit_down_count: number;
  bomb_rate: number;
  risk_level: string;
  decision_summary: string;
  market_temperature: number;
  yesterday_limit_up_avg_pcp: number;
}

export interface SentimentHistoryDay {
  date: string;
  sentiment: number;
  limit_up_count: number;
  bomb_rate: number;
  rise_ratio: number;
}

export interface Stock {
  symbol: string;
  stock_chi_name: string;
  price: number;
  change_percent: number;
  limit_up_days: number;
  turnover_ratio: number;
  total_capital: number;
  break_limit_up_times: number;
  surge_reason: {
    stock_reason: string;
    related_plates: Array<{ plate_name: string }>;
  } | null;
}

export interface FieldPlate {
  name: string;
  count: number;
  reason: string;
  list: Array<{
    code: string;
    name: string;
    article?: {
      action_info?: {
        time?: string;
        num?: string;
        expound?: string;
      };
    };
  }>;
}

// ── Overview ───────────────────────────────────────────

export const overview = reactive({
  data: null as OverviewData | null,
  loading: false,
  error: "",
});

export async function fetchOverview() {
  overview.loading = true;
  overview.error = "";
  try {
    overview.data = await fetchJson<OverviewData>("/api/overview");
  } catch (e: any) {
    overview.error = e.message || String(e);
  } finally {
    overview.loading = false;
  }
}

// ── Limit Up ───────────────────────────────────────────

export const limitUp = reactive({
  data: [] as Stock[],
  loading: false,
  error: "",
});

export async function fetchLimitUp() {
  limitUp.loading = true;
  limitUp.error = "";
  try {
    limitUp.data = await fetchJson<Stock[]>("/api/limit-up");
  } catch (e: any) {
    limitUp.error = e.message || String(e);
  } finally {
    limitUp.loading = false;
  }
}

// ── Field Items ────────────────────────────────────────

export const fieldItems = reactive({
  data: [] as FieldPlate[],
  loading: false,
  error: "",
  date: todayStr(),
});

export async function fetchFieldItems(date?: string) {
  fieldItems.loading = true;
  fieldItems.error = "";
  const d = date || fieldItems.date;
  try {
    fieldItems.data = await fetchJson<FieldPlate[]>(
      `/api/field-items?date=${d}`,
    );
    fieldItems.date = d;
  } catch (e: any) {
    fieldItems.error = e.message || String(e);
  } finally {
    fieldItems.loading = false;
  }
}

// ── Plate Abnormal ─────────────────────────────────────

export const plateAbnormal = reactive({
  data: [] as any[],
  loading: false,
  error: "",
});

export async function fetchPlateAbnormal() {
  plateAbnormal.loading = true;
  plateAbnormal.error = "";
  try {
    plateAbnormal.data = await fetchJson<any[]>("/api/plate-abnormal");
  } catch (e: any) {
    plateAbnormal.error = e.message || String(e);
  } finally {
    plateAbnormal.loading = false;
  }
}

// ── Timeline ───────────────────────────────────────────

export const timeline = reactive({
  data: [] as any[],
  loading: false,
  error: "",
});

export async function fetchTimeline() {
  timeline.loading = true;
  timeline.error = "";
  try {
    timeline.data = await fetchJson<any[]>("/api/timeline");
  } catch (e: any) {
    timeline.error = e.message || String(e);
  } finally {
    timeline.loading = false;
  }
}

// ── Super Stock ────────────────────────────────────────

export const superStock = reactive({
  data: [] as Stock[],
  loading: false,
  error: "",
});

export async function fetchSuperStock() {
  superStock.loading = true;
  superStock.error = "";
  try {
    superStock.data = await fetchJson<Stock[]>("/api/super-stock");
  } catch (e: any) {
    superStock.error = e.message || String(e);
  } finally {
    superStock.loading = false;
  }
}

// ── Dapan Abnormal ─────────────────────────────────────

export const dapanAbnormal = reactive({
  data: [] as any[],
  loading: false,
  error: "",
});

export async function fetchDapanAbnormal() {
  dapanAbnormal.loading = true;
  dapanAbnormal.error = "";
  try {
    dapanAbnormal.data = await fetchJson<any[]>("/api/dapan-abnormal");
  } catch (e: any) {
    dapanAbnormal.error = e.message || String(e);
  } finally {
    dapanAbnormal.loading = false;
  }
}

// ── Sentiment ──────────────────────────────────────────

export const sentiment = reactive({
  data: null as any,
  loading: false,
  error: "",
});

export async function fetchSentiment() {
  sentiment.loading = true;
  sentiment.error = "";
  try {
    sentiment.data = await fetchJson<any>("/api/sentiment");
  } catch (e: any) {
    sentiment.error = e.message || String(e);
  } finally {
    sentiment.loading = false;
  }
}

// ── Roles / Role Strategy ──────────────────────────────

export interface RoleEntry {
  name: string;
  display_name: string;
  strategy: string;
}

export interface RolePosition {
  id: number;
  symbol: string;
  stock_name: string;
  cost_price: number;
  current_price: number;
  shares: number;
  entry_reason: string;
  entry_date: string;
}

export interface RoleLog {
  date: string;
  action: string;
  symbol: string;
  price: number;
  shares: number;
  reason: string;
}

export const roles = reactive({
  data: [] as RoleEntry[],
  loading: false,
  error: "",
});

export async function fetchRoles() {
  roles.loading = true;
  roles.error = "";
  try {
    roles.data = await fetchJson<RoleEntry[]>("/api/roles");
  } catch (e: any) {
    roles.error = e.message || String(e);
  } finally {
    roles.loading = false;
  }
}

// Per-role positions cache
export const rolePositions = reactive<
  Record<string, { data: RolePosition[]; loading: boolean; error: string }>
>({});

export async function fetchRolePositions(name: string) {
  if (!rolePositions[name]) {
    rolePositions[name] = reactive({ data: [], loading: false, error: "" });
  }
  const entry = rolePositions[name];
  entry.loading = true;
  entry.error = "";
  try {
    entry.data = await fetchJson<RolePosition[]>(
      `/api/roles/${name}/positions`,
    );
  } catch (e: any) {
    entry.error = e.message || String(e);
  } finally {
    entry.loading = false;
  }
}

// Per-role logs cache
export const roleLogs = reactive<
  Record<string, { data: RoleLog[]; loading: boolean; error: string }>
>({});

export async function fetchRoleLogs(name: string) {
  if (!roleLogs[name]) {
    roleLogs[name] = reactive({ data: [], loading: false, error: "" });
  }
  const entry = roleLogs[name];
  entry.loading = true;
  entry.error = "";
  try {
    entry.data = await fetchJson<RoleLog[]>(`/api/roles/${name}/logs`);
  } catch (e: any) {
    entry.error = e.message || String(e);
  } finally {
    entry.loading = false;
  }
}

// ── Index Quotes ──────────────────────────────────────

export interface IndexQuote {
  symbol: string;
  name: string;
  last_px: number;
  px_change: number;
  px_change_rate: number;
  trade_status: string;
}

export const indexQuotes = reactive({
  data: [] as IndexQuote[],
  loading: false,
  error: "",
});

export async function fetchIndexQuotes() {
  indexQuotes.loading = true;
  indexQuotes.error = "";
  try {
    const data = await fetchJson<IndexQuote[]>("/api/index-quotes");
    indexQuotes.data = data;
  } catch (e: any) {
    indexQuotes.error = e.message || String(e);
  } finally {
    indexQuotes.loading = false;
  }
}

// ── Sentiment History ──────────────────────────────────

export const sentimentHistory = reactive({
  data: [] as SentimentHistoryDay[],
  loading: false,
  error: "",
});

export async function fetchSentimentHistory(days = 10) {
  sentimentHistory.loading = true;
  sentimentHistory.error = "";
  try {
    sentimentHistory.data = await fetchJson<SentimentHistoryDay[]>(
      `/api/sentiment-history?days=${days}`,
    );
  } catch (e: any) {
    sentimentHistory.error = e.message || String(e);
  } finally {
    sentimentHistory.loading = false;
  }
}

// ── Role P&L History ───────────────────────────────────

export interface PnlPoint {
  date: string;
  equity: number;
  cash: number;
  market_value: number;
  return_pct: number;
}

export interface PnlHistoryResponse {
  points: PnlPoint[];
  initial_capital: number;
}

// Per-role P&L history cache
export const rolePnlHistory = reactive<
  Record<string, { data: PnlPoint[]; loading: boolean; error: string }>
>({});

export async function fetchRolePnlHistory(name: string, days = 30) {
  if (!rolePnlHistory[name]) {
    rolePnlHistory[name] = reactive({ data: [], loading: false, error: "" });
  }
  const entry = rolePnlHistory[name];
  entry.loading = true;
  entry.error = "";
  try {
    const res = await fetchJson<PnlHistoryResponse>(
      `/api/roles/${name}/pnl-history?days=${days}`,
    );
    entry.data = res.points;
  } catch (e: any) {
    entry.error = e.message || String(e);
  } finally {
    entry.loading = false;
  }
}

// ── Role CRUD ──────────────────────────────────────────

export async function createRole(
  name: string,
  displayName: string,
  strategy: string,
) {
  await fetchJson("/api/roles", {
    method: "POST",
    body: JSON.stringify({ name, display_name: displayName, strategy }),
  });
}

export async function updateRoleStrategy(name: string, strategy: string) {
  await fetchJson(`/api/roles/${name}/strategy`, {
    method: "PUT",
    body: JSON.stringify({ strategy }),
  });
}

export async function deleteRole(name: string) {
  await fetchJson(`/api/roles/${name}`, { method: "DELETE" });
}

export async function resetRole(name: string) {
  await fetchJson(`/api/roles/${name}/reset`, { method: "POST" });
}

// Refresh single role's positions
export async function refreshRole(name: string) {
  await fetchRolePositions(name);
  await fetchRoleLogs(name);
  await fetchRolePnlHistory(name);
}