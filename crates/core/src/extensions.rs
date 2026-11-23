//! Persistence for built-in extension toggles and per-extension settings.
//!
//! pi-rs already owns the canonical `settings.json` fields — `extensionsEnabled`
//! (`HashMap<extension_id, bool>`) for the toggles and one section per
//! configurable extension (`webSearch` for `web_search`) — so this module
//! reuses them instead of introducing a second config file. The agent/session
//! layer declares the metadata (toggles + settings descriptors) and the filter
//! logic; core only reads and writes through [`SettingsManager`].

use std::collections::BTreeMap;

use pi_coding_agent::core::settings_manager::SettingsManager;

pub use openatrading_agent::session::{
    command_base_name, disabled_command_in_prompt, is_command_disabled, BuiltinExtension,
    ExtensionSetting, ExtensionToggles, BUILTIN_EXTENSIONS,
};

/// Open the same global settings file the rest of the app uses.
fn settings_manager() -> SettingsManager {
    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    SettingsManager::create(&cwd, None)
}

/// Load effective built-in extension toggles from pi-rs settings.
///
/// Missing ids default to enabled (see [`ExtensionToggles`]).
#[must_use]
pub fn load_extension_toggles() -> ExtensionToggles {
    let manager = settings_manager();
    ExtensionToggles::from_enabled_map(&manager.get_extensions_enabled())
}

/// Persist the full toggle map to `settings.json` as `extensionsEnabled`.
///
/// Unknown extension ids carried in the map are written back unchanged.
///
/// # Errors
/// The map could not be serialized.
pub fn save_extension_toggles(toggles: &ExtensionToggles) -> Result<(), String> {
    let mut manager = settings_manager();
    let value = serde_json::to_value(toggles.enabled_map())
        .map_err(|e| format!("failed to serialize extension settings: {e}"))?;
    manager.set_global("extensionsEnabled", value);
    Ok(())
}

// ============================================================================
// Per-extension settings（extensions declare the keys, pi-rs resolves them）
// ============================================================================

/// Effective extension setting values, flattened to dotted `settings.json`
/// paths (`webSearch.deepseek.baseUrl` ⇢ `"https://api.deepseek.com"`).
///
/// Reads the merged global + project settings, so what the settings UI shows
/// is what pi-rs resolves. Unset keys are simply absent.
#[must_use]
pub fn load_extension_setting_values() -> BTreeMap<String, String> {
    let manager = settings_manager();
    let mut values = BTreeMap::new();
    if let Ok(json) = serde_json::to_value(manager.get_settings()) {
        flatten_json(&json, "", &mut values);
    }
    values
}

/// Why [`save_extension_setting`] refused or failed to write.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExtensionSettingError {
    /// Unknown extension id.
    #[error("未知扩展: {0}")]
    UnknownExtension(String),
    /// The extension does not declare this key.
    #[error("未知设置项: {0}")]
    UnknownKey(String),
    /// The value is not one of the declared options.
    #[error("{label} 不支持取值: {value}")]
    InvalidValue { label: String, value: String },
    /// The settings file could not be written.
    #[error("{0}")]
    Storage(String),
}

/// Write one extension setting to the global `settings.json`.
///
/// An empty value removes the key so pi-rs falls back to its own
/// environment/default resolution. Only keys the extension declares are
/// accepted, and dropdown values are validated against the declared options.
/// Other keys in the same section (e.g. a manually configured
/// `webSearch.deepseek.apiKey`) are preserved.
///
/// # Errors
/// See [`ExtensionSettingError`].
pub fn save_extension_setting(
    id: &str,
    key: &str,
    value: &str,
) -> Result<(), ExtensionSettingError> {
    let ext = BUILTIN_EXTENSIONS
        .iter()
        .find(|ext| ext.id == id)
        .ok_or_else(|| ExtensionSettingError::UnknownExtension(id.to_string()))?;
    let setting = ext
        .setting(key)
        .ok_or_else(|| ExtensionSettingError::UnknownKey(key.to_string()))?;

    let value = value.trim();
    if !value.is_empty()
        && !setting.options.is_empty()
        && !setting.options.iter().any(|(option, _)| *option == value)
    {
        return Err(ExtensionSettingError::InvalidValue {
            label: setting.label.to_string(),
            value: value.to_string(),
        });
    }

    let Some((section, _)) = key.split_once('.') else {
        return Err(ExtensionSettingError::UnknownKey(key.to_string()));
    };

    let mut manager = settings_manager();
    let root = serde_json::to_value(manager.get_global_settings())
        .map_err(|e| ExtensionSettingError::Storage(e.to_string()))?;
    let mut section_value = root
        .get(section)
        .cloned()
        .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
    set_section_leaf(&mut section_value, key, value);
    manager.set_global(section, section_value);
    // `set_global` 不返回 `Result`：落盘失败只记录在管理器里，读不到就当作
    // 写成功会骗过设置页（显示已保存、实际没写）。
    let errors = manager.drain_errors();
    if let Some(error) = errors.first() {
        return Err(ExtensionSettingError::Storage(error.message.clone()));
    }
    Ok(())
}

