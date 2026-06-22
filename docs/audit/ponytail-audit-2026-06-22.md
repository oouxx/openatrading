# Ponytail Audit Report

**Date:** 2026-06-22
**Scope:** Full repository scan (backend Rust + frontend React/TypeScript)
**Mode:** `full`

---

## Backend Findings

### 1. `crates/agent/src/lib.rs` — Entire crate is a re-export shim

`delete:` 10 lines of `pub use openatrading_agent::*;` re-exports. The `agent` crate adds zero value — no new types, no wrappers, no logic. All consumers can depend on `openatrading_agent` directly.

**net:** -1 crate, -10 lines.

---

### 2. `crates/core/src/llm/tools.rs` — 20+ nearly identical tool factory functions

`shrink:` 852 lines containing 20+ functions of the form:

```rust
pub fn get_xxx_tool() -> Tool {
    Tool { name: "xxx", description: "...", parameters: json!({...}), execute_fn: Arc::new(execute_xxx) }
}
```

Each function duplicates the `Tool` struct construction boilerplate. The `ExecuteFn` type is a nested `Arc<dyn Fn + Pin + Box<dyn Future>>` — 4 layers of indirection.

**net:** -400+ lines with a macro or data-driven approach.

---

### 3. `crates/provider/src/llm.rs` — LlmClient: 9 methods, same boilerplate

`shrink:` 9 chat/stream methods (`chat`, `chat_with_messages`, `chat_stream`, `chat_stream_with_messages`, `chat_stream_with_messages_marked`, `chat_stream_with_tools`, `chat_simple`, `stream_simple`, `create_stream`). Each repeats the same `StreamOptions` construction.

Only one or two callers per variant in practice. Merge into 2-3 methods with optional params.

**net:** -150+ lines.

---

### 4. `crates/core/src/roles/mod.rs` + `crates/core/src/skills/mod.rs` — Duplicated YAML parsing

`delete:` Both files independently parse frontmatter via `gray_matter`:

- `roles/mod.rs`: `parse_frontmatter(text: &str)`
- `skills/mod.rs`: `parse_skill_frontmatter(text: &str)`

Identical logic, same dependency, same error handling. Extract once into a shared utility.

**net:** -40 lines, -1 dependency duplication.

---

### 5. `crates/channels/src/lib.rs` — Trait with blanket impl that always delegates

`yagni:` The `Channel` trait has 4 methods (`send_text`, `send_image`, `send_file`, `send_markdown`) all defaulting to `send_text`. The `NotificationSender` trait adds a blanket impl for `Arc<dyn Channel>`.

The abstraction exists for "one sender, multiple channels" but every producer sends text only. Collapse to a single `NotificationSender` type, drop the `Channel` trait.

**net:** -50 lines, -1 trait.

---

### 6. `crates/web/src/lib.rs:73-76` — Duplicate routes

`delete:` Lines 73-76 register `/api/skills` and `/api/tools` twice:

```rust
.route("/api/skills", get(chat::get_skills))
.route("/api/tools", get(chat::get_tools))
.route("/api/skills", get(chat::get_skills))  // dup
.route("/api/tools", get(chat::get_tools))    // dup
```

**net:** -2 lines.

---

### 7. `crates/core/src/llm/mcp_server.rs` — `build_market_tools()` duplicates ToolRegistry

`delete:` `build_market_tools()` constructs the same tool list as `ToolRegistry::new()`. All callers already hold a `ToolRegistry` via `AppContext`. Dead code, no unique callers.

**net:** -80 lines.

---

### 8. `crates/tui/src/chat.rs:326` — Unbounded channel

`fix:` Uses `tokio::sync::mpsc::unbounded_channel()` which violates CLAUDE.md:

> "不用 unbounded channel 替代有界 channel 掩盖背压问题"

Replace with `tokio::sync::mpsc::channel(256)` or similar bounded channel.

**net:** -0 lines (behavioral fix, not deletion).

---

### 9. `crates/core/src/context.rs` — `Arc<ArcSwap<ToolRegistry>>` overkill

`yagni:` `AppContext` wraps `ToolRegistry` in `Arc<ArcSwap<ToolRegistry>>` for live tool hot-swapping (MCP server add/remove at runtime). No code paths currently hot-swap tools — MCP servers are loaded once at startup and never reloaded.

Replace with `Arc<ToolRegistry>` and add `ArcSwap` when MCP hot-reload actually exists.

**net:** -1 dependency (`arc-swap`), -0 meaningful lines.

---

### 10. `crates/provider/src/lib.rs` — Thin re-export crate

`delete:` Exists only to re-export from `pi_ai` and `openatrading_provider`. The `api` crate follows the same pattern (`pub use openatrading_api::*;`). Inline the few symbols actually used by consumers and drop the crate.

**net:** -1 crate candidate.

---

### 11. `crates/core/src/llm/tools.rs:763-851` — Tests asserting 20 name strings

