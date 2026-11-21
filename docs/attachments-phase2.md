# Phase 2：附件带外上传（不再把文件塞进消息）

> 状态：**2a–2e 全部落地**。附件不再进消息；前端发送时上传、消息只带 id。
> 前置：Phase 1（WS 单帧 64MB + 前端 10 个/合计 40MB 预算）已完成。

## 为什么要做

Phase 1 把「发不出去」变成了「能发出去」，但没有改变**传输模型**：一条 prompt 的全部
附件被 base64 塞进一个 WebSocket 帧（或 Tauri 的一次 IPC invoke）。由此带来四条硬伤：

1. **体积上限被帧大小绑死**。后端附件预算是 40MB，base64 后 53.4MB，所以单帧开到
   64MB。想支持 200MB 文件就得把帧开到 267MB——内存翻倍、弱网下重传成本极高。
2. **每轮对话重发全量历史**。会话中每发一次消息，历史里的图片 base64 会随请求回到
   provider。Anthropic 官方文档明确劝退这条路（"use the Files API … referencing a
   `file_id`"），32MB 请求体上限一旦撞上，整个会话会**永久 413**。
3. **内存峰值不可控**。`attachments.rs` 现在一次性 `decode` 全部附件进 Vec，20MB 文件
   配 27MB base64 字符串同时驻留。
4. **两条传输路径行为不一致**。Web 走 WS（受帧限制），Tauri 走 JSON IPC（无帧限制但
   大 payload 慢），桌面端和 Web 端能传的东西不一样。

目标对齐 Claude Files API / OpenAI Files API 的形状：**文件先落盘，消息里只留 id**。

## 数据模型

```
~/.atrading/attachments/
└── <session_id>/
    └── <attachment_id>          ← 原始字节，文件名即 id
```

元信息**不建表**：落盘即事实源，`<attachment_id>.json` 旁挂（避免给 SQLite 加迁移）：

```jsonc
// <attachment_id>.json
{
  "id": "01J...",
  "sessionId": "abc123",
  "name": "年报.pdf",
  "size": 12345678,
  "sizeBytes": 12345678,
  "mimeType": "application/pdf",
  "createdAt": 1795226020514,
  "kind": "file"          // "file" | "image"
}
```

理由：附件是**会话的临时产物**，生命周期比会话短；塞进 `data.db` 会让「删会话」变成
跨库事务。旁挂 JSON 还让 `rm -rf <session_id>` 就等于彻底清理，天然对齐「设置 → 缓存」。

## 接口

### 1. 上传

```
POST /api/attachments
Content-Type: multipart/form-data
X-Session-Id: <session_id>       ← 或用 query ?session=

part "file" × N
→ 200 { "attachments": [ { "id", "name", "size", "mimeType", "kind" } ] }
```

要点：

- **流式落盘**，不整体读进内存。axum 侧用 `Multipart` 逐 chunk 写
  `tokio::fs::File`，落完再 `stat` 出真实 size，与后端现有「以自己算出的 size 为准」
  的约定一致（`FileAttachment.size` 目前就只是客户端元信息）。
- 单文件上限提到 **512MB**（对齐 ChatGPT 文档上限），按 `Content-Length` +
  写入计数双重限制，超限**立即中止并删除半成品**。
- 每会话附件总配额（建议 2GB）与文件数配额（建议 50）：超了就 4xx + 明确原因。
- 落盘目录 `0700`，文件名不用用户输入（`name` 只进 JSON，防路径穿越）。
- 上传前检查 `session_id` 存在，避免往任意路径写。

### 2. Prompt 协议（破坏性变更）

`PromptFile` 从「带 base64」变成「带 id」：

```ts
// 之前
{ kind: "file", name, size, mimeType, data: "<base64>" }

// 之后
{ kind: "file", id: "01J...", name, size, mimeType }
```

`ClientCommand::Prompt.files` / `ImageAttachment` 相应只保留 `id` + 展示用元信息。
**为兼容滚动升级**，`attachments.rs` 的 `FileAttachment` 保留 `data: Option<String>`：
`data` 存在走内存路径（旧客户端），只有 `id` 走磁盘路径（新客户端）。两条路径共用
后面的提取/图片归一化逻辑，所以行为不会漂移。

### 3. 生命周期

```
上传 → 引用（prompt 里出现 id）→ 会话归档/删除 → 清理
```

- **删除会话**：`DELETE /api/ai/sessions/:id` 时顺手 `rm -rf attachments/<id>`。
- **孤儿 GC**：启动时 + 每 6 小时扫一次，删掉「所属会话不存在」或「超过 7 天未被任何
  消息引用」的目录。「被引用」靠扫 `agent/sessions/*.jsonl` 里出现过的 id（一次性
  索引，不用维护引用计数，也就不存在计数漂移）。
- **配额满**：先按 LRU 删未引用的附件，而不是直接拒用户。

## 服务端读取路径

`crates/core/src/utils/attachments.rs` 需要新增一个入口，其余逻辑复用：

```rust
pub enum AttachmentSource<'a> {
    Inline { data: &'a str },      // 旧路径：base64 在消息里
    Stored { path: &'a Path },     // 新路径：从磁盘流式读
}
```