/// Flatten a JSON object into `path -> display string` pairs.
fn flatten_json(node: &serde_json::Value, prefix: &str, out: &mut BTreeMap<String, String>) {
    match node {
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten_json(value, &path, out);
            }
        }
        serde_json::Value::String(text) => {
            out.insert(prefix.to_string(), text.clone());
        }
        serde_json::Value::Null => {}
        other => {
            out.insert(prefix.to_string(), other.to_string());
        }
    }
}

/// Set (or, for an empty value, remove) the leaf of `key` (a dotted
/// `settings.json` path) inside its section value.
///
/// Path segments come from the extension's own descriptor, so they are trusted
/// literals — `webSearch.deepseek.baseUrl` never leaves an empty object behind.
fn set_section_leaf(node: &mut serde_json::Value, key: &str, value: &str) {
    let parts: Vec<&str> = key.split('.').skip(1).collect();
    set_section_leaf_parts(node, &parts, value);
}

fn set_section_leaf_parts(node: &mut serde_json::Value, parts: &[&str], value: &str) {
    let Some((head, rest)) = parts.split_first() else {
        return;
    };
    let Some(map) = node.as_object_mut() else {
        return;
    };
    if rest.is_empty() {
        if value.is_empty() {
            map.remove(*head);
        } else {
            map.insert(
                (*head).to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }
        return;
    }
    // 删除不存在的路径是空操作：不要为了清空一个键而造出 `{}` 容器。
    if value.is_empty() && !map.contains_key(*head) {
        return;
    }
    let child = map
        .entry((*head).to_string())
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    if !child.is_object() {
        *child = serde_json::Value::Object(serde_json::Map::new());
    }
    set_section_leaf_parts(child, rest, value);
    // 清空最后一个叶子后不留下 `{}`：pi-rs 把缺失的段当作未配置。
    if child.as_object().is_some_and(serde_json::Map::is_empty) {
        map.remove(*head);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 写入/清空都要落在叶子键上，并且不留下空容器：
    /// pi-rs 把缺失的段当作「未配置」，`{}` 不是。
    #[test]
    fn section_leaf_writes_drop_empty_containers() {
        let mut section = serde_json::json!({});
        set_section_leaf(&mut section, "webSearch.provider", "deepseek");
        assert_eq!(section, serde_json::json!({ "provider": "deepseek" }));

        set_section_leaf(&mut section, "webSearch.deepseek.baseUrl", "https://x");
        assert_eq!(
            section,
            serde_json::json!({ "provider": "deepseek", "deepseek": { "baseUrl": "https://x" } })
        );

        set_section_leaf(&mut section, "webSearch.deepseek.baseUrl", "");
        assert_eq!(section, serde_json::json!({ "provider": "deepseek" }));

        // 清空一个从未配置过的键：不制造 `ollama: {}`。
        set_section_leaf(&mut section, "webSearch.ollama.host", "");
        assert_eq!(section, serde_json::json!({ "provider": "deepseek" }));
    }

    #[test]
    fn flatten_reports_dotted_paths() {
        let mut values = BTreeMap::new();
        flatten_json(
            &serde_json::json!({ "webSearch": { "provider": "auto", "ollama": { "host": "http://h" } } }),
            "",
            &mut values,
        );
        assert_eq!(
            values.get("webSearch.provider").map(String::as_str),
            Some("auto")
        );
        assert_eq!(
            values.get("webSearch.ollama.host").map(String::as_str),
            Some("http://h")
        );
    }

    /// 只有扩展声明过的键能写；下拉项的取值必须命中声明的候选值。
    #[test]
    fn unknown_keys_and_values_are_rejected() {
        assert_eq!(
            save_extension_setting("nope", "webSearch.provider", "auto"),
            Err(ExtensionSettingError::UnknownExtension("nope".to_string()))
        );
        assert_eq!(
            save_extension_setting("web_search", "webSearch.deepseek.apiKey", "sk-x"),
            Err(ExtensionSettingError::UnknownKey(
                "webSearch.deepseek.apiKey".to_string()
            ))
        );
        assert!(matches!(
            save_extension_setting("web_search", "webSearch.provider", "bing"),
            Err(ExtensionSettingError::InvalidValue { .. })
        ));
    }
}
