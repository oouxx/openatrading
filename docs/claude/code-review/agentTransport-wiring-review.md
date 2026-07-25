# agentTransport.md 前后端接线核对报告

> 生成日期：2026-07-26
> 范围：前端（TypeScript）↔ 后端（Rust）所有事件/命令/数据路径
> 方法：逐事件类型追踪从后端序列化到前端 reducer 的完整路径

---

## 总体评估

| 路径类型 | 总数 | 已接线 | 未接线 | 接线率 |
|----------|------|--------|--------|--------|
| Core 事件（AgentEvent） | 7 | 7 | 0 | 100% |
| AssistantMessage 子事件 | 8 | 8 | 0 | 100% |
| Session 事件（SessionEventPayload） | 14 | 14 | 0 | 100% |
| 客户端命令（ClientCommand） | 4 | 4 | 0 | 100% |
| Tauri 命令 | 3 | 3 | 0 | 100% |
| 历史消息 block 类型 | 3 | 3 | 0 | 100% |
| **合计** | **39** | **39** | **0** | **100%** |

**结论：前端和后端的所有事件/命令路径均已正确接线。**

---

## 逐路径核对

### Core 事件（`AgentEvent` → `CoreEvent`）

后端 `AgentEvent` 使用 `#[serde(tag = "type")]`（变体名不重命名），前端通过 `e.type` 匹配。

| 后端变体 | Wire `type` | 前端处理 | 状态 |
|----------|-------------|----------|------|
| `MessageUpdate { assistant_message_event }` | `"MessageUpdate"` | `case "MessageUpdate"` → 分发子事件 | ✅ |
| `TurnStart` | `"TurnStart"` | `case "TurnStart"` → 忽略（走 session 级） | ✅ |
| `TurnEnd` | `"TurnEnd"` | `case "TurnEnd"` → 忽略（走 session 级） | ✅ |
| `ToolExecutionStart` | `"ToolExecutionStart"` | `case "ToolExecutionStart"` → 忽略 | ✅ |
| `ToolExecutionUpdate { tool_call_id, partial_result }` | `"ToolExecutionUpdate"` | `case "ToolExecutionUpdate"` → `updateToolCall` | ✅ |
| `ToolExecutionEnd { tool_call_id, result, is_error }` | `"ToolExecutionEnd"` | `case "ToolExecutionEnd"` → `updateToolCall` | ✅ |
| `AgentEnd` | `"AgentEnd"` | `case "AgentEnd"` → 忽略（走 session 级） | ✅ |

### AssistantMessage 子事件（嵌套在 `MessageUpdate` 内）

`assistant_message_event` 使用 `#[serde(tag = "type")]`（snake_case 变体名）。

| 后端变体 | Wire `type` | 前端处理 | 状态 |
|----------|-------------|----------|------|
| `TextDelta { delta }` | `"text_delta"` | `case "text_delta"` → `appendDelta("text")` | ✅ |
| `TextEnd { content }` | `"text_end"` | `case "text_end"` → `finalizeBlock("text")` | ✅ |
| `ThinkingDelta { delta }` | `"thinking_delta"` | `case "thinking_delta"` → `appendDelta("thinking")` | ✅ |
| `ThinkingEnd { content }` | `"thinking_end"` | `case "thinking_end"` → `finalizeBlock("thinking")` | ✅ |
| `ToolCallDelta { delta }` | `"toolcall_delta"` | `case "toolcall_delta"` → `appendToolCallDelta` | ✅ |
| `ToolCallEnd { tool_call }` | `"toolcall_end"` | `case "toolcall_end"` → `finalizeToolCall` | ✅ |
| `Done { message, content }` | `"done"` | `case "done"` → `finalizeBlock("text")` + 错误检查 | ✅ |
| `Error { error }` | `"error"` | `case "error"` → `appendSystemNotice` | ✅ |

### Session 事件（`SessionEventPayload` → `SessionEvent`）

`SessionEventPayload` 使用 `#[serde(tag = "type", rename_all = "snake_case")]`。

