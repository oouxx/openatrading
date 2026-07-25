# pi-rs Transcript 渲染架构设计

## 目标

- 前端渲染逻辑（数据模型、reducer、UI 组件）在 Web 和 Tauri 两端 100% 复用
- 只有 transport 层（事件怎么从 pi-rs 传到前端）在两端不同
- 统一走 WebSocket（Web 场景），Tauri 场景走原生 IPC，两者通过同一个前端接口抽象

---

## 1. 整体架构

```
┌─────────────────────────────────────────────────────────┐
│                     AgentSession (pi-rs 核心)              │
│                                                           │
│  subscribe(async_listener)         → AgentEvent (异步)     │
│  subscribe_session_events(sync_fn) → AgentSessionEvent (同步) │
└───────────────────────┬───────────────────────────────────┘
                         │  两路事件统一汇入 channel
                         ▼
              ┌─────────────────────┐
              │  unbounded_channel   │  ← 合并后的事件流
              │  (Core / Session)    │
              └──────────┬───────────┘
              ┌───────────┴───────────┐
      ┌───────▼────────┐      ┌───────▼────────┐
      │  axum WS server │      │ Tauri commands  │
      │  (独立进程，Web)  │      │ (同进程 IPC)     │
      └───────┬─────────┘      └───────┬─────────┘
              │ WebSocket                │ tauri emit/invoke
              ▼                          ▼
      ┌────────────────────────────────────────┐
      │         AgentTransport (前端接口)         │
      │   WebSocketTransport / TauriTransport    │
      └──────────────────┬───────────────────────┘
                          ▼
              ┌───────────────────────┐
              │  applyEvent reducer    │  ← 纯函数，两端复用
              │  TranscriptBlock 模型  │
              └───────────┬────────────┘
                          ▼
              ┌───────────────────────┐
              │   自定义 React 组件树    │  ← 不依赖 assistant-ui
              └───────────────────────┘
```

---

## 2. 后端事件模型（基于 pi-coding-agent 实际类型）

pi-rs 暴露两类事件，注册方式不同：

| 事件类型            | 分发方式                           | 订阅接口                                    | 典型内容                                                                                           |
| ------------------- | ---------------------------------- | ------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| `AgentEvent`        | **异步**（listener 返回 `Future`） | `session.subscribe(async_listener).await`   | `MessageUpdate`（内部含 `TextDelta`/`ThinkingDelta`/`ToolCallDelta`/`ToolCallEnd`/`Done`/`Error`） |
| `AgentSessionEvent` | **同步**（普通闭包，无 `Future`）  | `session.subscribe_session_events(sync_fn)` | `TurnStart`/`TurnEnd`、`CompactionStart`/`End`、`AutoRetryStart`/`End`、`AgentSettled`、`AgentEnd` |

两者都通过 `tx.send(event)` 塞进同一个（或两个）`unbounded_channel`，因为 `send()` 本身是非阻塞同步调用，异步 listener 里可以额外做 `.await` 逻辑，同步闭包里不能。

### 中断信号走独立通道

`async_listener` 签名里带了 `Option<watch::Receiver<bool>>`，说明中断（interrupt）不复用事件流，而是通过单独的 `watch` channel 传递。对应到 transport 层：**中断是一个独立的控制方法，不是往事件流里塞一条消息**。

---

## 3. 传输层：统一走 WebSocket

### 3.1 为什么两端都用 WebSocket（而不是 Web 用 SSE）

- 之前的实现（trending-agent）用 SSE 是因为每轮对话开一条新连接、状态靠 `session_id` 在服务端持久化，天然适合单次请求-流式响应模型
- pi-rs 的 transcript 场景是**持续交互式 session**：同一个会话要能连续发多条 prompt、支持中途打断、需要看到 `queue_update` 这类排队反馈——这些都要求同一条连接双向通信
- 统一用 WebSocket 也让 Web 和 Tauri 两端的"心智模型"一致：都是一条长连接，发送 prompt / 中断信号、接收事件

### 3.2 后端：axum WebSocket handler

