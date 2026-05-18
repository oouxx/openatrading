#!/bin/bash
# SessionStart hook: if code was changed last session, remind Claude to update .claude/context/ docs.

PENDING_FILE="$CLAUDE_PROJECT_DIR/.claude/context/.pending-update.json"

if [ ! -f "$PENDING_FILE" ]; then
  exit 0
fi

DATE=$(jq -r '.last_session_date' "$PENDING_FILE" 2>/dev/null)
CHANGES=$(jq -r '.changed_files' "$PENDING_FILE" 2>/dev/null)

if [ -z "$DATE" ] || [ -z "$CHANGES" ]; then
  rm -f "$PENDING_FILE"
  exit 0
fi

jq -n --arg date "$DATE" --arg changes "$CHANGES" \
  '{"hookSpecificOutput": {
    "additionalContext": "## 上次会话变更提醒\n\n上次会话结束于 \($date)，以下文件发生了未提交的变更：\n\n```\n\($changes)\n```\n\n请检查这些变更是否需要更新 `.claude/context/` 下的对应文档：\n- Rust 核心逻辑变更 → engineering.md / cli.md\n- 前端 UI 变更 → ui.md / design-system.md\n- 新功能/模块 → product.md / engineering.md\n- 新术语 → glossary.md\n\n如果有需要更新的文档，请主动更新后再继续当前任务。"
  }}'

rm -f "$PENDING_FILE"
exit 0