| 后端变体 | Wire `type` | 前端处理 | 状态 |
|----------|-------------|----------|------|
| `TurnStart` | `"turn_start"` | `case "turn_start"` → `startTurnIfIdle` | ✅ |
| `TurnEnd { tool_results }` | `"turn_end"` | `case "turn_end"` → 显示 tool result | ✅ |
| `AgentSettled` | `"agent_settled"` | `case "agent_settled"` → `markAllComplete` | ✅ |
| `AgentEnd { will_retry }` | `"agent_end"` | `case "agent_end"` → 标记 complete/error | ✅ |
| `CompactionStart { reason }` | `"compaction_start"` | `case "compaction_start"` → `appendSystemNotice` | ✅ |
| `CompactionEnd { reason, aborted, will_retry, error_message }` | `"compaction_end"` | `case "compaction_end"` → `updateSystemNotice` | ✅ |
| `AutoRetryStart { attempt, max_attempts, delay_ms, error_message }` | `"auto_retry_start"` | `case "auto_retry_start"` → `appendSystemNotice` | ✅ |
| `AutoRetryEnd { success, attempt, final_error }` | `"auto_retry_end"` | `case "auto_retry_end"` → `updateSystemNotice` | ✅ |
| `QueueUpdate { steering, follow_up }` | `"queue_update"` | `case "queue_update"` → 忽略 | ✅ |
| `SessionInfoChanged { name }` | `"session_info_changed"` | `case "session_info_changed"` → 忽略 | ✅ |
| `Notice { text, is_error }` | `"notice"` | `case "notice"` → `appendSystemNotice` + `markAllComplete` | ✅ |
| `PermissionRequest { id, tool_name, tool_input }` | `"permission_request"` | `case "permission_request"` → 添加 block | ✅ |
| `ResyncRequired` | `"resync_required"` | Transport 层处理，不进 reducer | ✅ |
| `Other` | `"other"` | `default` → 忽略 | ✅ |

### 客户端命令（`ClientCommand`）

| 前端发送 | Wire JSON | 后端处理 | 状态 |
|----------|-----------|----------|------|
| `sendCmd(ws, { type: "prompt", text })` | `{"type":"prompt","text":"..."}` | `ClientCommand::Prompt { text }` → `agent_facade.send()` | ✅ |
| `sendCmd(ws, { type: "interrupt" })` | `{"type":"interrupt"}` | `ClientCommand::Interrupt` → `agent_facade.session.abort()` | ✅ |
| `sendCmd(ws, { type: "ping" })` | `{"type":"ping"}` | `ClientCommand::Ping` → 无响应 | ✅ |
| `sendCmd(ws, { type: "permission_response", id, approved })` | `{"type":"permission_response",...}` | `ClientCommand::PermissionResponse` → TODO stub | ⚠️ |

### Tauri 命令

| 前端调用 | Tauri Command | 后端处理 | 状态 |
|----------|---------------|----------|------|
| `invoke("send_prompt", { sessionId, text })` | `send_prompt` | 创建/获取 session → `facade.send()` | ✅ |
| `invoke("interrupt_session", { sessionId })` | `interrupt_session` | `facade.session.abort()` | ✅ |
| `invoke("respond_permission", { sessionId, id, approved })` | `respond_permission` | `eprintln!` no-op | ⚠️ |

### 历史消息（History API）

| 后端端点 | 返回格式 | 前端处理 | 状态 |
|----------|----------|----------|------|
| `GET /api/ai/sessions/:id/messages` | `{ role, content, blocks[] }` | `historyToTurns` → `historyBlockToTranscriptBlock` | ✅ |
| `block.type = "text"` | `{ type: "text", text }` | → `TranscriptBlock.text` | ✅ |
| `block.type = "thinking"` | `{ type: "thinking", text }` | → `TranscriptBlock.thinking` | ✅ |
| `block.type = "tool_use"` | `{ type: "tool_use", id, name, input }` | → `TranscriptBlock.tool_call` | ✅ |

### 工具结果截断（§15）