```rust
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use tokio::sync::{mpsc, watch};

#[derive(serde::Serialize)]
#[serde(tag = "kind")]
enum TranscriptEvent {
    Core(pi_coding_agent::pi_agent_core::types::AgentEvent),
    Session(pi_coding_agent::core::agent_session::AgentSessionEvent),
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_session(socket, state))
}

async fn handle_session(mut socket: WebSocket, state: Arc<AppState>) {
    let (evt_tx, mut evt_rx) = mpsc::unbounded_channel::<TranscriptEvent>();
    let (interrupt_tx, interrupt_rx) = watch::channel(false);

    let mut agent_facade = match state.ctx.runtime.create_session(None, None, Some(true), None, None).await {
        Ok(h) => h,
        Err(e) => {
            let _ = socket.send(Message::Text(format!(r#"{{"kind":"error","message":"{e}"}}"#).into())).await;
            return;
        }
    };

    // AgentEvent（异步）
    let tx1 = evt_tx.clone();
    let async_listener: Arc<dyn Fn(_, Option<watch::Receiver<bool>>) -> _ + Send + Sync> =
        Arc::new(move |event, _signal| {
            let tx = tx1.clone();
            Box::pin(async move { let _ = tx.send(TranscriptEvent::Core(event)); })
        });
    let _h1 = agent_facade.session.subscribe(async_listener).await;

    // AgentSessionEvent（同步）
    let tx2 = evt_tx.clone();
    let sync_listener: AgentSessionEventListener = Arc::new(move |event| {
        let _ = tx2.send(TranscriptEvent::Session(event));
    });
    let _h2 = agent_facade.session.subscribe_session_events(sync_listener);

    loop {
        tokio::select! {
            // 事件 → 前端
            Some(event) = evt_rx.recv() => {
                let Ok(json) = serde_json::to_string(&event) else { continue };
                if socket.send(Message::Text(json.into())).await.is_err() { break; }
            }
            // 前端 → 指令
            Some(Ok(msg)) = socket.recv() => {
                if let Message::Text(text) = msg {
                    match serde_json::from_str::<ClientCommand>(&text) {
                        Ok(ClientCommand::Prompt { text }) => { agent_facade.send(&text).await; }
                        Ok(ClientCommand::Interrupt) => { let _ = interrupt_tx.send(true); }
                        Err(_) => {} // 忽略无法解析的消息，不 panic
                    }
                }
            }
            else => break,
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientCommand {
    Prompt { text: String },
    Interrupt,
}
```

### 3.3 Tauri：同进程 IPC，复用同一个 AgentSession 创建逻辑

```rust
#[tauri::command]
async fn send_prompt(session: State<'_, AgentFacade>, text: String) -> Result<(), String> {
    session.send(&text).await;
    Ok(())
}

#[tauri::command]
async fn interrupt_session(interrupt_tx: State<'_, watch::Sender<bool>>) -> Result<(), String> {
    interrupt_tx.send(true).map_err(|e| e.to_string())
}

fn setup_session(app: &AppHandle) {
    let (evt_tx, mut evt_rx) = mpsc::unbounded_channel::<TranscriptEvent>();
    // ... 同样的 subscribe 逻辑，tx.send 替代 socket.send

    let handle = app.clone();
    tokio::spawn(async move {
        while let Some(event) = evt_rx.recv().await {
            let _ = handle.emit("agent-event", &event); // 序列化逻辑与 WS 端一致
        }
    });
}
```

**关键点**：两端的"事件产生"逻辑（`subscribe` 注册、`TranscriptEvent` 组装）完全一致，只有"事件怎么送到前端"不同（`socket.send` vs `app.emit`）。建议把 `setup_session` 这部分抽成一个共享函数，接受一个 `Sender<TranscriptEvent>`，WS handler 和 Tauri setup 各自决定怎么消费。

---

## 4. 前端：Transport 抽象

```ts
interface AgentTransport {
  onEvent(cb: (event: TranscriptEvent) => void): () => void;
  sendPrompt(text: string): Promise<void>;
  interrupt(): Promise<void>;
}

// Web 场景
class WebSocketTransport implements AgentTransport {
  private ws: WebSocket;
  constructor(url: string) {
    this.ws = new WebSocket(url);
  }

  onEvent(cb: (e: TranscriptEvent) => void) {
    const handler = (msg: MessageEvent) => cb(JSON.parse(msg.data));
    this.ws.addEventListener("message", handler);
    return () => this.ws.removeEventListener("message", handler);
  }
  sendPrompt(text: string) {
    this.ws.send(JSON.stringify({ type: "prompt", text }));
    return Promise.resolve();
  }
  interrupt() {
    this.ws.send(JSON.stringify({ type: "interrupt" }));
    return Promise.resolve();
  }
}

// Tauri 场景
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

class TauriTransport implements AgentTransport {
  onEvent(cb: (e: TranscriptEvent) => void) {
    let unlisten: () => void;
    listen<TranscriptEvent>("agent-event", (e) => cb(e.payload)).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  }
  sendPrompt(text: string) {
    return invoke("send_prompt", { text });
  }
  interrupt() {
    return invoke("interrupt_session");
  }
}

function createTransport(): AgentTransport {
  return "__TAURI_INTERNALS__" in window
    ? new TauriTransport()
    : new WebSocketTransport(`ws://${location.host}/session`);
}
```

---

## 5. Block 数据模型 + reducer（两端复用）

```ts
type TranscriptBlock =
  | { type: "text"; id: string; content: string; streaming: boolean }
  | { type: "thinking"; id: string; content: string; streaming: boolean }
  | {
      type: "tool_call";
      id: string;
      name: string;
      input: unknown;
      status: "pending" | "running" | "done" | "error";
    }
  | {
      type: "system_notice";
      kind: "compaction" | "retry" | "turn";
      detail: string;
    };