- **文本类**（PDF / OOXML / UTF-8）：改成 `File` + 流式读，不要 `read_to_end` 到 Vec。
  `pdf_extract` 需要 `&[u8]`，但 512MB PDF 本来就不该整体进内存 —— 设 64MB 的
  「单文件解析上限」，超过就只提取前 N 页并加 `[内容已截断]` 注记（与现有
  `MAX_TEXT_BYTES` 的截断语义一致）。
- **图片**：`normalize_image` 需要完整字节（要解码），但上限仍是 20MB；超 20MB 的图片
  直接在上传期就拒（不必等到 prompt）。
- `MAX_TEXT_BYTES` 1MB / 合计 2MB **保持不变**：那是 token 守卫，与传输无关。

## 客户端改动

| 位置 | 改动 |
|---|---|
| `src/src/lib/composer-attachments.ts` | `readComposerAttachments` 不再读 base64，改成 POST 上传后拿 id；进度回调 |
| `src/src/components/ChatView.tsx` | 附件 chip 用 `id` 渲染预览（图片预览需要 `GET /api/attachments/:id` 或本地 blob URL） |
| `src/src/agents/types.ts` | `PromptFile` / `PromptImage` 去 `data`、加 `id` |
| `src-tauri/src/main.rs` | `send_prompt` 参数同上；Tauri 端**可以直接传本地路径**（同机），省掉一次上传——但要经同一套校验/归一化 |
| 发送失败回滚 | 现在靠「客户端还握着 base64」重发；改成 id 后重发只需重发 id，附件不再重复上传 |

## 与「设置 → 缓存」的衔接

Phase 1 已经建好了 `attachments/` 目录、用量统计与手动清理，Phase 2 只需：

1. 让上传真正往里写（当前该行恒为 0）；
2. 清理时**跳过正在被在途 prompt 引用的文件**：清缓存前先查一次「在途附件集合」
   （进程内 `HashSet<attachment_id>`，prompt 开始时插入、结束时移除），命中就跳过并
   在 `errors` 里说明。当前实现没有这个互斥——Phase 1 时目录是空的，所以问题不存在。

## 2a 已实现的内容（现状）

| 项 | 位置 | 说明 |
|---|---|---|
| `POST /api/attachments?session=<id>` | `crates/web/src/attachments_api.rs` | multipart **流式落盘**；路由层 `DefaultBodyLimit::disable()` |
| `GET /api/attachments/:id?session=<id>` | 同上 | 读回原始字节（图片预览/下载），失效返回 404 |
| 落盘 | `attachments/<session>/<uuid>` + `<uuid>.json` | 先写 `<uuid>.part`，校验通过才 rename |
| 会话 id 校验 | `crates/core/src/utils/attachment_store.rs` | 白名单 `[A-Za-z0-9_-]{1,128}`（它同时是目录名） |
| 配额 | 同上 | 单文件 512MB、单会话 2GB / 50 个；边写边判，超了立刻回滚 |
| 计数口径 | 同上 `dir_usage` | 「N 个附件」不数 `.json`/`.part`；字节数按真实磁盘占用 |

实测（真实 gateway）：30MB 文件上传 → sha256 与源文件一致；非法 session → 400；
缺失附件 → 404；`/api/cache/stats` 显示 `1 个 · 5.0MB`（不含元信息），清理后归零。

**尚未接线**：prompt 仍然只认 base64（2b），所以上传的附件目前只能通过
`GET /api/attachments/:id` 取回，还没有任何消息会引用它们。

## 2b 已实现的内容（现状）

`crates/core/src/utils/attachments.rs` 的附件载荷改成二选一，**两条路径共享同一套
提取/归一化逻辑**（否则"内联发的 PDF"和"上传后发的 PDF"行为会慢慢漂移）：

```rust
pub struct FileAttachment {
    pub name: String,            // 走 id 时可省略，回落到元信息里的文件名
    pub data: Option<String>,    // 旧路径：base64 内联（优先）
    pub id: Option<String>,      // 新路径：POST /api/attachments 返回的 id
    pub mime_type: Option<String>,
    pub size: Option<u64>,
}
```

- 解析上下文 `StoredAttachmentScope { data_dir, session_id }`：**会话不匹配就是
  "附件不存在"**，读不到别的会话的文件。
- 新增错误：`MissingPayload`（两者都没给）、`AttachmentMissing`（id 无效/跨会话/
  无 scope）、`StoredTooLarge`（超过 `MAX_STORED_READ_BYTES` 64MB 的对话解析上限）。
- 解析上限先按元信息里的 `size` 判，再读盘：不会为超大文件先把内存吃下去。
- 三个传输调用点（WS / Tauri / chat 测试）都传入 scope。

实测（真实 gateway）：

| 场景 | 结果 |
|---|---|
| 上传 `q3.md` → 发 `files:[{id}]`（无 base64） | prompt **accepted**，会话 JSONL 里出现提取出的原文 |
| 发一个不存在的 id | 拒绝并提示「附件 … 不存在或已失效，请重新上传」 |
| 老客户端发内联 base64 | prompt **accepted**（向后兼容） |
| 同会话/跨会话/无 scope | 只有同会话可读（单测覆盖） |