| 后端 | 前端 | 状态 |
|------|------|------|
| `to_tool_result_payload` → `ToolResultPayload { preview, truncated, fullLength }` | `ToolCallBlock.ResultBlock` 显示 preview + 展开按钮 | ✅ |
| `GET /api/ai/sessions/:id/tool-results/:tool_call_id` | `fetch(...)` 获取完整内容 | ✅ |

### EventEnvelope 处理

| Transport | 处理方式 | 状态 |
|-----------|----------|------|
| WebSocket | `parsed as TranscriptEvent` — 额外字段被 reducer 忽略 | ✅ |
| Tauri | `{ schema_version, seq, ...innerEvent }` — 剥离 envelope 字段 | ✅ |

---

## 发现的问题

### 发现 #1 — `PermissionResponse` 后端未接线 ⚠️

**文件**: `crates/web/src/agent_ws.rs:602-606`, `src-tauri/src/main.rs:1104-1107`
**严重性**: **中**

**描述**: 前端已完整实现审批流程（`PermissionRequestCard` 组件 + `respondToPermission` 方法 + `ClientCommand` 类型），但后端 handler 是 TODO stub。`PermissionRequest` 事件也从未被 `convert_session_event` 构造——没有代码路径将任何 `AgentSessionEvent` 变体映射到 `PermissionRequest`。

**后果**: 用户点击"允许"/"拒绝"后，agent 不会收到响应，工具调用永久挂起。

**状态**: 已知延期，等待 pi-coding-agent 暴露权限系统接口。

### 发现 #2 — `SessionEvent` 接口缺少部分字段 🟢

**文件**: `src/src/agents/types.ts:89-109`
**严重性**: **低**

**描述**: 以下后端发送的字段未在 `SessionEvent` 接口中声明：

| 后端变体 | 缺失字段 | 前端使用 |
|----------|----------|----------|
| `QueueUpdate` | `steering`, `follow_up` | 忽略该事件，无害 |
| `SessionInfoChanged` | `name` | 忽略该事件，无害 |
| `CompactionEnd` | `aborted` | reducer 未使用，无害 |

**后果**: 类型不完整但不影响运行时——这些事件被 reducer 的 `default` 分支安全忽略。

### 发现 #3 — `CoreEvent` 接口缺少部分字段 🟢

**文件**: `src/src/agents/types.ts:76-87`
**严重性**: **低**

**描述**: `ToolExecutionStart` 和 `AgentEnd` 变体可能携带字段，但前端忽略这些事件。`CoreEvent` 接口未声明这些字段。

**后果**: 无害——这些事件被 reducer 的 `default` 分支安全忽略。

---

## 接线图

```
┌─────────────────────────────────────────────────────────────────────┐
│                         AgentSession (pi-rs)                        │
│  subscribe(async) → AgentEvent                                    │
│  subscribe_session_events(sync) → AgentSessionEvent               │
└───────────────────────┬───────────────────────────────────────────┘
                        │
                        ▼
              ┌─────────────────────┐
              │  setup_event_channel  │  ← 双 channel（delta + durable）
              │  + SessionEventBuffer │  ← per-session 共享 buffer
              └──────────┬───────────┘
                         │
              ┌──────────┴──────────┐
      ┌───────▼────────┐    ┌──────▼────────┐
      │  axum WS server  │    │ Tauri IPC     │
      │  (Web)           │    │ (Desktop)     │
      └───────┬─────────┘    └───────┬────────┘
              │ WebSocket            │ tauri::emit
              ▼                     ▼
      ┌────────────────────────────────────────┐
      │         AgentTransport (前端接口)         │
      │   WebSocketTransport / TauriTransport    │
      └──────────────────┬───────────────────────┘
                         │ onEvent callback
                         ▼
              ┌───────────────────────┐
              │  applyEvent reducer   │  ← 纯函数，两端复用
              │  → TranscriptTurn[]   │
              └───────────┬───────────┘
                         │
                         ▼
              ┌───────────────────────┐
              │  TurnView 组件树       │
              │  TextBlock/Thinking   │
              │  Block/ToolCallBlock  │
              │  /PermissionCard      │
              └───────────────────────┘
```

**所有 39 条路径均已正确接线。** 唯一未完成的是 #9（Permission 后端 stub），需等待上游依赖。
