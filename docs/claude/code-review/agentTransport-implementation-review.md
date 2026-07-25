# agentTransport.md 实施情况核对报告

> 生成日期：2026-07-25
> 范围：`docs/agentTransport.md` 各章节 vs 实际代码实现
> 方法：逐节核对 + 8 角度代码审查（正确性、缺失行为、跨文件追踪、复用、简化、效率、深度、约定）

---

## 总体评估

| 指标 | 值 |
|------|-----|
| 总章节数 | 15（§1–§17，不含空节 §8/§13） |
| 完全实现 | 10 |
| 部分实现 | 3 |
| 未实现 | 2 |
| 实现偏差 | 3 |
| 严重问题 | 4（其中 2 个使断线重连完全失效） |

---

## 逐节核对

### §1 整体架构 — ✅ 完全实现

- 单 channel 合并两路事件流 ✅
- WebSocket（Web）+ Tauri IPC（桌面）双 transport ✅
- 前端 reducer/UI 两端复用 ✅

### §2 后端事件模型 — ✅ 完全实现

- `AgentEvent`（异步）通过 `subscribe` 注册 ✅
- `AgentSessionEvent`（同步）通过 `subscribe_session_events` 注册 ✅
- 统一 `TranscriptEvent` enum ✅
- 中断走独立通道（`session.abort()`） ✅

### §3 传输层 — ✅ 完全实现

#### 3.1 统一 WebSocket — ✅
#### 3.2 axum WebSocket handler — ✅
- `ws_handler` + `WebSocketUpgrade` ✅
- `TranscriptEvent` 带 `#[serde(tag = "kind")]` ✅
- `ClientCommand` 含 Prompt/Interrupt/Ping/PermissionResponse ✅
- 错误处理 ✅

#### 3.3 Tauri IPC — ✅
- `send_prompt` / `interrupt_session` / `respond_permission` commands ✅
- `TauriSessionRegistry` 管理 session 生命周期 ✅
- `app.emit("agent-event", ...)` 推送事件 ✅

### §4 前端 Transport 抽象 — ⚠️ 实现偏差

- `AgentTransport` interface（onEvent/sendPrompt/interrupt/respondToPermission/dispose/ready） ✅
- `WebSocketTransport` class ✅
- `TauriTransport` class ✅
- `createTransport` 工厂函数 ✅
- **`readyPromise` 在重连后未刷新** ⚠️ — 见发现 #3
- **`onclose` 发送合成 `agent_end`** ⚠️ — 见发现 #4

### §5 Block 数据模型 + Reducer — ⚠️ 部分实现

- `TranscriptBlock` / `TranscriptTurn` 类型 ✅
- `applyEvent` 纯函数 ✅
- Core 事件处理（text_delta/thinking_delta/toolcall_delta/toolcall_end/done/error） ✅
- Session 事件处理（turn_start/turn_end/agent_settled/agent_end/compaction/retry/permission_request/notice） ✅
- **`Done` 事件未覆盖完整内容** ⚠️ — 见发现 #5
- **`appendDelta`/`finalizeBlock`/`appendToolCallDelta` 查找逻辑有 bug** ⚠️ — 见发现 #6

### §6 UI 渲染 — ✅ 完全实现

- `TurnView` 组件 ✅
- Block 类型分派 ✅
- Markdown 增量渲染（streaming → 纯文本，done → react-markdown） ✅
- Auto-scroll（threshold 判断） ✅
- 工具卡片按 name 分派 ✅
- Diff view for edit tools ✅

### §7 Session 列表/历史/切换 — ⚠️ 部分实现

#### 7.1 会话列表 API — ✅
- `GET /api/ai/sessions` ✅
- `GET /api/ai/sessions/:id/messages` ✅
- Tauri 通过 `api_fetch` 复用 ✅

#### 7.2 历史消息 ≠ 事件流重放 — ⚠️ 部分实现
- `historyToTurns` 独立函数 ✅
- 跳过 delta 拼接 ✅
- **`historyToTurns` 未处理 tool_use/thinking block** ⚠️ — 见发现 #10