interface TranscriptTurn {
  id: string;
  role: "user" | "assistant";
  blocks: TranscriptBlock[];
  status: "in_progress" | "complete";
}

function applyEvent(
  turns: TranscriptTurn[],
  event: TranscriptEvent,
): TranscriptTurn[] {
  if (event.kind === "Core") {
    const inner = event.MessageUpdate?.assistant_message_event;
    if (inner?.TextDelta)
      return appendToBlock(turns, "text", inner.TextDelta.delta);
    if (inner?.ThinkingDelta)
      return appendToBlock(turns, "thinking", inner.ThinkingDelta.delta);
    if (inner?.ToolCallDelta)
      return updateToolCallInput(turns, inner.ToolCallDelta.delta);
    if (inner?.ToolCallEnd)
      return finalizeToolCall(turns, inner.ToolCallEnd.tool_call);
    if (inner?.Done) return markTextComplete(turns);
    if (inner?.Error)
      return appendSystemNotice(
        turns,
        "retry",
        inner.Error.error.error_message ?? "unknown error",
      );
    return turns;
  }
  // event.kind === "Session"
  switch (event.Session.type) {
    case "TurnStart":
      return startTurn(turns);
    case "TurnEnd":
      return endTurn(turns, event.Session.tool_results);
    case "CompactionStart":
      return appendSystemNotice(turns, "compaction", event.Session.reason);
    case "AutoRetryStart":
      return appendSystemNotice(
        turns,
        "retry",
        `retry ${event.Session.attempt}/${event.Session.max_attempts}`,
      );
    case "AgentSettled":
      return markAllComplete(turns);
    default:
      return turns;
  }
}
```

> 注：实际字段名以后端 `serde` 序列化后的 JSON 结构为准，上面是示意，接入时需要对照 Rust 端 enum 的实际 `#[serde(...)]` 标注核对。

---

## 6. UI 渲染（纯自定义，不依赖 assistant-ui）

```tsx
function TranscriptView({ turns }: { turns: TranscriptTurn[] }) {
  const scrollRef = useAutoScroll(); // 自己实现：判断是否已在底部再决定是否跟随滚动
  return (
    <div className="transcript" ref={scrollRef}>
      {turns.map((turn) => (
        <TurnView key={turn.id} turn={turn} />
      ))}
    </div>
  );
}

function BlockView({ block }: { block: TranscriptBlock }) {
  switch (block.type) {
    case "text":
      return (
        <MarkdownBlock content={block.content} streaming={block.streaming} />
      );
    case "thinking":
      return <ThinkingBlock content={block.content} />;
    case "tool_call":
      return <ToolCallCard block={block} />;
    case "system_notice":
      return <SystemNotice block={block} />;
  }
}
```

**需要自己处理的细节**（不用 assistant-ui 后要补的坑）：

1. **markdown 增量渲染不闪烁**：流式阶段展示为纯文本或轻量 markdown，`streaming: false`（对应 `Done`/`TextEnd`）后再切换成完整 `react-markdown` + `rehype-highlight` 渲染，避免逐字重新解析导致 DOM 抖动
2. **auto-scroll**：`scrollHeight - scrollTop - clientHeight < threshold` 判断是否贴底，只有贴底时新事件才触发滚动
3. **工具卡片按 `name` 分派**渲染组件（`bash` → 命令卡片，`edit` → diff view，用 `diff` npm 包计算 + 自绘高亮）
4. **虚拟列表**：session 过长时按需引入，非必须起步项

---

## 7. Session 列表 / 历史消息 / 切换

### 7.1 会话列表：独立于实时事件流的一套 API

`SessionManager` 本身已有 JSONL 持久化，包一层列表/加载接口，不复用 WebSocket：

```rust
// GET /sessions
async fn list_sessions(State(state): State<Arc<AppState>>) -> Json<Vec<SessionSummary>> {
    let sessions = state.ctx.session_manager.list().await.unwrap_or_default();
    Json(sessions.into_iter().map(|s| SessionSummary {
        id: s.id,
        title: s.title.unwrap_or_else(|| "Untitled".into()),
        updated_at: s.updated_at,
        message_count: s.message_count,
        status: s.status, // "idle" | "running" —— 见 7.4
    }).collect())
}

// GET /sessions/:id/messages
async fn load_session_history(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<AgentMessage>>, StatusCode> {
    state.ctx.session_manager.load_messages(&id).await
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}
```

