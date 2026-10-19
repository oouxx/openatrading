//! Provider 凭据管理：pi-rs `auth.json` 的读写投影。
//!
//! 读写一律走 pi-rs 的 [`AuthStorage`]，provider 名单与状态判定复用
//! [`ModelRegistry`]——本模块不解析也不序列化 `auth.json`，也不自建 provider 表，
//! 避免与模型选择器给出的口径不一致。

use std::path::{Path, PathBuf};

use serde::Serialize;

use pi_coding_agent::core::auth_storage::{AuthCredential, AuthStorage};
use pi_coding_agent::core::model_registry::ModelRegistry;
use pi_coding_agent::core::provider_display_names::get_provider_display_name;

// ============================================================================
// 模型注册表
// ============================================================================

/// 构建与会话工厂同口径的 `ModelRegistry`，凭据解析指向 `auth_path`。
///
/// **必须**接上 `wire_auth_resolver`：不接的话 `has_configured_auth` 看不到
/// `auth.json` 里存的 key（pi-rs 只在会话装配路径上接了它），模型选择器会把
/// 已配置的 provider 整片判成未配置、全部置灰。
#[must_use]
pub fn model_registry_with_auth(auth_path: PathBuf) -> ModelRegistry {
    // 与 `create_agent_session` 一致：先注册内置 provider，再构建注册表。
    pi_coding_agent::pi_agent_core::pi_ai::providers::register_builtins::register_built_in_api_providers();
    let mut registry = ModelRegistry::new(ModelRegistry::builtin_models_list());
    registry.wire_auth_resolver(auth_path);
    registry
}

/// 按 pi-rs 默认位置（`{agent_dir}/auth.json`，随 `PI_RS_HOME` 变化）构建注册表。
#[must_use]
pub fn wired_model_registry() -> ModelRegistry {
    model_registry_with_auth(pi_coding_agent::config::get_auth_path())
}

// ============================================================================
// 读写形状
// ============================================================================

/// 单个 provider 的凭据状态。
///
/// **不含密钥原文**：`label` 是来源标识（环境变量名、`--api-key` 等），
/// 由 pi-rs `get_auth_status` 给出。
///
/// `rename_all = "camelCase"`：这个 view 直接序列化成 `GET /api/agent/auth` 的
/// 响应体，多词字段必须是 camelCase（前端 `useAgentAuth` 声明的是 `displayName`）。
/// 漏掉它时前端读到 `undefined`——provider 下拉框每行都是空白，看着像“没有 provider”。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderAuthView {
    pub provider: String,
    /// 人类可读名，取不到时回落到 provider id。
    pub display_name: String,
    pub configured: bool,
    /// `"stored"`（`auth.json`）/ `"environment"` / `"runtime"` / `"fallback"`；
    /// 未配置为 `None`。只有 `"stored"` 的条目能通过本模块删除。
    pub source: Option<String>,
    pub label: Option<String>,
}

/// 凭据读写的错误。
#[derive(Debug, thiserror::Error)]
pub enum AgentAuthError {
    /// provider 不在模型注册表里——拒绝写入，避免往 `auth.json` 塞任意键。
    #[error("未知的 provider: {0}")]
    UnknownProvider(String),
    #[error("API Key 不能为空")]
    EmptyKey,
    #[error("API Key 不能包含换行符")]
    MultilineKey,
    /// 该 provider 没有存在 `auth.json` 里的凭据（环境变量提供的配置不归这里管）。
    #[error("{0} 没有已保存的 API Key")]
    NotFound(String),
    /// 落盘失败。
    #[error("{0}")]
    Storage(String),
}

// ============================================================================
// 读写入口
// ============================================================================

/// `auth.json` 的读写入口。
#[derive(Debug)]
pub struct AgentAuthStore {
    auth_path: PathBuf,
}

impl AgentAuthStore {
    /// 显式指定 `auth.json` 路径（测试用）。
    #[must_use]
    pub fn new(auth_path: PathBuf) -> Self {
        Self { auth_path }
    }