#### 7.3 切换 session 时序 — ✅
- 先 dispose 旧 transport ✅
- 再 REST 加载历史 ✅
- 最后建新 transport ✅

#### 7.4 正在运行的 session 边界 — ✅
- `running_sessions` 跟踪 ✅
- 加载历史时插入占位 turn ✅
- `AgentEnd`/`AgentSettled` 清除 running 状态 ✅

#### 7.5 Session 侧边栏 — ❌ 未实现
- 无 `SessionSidebar` 组件
- 无轮询更新机制

### §9 WS 断线重连 — ❌ 完全失效

#### 9.1 后端事件缓冲 — ❌ 失效
- `SessionEventBuffer` 环形缓冲 ✅（实现正确）
- **`seq` 始终为 0** ❌ — 见发现 #1（序列化在 `push()` 之前）
- **Buffer 是 per-connection 而非 per-session** ❌ — 见发现 #2
- `resync_required` 信号 ✅（但永远不会触发，因为 buffer 始终为空）

#### 9.2 前端心跳 + 退避重连 — ⚠️ 部分实现
- 15s 心跳 ping ✅
- 指数退避（1s → 30s） ✅
- `lastSeq` 追踪 ✅（但始终为 0，见发现 #1）
- `onResync` 回调 ✅
- **`readyPromise` 重连后未刷新** ❌ — 见发现 #3
- **合成 `agent_end` 导致 turn 提前结束** ❌ — 见发现 #4

### §10 事件顺序保证 — ⚠️ 实现偏差

#### 10.1 单 channel 定序 — ⚠️ 实现偏差
- 实际使用双 channel（delta + durable）而非 spec 描述的单 channel
- 这是有意升级（增加背压），但引入了优先级问题

#### 10.2 前端防御乱序 — ✅
- `appendDelta` 自动创建兜底 turn ✅

### §11 工具调用审批 — ⚠️ 部分实现

#### 11.1 后端 — ❌ 未接线
- `PermissionResponse` handler 仅打 log，未接入 pi-coding-agent 权限系统
- `respond_permission` 方法为 no-op
- `PermissionRequest` 事件从未被 `convert_session_event` 构造

#### 11.2 前端 — ✅
- `permission_request` block 类型 ✅
- `respondToPermission` transport 方法 ✅
- `PermissionRequestCard` 组件 ✅
- **`ClientCommand` 类型缺少 `permission_response`** ⚠️ — 见发现 #7

### §12 Session CRUD — ✅ 完全实现

- `POST /api/ai/sessions` ✅
- `DELETE /api/ai/sessions/:id` ✅
- `PATCH /api/ai/sessions/:id` ✅
- **Title 自动生成** ❌ 未实现（spec 标注为可选）

### §14 Channel 背压 — ⚠️ 实现偏差

- 双 channel 设计（delta 64 / durable 256） ✅
- Delta 事件 `try_send`（满则丢） ✅
- Durable 事件 `await send`（阻塞等待） ✅
- **`tokio::select!` 未使用 `biased`** ⚠️ — 见发现 #8
- **`Done` 事件未覆盖完整内容** ⚠️ — 见发现 #5

### §15 大体积工具输出截断 — ✅ 完全实现

- `TOOL_RESULT_PREVIEW_LIMIT = 2000` ✅
- `to_tool_result_payload` 函数 ✅
- `ToolResultPayload` struct ✅
- `GET /api/ai/sessions/:id/tool-results/:tool_call_id` 端点 ✅
- 前端 `ResultBlock` 展开按钮 ✅

### §16 Schema 版本兼容 — ✅ 完全实现

#### 16.1 前端未知事件静默忽略 — ✅
- `applyEvent` default 分支返回 turns ✅
- `applyCoreEvent` / `applySessionEvent` default 分支 ✅