`shrink:` Tests enumerate every tool name as a string assertion. Brittle — adding/renaming a tool requires updating 20 assertions. Test the structural invariant instead (all tools have unique, non-empty names).

**net:** -70 lines.

---

## Frontend Findings

### 1. `src/src/components/assistant-ui/` — Forked UI library (9 files, ~2630 lines)

`delete:` Full directory forks `@assistant-ui/react` defaults with minimal customization:

| File | Lines | Notes |
|------|-------|-------|
| `thread.tsx` | 655 | Copied default Thread component, minor style tweaks |
| `reasoning.tsx` | 365 | Forked reasoning display |
| `tool-fallback.tsx` | 585 | Forked tool call fallback UI |
| `markdown-text.tsx` | 260 | Forked markdown renderer |
| `thread-list.tsx` | 182 | Forked thread list |
| `tool-group.tsx` | 233 | Forked tool call grouping |
| `attachment.tsx` | 226 | Forked attachment UI |
| `threadlist-sidebar.tsx` | 75 | Thin wrapper |
| `tooltip-icon-button.tsx` | 49 | Thin wrapper |

Most of these are copy-paste with `< 10 lines of actual customization. Use `@assistant-ui/react` exports directly + style overrides via CSS/Tailwind.

**net:** ~2500 lines deletable, -1 heavy maintenance burden.

---

### 2. `src/src/hooks/useConversations.ts` — Always-empty, never populated

`delete:` Returns an always-empty array. Never populated by any code path. Zero callers after the assistant-ui fork was inlined.

**net:** -30 lines.

---

### 3. `src/src/composables/useSpeech.ts` — Unused module

`delete:` Speech synthesis composable. Zero imports across the entire frontend.

**net:** -60 lines.

---

### 4. `src/src/components/ThemeToggle.tsx` — Duplicates store logic

`delete:` Implements theme toggle UI that duplicates `stores/ui.ts` theme logic. The store already exposes `theme` + `setTheme`. This component re-implements the same toggle.

**net:** -40 lines.

---

### 5. `src/src/components/SettingsView.tsx` — Mockup with unsubmitted form

`yagni:` Full settings form with fields, validation, and state. But no submission handler, no API endpoint, no persistence. Dead UI.

**net:** -120 lines (or wire up the API).

---

### 6. `src/src/hooks/useChatThreads.ts` — archive/unarchive stubs

`delete:` Contains `archiveThread` / `unarchiveThread` that log "not implemented" and return. Dead code paths.

**net:** -15 lines.

---

### 7. `src/src/components/ResizableHandle.tsx` — No callers pass `onResize`

`yagni:` Custom drag-to-resize component. The `onResize` callback is never passed by any consumer. If resize notification isn't needed, drop the callback parameter; if the whole component is unused, drop it entirely.

**net:** -40 lines (or -80 if unused entirely).

---

### 8. `src/src/TradingView.tsx` — Duplicate AiSidebar for mobile

`shrink:` Renders a second `AiSidebar` instance behind a mobile overlay check. This duplicates the sidebar layout logic already in `AiSidebar`. Use CSS media queries or a single responsive component instead.

**net:** -30 lines.

---

### 9. `normalizeSymbol` duplicated 3x, `getKlineImageUrl` duplicated 2x

`delete:` Duplicate utility functions across files:

- `normalizeSymbol` in `BreakoutView.tsx:80-88`, `KlineDrawer.tsx:26-34`, `MiniChartPreview.tsx:9-17`
- `getKlineImageUrl` in `BreakoutView.tsx:90-99`, `KlineDrawer.tsx:37-46`

Extract to a shared utility module.

**net:** -40 lines.

---

### 10. `src/src/stores/data.ts` — Types mixed with runtime (182 lines)

`shrink:` Exports TypeScript types/interfaces alongside runtime zustand stores and helper functions. Split types into a `.d.ts` or separate types file.

**net:** -0 lines (reorganization).

---

## Summary

| Category | Backend | Frontend | Total |
|----------|---------|----------|-------|
| Lines deletable | ~800 | ~2900 | **~3700** |
| Crates deletable | 2 | — | **2** |
| Files deletable | 2 | ~15 | **~17** |
| Dependencies removable | 1 (`arc-swap`) | — | **1** |

### Top 3 quick wins

1. **Delete duplicate routes** (`crates/web/src/lib.rs:73-76`) — 2 lines, zero risk
2. **Delete `crates/agent/src/lib.rs`** — -1 crate, -10 lines, all consumers already depend on `openatrading_agent`
3. **Delete `useConversations.ts` + `useSpeech.ts`** — -90 lines, truly dead code

### Top 3 high-impact items

1. **Delete `assistant-ui/` fork** — reclaim ~2500 lines, stop maintaining forked library code
2. **Collapse `LlmClient` methods** — merge 9 methods into 2-3, remove 150+ boilerplate lines
3. **Unify YAML frontmatter parsing** — remove duplication across roles/skills modules