    /// 按 pi-rs 默认位置（`{agent_dir}/auth.json`）构造。
    #[must_use]
    pub fn from_pi_rs() -> Self {
        Self::new(pi_coding_agent::config::get_auth_path())
    }

    /// 受管的 `auth.json` 路径。
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.auth_path
    }

    /// 全部已知 provider 及其凭据状态。
    ///
    /// 名单来自模型注册表（与 `/api/agent/models`、会话工厂同一来源），因此
    /// 设置页能配的 provider 与模型选择器能选的 provider 恒等。
    #[must_use]
    pub fn providers(&self) -> Vec<ProviderAuthView> {
        let storage = self.storage();
        wired_model_registry()
            .get_providers()
            .into_iter()
            .map(|provider| {
                let status = storage.get_auth_status(&provider);
                ProviderAuthView {
                    display_name: get_provider_display_name(&provider)
                        .map_or_else(|| provider.clone(), str::to_string),
                    provider,
                    configured: status.configured,
                    source: status.source,
                    label: status.label,
                }
            })
            .collect()
    }

    /// 写入（或覆盖）`provider` 的 API Key。
    ///
    /// # Errors
    /// provider 未知 → [`AgentAuthError::UnknownProvider`]；key 为空/含换行 →
    /// [`AgentAuthError::EmptyKey`] / [`AgentAuthError::MultilineKey`]；落盘失败 →
    /// [`AgentAuthError::Storage`]。
    pub fn set_api_key(&self, provider: &str, key: &str) -> Result<(), AgentAuthError> {
        let provider = require_known_provider(provider)?;
        let key = validate_key(key)?;
        let mut storage = self.storage();
        storage.set(
            &provider,
            AuthCredential::ApiKey {
                key: Some(key),
                env: None,
            },
        );
        // 写盘错误在 pi-rs 侧被吞掉（见 `reload_from_disk` 注释），只能回读确认。
        if !self.reload_from_disk().has(&provider) {
            return Err(AgentAuthError::Storage(format!(
                "API Key 写入 {} 失败，请检查该文件的写权限",
                self.auth_path.display()
            )));
        }
        harden_permissions(&self.auth_path);
        Ok(())
    }

    /// 删除 `provider` 存在 `auth.json` 里的凭据。
    ///
    /// # Errors
    /// provider 未知 → [`AgentAuthError::UnknownProvider`]；凭据不在文件里（例如来自
    /// 环境变量）→ [`AgentAuthError::NotFound`]；落盘失败 → [`AgentAuthError::Storage`]。
    pub fn delete_api_key(&self, provider: &str) -> Result<(), AgentAuthError> {
        let provider = require_known_provider(provider)?;
        let mut storage = self.storage();
        if !storage.has(&provider) {
            // 环境变量/扩展提供的凭据删不掉，也不该假装删掉了。
            return Err(AgentAuthError::NotFound(provider));
        }
        storage.remove(&provider);
        if self.reload_from_disk().has(&provider) {
            return Err(AgentAuthError::Storage(format!(
                "API Key 删除失败：{} 不可写",
                self.auth_path.display()
            )));
        }
        Ok(())
    }

    /// 新建一个 `AuthStorage`：每次都重新读文件，因此能看到外部改动。
    fn storage(&self) -> AuthStorage {
        AuthStorage::create(self.auth_path.clone())
    }

    /// 重新从磁盘读一遍，用于确认写入/删除真的落盘了。
    ///
    /// pi-rs 的 `FileAuthStorageBackend::write_content` 丢弃 `fs::write` 的 `Result`，
    /// `drain_errors()` 也恒为空（该字段无人写入），所以只能回读校验。
    fn reload_from_disk(&self) -> AuthStorage {
        self.storage()
    }
}

/// provider 必须在模型注册表里，否则拒绝写入 `auth.json`。
fn require_known_provider(provider: &str) -> Result<String, AgentAuthError> {
    let trimmed = provider.trim();
    if wired_model_registry()
        .get_providers()
        .iter()
        .any(|known| known == trimmed)
    {
        Ok(trimmed.to_string())
    } else {
        Err(AgentAuthError::UnknownProvider(provider.to_string()))
    }
}

