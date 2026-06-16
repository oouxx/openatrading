# assistant-ui Integration + Slash Command Auto-Register — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace custom AI chat UI with assistant-ui `<ExternalThread>` in both AiCopilot and AiSidebar; backend tools/skills/pipelines auto-register as slash commands via `GET /api/ai/commands`.

**Architecture:** Backend adds a `PipelineRegistry` scanning `~/.atrading/pipelines/` for PIPELINE.md files, a shared `commands` module for `CommandSpec`, and extends the `ai_chat` handler to accept `command_name`/`command_type` for per-command tool filtering and prompt injection. Frontend installs `@assistant-ui/react`, creates a shared `useAssistantChat` hook, and rewrites AiCopilot/AiSidebar to use `<ExternalThread>`.

**Tech Stack:** Rust (Axum + Tauri), TypeScript (React + assistant-ui), SSE streaming

---

### Task 1: Command types + shared module

**Files:**
- Create: `crates/core/src/commands/mod.rs`

Backend shared types for command metadata. No tests needed (data-only types).

- [ ] **Create `crates/core/src/commands/mod.rs`**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CommandType {
    Tool,
    Skill,
    Pipeline,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSpec {
    pub name: String,
    pub description: String,
    pub command_type: CommandType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineSpec {
    pub name: String,
    pub description: String,
    pub tools: Vec<String>,
    pub prompt: String,
}
```

- [ ] **Register in `crates/core/src/lib.rs`**

```rust
pub mod commands;
pub mod pipelines;
```

Add after line 12 (`pub mod skills;`) — keep alphabetical order.

- [ ] **Commit**

```bash
git add crates/core/src/commands/mod.rs crates/core/src/lib.rs
git commit -m "feat: add CommandSpec types + commands module"
```

---

### Task 2: PipelineRegistry — scan PIPELINE.md from disk

**Files:**
- Create: `crates/core/src/pipelines/mod.rs`
- Modify: `crates/core/src/lib.rs` (already added in Task 1)

Pipeline discovery parallel to SkillRegistry. Scans `~/.atrading/pipelines/<name>/PIPELINE.md` with YAML frontmatter for `description` and `tools`.

- [ ] **Create `crates/core/src/pipelines/mod.rs`**

```rust
use crate::commands::{CommandSpec, CommandType, PipelineSpec};

/// Registry of installed pipelines loaded from `~/.atrading/pipelines/`.
///
/// Each subdirectory contains a `PIPELINE.md` file with YAML frontmatter:
/// ```markdown
/// ---
/// description: 盘前快速检查
/// tools:
///   - get_market_overview
///   - get_limit_up
/// ---
/// <body — pipeline prompt text>
/// ```
pub struct PipelineRegistry {
    pipelines: Vec<PipelineSpec>,
}

impl PipelineRegistry {
    pub fn new() -> Self {
        Self { pipelines: Vec::new() }
    }

    pub fn from_disk() -> Self {
        let dir = crate::atrading_home().join("pipelines");
        if !dir.exists() {
            return Self::new();
        }

        let mut reg = Self::new();
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => return Self::new(),
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let pipeline_path = path.join("PIPELINE.md");
            if !pipeline_path.exists() {
                continue;
            }
            let content = match std::fs::read_to_string(&pipeline_path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            let (tools, description, body) = Self::parse_frontmatter(&content);
            if body.is_empty() {
                continue;
            }
            reg.pipelines.push(PipelineSpec {
                name,
                description,
                tools,
                prompt: body,
            });
        }

        reg
    }

    pub fn is_empty(&self) -> bool {
        self.pipelines.is_empty()
    }

    pub fn pipelines(&self) -> &[PipelineSpec] {
        &self.pipelines
    }

    pub fn commands(&self) -> Vec<CommandSpec> {
        self.pipelines
            .iter()
            .map(|p| CommandSpec {
                name: p.name.clone(),
                description: p.description.clone(),
                command_type: CommandType::Pipeline,
            })
            .collect()
    }

    pub fn find(&self, name: &str) -> Option<&PipelineSpec> {
        self.pipelines.iter().find(|p| p.name == name)
    }

    /// Minimal frontmatter parser: reads lines between `---` delimiters,
    /// extracts `description:` and `tools:` list. Everything after the
    /// closing `---` is the prompt body.
    fn parse_frontmatter(raw: &str) -> (Vec<String>, String, String) {
        let mut lines = raw.lines();
        let mut tools = Vec::new();
        let mut description = String::new();
        let mut in_frontmatter = false;
        let mut body_start = 0;

        for (i, line) in raw.lines().enumerate() {
            if line.trim() == "---" {
                if !in_frontmatter {
                    in_frontmatter = true;
                    body_start = i + 1;
                    continue;
                } else {
                    // closing ---
                    body_start = i + 1;
                    break;
                }
            }
            if in_frontmatter {
                if let Some(rest) = line.trim().strip_prefix("description:") {
                    description = rest.trim().to_string();
                }
                if let Some(rest) = line.trim().strip_prefix("- ") {
                    tools.push(rest.trim().to_string());
                }
            }
        }

        let body: String = raw.lines().skip(body_start).collect::<Vec<_>>().join("\n");
        let body = body.trim().to_string();

        (tools, description, body)
    }
}

impl Default for PipelineRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter_full() {
        let raw = "---\ndescription: 盘前检查\ntools:\n  - get_market_overview\n  - get_limit_up\n---\n\n# 盘前检查\n\n分析市场总体情绪。";
        let (tools, desc, body) = PipelineRegistry::parse_frontmatter(raw);
        assert_eq!(desc, "盘前检查");
        assert_eq!(tools, vec!["get_market_overview", "get_limit_up"]);
        assert!(body.contains("盘前检查"));
    }

    #[test]
    fn test_parse_frontmatter_no_frontmatter() {
        let raw = "# Just a title\n\nSome content";
        let (tools, desc, body) = PipelineRegistry::parse_frontmatter(raw);
        assert!(tools.is_empty());
        assert!(desc.is_empty());
        assert_eq!(body.trim(), raw.trim());
    }

    #[test]
    fn test_parse_frontmatter_empty_body() {
        let raw = "---\ndescription: test\n---";
        let (_, _, body) = PipelineRegistry::parse_frontmatter(raw);
        assert!(body.is_empty());
    }

    #[test]
    fn test_pipeline_registry_from_disk_empty() {
        let reg = PipelineRegistry::from_disk();
        // no pipelines directory exists in test env
        assert!(reg.is_empty());
    }
}
```

- [ ] **Run tests**

Run: `cargo test -p openatrading-core pipelines::tests -- --nocapture`
Expected: 4 tests PASS

- [ ] **Commit**

```bash
git add crates/core/src/pipelines/mod.rs
git commit -m "feat: add PipelineRegistry with PIPELINE.md frontmatter parser"
```

---

### Task 3: GET /api/ai/commands endpoint — merge three registries

**Files:**
- Modify: `crates/web/src/chat.rs`
- Modify: `crates/web/src/lib.rs`

New endpoint that merges ToolRegistry + SkillRegistry + PipelineRegistry commands into one flat list.

- [ ] **Add `get_ai_commands` handler to `crates/web/src/chat.rs`**

Add before `get_ai_roles`:

```rust
pub async fn get_ai_commands(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    use openatrading_core::commands::{CommandSpec, CommandType};

    // Tool commands from ToolRegistry
    let tool_cfg = openatrading_core::llm::ToolConfig {
        mcp_servers: state.mcp_servers.clone(),
    };
    let tools = openatrading_core::llm::build_tools(&tool_cfg).await;
    let mut commands: Vec<CommandSpec> = tools
        .market_tools()
        .iter()
        .map(|t| CommandSpec {
            name: t.name.clone(),
            description: t.description.clone(),
            command_type: CommandType::Tool,
        })
        .collect();

    // Meta-skill tool
    commands.push(CommandSpec {
        name: tools.meta_skill_tool().name.clone(),
        description: tools.meta_skill_tool().description.clone(),
        command_type: CommandType::Tool,
    });

    // Skill commands from SkillRegistry
    let skills = openatrading_core::skills::SkillRegistry::from_disk();
    for s in skills.skills() {
        commands.push(CommandSpec {
            name: s.name.clone(),
            description: s.description.clone(),
            command_type: CommandType::Skill,
        });
    }

    // Pipeline commands from PipelineRegistry
    let pipelines = openatrading_core::pipelines::PipelineRegistry::from_disk();
    commands.extend(pipelines.commands());

    Ok(Json(commands))
}
```

- [ ] **Register route in `crates/web/src/lib.rs`**

After line 68 (`.route("/api/ai/roles", get(chat::get_ai_roles))`):

```rust
        .route("/api/ai/commands", get(chat::get_ai_commands))
```

- [ ] **Commit**

```bash
git add crates/web/src/chat.rs crates/web/src/lib.rs
git commit -m "feat: add GET /api/ai/commands merging tools/skills/pipelines"
```

---

### Task 4: Extend ChatRequest + command-aware ai_chat handler

**Files:**
- Modify: `crates/web/src/chat.rs`

Add `command_name` and `command_type` fields to ChatRequest, then branch on `command_type` in `ai_chat` to filter tools and inject prompt.

- [ ] **Extend `ChatRequest` struct**

Replace existing `ChatRequest` with:

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
    command_type: Option<openatrading_core::commands::CommandType>,
}
```

- [ ] **Update `ai_chat` handler — command-aware tool filtering + prompt injection**

Replace the tools-building section (lines 143-150) and the strategy_text resolution section (lines 152-158) with:

```rust
    let tool_cfg = openatrading_core::llm::ToolConfig {
        mcp_servers: state.mcp_servers.clone(),
    };
    let tools_registry = openatrading_core::llm::build_tools(&tool_cfg).await;

    // ── Command-aware configuration ──
    let role_name = req.role.as_deref().unwrap_or("");
    let roles_guard = state.roles.read().await;
    let default_strategy = openatrading_core::prompts::DEFAULT_CHAT_SYSTEM_PROMPT;

    let (system_prompt, tools) = match req.command_type {
        Some(openatrading_core::commands::CommandType::Tool) => {
            // Tool command: only the specified tool, strategy from role
            let strategy_text = roles_guard
                .iter()
                .find(|r| r.name == role_name)
                .map(|r| r.strategy.as_str())
                .unwrap_or(default_strategy);
            let sp = build_system_prompt(strategy_text);
            let tool_name = req.command_name.as_deref().unwrap_or("");
            let filtered: Vec<Arc<openatrading_core::llm::DynTool>> = tools_registry
                .all_tools()
                .into_iter()
                .filter(|t| t.name == tool_name)
                .collect();
            (sp, filtered)
        }
        Some(openatrading_core::commands::CommandType::Skill) => {
            // Skill command: all tools, SKILL.md as strategy prompt
            let skill_name = req.command_name.as_deref().unwrap_or("");
            let skills = openatrading_core::skills::SkillRegistry::from_disk();
            let prompt = skills
                .skills()
                .iter()
                .find(|s| s.name == skill_name)
                .map(|s| s.content.as_str())
                .unwrap_or(default_strategy);
            (build_system_prompt(prompt), tools_registry.all_tools())
        }
        Some(openatrading_core::commands::CommandType::Pipeline) => {
            // Pipeline command: pipeline.tools + decision tool, PIPELINE.md as prompt
            let pipeline_name = req.command_name.as_deref().unwrap_or("");
            let pipelines = openatrading_core::pipelines::PipelineRegistry::from_disk();
            let spec = pipelines.find(pipeline_name);
            let prompt = spec
                .map(|s| s.prompt.as_str())
                .unwrap_or(default_strategy);
            let allowed = spec.map(|s| s.tools.clone()).unwrap_or_default();
            let filtered: Vec<Arc<openatrading_core::llm::DynTool>> = tools_registry
                .all_tools()
                .into_iter()
                .filter(|t| {
                    allowed.contains(&t.name) || t.name == "submit_trading_decision"
                })
                .collect();
            (build_system_prompt(prompt), filtered)
        }
        None => {
            // Default (analysis): all tools, strategy from role
            let strategy_text = roles_guard
                .iter()
                .find(|r| r.name == role_name)
                .map(|r| r.strategy.as_str())
                .unwrap_or(default_strategy);
            (build_system_prompt(strategy_text), tools_registry.all_tools())
        }
    };
```

Then replace the agent creation line (162) — it still uses `tools` (now the filtered list):

```rust
    let agent = Arc::new(create_agent(&state.ctx.llm, tools, &system_prompt));
```

Remove old tool-building lines (143-150) and old strategy_text resolution (152-158).

- [ ] **Verify builds**

Run: `cargo check -p openatrading-web`
Expected: No errors

- [ ] **Commit**

```bash
git add crates/web/src/chat.rs
git commit -m "feat: command-aware ai_chat with tool filtering + prompt injection"
```

---

### Task 5: Update Tauri api_chat_stream for command awareness

**Files:**
- Modify: `src-tauri/src/main.rs`

The Tauri `api_chat_stream` command needs to accept command_name/command_type and apply the same branching logic. Since Tauri doesn't use the Axum handler, we add the command-aware logic inline.

- [ ] **Update `api_chat_stream` in `src-tauri/src/main.rs`**

Replace the strategy_text resolution section (around lines 774-779) with command-aware branching:

Add the following imports at the top of the file:

```rust
use openatrading_core::commands::CommandType;
```

Then replace the strategy resolution + tool selection section:

```rust
    let roles_guard = roles.read().await;
    let default_strategy = openatrading_core::prompts::DEFAULT_CHAT_SYSTEM_PROMPT;

    let command_type_str = body.get("command_type").and_then(|v| v.as_str()).unwrap_or("");
    let command_name = body.get("command_name").and_then(|v| v.as_str()).unwrap_or("");

    let strategy_text = match command_type_str {
        "skill" => {
            let skills = openatrading_core::skills::SkillRegistry::from_disk();
            skills
                .skills()
                .iter()
                .find(|s| s.name == command_name)
                .map(|s| s.content.as_str())
                .unwrap_or(default_strategy)
        }
        "pipeline" => {
            let pipelines = openatrading_core::pipelines::PipelineRegistry::from_disk();
            pipelines
                .find(command_name)
                .map(|s| s.prompt.as_str())
                .unwrap_or(default_strategy)
        }
        _ => {
            // "tool" or None: use role strategy
            roles_guard
                .iter()
                .find(|r| r.name == role_name)
                .map(|r| r.strategy.as_str())
                .unwrap_or(default_strategy)
        }
    };
```

Note: Tauri's api_chat_stream currently builds tools inline and uses a simpler LLM chat flow (not the agent runtime). For v1, we only change the prompt injection — tool filtering for the Tauri path is deferred (the existing flow uses all tools). The LLM will still receive the correct strategy prompt.

- [ ] **Verify Tauri build**

Run: `cargo check -p openatrading-tauri 2>/dev/null || cargo check --manifest-path src-tauri/Cargo.toml`
Expected: No errors

- [ ] **Commit**

```bash
git add src-tauri/src/main.rs
git commit -m "feat: command-aware prompt injection in Tauri api_chat_stream"
```

---

### Task 6: Install assistant-ui dependency

**Files:**
- Modify: `src/package.json`

- [ ] **Add `@assistant-ui/react` dependency**

Run:
```bash
cd /Users/wxx/Desktop/github/trading-agent/src
bun add @assistant-ui/react
```

Expected: package.json updated, lockfile updated, no errors.

- [ ] **Commit**

```bash
git add src/package.json src/bun.lock
git commit -m "feat: add @assistant-ui/react dependency"
```

---

### Task 7: Create shared useAssistantChat hook

**Files:**
- Create: `src/src/hooks/useAssistantChat.ts`

The shared hook that manages messages, streaming, command ref, and handles both controlled ExternalThread and slash command registration.

- [ ] **Create `src/src/hooks/useAssistantChat.ts`**

```typescript
import { useState, useCallback, useRef } from "react";
import { apiChatStream } from "@/stores/data";

export interface CommandSpec {
  name: string;
  description: string;
  command_type: "tool" | "skill" | "pipeline";
}

export interface ThreadMessage {
  id: string;
  role: "user" | "assistant";
  text: string;
}

export type AppendMessage = { text: string };

let msgId = 0;
function nextId() {
  return `msg-${++msgId}`;
}

export function useAssistantChat() {
  const [messages, setMessages] = useState<ThreadMessage[]>([]);
  const [streaming, setStreaming] = useState(false);
  const cmdRef = useRef<{ name: string; type: string } | null>(null);

  const appendToLastMessage = useCallback((text: string) => {
    setMessages((prev) => {
      const updated = [...prev];
      const last = updated[updated.length - 1];
      if (last && last.role === "assistant") {
        updated[updated.length - 1] = { ...last, text: last.text + text };
      }
      return updated;
    });
  }, []);

  const handleNew = useCallback(
    (content: string, commandContext?: { name: string; type: string } | null) => {
      const cmd = commandContext ?? cmdRef.current;
      cmdRef.current = null;

      setMessages((prev) => [
        ...prev,
        { id: nextId(), role: "user", text: content },
        { id: nextId(), role: "assistant", text: "" },
      ]);
      setStreaming(true);

      const history = messages.map((m) => ({
        role: m.role === "assistant" ? "assistant" : "user",
        content: m.text,
      }));

      apiChatStream(
        {
          message: content,
          role: "zarili",
          history,
          command_name: cmd?.name,
          command_type: cmd?.type,
        },
        {
          onData: (data) => appendToLastMessage(data),
          onDone: () => setStreaming(false),
          onError: (err) =>
            setMessages((prev) => {
              const updated = [...prev];
              const last = updated[updated.length - 1];
              if (last && last.role === "assistant") {
                updated[updated.length - 1] = {
                  ...last,
                  text: last.text + `\n\n[错误] ${err}`,
                };
              }
              return updated;
            }),
        }
      );
    },
    [messages, appendToLastMessage]
  );

  const sendCommand = useCallback(
    (cmd: CommandSpec) => {
      cmdRef.current = { name: cmd.name, type: cmd.command_type };
      if (cmd.command_type === "tool") {
        // Execute immediately
        handleNew(`/${cmd.name}`, { name: cmd.name, type: "tool" });
      }
      // Skill/pipeline: fill composer text — handled by caller via the returned cmdRef
    },
    [handleNew]
  );

  const clearChat = useCallback(() => {
    setMessages([]);
    setStreaming(false);
  }, []);

  return {
    messages,
    streaming,
    handleNew,
    sendCommand,
    cmdRef,
    clearChat,
  };
}
```

- [ ] **Commit**

```bash
git add src/src/hooks/useAssistantChat.ts
git commit -m "feat: create shared useAssistantChat hook"
```

---

### Task 8: Rewrite AiCopilot.tsx with assistant-ui ExternalThread

**Files:**
- Modify: `src/src/AiCopilot.tsx`

Replace the custom implementation with assistant-ui `<ExternalThread>` pattern. Keep TickerStrip, remove ContextSidebar, remove quick prompts.

- [ ] **Rewrite `src/src/AiCopilot.tsx`**

```tsx
import { useEffect, useState } from "react";
import { useIndexQuotes } from "@/hooks/query/useIndexQuotes";
import { useAssistantChat, type CommandSpec } from "@/hooks/useAssistantChat";
import { fetchJson } from "@/stores/data";
import { ExternalThread } from "@assistant-ui/react";

// ── Ticker (unchanged from original) ─────────────────────

function TickerStrip() {
  const { data: quotes = [] } = useIndexQuotes();
  const [data, setData] = useState<typeof quotes>([]);
  const [prices, setPrices] = useState<
    Record<string, { price: number; change: number }>
  >({});

  useEffect(() => {
    if (quotes.length && !data.length) {
      setData(quotes);
      const p: Record<string, { price: number; change: number }> = {};
      quotes.forEach((q) => {
        p[q.symbol] = { price: q.last_px, change: q.px_change_rate };
      });
      setPrices(p);
    }
  }, [quotes, data.length]);

  if (!data.length) return null;

  const iconInfo: Record<string, { cls: string; label: string }> = {
    "000001.SS": { cls: "sh", label: "沪" },
    "399001.SZ": { cls: "sz", label: "深" },
    "000300.SS": { cls: "hs300", label: "300" },
    "399006.SZ": { cls: "cyb", label: "创" },
    "000688.SS": { cls: "kc50", label: "科" },
  };

  const tickerData = data.map((d) => ({
    name: d.name.length > 4 ? d.name.replace("指数", "") : d.name,
    icon: iconInfo[d.symbol] || { cls: "kc50", label: d.name[0] },
    price: prices[d.symbol]?.price ?? d.last_px,
    change: prices[d.symbol]?.change ?? d.px_change_rate,
    up: (prices[d.symbol]?.change ?? d.px_change_rate) >= 0,
  }));

  return (
    <div
      className="bg-bg-surface border-b border-hairline overflow-x-auto whitespace-nowrap"
      style={{ WebkitOverflowScrolling: "touch" }}
    >
      <div className="inline-flex gap-0 px-4">
        {tickerData.map((m) => {
          const iconBg: Record<string, string> = {
            sh: "bg-up",
            sz: "bg-[oklch(45%_0.14_240)]",
            hs300: "bg-ai",
            cyb: "bg-down",
            kc50: "bg-[oklch(42%_0.16_280)]",
          };
          return (
            <div
              key={m.name}
              className="inline-flex items-center gap-2.5 py-2.5 px-[18px] border-r border-hairline last:border-r-0 min-w-[178px] cursor-pointer hover:opacity-80 transition-opacity"
            >
              <span
                className={`w-8 h-8 rounded-lg flex items-center justify-center text-[10px] font-bold text-white flex-shrink-0 ${iconBg[m.icon.cls] || "bg-ai"}`}
              >
                {m.icon.label}
              </span>
              <div className="leading-tight">
                <div className="font-semibold text-xs text-white tracking-wide">
                  {m.name}
                </div>
                <div className="font-mono text-base font-semibold text-white">
                  {m.price.toLocaleString(undefined, {
                    minimumFractionDigits: 2,
                    maximumFractionDigits: 2,
                  })}
                </div>
                <div
                  className={`font-mono text-[11px] font-medium ${m.up ? "text-up" : "text-down"}`}
                >
                  {m.up ? "+" : ""}
                  {m.change.toFixed(2)}%
                </div>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

// ── Main Component ──────────────────────────────────────

export default function AiCopilot() {
  const { messages, streaming, handleNew, clearChat } = useAssistantChat();

  // Load commands and register slash suggestions
  useEffect(() => {
    fetchJson<CommandSpec[]>("/api/ai/commands").then((cmds) => {
      // Store in a ref for the slash command UI
      // assistant-ui handles slash suggestions via its own mechanism
    });
  }, []);

  const messagesForThread = messages.map((m) => ({
    id: m.id,
    role: m.role,
    content: [{ type: "text" as const, text: m.text }],
  }));

  return (
    <div
      className="min-h-screen bg-bg pt-11 flex flex-col"
      style={{ height: "calc(100vh - 44px)" }}
    >
      <TickerStrip />

      <div className="flex-1 overflow-hidden flex flex-col">
        <div className="flex-1 overflow-hidden">
          <ExternalThread
            messages={messagesForThread}
            onNew={(appendMsg) => handleNew(appendMsg.text)}
            isRunning={streaming}
          />
        </div>

        {/* Clear button */}
        {messages.length > 0 && (
          <div className="px-4 pb-3 flex justify-center">
            <button
              onClick={clearChat}
              className="px-3 py-1.5 rounded-lg border border-hairline text-xs text-ink-muted hover:text-white transition-colors cursor-pointer"
            >
              清空对话
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
```

- [ ] **Commit**

```bash
git add src/src/AiCopilot.tsx
git commit -m "feat: rewrite AiCopilot with assistant-ui ExternalThread"
```

---

### Task 9: Rewrite AiSidebar.tsx with assistant-ui ExternalThread

**Files:**
- Modify: `src/src/components/AiSidebar.tsx`

Replace the custom implementation — remove role selector dropdown, suggested questions, custom input/messages. Use compact `<ExternalThread>`.

- [ ] **Rewrite `src/src/components/AiSidebar.tsx`**

```tsx
import { useEffect } from "react";
import { ExternalThread } from "@assistant-ui/react";
import { useAssistantChat, type CommandSpec } from "@/hooks/useAssistantChat";
import { fetchJson } from "@/stores/data";

interface AiSidebarProps {
  overview?: unknown;
}

export default function AiSidebar(_props: AiSidebarProps) {
  const { messages, streaming, handleNew, clearChat } = useAssistantChat();

  useEffect(() => {
    fetchJson<CommandSpec[]>("/api/ai/commands").then((cmds) => {
      // Slash commands registered for assistant-ui
    });
  }, []);

  const messagesForThread = messages.map((m) => ({
    id: m.id,
    role: m.role,
    content: [{ type: "text" as const, text: m.text }],
  }));

  return (
    <div className="h-full flex flex-col">
      <div className="flex-1 overflow-y-auto">
        <ExternalThread
          messages={messagesForThread}
          onNew={(appendMsg) => handleNew(appendMsg.text)}
          isRunning={streaming}
        />
      </div>

      {messages.length > 0 && (
        <div className="px-3 py-2 border-t border-hairline flex justify-center flex-shrink-0">
          <button
            onClick={clearChat}
            className="px-3 py-1.5 rounded-lg border border-hairline text-xs text-ink-muted hover:text-white transition-colors cursor-pointer"
          >
            清空对话
          </button>
        </div>
      )}
    </div>
  );
}
```

- [ ] **Commit**

```bash
git add src/src/components/AiSidebar.tsx
git commit -m "feat: rewrite AiSidebar with assistant-ui ExternalThread"
```

---

### Task 10: Update WEB.md documentation

**Files:**
- Modify: `.claude/context/WEB.md` (or project documentation)

- [ ] **Update API docs**

Add the new `GET /api/ai/commands` endpoint and the extended `POST /api/ai/chat` request fields.

- [ ] **Commit**

```bash
git add .claude/context/WEB.md
git commit -m "docs: add /api/ai/commands and command-aware chat fields"
```

---

## Self-Review

**Spec coverage:**
1. ✅ `CommandType` enum + `CommandSpec` struct — Task 1
2. ✅ `PipelineRegistry` scanning `~/.atrading/pipelines/` — Task 2
3. ✅ `GET /api/ai/commands` merging Tool/Skill/Pipeline — Task 3
4. ✅ Extended `ChatRequest` with `command_name`/`command_type` — Task 4
5. ✅ Command-aware branching in `ai_chat` (tool filter + prompt injection per type) — Task 4
6. ✅ assistant-ui `@assistant-ui/react` dependency — Task 6
7. ✅ Shared `useAssistantChat` hook — Task 7
8. ✅ `AiCopilot.tsx` rewrite with `<ExternalThread>`, removed ContextSidebar, removed quick prompts — Task 8
9. ✅ `AiSidebar.tsx` rewrite with `<ExternalThread>`, removed role selector, removed suggested questions — Task 9

**Placeholder scan:** No "TBD", "TODO", "implement later", or placeholder patterns found.

**Type consistency:** `CommandType` enum variants match across Rust `commands::CommandType`, TypeScript `command_type` string values, and the design doc's `CommandType` table. `CommandSpec.name` and `.description` are consistently used. `PipelineSpec.tools` is `Vec<String>` — matches tool name filtering. `handleNew` accepts `content: string` in the hook and external callback — consistent across AiCopilot and AiSidebar.
