# assistant-ui Integration + Auto-Register Slash Commands

## Overview

Replace the custom AI chat UI (`AiCopilot.tsx`) with [assistant-ui](https://www.assistant-ui.com/) React components, use [AG-UI](https://docs.ag-ui.com/) (`@ag-ui/client`) as the protocol layer for SSE communication, and automatically register backend tools and skills as slash commands. The slash command layer routes to the agent runtime with per-command configuration (tool filtering, skill injection).

## Architecture

```
assistant-ui <Thread> + <Composer>        ← React UI layer
                    │
            RuntimeAdapter                 ← bridges assistant-ui ↔ AG-UI
                    │
     @ag-ui/client HttpAgent               ← protocol / SSE layer
                    │
         AG-UI Protocol Events (SSE)
                    │
              Slash Command Layer
          (auto-registered from /api/ai/commands)
        ┌────────────┬────────────────────┐
        │            │                    │
    Tool cmd     Skill cmd          Analysis cmd
        │            │                    │
        └────── Agent Runtime ────────────┘
        (command-aware: tool filtering,
         skill injection in system prompt)
                    │
            Execution Engine
                    │
            Exchange / Data
```

## Backend Changes

### 1. CommandSpec types (`crates/core/src/commands/mod.rs`)

```rust
#[derive(Serialize, Deserialize, Clone)]
pub enum CommandType { Skill, Tool, Pipeline }

#[derive(Serialize, Deserialize, Clone)]
pub struct CommandSpec {
    pub name: String,
    pub description: String,
    pub command_type: CommandType,
}
```

### 2. Commands collection

`ToolRegistry::commands()` — maps all market tools + meta-skill to `CommandSpec` with `CommandType::Tool`.

`SkillRegistry::commands()` — maps all installed skills to `CommandSpec` with `CommandType::Skill`.

New endpoint `GET /api/ai/commands` merges both into a single flat list.

### 3. Extended ChatRequest (`crates/web/src/chat.rs`)

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

### 4. Command-aware agent configuration

In `ai_chat` handler:

- **Skill commands**: find the named skill in `SkillRegistry::from_disk()`, inject its content into the system prompt
- **Tool commands**: filter the tool list to only the requested tool, reducing LLM latency and preventing wrong-tool calls
- **Analysis commands** (no command): current behavior with all tools and default prompt

## Frontend Changes

### Dependencies

Add to `src/package.json`:

- `@assistant-ui/react` — UI components (Thread, Composer)
- `@ag-ui/client` — protocol layer (HttpAgent)
- `@ag-ui/core` — AG-UI event types

### 1. Layout (`AiCopilot.tsx`)

- **Keep**: `TickerStrip` (index quotes ticker at top)
- **Remove**: `ContextSidebar`
- **Replace**: custom textarea + message list → assistant-ui `<Thread>` + `<Composer>`

### 2. assistant-ui integration

- Import `@assistant-ui/react` package
- Use `<Thread>` for message list, `<Composer>` for input
- Wire to a custom `RuntimeAdapter` that delegates to `@ag-ui/client`
- Render markdown using assistant-ui's built-in markdown components

### 3. AG-UI communication layer

The existing backend SSE endpoint (`POST /api/ai/chat`) stays unchanged. Instead of modifying the backend to emit AG-UI protocol events, we create a thin client-side adapter that wraps our backend's SSE format into AG-UI types:

```ts
import { AbstractAgent, type RunAgentInput } from "@ag-ui/client";

class TradingAgent extends AbstractAgent {
  async run(input: RunAgentInput) {
    // 1. POST to our existing /api/ai/chat endpoint
    // 2. Parse raw SSE data: lines → AG-UI events
    // 3. Emit TextDelta, ToolCall*, Error, etc.
  }
}
```

This keeps the backend unchanged while providing AG-UI's standardized event types on the frontend for the assistant-ui runtime adapter to consume.

### 4. Runtime adapter

Bridge `@ag-ui/client` events to assistant-ui's internal thread state:

```
assistant-ui RuntimeAdapter.send()
        │
        ▼
HttpAgent.runAgent({ messages, command_name, command_type })
        │
        ▼
AG-UI event stream:
  - TextDelta → assistant-ui appendText()
  - ToolCallStart/Args/End → assistant-ui appendToolCall()
  - Error → assistant-ui appendError()
  - Done → assistant-ui finalize()
```

### 5. Auto-registration

On mount, fetch `GET /api/ai/commands` and register each as a slash command:

```ts
commands.map(cmd => ({
  name: cmd.name,
  description: cmd.description,
  onSelect: () => sendWithCommand(cmd),
}))
```

`sendWithCommand` sends the message with `command_name` and `command_type` in the request body via the `HttpAgent`.

## Data Flow

### Tool command (e.g. `/get_limit_up`)

```
1. User types "/get_limit_up" in assistant-ui composer
2. onSelect fires → adapter.send({ messages, command_name, command_type: "tool" })
3. RuntimeAdapter → HttpAgent.runAgent() → AG-UI POST /api/ai/chat
4. Server: filter tools to only "get_limit_up"
5. Agent runtime: user message + 1 tool → call tool → stream result
6. AG-UI event stream → RuntimeAdapter → assistant-ui Thread renders markdown
```

### Skill command (e.g. `/breakout-pullback`)

```
1. User types "/breakout-pullback"
2. onSelect fires → adapter.send({ messages, command_name, command_type: "skill" })
3. RuntimeAdapter → HttpAgent.runAgent() → AG-UI POST /api/ai/chat
4. Server: load breakout-pullback SKILL.md into system prompt
5. Agent runtime: skill-aware prompt + all tools → stream analysis
```

### Analysis command (default)

```
1. User types a free-form question
2. adapter.send({ messages }) — no command fields
3. RuntimeAdapter → HttpAgent.runAgent() → AG-UI POST /api/ai/chat
4. Server: current behavior (default prompt + all tools)
```

## Out of Scope

- Pipeline command type — reserved in the enum, no behavior implemented
- Structured tool call cards — tools return JSON that renders as markdown, sufficient for v1
- Desktop/Tauri side — same SSE bridge applies, no separate changes needed
- assistant-ui built-in tool call UI components — backend-driven tool calls are transparent to the frontend
- Backend AG-UI protocol — backend stays as raw SSE; client-side `AbstractAgent` adapts to AG-UI types

## Why This Approach

- **Single execution path**: all requests go through the agent runtime, no duplicate tool logic
- **Command-aware routing**: per-command configuration without splitting the codebase
- **Minimal frontend changes**: preserve ticker, drop sidebar, replace only the chat area
- **Auto-discovery**: new tools and skills automatically appear as slash commands without frontend code changes