/// 校验 API Key：去首尾空白后非空、不含换行。
///
/// 换行会破坏 `Authorization` 头，也说明粘贴时混进了别的内容。
fn validate_key(key: &str) -> Result<String, AgentAuthError> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err(AgentAuthError::EmptyKey);
    }
    if trimmed.contains(['\n', '\r']) {
        return Err(AgentAuthError::MultilineKey);
    }
    Ok(trimmed.to_string())
}

/// 把 `auth.json` 权限收紧到 `0600`。
///
/// pi-rs 用 `fs::write` 落盘，权限沿用进程 umask（通常 0644，同机其他用户可读），
/// 而该文件存的是明文密钥。收紧失败只告警不报错——这是加固，不是保存的前置条件。
#[cfg(unix)]
fn harden_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Err(e) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)) {
        crate::log::warn!("收紧 {} 权限失败: {e}", path.display());
    }
}

#[cfg(not(unix))]
fn harden_permissions(_path: &Path) {}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use pi_coding_agent::core::auth_storage::find_env_keys;

    /// 测试用 provider：内置模型列表里确实存在的一个。
    const PROVIDER: &str = "opencode-go";

    fn store(dir: &tempfile::TempDir) -> AgentAuthStore {
        AgentAuthStore::new(dir.path().join("auth.json"))
    }

    fn find<'a>(views: &'a [ProviderAuthView], provider: &str) -> &'a ProviderAuthView {
        views
            .iter()
            .find(|v| v.provider == provider)
            .unwrap_or_else(|| panic!("{provider} 不在 provider 列表里"))
    }

    /// Happy path：写入 → 状态变 `stored` → 删除 → 回到未配置。
    #[test]
    fn set_then_delete_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);

        assert!(!find(&store.providers(), PROVIDER).configured);

        store.set_api_key(PROVIDER, "sk-test-123").unwrap();
        let after_set = store.providers();
        let view = find(&after_set, PROVIDER);
        assert!(view.configured);
        assert_eq!(view.source.as_deref(), Some("stored"));

        store.delete_api_key(PROVIDER).unwrap();
        assert!(!find(&store.providers(), PROVIDER).configured);
    }

    /// 视图里绝不能出现密钥原文（它会被送到放开 CORS 的管理接口）。
    #[test]
    fn views_never_leak_the_key() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);
        store.set_api_key(PROVIDER, "sk-super-secret").unwrap();

        let json = serde_json::to_string(&store.providers()).unwrap();
        assert!(!json.contains("sk-super-secret"), "{json}");
    }

    /// 契约回归：`GET /api/agent/auth` 的多词字段必须是 camelCase。
    ///
    /// 前端读的是 `displayName`；漏掉 `rename_all` 时该字段为 `undefined`，
    /// 设置页 provider 下拉框整列空白。
    #[test]
    fn provider_view_serializes_camel_case_keys() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);

        let json = serde_json::to_value(store.providers()).unwrap();
        let first = &json.as_array().expect("providers 应为数组")[0];
        assert!(
            first.get("displayName").and_then(|v| v.as_str()).is_some(),
            "displayName 缺失或非字符串: {first}"
        );
        assert!(first.get("display_name").is_none(), "不该出现 snake_case: {first}");
        // 单词字段不受 rename 影响，仍按原名透给前端
        for key in ["provider", "configured", "source", "label"] {
            assert!(first.get(key).is_some(), "缺字段 {key}: {first}");
        }
    }

    /// provider 白名单：不在模型注册表里的名字拒绝写入，避免污染 `auth.json`。
    #[test]
    fn unknown_provider_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);
        assert!(matches!(
            store.set_api_key("not-a-real-provider", "sk-test"),
            Err(AgentAuthError::UnknownProvider(_))
        ));
        assert!(!store.path().exists(), "拒绝的请求不应创建 auth.json");
    }

    /// 边界：空 key、纯空白 key、key 内部带换行都必须拒绝。
    ///
    /// 首尾换行不算——那是粘贴的常见残留，`trim` 会裁掉（见 `key_is_trimmed_*`）。
    #[test]
    fn empty_and_multiline_keys_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);

        for bad in ["", "   ", "\n", "\t "] {
            assert!(
                matches!(
                    store.set_api_key(PROVIDER, bad),
                    Err(AgentAuthError::EmptyKey)
                ),
                "应判为空: {bad:?}"
            );
        }
        for bad in ["sk-a\nsk-b", "sk-a\r\nsk-b", "sk-a\rsk-b"] {
            assert!(
                matches!(
                    store.set_api_key(PROVIDER, bad),
                    Err(AgentAuthError::MultilineKey)
                ),
                "应判为多行: {bad:?}"
            );
        }
    }

    /// 边界：key 首尾空白（含粘贴常见的尾随换行）被裁掉后写入。
    #[test]
    fn key_is_trimmed_before_storing() {
        for raw in ["  sk-padded  ", "sk-padded\n", "\n\tsk-padded"] {
            let dir = tempfile::tempdir().unwrap();
            let store = store(&dir);
            store.set_api_key(PROVIDER, raw).unwrap();

            let storage = AuthStorage::create(store.path().to_path_buf());
            match storage.get(PROVIDER) {
                Some(AuthCredential::ApiKey { key: Some(key), .. }) => {
                    assert_eq!(key, "sk-padded", "raw={raw:?}");
                }
                other => panic!("expected stored api key, got {other:?}"),
            }
        }
    }

    /// Failure path：没有已保存凭据时删除应报 `NotFound`，而不是静默成功。
    #[test]
    fn delete_without_stored_credential_is_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);
        assert!(matches!(
            store.delete_api_key(PROVIDER),
            Err(AgentAuthError::NotFound(_))
        ));
    }

    /// Failure path：`auth.json` 所在目录不可写时，写入必须报错而不是假装成功。
    #[test]
    fn unwritable_auth_path_surfaces_as_storage_error() {
        let dir = tempfile::tempdir().unwrap();
        // 父路径是个文件 → 建目录/写文件必然失败。
        let blocker = dir.path().join("blocker");
        std::fs::write(&blocker, "x").unwrap();
        let store = AgentAuthStore::new(blocker.join("agent").join("auth.json"));

        assert!(matches!(
            store.set_api_key(PROVIDER, "sk-test"),
            Err(AgentAuthError::Storage(_))
        ));
    }

    /// 回归：接上 `wire_auth_resolver` 后，`auth.json` 里的 key 才参与
    /// `is_provider_configured` 判定——这正是模型列表整片置灰的根因。
    #[test]
    fn wired_registry_sees_stored_keys() {
        let dir = tempfile::tempdir().unwrap();
        let auth_path = dir.path().join("auth.json");

        // 未接 resolver 的注册表看不到文件里的 key。若本机恰好设了
        // `OPENCODE_API_KEY`，环境变量本身就会让该 provider 判为已配置，
        // 这一半断言失去区分度，跳过（下面「接了就必须为真」不受影响）。
        if find_env_keys(PROVIDER).is_empty() {
            let unwired = ModelRegistry::new(ModelRegistry::builtin_models_list());
            assert!(!unwired.is_provider_configured(PROVIDER));
        }

        AgentAuthStore::new(auth_path.clone())
            .set_api_key(PROVIDER, "sk-test")
            .unwrap();

        let wired = model_registry_with_auth(auth_path);
        assert!(
            wired.is_provider_configured(PROVIDER),
            "wire_auth_resolver 未生效，模型列表会整片置灰"
        );
        // 该 provider 下的模型也应随之可选。
        let models = wired.get_models_for_provider(PROVIDER);
        assert!(!models.is_empty(), "{PROVIDER} 应至少有一个模型");
        assert!(models.iter().all(|m| wired.has_configured_auth(m)));
    }

    /// `auth.json` 落盘后权限应收紧到 0600（默认 umask 下是 0644）。
    #[cfg(unix)]
    #[test]
    fn stored_file_is_not_world_readable() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);
        store.set_api_key(PROVIDER, "sk-test").unwrap();

        let mode = std::fs::metadata(store.path())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "auth.json 权限应为 600，实际 {mode:o}");
    }
}
