//! Built-in pi-rs extension toggles (`extensionsEnabled` in settings.json).
//!
//! The extension registry always registers every built-in so a live session
//! can be toggled without a restart. Enabling/disabling is therefore
//! expressed as an *active tool / command filter*, not by rebuilding the
//! registry:
//!
//! - tools are removed from (or restored to) `agent.state.tools`;
//! - disabled extension slash commands are rejected by the session actor.
//!
//! Missing keys in `extensionsEnabled` default to enabled. That preserves this
//! app's historical behaviour (all built-ins on when no settings exist) and
//! keeps `settings.json` written by older versions valid.

use std::collections::{BTreeMap, HashMap};

/// Metadata for one built-in extension owned by this app's settings UI.
#[derive(Debug, Clone, Copy)]
pub struct BuiltinExtension {
    /// pi-rs extension id, also the key in `extensionsEnabled`.
    pub id: &'static str,
    /// Human-readable name shown in settings.
    pub name: &'static str,
    /// One-line description shown in settings.
    pub description: &'static str,
    /// Tools registered by the extension.
    pub tools: &'static [&'static str],
    /// Slash commands registered by the extension (without the leading `/`).
    pub commands: &'static [&'static str],
    /// pi-rs `settings.json` keys the extension reads. Empty = not
    /// configurable (the extension has no settings section).
    pub settings: &'static [ExtensionSetting],
    /// Footnote shown once under the settings panel (credentials source,
    /// when a change takes effect). Only set when a field can't explain it.
    pub settings_note: Option<&'static str>,
}

/// One pi-rs `settings.json` key an extension reads.
///
/// `key` is the settings path (camelCase section + leaf), i.e. exactly what
/// pi-rs resolves — `webSearch.provider` ⇢ `{"webSearch": {"provider": …}}`.
/// The settings UI renders one control per entry, so adding a key to an
/// extension never requires a frontend change.
#[derive(Debug, Clone, Copy)]
pub struct ExtensionSetting {
    /// Dotted path inside `settings.json`.
    pub key: &'static str,
    /// Label shown next to the control.
    pub label: &'static str,
    /// Only what the label can't say (value semantics, fallback order).
    pub hint: Option<&'static str>,
    /// Example value shown while the key is unset (text inputs only).
    pub placeholder: Option<&'static str>,
    /// Allowed `(value, label)` pairs. Non-empty = dropdown, first entry is
    /// pi-rs's default. Empty = free-form text.
    pub options: &'static [(&'static str, &'static str)],
}

/// The pi-rs built-ins exposed in the settings UI.
///
/// `goal` was removed in pi-rs v1.87.10; the remaining built-ins register
/// tools but no slash commands.
pub const BUILTIN_EXTENSIONS: &[BuiltinExtension] = &[
    BuiltinExtension {
        id: "subagent",
        name: "子代理",
        description: "允许 Agent 把独立任务交给拥有独立上下文的子代理执行。",
        tools: &["subagent"],
        commands: &[],
        // pi-rs 没有 `subagent` 的 settings.json 配置项：它只认
        // `{agent_dir}/agents/*.md` 里的 agent 定义。
        settings: &[],
        settings_note: None,
    },
    BuiltinExtension {
        id: "web_search",
        name: "网页搜索",
        description: "允许 Agent 搜索并抓取网页内容（web_search / web_fetch）。",
        tools: &["web_search", "web_fetch"],
        commands: &[],
        settings: WEB_SEARCH_SETTINGS,
        settings_note: Some(
            "DeepSeek 凭据取自 Agent 页的 API Key（pi-rs auth.json）或 DEEPSEEK_API_KEY；设置对之后新建的会话生效。",
        ),
    },
];