#### 16.2 后端事件信封带版本号 — ✅
- `EventEnvelope` 含 `schema_version` + `seq` ✅
- `SCHEMA_VERSION = 1` ✅

### §17 两端差异总结 — ✅ 完全实现

- WebSocketTransport / TauriTransport ✅
- 共享 reducer/UI ✅

---

## 发现的问题

### 发现 #1 — `seq` 始终为 0（断线重连完全失效）🔴

**文件**: `crates/web/src/agent_ws.rs:500-507, 518-525`, `src-tauri/src/main.rs:1160-1167`
**严重性**: **严重**
**来源**: Angle A, Angle F, Angle G

**描述**: `EventEnvelope` 以 `seq: 0` 构造，序列化为 JSON，然后才调用 `buffer.push(envelope)` 分配真实 seq。客户端收到的所有事件 `seq` 都是 0，`lastSeq` 永远无法推进。

```rust
// agent_ws.rs:500-507
let envelope = EventEnvelope {
    schema_version: SCHEMA_VERSION,
    seq: 0,          // ← 硬编码 0
    event,
};
let json = serde_json::to_string(&envelope);  // ← 序列化时 seq=0
let _seq = buffer.lock().await.push(envelope); // ← push 修改了 envelope.seq，但 json 已生成
```

**后果**: 客户端 `lastSeq` 始终为 0。重连时发送 `?last_seq=0`，服务端 `events_since(Some(0))` 返回所有 seq>0 的事件（即全部），导致重复回放。每次重连都重复所有事件，且 `resync_required` 永远不会触发（因为 `last_seq=0` 始终在 buffer 窗口内）。

**修复**: 先 `push` 获取真实 seq，再序列化。

---

### 发现 #2 — Buffer 是 per-connection 而非 per-session（重连回放永远读空 buffer）🔴

**文件**: `crates/web/src/agent_ws.rs:463-492`
**严重性**: **严重**
**来源**: Angle B, Angle D

**描述**: `handle_session` 在每次连接时创建全新的空 buffer（line 463），然后立即查询回放（line 468）。事件 channel 在 line 492 才建立——在回放块之后。即使 buffer 有数据，它也是局部变量，连接断开时被丢弃。

```rust
// agent_ws.rs:463
let buffer = Arc::new(Mutex::new(SessionEventBuffer::new(500)));  // ← 每次连接新建

// 回放块（line 466-490）— buffer 为空，永远回放不出事件
{
    let buf = buffer.lock().await;
    match buf.events_since(last_seq) { ... }
}

// 事件 channel（line 492）— 在回放块之后才建立
let (mut delta_rx, mut durable_rx) = setup_event_channel(&agent_facade).await;
```

`setup_event_channel_with_buffer` 函数（line 401）存在但未被 `handle_session` 调用——它只在 Tauri 端使用。

**后果**: 客户端断线后重连，buffer 为空，所有断线期间产生的事件永久丢失。`resync_required` 信号也不会发送（因为空 buffer 不会触发 eviction 检查）。

**修复**: 将 buffer 存储在 `AppState` 中，按 session_id 共享。使用 `setup_event_channel_with_buffer` 替代 `setup_event_channel`。

---

### 发现 #3 — `readyPromise` 重连后未刷新（`sendPrompt` 可能在关闭的 socket 上发送）🔴

**文件**: `src/src/agents/transport.ts:66-69, 85-87, 173-175`
**严重性**: **严重**
**来源**: Angle A, Angle F

**描述**: `readyPromise` 在构造函数中创建一次。`onopen` 将其 resolve 后设置 `readyResolve = null`。后续重连时 `onopen` 再次触发，但 `readyResolve?.()` 是 no-op（为 null）。`ready()` 永远返回已 resolve 的旧 promise，即使 WebSocket 处于 `CONNECTING` 状态。

```ts
// transport.ts:66-69
this.readyPromise = new Promise<void>((resolve, reject) => {
    this.readyResolve = resolve
    this.readyReject = reject
})

// transport.ts:85-87 — 首次连接成功后
this.readyResolve?.()  // resolve
this.readyResolve = null  // ← 设为 null

// 重连时 onopen 再次触发，但 readyResolve 为 null，readyPromise 已 resolve
```

