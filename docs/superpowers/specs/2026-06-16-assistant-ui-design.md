# assistant-ui Integration + Auto-Register Slash Commands

## Overview

Replace the custom AI chat UI with assistant-ui (`<ExternalThread>`) in both `AiCopilot.tsx` and `AiSidebar.tsx`. Backend tools and skills auto-register as slash commands. The slash command layer routes to the agent runtime with per-command configuration (tool filtering, skill injection). SSE communication uses the existing `apiChatStream` handling.

## Architecture

```
assistant-ui <ExternalThread>             ← controlled component
  messages + onNew                        ← state management
                    │
      apiChatStream (manual SSE)          ← unchanged
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

## Decisions (from design review)

| Decision | Choice |
|----------|--------|
| assistant-ui interface | `<ExternalThread>` controlled component, not RuntimeAdapter |
| Command metadata flow | `useRef` — slash command `onSelect` sets a ref, `onNew` reads it |
| Tool command behavior | Execute immediately, tool list filtered to only that tool |
| Skill command behavior | Fill composer, user edits/presses Enter, all tools available |
| Role & skill relationship | One-to-one — skill (SKILL.md) is a strategy, same as a role's prompt |
| Skill vs role conflict | Skill wins — SKILL.md replaces the role strategy prompt |
| Quick prompts | Removed — slash commands cover them |
| Role selector in sidebar | Removed — roles accessible via slash commands |
| AiSidebar | Also replaced with assistant-ui, shares slash commands |

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

In `ai_chat` handler. Strategy prompt comes from one source — `role` and skill command are alternatives, not combined:

- **Default (no command)**: current behavior — `role` → strategy prompt, or fallback. All tools available.
- **Tool command**: `role` strategy prompt unchanged, tool list filtered to only the requested tool (excludes `submit_trading_decision` and `create_skill`).
- **Skill command**: SKILL.md content replaces the strategy prompt (overrides `role`). All tools available.
- **Pipeline command**: future — composite tools loaded, strategy from role.

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

### 2. assistant-ui integration (shared hook)

Both components share the same integration hook `useAssistantChat`:

```tsx
// hooks/useAssistantChat.ts
export function useAssistantChat() {
  const [messages, setMessages] = useState<ThreadMessage[]>([]);
  const [streaming, setStreaming] = useState(false);
  const cmdRef = useRef<{ name: string; type: CommandType } | null>(null);

  const handleNew = useCallback((appendMsg: AppendMessage) => {
    const cmd = cmdRef.current;
    cmdRef.current = null;

    // Add user + empty assistant message
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
      // Execute immediately: push message + trigger send
      // (directly modify messages state and call apiChatStream)
    } else {
      // Fill composer text (skill): update composer content
    }
  }, []);

  return { messages, streaming, handleNew, sendCommand };
}
```

### 3. Controlled component pattern

```tsx
// AiCopilot.tsx
const { messages, streaming, handleNew, sendCommand } = useAssistantChat();

// Register slash commands
useEffect(() => {
  fetch("/api/ai/commands").then(res => res.json()).then(commands => {
    registerSlashCommands(commands, sendCommand);
  });
}, []);

return (
  <div className="min-h-screen bg-bg pt-11 flex flex-col">
    <TickerStrip />
    <div className="flex-1 overflow-hidden">
      <ExternalThread
        messages={messages}
        onNew={handleNew}
        isRunning={streaming}
      />
    </div>
  </div>
);
```

The Tauri branch in `apiChatStream` (uses `Channel` + `invoke`) works transparently.

### 4. Slash command behavior

- **Tool commands** (`/get_limit_up`): `sendCommand` sets the ref AND immediately generates a message (e.g., "获取涨停数据"), adding it to messages and calling `apiChatStream` directly — user doesn't see composer
- **Skill commands** (`/breakout-pullback`): `sendCommand` sets the ref AND fills the composer text ("执行 breakout-pullback 策略"), user edits/presses Enter → `onNew` reads the ref

## Data Flow

### Tool command (e.g. `/get_limit_up`)

```
1. User types "/get_limit_up" in assistant-ui composer
2. Slash command fires → sendCommand → sets cmdRef + calls apiChatStream directly
3. POST /api/ai/chat { message: "获取涨停数据", command_name: "get_limit_up", command_type: "tool" }
4. Server: filter tools to only "get_limit_up"
5. Agent runtime: user message + 1 tool → call tool → stream result
6. SSE onData → appendToLastMessage() → ExternalThread re-renders
```

### Skill command (e.g. `/breakout-pullback`)

```
1. User types "/breakout-pullback"
2. Slash command fires → sendCommand → sets cmdRef + fills composer
3. User edits or presses Enter → onNew → reads cmdRef
4. POST /api/ai/chat { message: "执行策略分析", command_name: "breakout-pullback", command_type: "skill" }
5. Server: load breakout-pullback SKILL.md into system prompt
6. Agent runtime: skill-aware prompt + all tools → stream analysis
```

### Analysis command (default)

```
1. User types a free-form question
2. onNew → cmdRef is null → no command fields
3. POST /api/ai/chat (current behavior)
4. Server: default prompt + all tools
```

## Out of Scope

- Pipeline command type — composite command (e.g. `/morning-check`). Not implemented
- Structured tool call cards — tools return JSON rendered as markdown, sufficient for v1
- AG-UI protocol — deferred to future iteration
- Role selector — removed; roles accessible via slash commands

## Why This Approach

- **Single execution path**: all requests go through the agent runtime, no duplicate tool logic
- **Command-aware routing**: per-command configuration without splitting the codebase
- **Minimal frontend changes**: preserve ticker, drop sidebar + role selector + quick prompts
- **Auto-discovery**: new tools and skills automatically appear as slash commands
- **Unified component**: `AiSidebar` and `AiCopilot` share the same `useAssistantChat` hook