Tauri 侧对应两个 command，直接调同一个 `session_manager`，逻辑不重复写：

```rust
#[tauri::command]
async fn list_sessions(state: State<'_, AppState>) -> Result<Vec<SessionSummary>, String> {
    state.session_manager.list().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn load_session_history(state: State<'_, AppState>, id: String) -> Result<Vec<AgentMessage>, String> {
    state.session_manager.load_messages(&id).await.map_err(|e| e.to_string())
}
```

### 7.2 历史消息 ≠ 事件流重放

不要把历史消息重新播放成一遍 `text_delta` 事件塞进 `applyEvent`。存储的是已经 assembled 好的完整 `AgentMessage`，走另一条转换路径，跳过增量拼接：

```ts
function historyToTurns(messages: AgentMessage[]): TranscriptTurn[] {
  return messages.map((msg) => ({
    id: msg.id,
    role: msg.role === "user" ? "user" : "assistant",
    status: "complete",
    blocks: msg.content.map(contentBlockToTranscriptBlock),
  }));
}

function contentBlockToTranscriptBlock(block: ContentBlock): TranscriptBlock {
  switch (block.type) {
    case "text":
      return {
        type: "text",
        id: block.id,
        content: block.text,
        streaming: false,
      };
    case "thinking":
      return {
        type: "thinking",
        id: block.id,
        content: block.text,
        streaming: false,
      };
    case "tool_use":
      return {
        type: "tool_call",
        id: block.id,
        name: block.name,
        input: block.input,
        status: "done",
      };
    default:
      return { type: "system_notice", kind: "turn", detail: "" };
  }
}
```

`applyEvent`（增量流）和 `historyToTurns`（一次性快照）是两个独立函数，共用 `TranscriptBlock` 类型但不共用转换逻辑——一个处理 delta，一个处理已完成内容，硬合并只会让两边状态机纠缠。

### 7.3 切换 session 的时序

原则：**先用 REST/IPC 秒开历史，再单独建立/重连 WS 续接实时流**，不要用一条 WS 连接"查历史+接实时"一起做，否则难以区分"这是回放的历史"还是"刚发生的新事件"。

```ts
async function switchSession(
  id: string,
  transportRef: MutableRef<AgentTransport | null>,
  sessionApi: SessionApi,
) {
  transportRef.current?.dispose(); // 1. 断开当前实时连接

  const history = await sessionApi.loadHistory(id); // 2. 秒开历史
  setTurns(historyToTurns(history));

  transportRef.current = createTransport(id); // 3. 建立新连接，只接收此刻之后的新事件
  transportRef.current.onEvent((event) =>
    setTurns((prev) => applyEvent(prev, event)),
  );
}
```

`WebSocketTransport` 加 session id 参数：

```ts
class WebSocketTransport implements AgentTransport {
  constructor(sessionId: string) {
    this.ws = new WebSocket(`ws://${location.host}/session/${sessionId}`);
  }
}
```

后端从 URL 拿 `session_id`，用它恢复而不是每次 `create_session`：

```rust
async fn ws_handler(
    Path(session_id): Path<String>,
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_session(socket, state, session_id))
}

