---
name: fix-with-doc
description: >
  Read review report from docs/claude/code-review/ and apply the recommended
  fixes. Pairs with /review-with-doc: review first, then fix.
model: opus
---

# fix-with-doc

Read a review report from `docs/claude/code-review/` and apply the fixes it
recommends. Designed to run after `/review-with-doc`.

## Usage

```
/fix-with-doc docs/claude/code-review/agentTransport-review.md
```

## Workflow

1. **Read the report** — understand every finding, its severity, and its
   recommended fix.

2. **Prioritize by severity** — fix in order:
   - 🔴 Critical (data loss, crash, broken feature)
   - 🟠 High (incorrect behavior, type errors)
   - 🟡 Medium (UX issues, incomplete features)
   - 🟢 Low (cleanup, conventions)

3. **For each finding**, apply the fix:
   - Read the affected file(s)
   - Apply the minimal change that fixes the root cause
   - Verify the fix doesn't break adjacent code

4. **Update the report** — mark each finding as `✅ fixed` or `⏭️ skipped`
   with a brief reason.

## Principles

- **One fix per finding** — don't scope-creep into unrelated refactoring
- **Root cause, not symptom** — if the report says "add null check," check
  whether the null should never happen (fix the producer) vs. can happen
  (fix the consumer)
- **Verify after fix** — run `bun run lint` for TS, `cargo check` for Rust
- **Commit separately** — each logical fix group gets its own commit

## Gotchas

- Some findings may be intentional simplifications (noted in the report).
  Skip those — don't "fix" a deliberate trade-off.
- If a fix reveals a deeper issue not in the report, create a new finding
  inline rather than silently fixing it.