**后果**: 网络闪断后，用户在退避期间发送消息。`sendPrompt` 调用 `await this.ready()` 立即返回（promise 已 resolve），然后 `ws.send()` 在未打开的 socket 上调用，消息丢失。

**修复**: 每次 `connect()` 时创建新的 `readyPromise`。

---

### 发现 #4 — 每次 WS 断开都发送合成 `agent_end`（导致 turn 提前结束）🔴

**文件**: `src/src/agents/transport.ts:122-129`
**严重性**: **严重**
**来源**: Angle A, Angle F

**描述**: `onclose` 无条件向所有 listener 发送 `{ kind: "session", type: "agent_end", will_retry: false }`。reducer 将 `agent_end` 视为终端信号（`terminal: true`），标记当前 turn 为 complete。WS 重连后新 delta 到达时，turn 已 complete，delta 被放入新的兜底 turn。

```ts
// transport.ts:122-129
this.ws.onclose = () => {
    this.clearHeartbeat()
    if (this.disposed) return
    for (const cb of this.listeners) {
        cb({ kind: "session", type: "agent_end", will_retry: false } as TranscriptEvent)
    }
    // ... 调度重连
}
```

**后果**: 网络闪断 2 秒，agent 正在流式输出。`onclose` 发送合成 `agent_end`，reducer 标记 turn complete。WS 重连后 agent 继续输出，delta 到达时 turn 已 complete，`appendDelta` 创建新的兜底 turn。用户看到一条消息被分割成两个 turn。

**修复**: 合成 `agent_end` 只应在 `dispose()` 时发送，不在 `onclose` 中发送。或者使用 `will_retry: true` 让 reducer 知道即将重连。

---

### 发现 #5 — `Done` 事件未覆盖完整内容（背压恢复机制不完整）🟠

**文件**: `src/src/agents/reducer.ts:286-291`
**严重性**: **高**
**来源**: Angle B, Angle G

**描述**: §14.2 明确要求 `Done` 事件携带完整 `content` 并覆盖当前 block，以纠正因背压丢弃 `text_delta` 导致的文本缺失。但当前实现仅检查 `sub.message?.errorMessage`，未读取 `sub.content` 也未调用 `finalizeBlock`。

```ts
// reducer.ts:286-291
case "done": {
    const err = sub.message?.errorMessage
    if (err) return appendSystemNotice(turns, "notice", err, { isError: true, status: "error" })
    return turns  // ← 没有覆盖 block content！
}
```

**后果**: 如果 `text_delta` 因背压被丢弃，前端展示的文本永久少于实际生成内容，用户看到不完整的回复。

**修复**: 在 `done` 分支中读取 `sub.content`，若存在则调用 `finalizeBlock(turns, "text", sub.content)` 覆盖当前 text block。

---

### 发现 #6 — `appendDelta`/`finalizeBlock`/`appendToolCallDelta` 查找逻辑有 bug 🟠

**文件**: `src/src/agents/reducer.ts:44-55, 82-89, 111-118`
**严重性**: **高**
**来源**: Angle A, Angle G, Angle F

**描述**: 三个函数中查找最后一个匹配 block 的循环在第一次迭代后无条件 `break`，导致只能检查最后一个 block。

```ts
// reducer.ts:44-55
let idx = -1
for (let i = blocks.length - 1; i >= 0; i--) {
    const b = blocks[i]
    if (b.type === blockType) {
        idx = i
        break
    }
    break  // ← 无条件 break！如果最后一个 block 不是目标类型，循环直接结束
}
```

**后果**: 如果 turn 的最后一个 block 不是 text/thinking（例如是 tool_call），`appendDelta` 会错误地创建新 block 而不是追加到已有的 text/thinking block。例如 `[text:"Hello", thinking:"Hmm..."]` 收到 `text_delta " world"` 时，会创建 `[text:"Hello", thinking:"Hmm...", text:" world"]` 而不是正确的 `[text:"Hello world", thinking:"Hmm..."]`。