async fn handle_session(mut socket: WebSocket, state: Arc<AppState>, session_id: String) {
    let mut agent_facade = match state.ctx.runtime.resume_session(&session_id).await {
        Ok(h) => h,
        Err(e) => {
            let _ = socket.send(Message::Text(format!(r#"{{"kind":"error","message":"{e}"}}"#).into())).await;
            return;
        }
    };
    // ... subscribe 逻辑不变
}
```

### 7.4 边界情况：切回一个"正在跑"的 session

用户 A 发消息未跑完就切走、再切回来时，WS 重连后收到的第一个事件可能是某轮 turn 中间的 delta，前端却没这个 turn 的开头（没订阅到 `TurnStart`）。两种处理：

- **简单**：`SessionSummary` 带 `status: "idle" | "running"`；`running` 时历史里最后一条不完整的 assistant 消息标 `status: "in_progress"`，UI 显示"生成中"占位，等新事件补完
- **更完整**：后端在 session 内部缓存"当前 turn 已产生的全部 block"，WS 一连上先补发一份快照事件，前端收到快照直接替换而不是追加

第一种成本低，多数场景够用；pi-rs 若经常有长时间 turn（跑测试、编译），第二种体验更好。

### 7.5 UI：session 列表

```tsx
function SessionSidebar({ sessions, activeId, onSelect }: SessionSidebarProps) {
  return (
    <ul className="session-list">
      {sessions.map((s) => (
        <li
          key={s.id}
          className={s.id === activeId ? "active" : ""}
          onClick={() => onSelect(s.id)}
        >
          <span className="title">{s.title}</span>
          <time>{formatRelative(s.updatedAt)}</time>
        </li>
      ))}
    </ul>
  );
}
```

列表更新频率低，轮询几秒一次即可，没必要为它单独设计推送机制。

---

## 9. WS 断线重连与事件补发

网络抖动、设备休眠唤醒、Tauri 应用切后台再切回都会断开 WS。若断线期间后端仍有事件产生（比如长 turn 还在跑），单纯重连会丢掉这段。

### 9.1 后端：事件加序号 + 环形缓冲

```rust
struct SessionEventBuffer {
    events: VecDeque<(u64, TranscriptEvent)>, // (seq, event)
    next_seq: u64,
    capacity: usize, // 环形缓冲上限，超出丢最旧的
}

impl SessionEventBuffer {
    fn push(&mut self, event: TranscriptEvent) -> u64 {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.events.push_back((seq, event));
        if self.events.len() > self.capacity {
            self.events.pop_front();
        }
        seq
    }

    fn events_since(&self, last_seq: Option<u64>) -> Vec<(u64, &TranscriptEvent)> {
        match last_seq {
            Some(seq) => self.events.iter().filter(|(s, _)| *s > seq).map(|(s, e)| (*s, e)).collect(),
            None => self.events.iter().map(|(s, e)| (*s, e)).collect(),
        }
    }
}
```

WS 连接时带上 `?last_seq=123`（首次连接不带），后端先补发缺失事件再继续正常推流：

```rust
async fn handle_session(mut socket: WebSocket, state: Arc<AppState>, session_id: String, last_seq: Option<u64>) {
    let buffer = state.get_or_create_buffer(&session_id);

    // 补发断线期间漏掉的事件
    for (seq, event) in buffer.events_since(last_seq) {
        let json = serde_json::to_string(&SeqEvent { seq, event })?;
        if socket.send(Message::Text(json.into())).await.is_err() { return; }
    }

    // 继续正常事件流（每条都过一遍 buffer.push 记录 seq）
    // ...
}
```

若环形缓冲已经把请求的 `last_seq` 之前的部分淘汰（断线太久），后端返回一个 `resync_required` 信号，前端退回"重新拉取历史 + 从当前状态续接"的兜底路径。

### 9.2 前端：心跳 + 指数退避重连 + seq 追踪

```ts
class WebSocketTransport implements AgentTransport {
  private ws?: WebSocket;
  private lastSeq?: number;
  private retryDelay = 1000;
  private readonly maxRetryDelay = 30_000;
  private heartbeatTimer?: ReturnType<typeof setInterval>;

  constructor(
    private sessionId: string,
    private onResyncRequired: () => void,
  ) {
    this.connect();
  }

  private connect() {
    const url = new URL(`ws://${location.host}/session/${this.sessionId}`);
    if (this.lastSeq !== undefined)
      url.searchParams.set("last_seq", String(this.lastSeq));
    this.ws = new WebSocket(url);

    this.ws.onopen = () => {
      this.retryDelay = 1000; // 连上后重置退避
      this.heartbeatTimer = setInterval(
        () => this.ws?.send(JSON.stringify({ type: "ping" })),
        15_000,
      );
    };
    this.ws.onmessage = (msg) => {
      const parsed = JSON.parse(msg.data);
      if (parsed.type === "resync_required") {
        this.onResyncRequired();
        return;
      }
      if (parsed.seq !== undefined) this.lastSeq = parsed.seq;
      this.dispatch(parsed);
    };
    this.ws.onclose = () => {
      clearInterval(this.heartbeatTimer);
      setTimeout(() => this.connect(), this.retryDelay);
      this.retryDelay = Math.min(this.retryDelay * 2, this.maxRetryDelay);
    };
  }

  dispose() {
    clearInterval(this.heartbeatTimer);
    this.ws?.close();
  }
  // onEvent/sendPrompt/interrupt 略，dispatch 内部调用注册的回调
}
```

`onResyncRequired` 回调触发时执行 7.3 里的 `switchSession` 同一套逻辑（重新拉历史 + 建新连接），本质是把"缓冲区已过期"和"用户主动切 session"归为同一条恢复路径，不用另写一套。

---

## 10. 事件顺序保证

`AgentEvent`（异步）和 `AgentSessionEvent`（同步）目前各自一个 channel，`tokio::select!` 里谁先到就先处理谁——`TurnStart` 理论上应先于同一轮的 `text_delta` 到达，但两个 channel 并发写入时调度顺序不保证，前端可能先收到 delta 再收到 `TurnStart`。

### 10.1 后端：合并到单一 channel，在写入侧定序

比起在 select 里做优先级，更可靠的做法是让两路事件在写入前就汇入**同一个** channel（而不是两个 channel 在读取时用 select 抢），这样天然保序：

```rust
let (evt_tx, mut evt_rx) = mpsc::unbounded_channel::<TranscriptEvent>(); // 只有一个 channel

// AgentEvent（异步）和 AgentSessionEvent（同步）都 send 进同一个 tx
let tx1 = evt_tx.clone();
let async_listener = Arc::new(move |event, _signal| {
    let tx = tx1.clone();
    Box::pin(async move { let _ = tx.send(TranscriptEvent::Core(event)); })
});

let tx2 = evt_tx.clone();
let sync_listener: AgentSessionEventListener = Arc::new(move |event| {
    let _ = tx2.send(TranscriptEvent::Session(event));
});
```

`mpsc::unbounded_channel` 保证多个生产者 `send()` 的调用顺序性——只要 pi-rs 内部触发 `AgentSessionEvent::TurnStart` 的代码路径确实先于触发第一个 `AgentEvent::MessageUpdate`（这是 agent loop 内部的时序，不是 channel 的问题），单 channel 模式下顺序就是有保证的。之前方案里用两个 channel 再 `tokio::select!` 合并，才是引入乱序风险的地方——改成两路都写向同一个 channel 即可消除这个问题，不需要额外定序逻辑。

### 10.2 前端兜底：防御乱序

即使后端保证了顺序，网络层面 WS 消息本身是有序的（TCP 保证），所以前端一般不需要额外处理。但为了健壮性，`applyEvent` 对"delta 到达时找不到对应 turn"这种情况做个防御：

```ts
function appendToBlock(
  turns: TranscriptTurn[],
  type: BlockType,
  delta: string,
): TranscriptTurn[] {
  const last = turns[turns.length - 1];
  if (!last || last.status === "complete") {
    // 没有进行中的 turn，说明 TurnStart 事件丢失或未到——自动创建一个兜底 turn
    turns = [
      ...turns,
      {
        id: crypto.randomUUID(),
        role: "assistant",
        blocks: [],
        status: "in_progress",
      },
    ];
  }
  // ... 正常 append 逻辑
  return turns;
}
```

---

## 11. 工具调用审批（human-in-the-loop）

pi 的 permission policy 意味着部分工具（危险 bash 命令等）执行前需要用户确认，这是"请求-等待响应"模式，不是单向事件。

### 11.1 后端：审批请求走同一 WS 连接，但需要关联 id

```rust
#[derive(serde::Serialize)]
struct PermissionRequest {
    kind: &'static str, // "permission_request"
    id: String,
    tool_name: String,
    tool_input: serde_json::Value,
}

// AgentEvent 里遇到需要审批的工具调用时，发出请求并等待响应
async fn handle_permission_check(
    socket: &mut WebSocket,
    pending: &Arc<Mutex<HashMap<String, oneshot::Sender<bool>>>>,
    tool_name: &str,
    tool_input: &serde_json::Value,
) -> bool {
    let req_id = uuid::Uuid::new_v4().to_string();
    let (tx, rx) = oneshot::channel();
    pending.lock().await.insert(req_id.clone(), tx);

    let req = PermissionRequest { kind: "permission_request", id: req_id, tool_name: tool_name.into(), tool_input: tool_input.clone() };
    let Ok(json) = serde_json::to_string(&req) else { return false };
    if socket.send(Message::Text(json.into())).await.is_err() { return false; }

    rx.await.unwrap_or(false) // 连接断开时 oneshot 被 drop，视为拒绝
}
```

客户端指令里加一个响应类型，收到后从 `pending` map 里取出对应 `oneshot::Sender` 触发：

```rust
enum ClientCommand {
    Prompt { text: String },
    Interrupt,
    PermissionResponse { id: String, approved: bool },
}
```

### 11.2 前端：新增 block 类型 + transport 方法

```ts
type TranscriptBlock =
  | {
      /* ...之前的类型 */
    }
  | {
      type: "permission_request";
      id: string;
      toolName: string;
      toolInput: unknown;
      resolved: boolean;
    };

interface AgentTransport {
  // ...之前的方法
  respondToPermission(id: string, approved: boolean): Promise<void>;
}
```

UI 上 `permission_request` block 渲染成一个内嵌确认卡片（工具名 + 参数摘要 + 允许/拒绝按钮），点击后调用 `respondToPermission`，同时把该 block 标记 `resolved: true` 变成只读状态,不需要额外的模态框——审批请求在 transcript 时间线里本来的位置显示最直观,用户能看到"agent 想执行什么"和上下文的关系。

---

## 12. Session CRUD

```rust
// POST /sessions  → 创建新 session，不带 prompt
async fn create_session(State(state): State<Arc<AppState>>) -> Json<SessionSummary> {
    let id = state.ctx.session_manager.create(SessionMeta::default()).await;
    Json(SessionSummary { id, title: "New session".into(), updated_at: now(), message_count: 0, status: "idle".into() })
}

// DELETE /sessions/:id
async fn delete_session(Path(id): Path<String>, State(state): State<Arc<AppState>>) -> StatusCode {
    match state.ctx.session_manager.delete(&id).await {
        Ok(_) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::NOT_FOUND,
    }
}

// PATCH /sessions/:id  { "title": "..." }
async fn rename_session(Path(id): Path<String>, Json(req): Json<RenameRequest>, State(state): State<Arc<AppState>>) -> StatusCode {
    match state.ctx.session_manager.rename(&id, &req.title).await {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::NOT_FOUND,
    }
}
```

Tauri 侧对应三个 command，逻辑同样直接转发到 `session_manager`，不重复实现。

**Title 自动生成**：新建 session 首次收到用户消息后，可以异步调一次轻量 LLM 调用（或者简单截取首条消息前 N 个字符）生成 title,更新完通过一条 `session_meta_updated` 事件（走 8.5 提到的轮询或者复用已有 WS 广播）通知前端刷新列表里的标题,不需要用户手动重命名才有标题。

---

## 14. Channel 背压

之前所有代码示例用的都是 `mpsc::unbounded_channel`，如果 agent 输出速度（比如密集的 `text_delta`）远快于 WS 发送速度（网络慢、前端卡顿），channel 会无限堆积，最终把后端内存吃爆。改成有界 channel，并按事件类型区分背压策略。

### 14.1 高频可丢事件与语义完整事件分开处理

`text_delta`/`thinking_delta` 这类高频小更新，丢一两条用户感知不到；`tool_call`/`tool_result`/`AgentSettled` 这类语义完整的事件不能丢，否则前端状态会永久卡在"等待中"。用两个有界 channel 分流：

```rust
let (delta_tx, mut delta_rx) = mpsc::channel::<TranscriptEvent>(64);   // 允许丢
let (durable_tx, mut durable_rx) = mpsc::channel::<TranscriptEvent>(256); // 不允许丢

fn route_event(event: TranscriptEvent, delta_tx: &mpsc::Sender<TranscriptEvent>, durable_tx: &mpsc::Sender<TranscriptEvent>) {
    let is_high_frequency = matches!(
        &event,
        TranscriptEvent::Core(AgentEvent::MessageUpdate {
            assistant_message_event: AssistantMessageEvent::TextDelta { .. } | AssistantMessageEvent::ThinkingDelta { .. },
            ..
        })
    );

    if is_high_frequency {
        // 满了就丢弃这条，不阻塞、不报错
        let _ = delta_tx.try_send(event);
    } else {
        // 满了则等待发送，宁可短暂阻塞 agent loop 也不丢
        let tx = durable_tx.clone();
        tokio::spawn(async move { let _ = tx.send(event).await; });
    }
}
```

WS 发送侧用 `tokio::select!` 同时消费两个 channel，`durable_rx` 可以给一个隐式的优先级（比如每次循环先 `try_recv` 排空 durable 再处理 delta）：

```rust
loop {
    tokio::select! {
        biased; // durable 优先
        Some(event) = durable_rx.recv() => { send_to_socket(&mut socket, event).await; }
        Some(event) = delta_rx.recv() => { send_to_socket(&mut socket, event).await; }
        else => break,
    }
}
```

### 14.2 丢弃 delta 后前端如何兜底

如果 `text_delta` 被丢弃，前端展示的文本会比实际生成的少几个字符——这在流式阶段问题不大（视觉上仍在正常滚动），但 `AssistantMessageEvent::Done`（携带完整 `content`）到达时用完整内容**覆盖**当前 block 而不是继续 append，天然纠正掉丢失的部分：

```ts
case "Done":
  return replaceBlockContent(turns, blockId, event.Done.message.content); // 覆盖而非追加
```

这也是为什么 7.2 强调"历史消息走独立转换路径,不复用 delta 拼接逻辑"——`Done`/`TextEnd` 事件本质上和历史快照是同一种"完整内容"语义，用同一个覆盖式更新函数处理即可。

---

## 15. 大体积工具输出的截断与展开

`bash` 跑出几千行输出、`read` 读一个大文件，直接全量塞进事件流和 UI 会导致 WS 消息过大、页面卡顿。

### 15.1 后端：序列化前做预览截断

```rust
const PREVIEW_LIMIT: usize = 2000; // 字符数

fn to_tool_result_payload(content: &str) -> ToolResultPayload {
    if content.chars().count() <= PREVIEW_LIMIT {
        ToolResultPayload { preview: content.to_string(), truncated: false, full_length: content.len() }
    } else {
        let preview: String = content.chars().take(PREVIEW_LIMIT).collect();
        ToolResultPayload { preview, truncated: true, full_length: content.len() }
    }
}
```

完整内容不通过事件流推送，而是落盘/存进 session 存储，前端按需再拉：

```rust
// GET /sessions/:id/tool-results/:tool_call_id
async fn get_full_tool_result(
    Path((session_id, tool_call_id)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
) -> Result<String, StatusCode> {
    state.ctx.session_manager.load_tool_result(&session_id, &tool_call_id).await
        .map_err(|_| StatusCode::NOT_FOUND)
}
```

### 15.2 前端：block 类型带预览信息，展开时二次请求

```ts
type TranscriptBlock =
  | {
      /* ... */
    }
  | {
      type: "tool_call";
      id: string;
      name: string;
      input: unknown;
      status: "pending" | "running" | "done" | "error";
      output?: { preview: string; truncated: boolean; fullLength: number };
    };
```

```tsx
function ToolCallCard({
  block,
  sessionId,
}: {
  block: ToolCallBlock;
  sessionId: string;
}) {
  const [full, setFull] = useState<string | null>(null);

  const expand = async () => {
    const res = await fetch(`/sessions/${sessionId}/tool-results/${block.id}`);
    setFull(await res.text());
  };

  return (
    <div className="tool-card">
      <ToolHeader name={block.name} status={block.status} />
      <pre>{full ?? block.output?.preview}</pre>
      {block.output?.truncated && !full && (
        <button onClick={expand}>
          展开全部（{block.output.fullLength} 字符）
        </button>
      )}
    </div>
  );
}
```

历史消息（7.2 的 `historyToTurns`）走同样的截断规则，避免加载一个跑过大量命令的老 session 时一次性把所有输出塞进内存。

---

## 16. 事件 Schema 版本兼容

`AgentEvent`/`AgentSessionEvent` 这些 Rust enum 会随 pi-rs 迭代新增变体。前端遇到不认识的 `type` 字段如果直接崩溃会很脆弱，需要两层防御。

### 16.1 前端：未知事件静默忽略而非报错

```ts
function applyEvent(
  turns: TranscriptTurn[],
  event: TranscriptEvent,
): TranscriptTurn[] {
  switch (event.kind) {
    case "Core":
      return applyCoreEvent(turns, event);
    case "Session":
      return applySessionEvent(turns, event);
    default:
      console.warn("unknown event kind, ignored:", event); // 记录但不中断渲染
      return turns;
  }
}

function applySessionEvent(
  turns: TranscriptTurn[],
  event: SessionEventEnvelope,
): TranscriptTurn[] {
  switch (event.type) {
    case "TurnStart":
      return startTurn(turns);
    case "TurnEnd":
      return endTurn(turns, event.tool_results);
    // ... 已知分支
    default:
      return turns; // 未来新增的变体在这里被安全忽略，不需要每次都改前端才能接收新事件
  }
}
```

### 16.2 后端：事件信封带版本号，序列化时保留未知字段友好性

给顶层事件包一层 envelope，带上 schema 版本，方便前端做条件判断（比如"版本低于 3 的话某个字段可能不存在，做默认值兜底"）：

```rust
#[derive(serde::Serialize)]
struct EventEnvelope {
    schema_version: u32, // 每次给事件 enum 加不兼容变更时递增
    seq: u64,
    event: TranscriptEvent,
}
```

前端读到 `schema_version` 高于自己认识的最大版本时，只需要保证"忽略不认识的具体事件类型"这条底线不破——不需要为了兼容未来版本预先写好处理逻辑，只要保证不崩溃、不阻塞事件流即可。真正需要支持新事件类型时，照常升级前端代码。

---

## 17. 两端差异总结

|                   | Web                                       | Tauri                               |
| ----------------- | ----------------------------------------- | ----------------------------------- |
| Transport         | `WebSocketTransport`                      | `TauriTransport`                    |
| 后端事件产生      | 独立 axum 进程，`AgentSession` 常驻服务端 | 同应用进程，`AgentSession` 直接持有 |
| 事件送达          | `socket.send()` (WS 帧)                   | `app.emit()` (Tauri IPC)            |
| 指令送达          | `socket.send()` (WS 帧)                   | `invoke()` (Tauri command)          |
| 中断信号          | 独立 JSON 消息 `{"type":"interrupt"}`     | 独立 command `interrupt_session`    |
| 前端 reducer / UI | **完全一致**                              | **完全一致**                        |

核心原则：**`AgentSession` 的事件产生逻辑只写一份，两端只是搬运方式不同；前端只认 `AgentTransport` 接口，不关心背后是 WS 还是 IPC。**