/// `web_search` 的后端配置（pi-rs `settings.json` 的 `webSearch` 段）。
///
/// 不暴露 `webSearch.deepseek.apiKey`：密钥统一存在 pi-rs 的 `auth.json`
/// （「Agent」页的 API Key），扩展侧只需要地址与后端选择。
const WEB_SEARCH_SETTINGS: &[ExtensionSetting] = &[
    ExtensionSetting {
        key: "webSearch.provider",
        label: "搜索后端",
        hint: Some("自动：有 DeepSeek 凭据时走 DeepSeek，否则走本地 Ollama"),
        placeholder: None,
        options: &[
            ("auto", "自动"),
            ("deepseek", "DeepSeek"),
            ("ollama", "Ollama"),
        ],
    },
    ExtensionSetting {
        key: "webSearch.deepseek.baseUrl",
        label: "DeepSeek 地址",
        hint: None,
        placeholder: Some("https://api.deepseek.com"),
        options: &[],
    },
    ExtensionSetting {
        key: "webSearch.ollama.host",
        label: "Ollama 地址",
        hint: None,
        placeholder: Some("http://localhost:11434"),
        options: &[],
    },
];

impl BuiltinExtension {
    /// The settings entry this extension declares for `key`, if any.
    #[must_use]
    pub fn setting(&self, key: &str) -> Option<&'static ExtensionSetting> {
        self.settings.iter().find(|setting| setting.key == key)
    }
}

/// Resolved enable flags for the built-in extensions.
///
/// Unknown keys loaded from `settings.json` are preserved when saving so this
/// app never drops configuration for extensions it does not manage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionToggles {
    enabled: BTreeMap<String, bool>,
}

impl Default for ExtensionToggles {
    fn default() -> Self {
        Self {
            enabled: BUILTIN_EXTENSIONS
                .iter()
                .map(|ext| (ext.id.to_string(), true))
                .collect(),
        }
    }
}

impl ExtensionToggles {
    /// Build toggles from pi-rs's `extensionsEnabled` map.
    ///
    /// Missing built-in ids are enabled (backward-compatible default). Extra
    /// ids are retained verbatim for a lossless round-trip.
    #[must_use]
    pub fn from_enabled_map(map: &HashMap<String, bool>) -> Self {
        let mut enabled: BTreeMap<String, bool> =
            map.iter().map(|(id, value)| (id.clone(), *value)).collect();
        for ext in BUILTIN_EXTENSIONS {
            enabled.entry(ext.id.to_string()).or_insert(true);
        }
        Self { enabled }
    }

    /// Full map to persist back to `settings.json`, including unknown keys.
    #[must_use]
    pub fn enabled_map(&self) -> BTreeMap<String, bool> {
        self.enabled.clone()
    }

    /// Whether a known built-in extension is enabled. Unknown ids return
    /// `None` so callers can distinguish them from a disabled extension.
    #[must_use]
    pub fn is_enabled(&self, id: &str) -> Option<bool> {
        BUILTIN_EXTENSIONS
            .iter()
            .any(|ext| ext.id == id)
            .then(|| self.enabled.get(id).copied().unwrap_or(true))
    }

    /// Set a known built-in extension. Returns `false` for unknown ids.
    pub fn set(&mut self, id: &str, enabled: bool) -> bool {
        if !BUILTIN_EXTENSIONS.iter().any(|ext| ext.id == id) {
            return false;
        }
        self.enabled.insert(id.to_string(), enabled);
        true
    }

    /// Tool names that must be removed from a session's active tool set.
    #[must_use]
    pub fn disabled_tools(&self) -> Vec<String> {
        BUILTIN_EXTENSIONS
            .iter()
            .filter(|ext| !self.enabled.get(ext.id).copied().unwrap_or(true))
            .flat_map(|ext| ext.tools.iter().map(|tool| (*tool).to_string()))
            .collect()
    }

    /// Extension slash commands that must be rejected / hidden.
    #[must_use]
    pub fn disabled_commands(&self) -> Vec<String> {
        BUILTIN_EXTENSIONS
            .iter()
            .filter(|ext| !self.enabled.get(ext.id).copied().unwrap_or(true))
            .flat_map(|ext| ext.commands.iter().map(|cmd| (*cmd).to_string()))
            .collect()
    }
}

/// Base name of a pi-rs slash invocation.
///
/// pi-rs resolves name conflicts by appending `:N` (e.g. `review:1`), so both
/// the command list and the prompt guard must compare on the base name.
#[must_use]
pub fn command_base_name(invocation: &str) -> &str {
    invocation.split(':').next().unwrap_or(invocation)
}

