# OpenATrading Domain Glossary

## Command

An invocable action in the AI chat interface, auto-registered as a slash command. Divided into three types:

- **Tool command** — single backend tool, tool list filtered to only that tool. Executes immediately without composer.
- **Skill command** — loads an installed skill (SKILL.md) as the strategy prompt. Fills composer text for user review before sending.
- **Pipeline command** — composite command that triggers multiple tools and aggregates results (e.g. `/morning-check`). Not yet implemented.

## Role

A trading persona with an identity, strategy prompt, position tracking, and decision history. Roles are the primary way to select a trading strategy. Each role maps to a strategy prompt that guides the LLM's analysis.

## Skill

A markdown file (`SKILL.md`) containing reusable trading strategy rules. Installed in `~/.atrading/skills/`. Conceptually equivalent to a role's strategy prompt — a skill command loads a SKILL.md as the chat's strategy, analogous to selecting a role.

## Tool

A backend function exposing market data to the agent runtime. Defined in `ToolRegistry`, invoked by the LLM or directly via a tool command (which restricts the runtime to only that tool).

## Strategy prompt

The text that guides the LLM's analysis behavior. Comes from one of three sources:
1. A role's `strategy` field (when a role is selected)
2. A SKILL.md file (when a skill command is invoked)
3. The default system prompt (neither role nor skill specified)
