# assistant-ui Integration + Auto-Register Slash Commands

## Overview

Replace the custom AI chat UI with assistant-ui (`<ExternalThread>`) in both `AiCopilot.tsx` and `AiSidebar.tsx`. Backend tools, skills, and pipelines auto-register as slash commands. The slash command layer routes to the agent runtime with per-command configuration (tool filtering, skill/pipeline prompt injection). SSE communication uses the existing `apiChatStream` handling.

## Architecture

```
assistant-ui <ExternalThread>             ← controlled component
  messages + onNew                        ← state management
                    │
      apiChatStream (manual SSE)          ← unchanged
                    │
              Slash Command Layer
          (auto-registered from /api/ai/commands)
        ┌──────────┬──────────┬──────────┐
        │          │          │          │
    Tool cmd   Skill cmd  Pipeline cmd  Analysis
        │          │          │          │
        └──────────┴────┬─────┴──────────┘
                        │
              Agent Runtime (command-aware)
         ┌──────────────┼──────────────┐
         │              │              │
     filter tools  inject prompt  inject prompt
     (tool cmd)   (skill cmd)   + filter tools
                               (pipeline cmd)
                        │
                  Execution Engine
                        │
                  Exchange / Data
```

## Command Types

| Type | Strategy prompt | Tool availability | Composer behavior |
|------|----------------|-------------------|-------------------|
| **Tool** | unchaged (role or default) | only that tool | execute immediately |
| **Skill** | SKILL.md content (overrides role) | all tools | fill composer text |
| **Pipeline** | PIPELINE.md content (overrides role) | tools defined in pipeline spec | fill composer text |
| **Analysis** | role or default | all tools | user types free-form |

## Backend Changes

### 1. Shared types (`crates/core/src/commands/mod.rs`)

```rust
#[derive(Serialize, Deserialize, Clone)]
pub enum CommandType { Skill, Tool, Pipeline }

#[derive(Serialize, Deserialize, Clone)]
pub struct CommandSpec {
    pub name: String,
    pub description: String,
    pub command_type: CommandType,
}

// ── Pipeline ──
pub struct PipelineSpec {
    pub name: String,
    pub description: String,
    pub tools: Vec<String>,
    pub prompt: String,
}
```

### 2. PipelineRegistry (`crates/core/src/pipelines/mod.rs`)

Installed pipelines are stored in `~/.atrading/pipelines/<name>/PIPELINE.md`, parallel to skills.

**File format (`PIPELINE.md`):**

```markdown
---
description: 盘前快速检查——判断开盘强弱和当日方向
tools:
  - get_market_overview
  - get_limit_up
  - get_plate_abnormal
  - get_auction_quotes
---

# 盘前检查策略

## 流程
1. 调用 get_market_overview 判断市场整体温度
2. 调用 get_auction_quotes 检查竞价异动
3. 调用 get_limit_up 查看涨停梯队
4. 调用 get_plate_abnormal 发现资金攻击方向

## 输出
- 市场温度（热/正常/冷）
- 竞价方向（强/弱/分歧）
- 主线板块
- 今日策略建议
```

The frontmatter is parsed with `serde_yaml` (or manual line scan for simplicity). The body after the frontmatter is the prompt text.

```rust
pub struct PipelineRegistry {
    pipelines: Vec<PipelineSpec>,
}

impl PipelineRegistry {
    pub fn from_disk() -> Self { /* scan ~/.atrading/pipelines/ */ }

    pub fn commands(&self) -> Vec<CommandSpec> {
        self.pipelines.iter().map(|p| CommandSpec {
            name: p.name.clone(),
            description: p.description.clone(),
            command_type: CommandType::Pipeline,
        }).collect()
    }

    pub fn find(&self, name: &str) -> Option<&PipelineSpec> { /* ... */ }
}
```

### 3. Commands collection (`GET /api/ai/commands`)

Merges three registries into one flat list:

```
ToolRegistry::commands()    → CommandType::Tool
SkillRegistry::commands()   → CommandType::Skill
PipelineRegistry::commands() → CommandType::Pipeline
```

### 4. Extended ChatRequest (`crates/web/src/chat.rs`)

```rust
#[derive(Deserialize)]
pub struct ChatRequest {
    message: String,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    history: Vec<ChatMessage>,
    #[serde(default)]
    command_name: Option<String>,
    #[serde(default)]
    command_type: Option<CommandType>,
}
```

### 5. Command-aware agent configuration

In `ai_chat` handler, branching on `command_type`:

| Command type | Strategy prompt | Tool list |
|---|---|---|
| `None` (default) | role strategy or default | all tools |
| `Tool` | role strategy or default | only `command_name` tool |
| `Skill` | SKILL.md content | all tools |
| `Pipeline` | PIPELINE.md body | `pipeline.tools` + `submit_trading_decision` |