**修复**: 删除第 55 行的无条件 `break`，让循环继续向前搜索。

---

### 发现 #7 — `ClientCommand` 类型缺少 `permission_response` 🟠

**文件**: `src/src/agents/types.ts:110-113`
**严重性**: **高**
**来源**: Angle A, Angle F

**描述**: `ClientCommand` 联合类型只定义了 `prompt`、`interrupt` 和 `ping`。但 `WebSocketTransport.respondToPermission` 发送 `{ type: "permission_response", id, approved }`，不匹配任何成员。

```ts
// types.ts:110-113
export type ClientCommand =
    | { type: "prompt"; text: string }
    | { type: "interrupt" }
    | { type: "ping" }
    // ← 缺少 { type: "permission_response"; id: string; approved: boolean }
```

**后果**: TypeScript 编译错误（若 `skipLibCheck` 隐藏了它，则运行时后端 `ClientCommand` enum 期望 `PermissionResponse` 但前端类型系统不知道它）。即使通过 `as any` 绕过，后端 handler 也是 no-op stub。

**修复**: 在 `ClientCommand` 类型中添加 `permission_response` 变体。

---

### 发现 #8 — `tokio::select!` 未使用 `biased`（背压优先级失效）🟠

**文件**: `crates/web/src/agent_ws.rs:497`, `src-tauri/src/main.rs:1149-1153`
**严重性**: **高**
**来源**: Angle B, Angle F, Angle G

**描述**: §14 明确要求 `durable_rx` 优先于 `delta_rx`，应使用 `biased` 模式。但当前实现使用默认的 `tokio::select!`（unbiased，随机选择就绪分支）。

```rust
// agent_ws.rs:497
tokio::select! {
    // ← 缺少 biased;
    Some(event) = delta_rx.recv() => { ... }
    Some(event) = durable_rx.recv() => { ... }
    msg = socket.recv() => { ... }
}
```

**后果**: 在 delta 事件密集时，`TurnEnd`/`AgentSettled` 等关键生命周期事件可能被延迟处理。前端可能先收到 5 个 `text_delta` 再收到 `TurnStart`，导致 `appendDelta` 自动创建兜底 turn，然后 `startTurn` 又创建第二个 turn——产生重复 turn。

**修复**: 改为 `tokio::select! { biased; ... }`，并将 `durable_rx` 放在第一个分支。

---

### 发现 #9 — `PermissionResponse` 后端未接线 🟡

**文件**: `crates/web/src/agent_ws.rs:557-561`, `src-tauri/src/main.rs:1239-1241`
**严重性**: **中**
**来源**: Angle G, Angle B

**描述**: §11 设计了完整的审批流程，但后端 `PermissionResponse` handler 仅打印 warning，`respond_permission` 方法为 no-op。前端已完整实现（`PermissionRequestCard` + `respondToPermission`），但后端不处理。此外，`PermissionRequest` 事件从未被 `convert_session_event` 构造——没有代码路径将任何 `AgentSessionEvent` 变体映射到 `PermissionRequest`。

```rust
// agent_ws.rs:557-561
Ok(ClientCommand::PermissionResponse { id, approved }) => {
    log::warn!("permission response received but not yet wired: id={id}, approved={approved}");
}
```

**后果**: 前端用户点击"允许"/"拒绝"后，agent 不会收到响应，工具调用会永久挂起。

**修复**: 接入 pi-coding-agent 的 permission system（待 pi-rs 暴露接口后完成）。

---

### 发现 #10 — `historyToTurns` 丢弃 thinking/tool block 🟡

**文件**: `src/src/agents/reducer.ts:465-474`
**严重性**: **中**
**来源**: Angle F, Angle G

