# WebSocket 连接不可见 — 根因分析报告

> 问题：前端发送消息后，浏览器 Network 标签中看不到 WebSocket 连接
> 日期：2026-07-26
> 范围：ChatView.tsx `send` 流程 + transport.ts WebSocket 生命周期

---

## 根因分析

### 最可能原因：`transportRef.current` 竞态条件 🔴

**文件**: `src/src/components/ChatView.tsx:286-302`
**严重性**: **高**

**描述**: `send` 函数中，`onEvent` 回调在 terminal 事件到达时读取 `transportRef.current` 来 dispose transport。但如果在第一个 transport 的 terminal 事件到达前用户发送了第二条消息，`transportRef.current` 已被指向第二个 transport，导致第一个 transport 的 terminal handler 错误地 dispose 了第二个 transport。

```ts
// ChatView.tsx:286-302
transportRef.current?.dispose()          // 1. dispose 旧 transport T1
const transport = createTransport(sid)    // 2. 创建新 transport T2
transportRef.current = transport          // 3. transportRef.current = T2

transport.onEvent((event) => {
    // ...
    if (terminal) {
        transportRef.current?.dispose()   // 4. T1 的 terminal 事件到达 → 错误地 dispose T2！
    }
})
```

**触发条件**:
1. 用户发送消息 A → 创建 T1，T1 开始接收事件
2. T1 的 terminal 事件尚未到达（agent 还在运行）
3. 用户发送消息 B → dispose T1，创建 T2，`transportRef.current = T2`
4. T1 的 terminal 事件到达 → `onEvent` 回调读取 `transportRef.current`（现在是 T2）→ dispose T2
5. T2 的 WebSocket 刚创建就被关闭，浏览器 Network 标签看不到连接

**修复**: 在 `onEvent` 闭包中捕获 transport 的本地引用，而不是运行时读取 `transportRef.current`：

```ts
const t = transport  // 捕获本地引用
t.onEvent((event) => {
    const prev = threadTurnsRef.current.get(sid) ?? []
    const { turns, terminal } = applyEvent(prev, event)
    threadTurnsRef.current.set(sid, turns)
    setThreadTurns(new Map(threadTurnsRef.current))
    if (terminal) {
        setStreaming(false)
        streamThreadRef.current = null
        t.dispose()  // ← 使用捕获的引用，不是 transportRef.current
        transportRef.current = null
        onReload?.()
    }
})
```

---

### 第二可能原因：`isTauri` 检查导致返回 `TauriTransport` 🔴

**文件**: `src/src/agents/transport.ts:33, 295-297`
**严重性**: **高**

**描述**: `createTransport` 检查 `__TAURI_INTERNALS__ in window`。如果浏览器环境中有任何脚本/扩展注入了此全局变量，将返回 `TauriTransport`，它不使用 WebSocket。

```ts
const isTauri = "__TAURI_INTERNALS__" in window  // 模块加载时求值

export function createTransport(sessionId, onResync?) {
    return isTauri
        ? new TauriTransport(sessionId)        // ← 无 WebSocket
        : new WebSocketTransport(sessionId, onResync)
}
```

**触发条件**: 任何浏览器扩展、polyfill、或 Tauri dev server 注入 `window.__TAURI_INTERNALS__`。

**修复**: 使用更可靠的检测方式，例如构建时注入 `__TAURI__` 标志，或运行时动态检测。

---

### 第三可能原因：`apiUrl` 带路径导致 WebSocket URL 错误 🟠

**文件**: `src/src/agents/transport.ts:35-41`
**严重性**: **中**

**描述**: 如果用户在设置中配置了 `apiUrl = "http://localhost:8080/api"`，`wsBaseUrl()` 返回 `ws://localhost:8080/api`，WebSocket URL 变为 `ws://localhost:8080/api/api/ai/sessions/{id}/ws`（双 `/api`）。

```ts
function wsBaseUrl(): string {
    const base = getApiBaseUrl()  // 例如 "http://localhost:8080/api"
    if (base) {
        return base.replace(/^http:/, "ws:").replace(/^https:/, "wss:")
        // → "ws://localhost:8080/api"  ← 保留了路径
    }
    // ...
}
// URL: ws://localhost:8080/api/api/ai/sessions/{id}/ws  ← 双 /api！
```

**修复**: `wsBaseUrl()` 应只提取 host+port，或 `WebSocketTransport` 的 URL 构造应处理 base URL 的路径。

---

### 第四可能原因：`onclose` 后 `readyPromise` 未刷新 🟠

**文件**: `src/src/agents/transport.ts:135-158`
**严重性**: **中**

**描述**: `onclose` 后 `readyReject` 可能为 null（已被 `onopen` 消费），`readyPromise` 仍指向旧的已 resolve 的 promise。`sendPrompt` 调用 `await this.ready()` 立即返回，然后在已关闭的 socket 上发送。

**修复**: `onclose` 中应创建新的 `readyPromise`，或确保 `readyReject` 始终可用。

---

## 修复优先级

| 优先级 | 问题 | 修复难度 | 影响范围 |
|--------|------|----------|----------|
| 🔴 P0 | `transportRef.current` 竞态 | 低（捕获本地引用） | 多发消息时 WebSocket 被误 dispose |
| 🔴 P0 | `isTauri` 检测不可靠 | 中（改用构建时注入） | 浏览器中可能返回 TauriTransport |
| 🟠 P1 | `apiUrl` 带路径导致 URL 错误 | 低（strip 路径） | 配置了 apiUrl 的用户 |
| 🟠 P1 | `onclose` 后 readyPromise 未刷新 | 低（创建新 promise） | 重连后 sendPrompt 在关闭 socket 上发送 |
