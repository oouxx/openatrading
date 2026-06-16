# Web API

## AI Chat

### `POST /api/ai/chat` — AI chat stream (SSE)

Sends a chat message and streams the AI response via Server-Sent Events.

**Request body:**
```json
{
  "message": "今天大盘怎么样？",
  "role": "zarili",
  "history": [],
  "command_name": "breakout-pullback",
  "command_type": "skill"
}
```

Fields:
- `message` (required) — user message text
- `role` (optional) — role name for strategy prompt
- `history` (optional) — previous messages `[{role, content}]`
- `command_name` (optional) — slash command name
- `command_type` (optional) — `"tool"`, `"skill"`, `"pipeline"`

Command types behavior:
| type | Prompt source | Tools available |
|------|--------------|-----------------|
| (none) | role strategy | all tools |
| `tool` | role strategy | only `command_name` tool |
| `skill` | `SKILL.md` content | all tools |
| `pipeline` | `PIPELINE.md` body | pipeline-specified tools + `submit_trading_decision` |

### `GET /api/ai/roles` — List AI roles

Returns available AI roles as `[{name, display_name}]`.

### `GET /api/ai/commands` — List slash commands

Merges tools, skills, and pipelines into a flat list:
```json
[
  {"name": "get_limit_up", "description": "获取今日涨停...", "command_type": "tool"},
  {"name": "breakout-pullback", "description": "", "command_type": "skill"},
  {"name": "morning-check", "description": "盘前快速检查", "command_type": "pipeline"}
]
```