**描述**: §7.2 的 `contentBlockToTranscriptBlock` 函数定义了 tool_use → tool_call block 和 thinking block 的转换，但 `historyToTurns` 仅提取 text content，丢弃了 tool_use 和 thinking block。

```ts
// reducer.ts:465-474
export function historyToTurns(messages: HistoryMessage[]): TranscriptTurn[] {
    return messages.map((m) => ({
        id: newTurnId(),
        role: m.role === "user" ? "user" : "assistant",
        status: "complete" as const,
        blocks: m.content
            ? [{ type: "text" as const, id: newBlockId(), content: m.content, streaming: false }]
            : [],
    }))
}
```

**后果**: 加载历史 session 时，工具调用和思考过程不会显示，用户只能看到纯文本消息。

**修复**: 扩展 `HistoryMessage` 类型以包含结构化 content blocks，实现完整的 `contentBlockToTranscriptBlock` 转换。

---

### 发现 #11 — `convert_session_event` 使用 Debug 格式化用户可见字符串 🟡

**文件**: `crates/web/src/agent_ws.rs:175, 184`
**严重性**: **中**
**来源**: Angle F

**描述**: `CompactionStart` 和 `CompactionEnd` 的 `reason` 字段使用 `format!("{reason:?}")`（Debug 格式化），输出类似 `CompactionReason::ContextLengthExceeded(12345)` 而非人类可读字符串。

```rust
AgentSessionEvent::CompactionStart { reason } => SessionEventPayload::CompactionStart {
    reason: format!("{reason:?}"),  // ← Debug 格式化
},
```

**后果**: 用户看到 Rust 调试表示，如 `CompactionReason::ContextLengthExceeded(12345)`。

**修复**: 使用 `Display` 格式化或手动转换为可读字符串。

---

### 发现 #12 — Slash 命令结果缺少 `EventEnvelope` 包装 🟡

**文件**: `src-tauri/src/main.rs:1051-1062`
**严重性**: **中**
**来源**: Angle F

**描述**: Tauri 端 slash 命令结果直接以裸 `TranscriptEvent::Session(notice)` 发送，没有 `EventEnvelope` 包装（无 `schema_version`、无 `seq`）。而 forward task 发送的每个事件都经过 `EventEnvelope` 包装。

```rust
// main.rs:1051-1062
let payload = serde_json::json!({
    "session_id": session_id,
    "event": openatrading_web::agent_ws::TranscriptEvent::Session(notice),  // ← 裸事件
});
```

**后果**: 前端收到的事件形状不一致——有些有 envelope 字段，有些没有。`TauriTransport` 的 `lastSeq` 追踪检查 `typeof envelope.seq === "number"`，slash 结果中 `seq` 为 `undefined`，所以 `lastSeq` 不更新。Tauri 无重连所以无害，但代码不一致。

**修复**: 将 slash 结果包装在 `EventEnvelope` 中。

---

### 发现 #13 — 重复序列化 🟢

**文件**: `src-tauri/src/main.rs:1160-1169`
**严重性**: **低**
**来源**: Angle F

**描述**: Tauri forward task 中，envelope 先序列化为 `serde_json::Value`，然后包装到另一个对象中再次序列化。

```rust
let event_value = serde_json::to_value(&envelope).unwrap_or_default();  // 序列化 #1
let _seq = buffer.lock().await.push(envelope);
let payload = serde_json::json!({ "session_id": sid, "event": event_value });  // 序列化 #2
```

**后果**: 每个事件多一次序列化开销。在 delta 密集时 CPU 浪费可感知。

**修复**: 直接构造最终 payload 的 JSON 字符串，避免中间 `serde_json::Value`。

---

### 发现 #14 — `replaceLast` O(turns) 浅拷贝 🟢

**文件**: `src/src/agents/reducer.ts:181-185`
**严重性**: **低**
**来源**: Angle F

**描述**: 每次事件处理都通过 `replaceLast` 做 `turns.slice()`——完整浅拷贝 turns 数组。对于 N 个 turn 的 session 和 M 个事件，这是 O(N×M) 指针拷贝。