/// Whether an extension slash invocation targets a disabled extension.
#[must_use]
pub fn is_command_disabled(invocation: &str, disabled_commands: &[String]) -> bool {
    let base = command_base_name(invocation);
    disabled_commands.iter().any(|disabled| disabled == base)
}

/// The disabled extension command a user prompt would invoke, if any.
///
/// Only the first whitespace-separated token counts: `/review 这个改动` invokes
/// `review`, while `please /review` is ordinary text sent to the model.
#[must_use]
pub fn disabled_command_in_prompt<'a>(
    text: &str,
    disabled_commands: &'a [String],
) -> Option<&'a str> {
    let invocation = text
        .trim_start()
        .strip_prefix('/')?
        .split_whitespace()
        .next()?;
    let base = command_base_name(invocation);
    disabled_commands
        .iter()
        .find(|disabled| disabled.as_str() == base)
        .map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_enable_every_builtin() {
        let toggles = ExtensionToggles::default();
        for ext in BUILTIN_EXTENSIONS {
            assert_eq!(toggles.is_enabled(ext.id), Some(true));
        }
        assert!(toggles.disabled_tools().is_empty());
        assert!(toggles.disabled_commands().is_empty());
    }

    #[test]
    fn missing_keys_default_to_enabled() {
        let toggles =
            ExtensionToggles::from_enabled_map(&HashMap::from([("subagent".to_string(), false)]));
        assert_eq!(toggles.is_enabled("web_search"), Some(true));
        assert_eq!(toggles.is_enabled("subagent"), Some(false));
        assert_eq!(toggles.disabled_tools(), vec!["subagent".to_string()]);
    }

    #[test]
    fn disabling_subagent_hides_its_tool() {
        let mut toggles = ExtensionToggles::default();
        assert!(toggles.set("subagent", false));
        assert_eq!(toggles.disabled_tools(), vec!["subagent".to_string()]);
        // The remaining built-ins register tools but no slash commands.
        assert!(toggles.disabled_commands().is_empty());
    }

    #[test]
    fn unknown_ids_are_rejected_but_preserved() {
        let map = HashMap::from([
            ("subagent".to_string(), false),
            ("third_party".to_string(), false),
        ]);
        let mut toggles = ExtensionToggles::from_enabled_map(&map);
        assert_eq!(toggles.is_enabled("third_party"), None);
        assert!(!toggles.set("third_party", true));
        assert_eq!(
            toggles.enabled_map().get("third_party"),
            Some(&false),
            "unknown extension keys must survive a settings round-trip"
        );

        // A known key can be re-enabled, and the unknown key stays untouched.
        assert!(toggles.set("subagent", true));
        assert_eq!(toggles.enabled_map().get("subagent"), Some(&true));
        assert_eq!(toggles.enabled_map().get("third_party"), Some(&false));
    }

    /// pi-rs 冲突消解后的 `review:1` 与 `review` 属于同一个扩展命令。
    #[test]
    fn conflict_resolved_invocations_match_the_base_name() {
        let disabled = vec!["review".to_string()];
        assert_eq!(command_base_name("review:1"), "review");
        assert!(is_command_disabled("review", &disabled));
        assert!(is_command_disabled("review:1", &disabled));
        assert!(!is_command_disabled("skill:foo", &disabled));
        assert!(!is_command_disabled("compact", &disabled));
    }

    /// 只有 prompt 的第一个 token 才是斜杠调用，正文里出现 `/review` 不算。
    #[test]
    fn prompt_guard_only_reads_the_leading_invocation() {
        let disabled = vec!["review".to_string()];
        assert_eq!(
            disabled_command_in_prompt("/review", &disabled),
            Some("review")
        );
        assert_eq!(
            disabled_command_in_prompt("  /review:1 看看这个改动", &disabled),
            Some("review")
        );
        assert_eq!(
            disabled_command_in_prompt("请用 /review 看看", &disabled),
            None
        );
        assert_eq!(disabled_command_in_prompt("/reviewer", &disabled), None);
        assert_eq!(disabled_command_in_prompt("/", &disabled), None);
        assert_eq!(disabled_command_in_prompt("review", &disabled), None);
        assert_eq!(disabled_command_in_prompt("/review", &[]), None);
    }
}
