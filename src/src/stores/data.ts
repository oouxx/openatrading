const isTauri = "__TAURI_INTERNALS__" in window;

export function getApiBaseUrl(): string {
  if (isTauri) return "";
  return localStorage.getItem("openatrading-api-url") || __API_BASE_URL__;
}

export async function fetchJson<T>(path: string, init?: RequestInit): Promise<T> {
  if (isTauri) {
    const { invoke } = await import("@tauri-apps/api/core");
    if (
      init?.method === "POST" ||
      init?.method === "PUT" ||
      init?.method === "DELETE"
    ) {
      let body: Record<string, unknown> = {};
      if (init.body) {
        try {
          body = JSON.parse(String(init.body));
        } catch {
          /* use empty */
        }
      }
      return invoke("api_post", { path, body }) as Promise<T>;
    }
    return invoke("api_fetch", { path }) as Promise<T>;
  }
  const base = getApiBaseUrl();
  const headers: Record<string, string> = {};
  if (init?.body) headers["Content-Type"] = "application/json";
  const resp = await fetch(`${base}${path}`, {
    ...init,
    headers: { ...headers, ...(init?.headers as Record<string, string>) },
  });
  if (!resp.ok) throw new Error(`${resp.status}`);
  return resp.json();
}

export interface ChatStreamCallbacks {
  onData: (text: string) => void;
  onDone: () => void;
  onError: (err: string) => void;
}

export async function apiChatStream(
  body: {
    message: string;
    role: string;
    history: Array<{ role: string; content: string }>;
  },
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
      callbacks.onData(text);
    };
    console.log("[chat_stream] invoking api_chat_stream, body=", body);
    try {
      await invoke("api_chat_stream", { body, channel });
      console.log(`[chat_stream] invoke resolved, total chunks=${chunkCount}`);
      callbacks.onDone();
    } catch (e: unknown) {
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
        if (data === "[DONE]") {
          callbacks.onDone();
          return;
        } else if (data.startsWith("[ERROR]")) {
          callbacks.onError(data);
        } else {
          callbacks.onData(data.replace(/\\n/g, "\n"));
        }
      } else if (line.startsWith("event: tool")) {
        // 下一行就是 data: <tool_name>，在下一次循环会被 data: 分支处理
        // 如果需要区分 tool 事件可以在这里设个 flag
      }
    }
  }

  // 防止 reader 正常结束但没收到 [DONE] 的情况
  callbacks.onDone();
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
  yesterday_limit_up_days?: number;
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

export interface ClosedPosition {
  position_id: number;
  symbol: string;
  stock_name: string;
  entry_date: string;
  exit_date: string;
  entry_price: number;
  exit_price: number;
  shares: number;
  pnl_pct: number;
  entry_reason: string;
  exit_reason: string;
}

export interface IndexQuote {
  symbol: string;
  name: string;
  last_px: number;
  px_change: number;
  px_change_rate: number;
  trade_status: string;
}