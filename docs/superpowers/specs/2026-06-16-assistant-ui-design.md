# assistant-ui Integration + Auto-Register Slash Commands

## Overview

Replace the custom AI chat UI (`AiCopilot.tsx`) with [assistant-ui](https://www.assistant-ui.com/), and automatically register backend tools and skills as slash commands. The slash command layer routes to the agent runtime with per-command configuration (tool filtering, skill injection).

## Architecture

```
Assistant UI (assistant-ui <Thread> + <Composer>)
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

### 1. Layout (`AiCopilot.tsx`)

- **Keep**: `TickerStrip` (index quotes ticker at top)
- **Remove**: `ContextSidebar`
- **Replace**: custom textarea + message list → assistant-ui `<Thread>` + `<Composer>`

### 2. assistant-ui integration

- Import `@assistant-ui/react` package
- Use `<Thread>` for message list, `<Composer>` for input
- Bridge SSE stream to assistant-ui thread via custom runtime adapter
- Render markdown using assistant-ui's built-in markdown components

### 3. Auto-registration

On mount, fetch `GET /api/ai/commands` and register each as a slash command:

```ts
commands.map(cmd => ({
  name: cmd.name,
  description: cmd.description,
  onSelect: () => sendWithCommand(cmd),
}))
```

`sendWithCommand` sends the message with command_name and command_type in the request body.

### 4. Runtime adapter

Bridge the existing `apiChatStream` (SSE) to assistant-ui's internal thread state:

- `onData` → append text to the current assistant message
- `[DONE]` → finalize the message
- Tool events → transparent (tools execute server-side, results stream as text)

## Data Flow

### Tool command (e.g. `/get_limit_up`)

```
1. User types "/get_limit_up" in assistant-ui composer
2. onSelect fires → send({ message, command_name, command_type: "tool" })
3. POST /api/ai/chat with extended body
4. Server: filter tools to only "get_limit_up"
5. Agent runtime: user message + 1 tool → call tool → stream result
6. SSE stream → onData → assistant-ui Thread renders markdown
```

### Skill command (e.g. `/breakout-pullback`)

```
1. User types "/breakout-pullback"
2. onSelect fires → send({ message, command_name, command_type: "skill" })
3. POST /api/ai/chat with extended body
4. Server: load breakout-pullback SKILL.md into system prompt
5. Agent runtime: skill-aware prompt + all tools → stream analysis
```

### Analysis command (default)

```
1. User types a free-form question
2. POST /api/ai/chat without command fields
3. Server: current behavior (default prompt + all tools)
```

## Out of Scope

- Pipeline command type — reserved in the enum, no behavior implemented
- Structured tool call cards — tools return JSON that renders as markdown, sufficient for v1
- Desktop/Tauri side — same SSE bridge applies, no separate changes needed
- assistant-ui built-in tool call UI components — backend-driven tool calls are transparent to the frontend

## Why This Approach

- **Single execution path**: all requests go through the agent runtime, no duplicate tool logic
- **Command-aware routing**: per-command configuration without splitting the codebase
- **Minimal frontend changes**: preserve ticker, drop sidebar, replace only the chat area
- **Auto-discovery**: new tools and skills automatically appear as slash commands without frontend code changes
