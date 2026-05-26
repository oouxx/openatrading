# Tool Call 基础设施设计

## 概述

参考 [rig](https://github.com/0xPlaygrounds/rig) 的 tool system 模式，在项目中实现 OpenAI-compatible function calling 基础设施。

## 动机

- 当前 `chat_stream_with_tools` 已能解析 tool call delta，但 B11 bug 导致多事件被截断
- 现有 `ToolDef` 只定义 schema，缺少执行逻辑
- 缺少 agent loop（多轮 tool call 循环）
- 后续 AI Copilot 和 cron decision 需要真正的 tool calling 能力

## 不做什么

- 不实现 MCP (Model Context Protocol) 集成
- 不实现向量存储/RAG-based tool retrieval（ToolEmbedding）
- 不实现 tool call 结果的流式回传
- 不在本章实现具体的业务 tool（由后续 PR 添加）

## 核心抽象

### 1. `Tool` trait

```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    /// JSON Schema for the tool's parameters
    fn parameters(&self) -> serde_json::Value;
    /// Execute the tool with JSON arguments from the LLM
    fn execute(&self, arguments: &serde_json::Value) -> Result<String, ToolError>;
}
```

### 2. `ToolSet`

工具注册表，管理所有已注册的 tool：

```rust
pub struct ToolSet {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolSet {
    pub fn new() -> Self;
    pub fn add(&mut self, tool: impl Tool + 'static) -> &mut Self;
    pub fn to_openai_tools(&self) -> Vec<serde_json::Value>;
    pub fn call(&self, name: &str, arguments: &serde_json::Value) -> Result<String, ToolError>;
    pub fn is_empty(&self) -> bool;
}
```

### 3. `AgentLoop`

多轮 tool call 循环：

```
用户消息 → LLM（带 tools）
  ├─ 返回 text → 完成，返回最终回复
  └─ 返回 tool_calls → ToolSet.call(name, args)
       └─ construct tool result message → LLM（带 tools）
            └─ 重复直到 text 或达到 max_rounds
```

```rust
pub struct AgentLoop {
    llm: LlmClient,
    tools: Arc<ToolSet>,
    max_rounds: usize,  // default 5
}

impl AgentLoop {
    pub async fn run(&self, system_prompt: &str, user_message: &str)
        -> Result<AgentResult, AgentError>;
}

pub struct AgentResult {
    pub final_text: String,
    pub tool_calls_made: Vec<ToolCallRecord>,
}
```

## 文件结构

```
crates/core/src/llm/
├── mod.rs           # 重新导出
├── openrouter.rs    # 修复 chat_stream_with_tools, 新增 chat_with_tools
├── tool.rs          # Tool trait, ToolError, ToolSet
└── agent.rs         # AgentLoop
```

## 需要修复的问题

**B11**: `chat_stream_with_tools` 当前在一个 SSE chunk 产生多个 `StreamEvent` 时只返回第一个。修复为：
- 将 `ToolStreamResult` 改为 `Pin<Box<dyn Stream<Item = Result<Vec<StreamEvent>>> + Send>>`
- 或使用 `async_stream` 逐个 yield

**现有代码清理**：
- 移除 `roles/prompt.rs` 中的 `ToolDef`/`default_tools`（迁移到新的 tool 模块）
- 保留 `LlmClient::chat_stream_with_tools()` 签名不变

## 使用示例

```rust
let mut tools = ToolSet::new();
tools.add(QueryMarketMemory::new(memory.clone()));

let agent = AgentLoop::new(llm, tools);
let result = agent.run(
    "你是A股短线交易助手",
    "查询最近3天的市场温度变化"
).await?;

println!("{}", result.final_text);
// 后台发生了: LLM调用 tool_call → execute → LLM 总结 → 返回
```

## 迁移路径

1. 实现 tool.rs + agent.rs
2. 修复 openrouter.rs 的 B11
3. 移除旧的 ToolDef/default_tools，迁移到新模块
4. AI Copilot 聊天接入 AgentLoop（后续 PR）
5. Cron decision 接入 AgentLoop（后续 PR）