**尚未接线**：前端仍然只发 base64（2c），所以现在这条路径只有直接调 API 的客户端能用。

## 2c 已实现的内容（前端切到 id）

- `composer-attachments.ts`：附件只保存**本地 `File` + 预览 blob URL**，不再提前读 base64
  （选了不发就不占内存，也不需要"提前"知道会话 id）。
- 发送时（`ChatView.send`）先确定会话 id → 上传 → 用返回的 id 组 `PromptImage` / `PromptFile`。
  任何一个附件被拒 → **整条消息不发**（保持"要么都到、要么都不发"）。
- 乐观渲染用 `previewUrl`（本地 blob），发送成功/移除时 `revokeObjectURL`；
  历史消息仍用后端回放的 base64。
- Web 走 `POST /api/attachments` multipart；桌面走 IPC 命令 `store_attachments`
  （桌面端没有本地 HTTP 服务，路由是进程内 oneshot），两者最终落到同一份
  `attachment_store`，所以配额与元信息格式一致 → 原计划 2d 的"本地路径"不再需要。

## 2e 已实现的内容（生命周期）

| 能力 | 位置 | 行为 |
|---|---|---|
| 删会话联动 | `handlers::delete_session_data` | 删 jsonl 后顺手 `remove_session`，删不掉不阻塞（GC 兜底） |
| 孤儿 GC | `attachments_api::{spawn_gc,gc_once}` | 启动 + 每 6h；只删「会话文件不存在 **且** 静默 > 24h」的目录 |
| 清理互斥 | `in_flight.rs` | 解析期间登记 `<session>/<id>`，清缓存跳过并在 `errors` 里说明 |
| 读不到会话目录 | `gc_session_dirs` | 一个都不删（拿不到"谁是活的"时，误删是更坏的结果） |

实测：删会话 → `attachments/s-full/` 消失、stats 归零；启动时 GC 删掉
mtime 过期的孤儿目录，同时保留刚上传还没发消息的目录（宽限期生效）。

## 后续补齐的内容

| 项 | 位置 | 说明 |
|---|---|---|
| 上传进度与取消 | `lib/attachments-api.ts` | Web 用 `XMLHttpRequest`（`fetch` 没有 request-body 进度），桌面按文件计数；取消走 `AbortSignal`，服务端半成品由 `.part` 机制回收 |
| 桌面端按路径直读 | `store_from_path` + `store_attachments_from_paths` | 拖拽的文件只给绝对路径，直接流式复制落盘，**字节不进 IPC** |
| 桌面拖拽修复 | `ChatView` 的 `onDragDropEvent` | Tauri 默认拦截 OS 拖拽（`dragDropEnabled`），DOM `onDrop` 不触发——此前桌面端拖文件进输入框是**没反应**的 |
| GC 调用链测试 | `gc_once_with` + 单测 | 桌面与 gateway 共用同一个 `gc_once`，测试覆盖宽限期与活会话保护 |
| 用户向文档 | `docs/attachments.md` | 支持格式、上限、文件位置、清理时机、常见报错 |

## 分步落地

| 步骤 | 内容 | 可独立上线 |
|---|---|---|
| 2a | 上传接口 + 落盘 + 元信息 + 配额（不接 prompt） | ✅ **已完成** |
| 2b | `attachments.rs` 的 `Stored` 路径 + 双路径兼容 | ✅ **已完成** |
| 2c | Web 前端切到 id（含 WS 消息瘦身） | ✅ **已完成** |
| 2d | Tauri 上传 | ✅ 改为 IPC 命令 `store_attachments`（不再需要本地路径） |
| 2e | 会话删除联动 + 孤儿 GC + 清缓存互斥 | ✅ **已完成** |

每步都能单独发布，2c 之前随时可以回退到「塞 base64」。

## 风险与对策

| 风险 | 对策 |
|---|---|
| 磁盘被填满 | 会话配额 + 全局配额（建议 5GB）+ 配额满时 LRU 清理 + 设置页展示占用 |
| 上传中断留下半成品 | 写 `*.part` 后缀，落完后 `rename`；启动时清 `*.part` |
| 多实例（gateway + 桌面共用品牌根）同时写 | 目录按 `session_id` 分片，文件名用 ULID，天然无冲突；配额统计允许近似 |
| 引用失效（文件被手工删了） | prompt 时返回明确错误 `附件已失效，请重新上传`，而不是静默丢 |
| 旧会话历史里的 base64 | 不动。只有新消息走 id，历史保持原样，减少迁移面 |

## 验收标准

1. 上传 300MB PDF → 服务端内存峰值 < 50MB（`heaptrack`/RSS 采样验证），prompt 能正常提问。
2. 同一会话连发 10 轮带图消息 → provider 请求体不再随轮次线性增长（对比 Phase 1 的日志）。
3. 删会话后 `attachments/<id>` 消失；7 天未引用的目录被 GC 回收。
4. 「设置 → 缓存」的附件行显示真实占用，清理后在途 prompt 不失败。
