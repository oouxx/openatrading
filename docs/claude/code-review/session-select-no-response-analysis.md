# 选中 Session 后发送消息无响应 — 根因分析报告

> 问题：选中已有 session，发送消息后没有任何反应（无响应、无报错）
> 日期：2026-07-26
> 范围：ChatView.tsx session 选择 + 历史加载 + send 流程

---

## 根因分析

### 最可能原因：历史加载与 send 的竞态条件 🔴

**文件**: `src/src/components/ChatView.tsx:149-150, 276-278`
**严重性**: **高**

**描述**: 选中 session 时，`useEffect` 异步加载历史消息。如果用户在历史加载完成前发送消息，`send` 函数将用户 turn 写入 `threadTurnsRef.current`，但历史加载完成后的 `.then()` 会**覆盖**这个写入。

```ts
// 1. 选中 session → activeThreadId = "abc"
// 2. useEffect 启动异步 fetch
useEffect(() => {
    fetchJson(`/api/ai/sessions/${activeThreadId}/messages`)
        .then((msgs) => {
            // 4. ⚠️ 历史加载完成，覆盖用户刚写入的 turn
            threadTurnsRef.current.set(activeThreadId, turns)
            setThreadTurns(new Map(threadTurnsRef.current))
        })
}, [activeThreadId, ...])

// 3. 用户在历史加载完成前发送消息
const send = useCallback(async () => {
    // ...
    threadTurnsRef.current.set(sid, [...existing, userTurn])  // ← 写入用户 turn
    setThreadTurns(new Map(threadTurnsRef.current))
    // ... 创建 transport，发送消息
}, [...])
```

**触发条件**:
1. 用户选中 session "abc"
2. `useEffect` 启动历史加载（异步 fetch）
3. 用户在历史加载完成前输入消息并发送
4. `send()` 将用户 turn 写入 `threadTurnsRef.current`
5. 历史加载完成 → `.then()` 用历史数据**覆盖** `threadTurnsRef.current`
6. 用户 turn 从 UI 消失
7. 服务端处理了消息并返回响应 → 响应出现在 UI 中，但前面没有用户消息
8. 用户看到"没有反应"（实际上有反应，但用户消息被吞了）

**修复**: 在 `.then()` 中检查是否已有更新的 turns，或使用 ref 标记来避免覆盖：

```ts
.then((msgs) => {
    // 只在没有新数据时写入
    const existing = threadTurnsRef.current.get(activeThreadId)
    if (existing && existing.length > 0) return  // ← 已有数据，不覆盖
    // ...
})
```

---

### 第二可能原因：历史加载失败被静默吞掉 🔴

**文件**: `src/src/components/ChatView.tsx:153`
**严重性**: **高**

**描述**: 历史加载的 `.catch(() => {})` 静默吞掉所有错误。如果历史加载失败（网络错误、session 不存在、500 等），用户看到空白聊天界面，没有任何错误提示。

```ts
fetchJson<HistoryMessage[]>(`/api/ai/sessions/${activeThreadId}/messages`)
    .then((msgs) => {
        if (!msgs || msgs.length === 0) return  // ← 空数据也不提示
        // ...
    })
    .catch(() => {})  // ← 错误被静默吞掉
```

**触发条件**:
1. 用户选中 session
2. 历史加载失败（例如 session 文件损坏、服务端错误）
3. 用户看到空白聊天界面
4. 用户发送消息 → `send()` 使用 `activeThreadId` 创建 transport
5. WebSocket 连接可能成功（服务端 `create_session` 可能成功）
6. 消息被发送，但用户看不到历史上下文
7. 用户以为"没有反应"

**修复**: 至少记录错误，或显示错误提示：

```ts
.catch((e) => {
    console.error("failed to load history:", e)
    // 或显示错误提示
})
```

---

### 第三可能原因：WebSocket 连接后立即关闭 🟠

**文件**: `src/src/agents/transport.ts:96-110, 135-158, 177-181`
**严重性**: **中**

**描述**: WebSocket 连接成功（`onopen` 触发，`readyPromise` resolve）后立即关闭（`onclose` 触发）。此时 `readyResolve` 和 `readyReject` 已被设为 `null`，`onclose` 中的 `readyReject?.()` 是 no-op。`sendPrompt` 调用 `await this.ready()` 立即返回（promise 已 resolve），然后在已关闭的 socket 上发送。

```ts
// 1. WebSocket 连接成功
this.ws.onopen = () => {
    this.readyResolve?.()    // readyPromise resolve
    this.readyResolve = null  // ← 设为 null
    this.readyReject = null   // ← 设为 null
}

// 2. WebSocket 立即关闭
this.ws.onclose = () => {
    this.readyReject?.(...)  // ← no-op，readyReject 为 null
    // 调度重连...
}

// 3. sendPrompt 在已关闭的 socket 上发送
await this.ready()  // 立即返回（promise 已 resolve）
sendCmd(this.ws, ...)  // ← 在已关闭的 socket 上发送，抛出异常
```

**触发条件**: 服务端接受 WebSocket 连接后立即关闭（例如 session 恢复失败、代理超时）。

**修复**: `onclose` 中应创建新的 `readyPromise`，或 `sendCmd` 中检查 `ws.readyState`。

---

### 第四可能原因：`sendCmd` 未检查 WebSocket 状态 🟠

**文件**: `src/src/agents/transport.ts:43-45`
**严重性**: **中**

**描述**: `sendCmd` 无条件调用 `ws.send()`，不检查 `ws.readyState`。当 WebSocket 处于 `CONNECTING`、`CLOSING` 或 `CLOSED` 状态时，`send()` 抛出 `InvalidStateError`。

```ts
function sendCmd(ws: WebSocket, cmd: ClientCommand): void {
    ws.send(JSON.stringify(cmd))  // ← 不检查 readyState！
}
```

**触发条件**: 任何 `ready()` resolve 但 WebSocket 未处于 `OPEN` 状态的场景。

**修复**: 添加 readyState 检查：

```ts
function sendCmd(ws: WebSocket, cmd: ClientCommand): void {
    if (ws.readyState !== WebSocket.OPEN) {
        throw new Error("websocket not open")
    }
    ws.send(JSON.stringify(cmd))
}
```

---

## 修复优先级

| 优先级 | 问题 | 修复难度 | 影响范围 |
|--------|------|----------|----------|
| 🔴 P0 | 历史加载与 send 竞态 | 低（加检查） | 快速操作时用户消息被吞 |
| 🔴 P0 | 历史加载失败静默吞掉 | 低（加错误处理） | 所有 session 选择 |
| 🟠 P1 | WebSocket 连接后立即关闭 | 中（修复 readyPromise 生命周期） | 网络不稳定时 |
| 🟠 P1 | sendCmd 未检查 readyState | 低（加检查） | 所有 WebSocket 发送 |