```ts
function replaceLast(turns: TranscriptTurn[], turn: TranscriptTurn): TranscriptTurn[] {
    const next = turns.slice()  // ← 每次拷贝整个数组
    next[next.length - 1] = turn
    return next
}
```

**后果**: 30 个 turn 的 session 接收 800 个 text delta → 24,000 次不必要的引用拷贝。

**修复**: 使用 `turns.with(-1, turn)`（ES2023）或 immer。

---

### 发现 #15 — `unwrap()`/`expect()` 在生产路径 🟢

**文件**: 多处
**严重性**: **低**
**来源**: Angle H

**描述**: 多处使用 `unwrap()`/`expect()` 且无注释说明 panic 不可能发生的不变量。

| 文件 | 行 | 代码 |
|------|----|------|
| `agent_ws.rs` | 516, 548, 577 | `state.running_sessions.lock().unwrap()` |
| `main.rs` | 1072, 1158 | `sessions.running_sessions.lock().unwrap()` |
| `main.rs` | 1438, 1444, 1483 | `expect("...")` |

**违反规则**: CLAUDE.md 第 46 行："生产路径禁止 `unwrap()` / `expect()`"

---

### 发现 #16 — 丢弃的 `JoinHandle` 🟢

**文件**: `src-tauri/src/main.rs:1147, 1426`
**严重性**: **低**
**来源**: Angle H

**描述**: `tokio::spawn` 返回的 `JoinHandle` 被丢弃，panic 被静默吞掉。

**违反规则**: CLAUDE.md 第 40 行："不忽略 `spawn` 任务的 `JoinHandle` 错误"

---

### 发现 #17 — 硬编码魔法数字 🟢

**文件**: 多处
**严重性**: **低**
**来源**: Angle H

**描述**: 多处使用硬编码数字而非命名常量。

| 文件 | 行 | 值 | 含义 |
|------|----|----|------|
| `main.rs` | 1273, 1307 | `Duration::from_secs(3)` | 轮询间隔 |
| `main.rs` | 1427 | `Duration::from_millis(200)` | 重启延迟 |
| `agent_ws.rs` | 409, 463 | `500` | buffer 容量 |
| `transport.ts` | 90 | `15_000` | 心跳间隔 |

**违反规则**: CLAUDE.md 第 106 行："不在业务逻辑中硬编码魔法数字"

---

## 总结

| 严重性 | 数量 | 发现 |
|--------|------|------|
| 🔴 严重 | 4 | #1 seq 始终为 0、#2 buffer per-connection、#3 readyPromise 未刷新、#4 合成 agent_end |
| 🟠 高 | 4 | #5 Done 未覆盖内容、#6 appendDelta 查找 bug、#7 ClientCommand 缺类型、#8 缺少 biased |
| 🟡 中 | 4 | #9 Permission 未接线、#10 historyToTurns 丢 block、#11 Debug 格式化、#12 Slash 缺包装 |
| 🟢 低 | 4 | #13 重复序列化、#14 replaceLast 拷贝、#15 unwrap、#16 JoinHandle、#17 魔法数字 |

### 核心建议（按优先级）

1. **🔴 修复 seq 序列化顺序**：先 `push` 获取真实 seq，再序列化（#1）
2. **🔴 将 buffer 改为 per-session 共享**：存储在 `AppState` 中（#2）
3. **🔴 修复 `readyPromise` 刷新**：每次 `connect()` 创建新 promise（#3）
4. **🔴 移除 `onclose` 中的合成 `agent_end`**：只在 `dispose()` 时发送（#4）
5. **🟠 `Done` 事件覆盖 block content**：实现背压恢复（#5）
6. **🟠 修复 `appendDelta` 等函数的查找循环**：删除无条件 `break`（#6）
7. **🟠 添加 `biased` 到 `tokio::select!`**：确保 durable 事件优先（#8）
8. **🟠 修复 `ClientCommand` 类型**：添加 `permission_response`（#7）
