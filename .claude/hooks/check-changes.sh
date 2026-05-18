#!/bin/bash
# Stop hook: detect uncommitted code changes, write summary for next session startup.

PENDING_FILE="$CLAUDE_PROJECT_DIR/.claude/context/.pending-update.json"

CHANGES=$(cd "$CLAUDE_PROJECT_DIR" && git diff --name-only HEAD 2>/dev/null | grep -v '^\.claude/context/' | grep -v 'bun\.lock$' | grep -v 'Cargo\.lock$')

if [ -z "$CHANGES" ]; then
  rm -f "$PENDING_FILE"
  exit 0
fi

jq -n --arg date "$(date '+%Y-%m-%d %H:%M')" \
       --arg changes "$CHANGES" \
  '{"last_session_date": $date, "changed_files": $changes}' > "$PENDING_FILE"

exit 0
