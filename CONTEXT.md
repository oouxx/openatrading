# OpenATrading Domain Glossary

## Command

An invocable action in the AI chat interface, auto-registered as a slash command. Divided into three types:

- **Tool command** — single backend tool, tool list filtered to only that tool. Executes immediately without composer.
- **Skill command** — loads an installed skill (SKILL.md) as the strategy prompt. Fills composer text for user review before sending. All tools available.
- **Pipeline command** — loads an installed pipeline (PIPELINE.md) which defines both a strategy prompt and a restricted tool set. Pipeline = skill + tool binding. Fills composer text like skill.

## Role

A trading persona with an identity, strategy prompt, position tracking, and decision history. Roles are the primary way to select a trading strategy. Each role maps to a strategy prompt that guides the LLM's analysis.

## Skill

A markdown file (`SKILL.md`) containing reusable trading strategy rules. Installed in `~/.atrading/skills/`. Conceptually equivalent to a role's strategy prompt — a skill command loads a SKILL.md as the chat's strategy, analogous to selecting a role.

## Pipeline

A strategy + tool binding. Defined as a `PIPELINE.md` file in `~/.atrading/pipelines/<name>/`, containing YAML frontmatter with a tool list and a markdown body as the strategy prompt. A pipeline command loads the prompt and restricts the agent runtime to only the specified tools.

## Tool

A backend function exposing market data to the agent runtime. Defined in `ToolRegistry`, invoked by the LLM or directly via a tool command (which restricts the runtime to only that tool).

## Strategy prompt

The text that guides the LLM's analysis behavior. Comes from one of these sources, in priority order when both are present:
1. SKILL.md content (skill command)
2. PIPELINE.md body (pipeline command)
3. Role's `strategy` field (when a role is selected)
4. Default system prompt (fallback)