## Frontend Changes

### Dependencies

Add to `src/package.json`:

- `@assistant-ui/react` — UI components (Thread, Composer, ExternalThread)

### 1. Layout changes

**`AiCopilot.tsx`:**
- **Keep**: `TickerStrip` (index quotes ticker at top)
- **Remove**: `ContextSidebar`, quick prompts
- **Replace**: custom textarea + message list → assistant-ui `<ExternalThread>`

**`AiSidebar.tsx`:**
- **Remove**: role selector dropdown, suggested questions, custom input/messages
- **Replace**: message area + input → assistant-ui `<ExternalThread>` (compact variant)

### 2. Shared hook `useAssistantChat`

```tsx
// hooks/useAssistantChat.ts
export function useAssistantChat() {
  const [messages, setMessages] = useState<ThreadMessage[]>([]);
  const [streaming, setStreaming] = useState(false);
  const cmdRef = useRef<{ name: string; type: CommandType } | null>(null);

  const handleNew = useCallback((appendMsg: AppendMessage) => {
    const cmd = cmdRef.current;
    cmdRef.current = null;

    setMessages(prev => [...prev, toUserMsg(appendMsg), emptyAssistantMsg()]);
    setStreaming(true);

    apiChatStream({
      message: extractText(appendMsg),
      command_name: cmd?.name,
      command_type: cmd?.type,
      history: toHistory(messages),
    }, {
      onData: (text) => appendToLastMessage(text),
      onDone: () => setStreaming(false),
      onError: (err) => setLastMessageError(err),
    });
  }, [messages]);

  const sendCommand = useCallback((cmd: CommandSpec) => {
    cmdRef.current = { name: cmd.name, type: cmd.command_type };
    if (cmd.command_type === "tool") {
      // Execute immediately: push message + trigger apiChatStream
    } else {
      // Fill composer text (skill / pipeline): update composer content
    }
  }, []);

  return { messages, streaming, handleNew, sendCommand };
}
```

### 3. Controlled component pattern

```tsx
const { messages, streaming, handleNew, sendCommand } = useAssistantChat();

useEffect(() => {
  fetch("/api/ai/commands").then(res => res.json()).then(cmds => {
    registerSlashCommands(cmds, sendCommand);
  });
}, []);

return (
  <div className="min-h-screen bg-bg pt-11 flex flex-col">
    <TickerStrip />
    <div className="flex-1 overflow-hidden">
      <ExternalThread messages={messages} onNew={handleNew} isRunning={streaming} />
    </div>
  </div>
);
```

### 4. Slash command behavior

- **Tool** (`/get_limit_up`): execute immediately — sets cmdRef + generates message + calls apiChatStream directly
- **Skill** (`/breakout-pullback`): fill composer — sets cmdRef + inserts text, user edits/presses Enter → onNew reads cmdRef
- **Pipeline** (`/morning-check`): fill composer — same as skill

## Data Flow

### Tool command

```
1. User types "/get_limit_up"
2. sendCommand → sets cmdRef + calls apiChatStream directly
3. POST /api/ai/chat { command_name: "get_limit_up", command_type: "tool" }
4. Server: strategy from role, only get_limit_up tool
5. SSE onData → appendToLastMessage() → ExternalThread re-renders
```

### Skill command

```
1. User types "/breakout-pullback"
2. sendCommand → sets cmdRef + fills composer
3. User presses Enter → onNew → reads cmdRef
4. POST /api/ai/chat { command_name: "breakout-pullback", command_type: "skill" }
5. Server: load SKILL.md as prompt, all tools available
```

### Pipeline command

```
1. User types "/morning-check"
2. sendCommand → sets cmdRef + fills composer
3. User presses Enter → onNew → reads cmdRef
4. POST /api/ai/chat { command_name: "morning-check", command_type: "pipeline" }
5. Server: load PIPELINE.md prompt, filter to pipeline.tools + decision tool
6. Agent runtime: pipeline prompt + 4 specific tools → stream analysis
```

### Analysis (default)

```
1. User types free-form question
2. onNew → cmdRef is null → no command fields
3. POST /api/ai/chat (current behavior)
4. Server: default prompt + all tools
```

## Out of Scope

- Structured tool call cards — tools return JSON rendered as markdown, sufficient for v1
- AG-UI protocol — deferred to future iteration
- Role selector — removed; roles accessible via slash commands

## Why This Approach

- **Single execution path**: all requests through agent runtime, no duplicate tool logic
- **Command-aware routing**: per-command configuration without splitting the codebase
- **Minimal frontend changes**: preserve ticker, drop sidebar + role selector + quick prompts
- **Auto-discovery**: new tools, skills, and pipelines automatically appear as slash commands
- **Unified component**: `AiSidebar` and `AiCopilot` share the same `useAssistantChat` hook
