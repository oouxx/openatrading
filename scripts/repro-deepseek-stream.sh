#!/usr/bin/env bash
# Reproduce the "Stream ended without finish_reason" error against DeepSeek.
#
# Streams a minimal chat completion and dumps the RAW SSE chunks so we can see
# whether the connection is dropped mid-stream, ends with an empty [DONE],
# or returns finish_reason in a non-standard field.
#
# Usage:
#   DEEPSEEK_API_KEY=sk-xxx ./scripts/repro-deepseek-stream.sh
#
# Then inspect the last few lines — a healthy stream ends with:
#   data: {"choices":[{"delta":{},"finish_reason":"stop"}]}
#   data: [DONE]
#
# The bug manifests as the stream closing WITHOUT a finish_reason chunk.

set -euo pipefail

KEY="${DEEPSEEK_API_KEY:-${LLM_API_KEY:-}}"
if [[ -z "$KEY" ]]; then
  echo "ERROR: set DEEPSEEK_API_KEY or LLM_API_KEY" >&2
  exit 1
fi

MODEL="${DEEPSEEK_MODEL:-deepseek-chat}"
URL="https://api.deepseek.com/chat/completions"

echo ">>> model=$MODEL url=$URL"
echo ">>> sending streaming request..."

# -N disables buffering so we see chunks live.
curl -sN -X POST "$URL" \
  -H "Authorization: Bearer $KEY" \
  -H "Content-Type: application/json" \
  -d "{
    \"model\": \"$MODEL\",
    \"stream\": true,
    \"stream_options\": {\"include_usage\": true},
    \"max_tokens\": 256,
    \"messages\": [
      {\"role\": \"system\", \"content\": \"你是A股短线交易助手。\"},
      {\"role\": \"user\", \"content\": \"用三句话说说今天大盘怎么看。\"}
    ]
  }" | tee /tmp/deepseek-stream-dump.txt

echo
echo ">>> last 5 non-empty lines:"
grep -v '^$' /tmp/deepseek-stream-dump.txt | tail -5
echo
echo ">>> finish_reason occurrences:"
grep -o '"finish_reason":"[^"]*"' /tmp/deepseek-stream-dump.txt | sort | uniq -c || echo "(none found — this is the bug)"
echo
echo ">>> [DONE] present:"
grep -c '\[DONE\]' /tmp/deepseek-stream-dump.txt || echo "0"
