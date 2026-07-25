---
name: review-with-doc
description: >
  Review implementation against a design document. Reads the doc, runs 8-angle
  code review (correctness, missing behavior, cross-file trace, reuse,
  simplification, efficiency, altitude, conventions), saves structured report
  to docs/claude/code-review/.
model: opus
---

# review-with-doc

Review implementation code against a design document (`docs/*.md`). Produces a
structured report at `docs/claude/code-review/<doc-name>-review.md`.

## Usage

```
/review-with-doc docs/agentTransport.md
```

## Workflow

1. **Read the doc** — understand every section, especially behavior specs
   (§numbered sections), code examples, and edge-case notes.

2. **Read the implementation** — trace the actual code paths that implement
   (or should implement) each section. Key files to check:
   - Rust backend: `crates/web/src/agent_ws.rs`, `crates/web/src/chat.rs`
   - Tauri: `src-tauri/src/main.rs`
   - Frontend: `src/src/agents/reducer.ts`, `transport.ts`, `types.ts`
   - UI: `src/src/components/ChatView.tsx`, `src/src/components/chat/*.tsx`

3. **Run 8-angle review** — for each section of the doc, check:
   - **A: Correctness** — line-by-line: wrong conditions, missing await,
     null/undefined deref, error swallowed
   - **B: Missing behavior** — spec says X but code doesn't do X
   - **C: Cross-file trace** — function signature changes break callers
   - **D: Reuse** — code re-implements what already exists
   - **E: Simplification** — redundant state, copy-paste, dead code
   - **F: Efficiency** — wasted work, redundant I/O, O(n²) patterns
   - **G: Altitude** — bandaid vs root cause, right abstraction depth
   - **H: Conventions** — CLAUDE.md violations (unwrap, JoinHandle, magic numbers)

4. **Save report** to `docs/claude/code-review/<doc-name>-review.md` with:
   - Per-section implementation status (✅/⚠️/❌)
   - Ranked findings (severity: 🔴🟠🟡🟢)
   - Concrete failure scenarios
   - Fix recommendations

## Output format

```markdown
# <doc-name> 实施情况核对报告

> 生成日期：<date>
> 范围：<doc-path>

## 总体评估

| 完全实现 | 部分实现 | 未实现 | 严重问题 |
|----------|----------|--------|----------|
| N        | N        | N      | N        |

## 逐节核对

### §N — <title> — ✅/⚠️/❌

- 功能点 ✅
- 功能点 ⚠️ — 见发现 #N

## 发现的问题

### 发现 #N — <标题> 🔴/🟠/🟡/🟢

**文件**: `path:line`
**严重性**: 严重/高/中/低

**描述**: ...

**后果**: ...

**修复**: ...
```

## Gotchas

- The doc may describe ideal behavior that the code intentionally simplified
  (e.g., "use unbounded_channel" but code uses bounded for backpressure).
  Note these as "implementation deviation" not "bug" — explain the trade-off.
- Some sections may be aspirational (marked TODO in code). Note them as
  "not implemented" with the TODO reference.
- The `docs/claude/code-review/` directory may not exist — create it.
