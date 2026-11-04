//! MCP 客户端：配置读写 + pi-rs SDK 的再导出。
//!
//! **协议实现与连接完全由 pi-rs SDK 负责**，本项目不再持有任何 MCP 客户端代码。
//! 会话工厂走的 `create_agent_session_from_services` 最终调到
//! `pi_coding_agent::core::sdk::create_agent_session`，后者在每次建会话时读
//! `{agent_dir}/mcp.json` + `{cwd}/.pi-rs/mcp.json` 并连接（见 pi-rs
//! `core/sdk.rs` 的 MCP 段），因此配置变更对新会话立即生效，无需重启。
//!
//! 本模块只提供三件事：
//!
//! 1. 再导出 pi-rs 的配置/连接原语，使 web / cli 不必直接依赖 `pi-coding-agent`；
//! 2. [`McpConfigStore`]——管理界面与 `doctor` 用的读写投影（列表视图 + 增删改启停）；
//! 3. [`McpEndpoint`]——CLI 对单个 HTTP 端点做一次性连接/列工具/调用工具。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub use pi_coding_agent::core::mcp_config::{
    add_server, global_config_path, load_servers, project_config_path, remove_server, set_enabled,
    LoadedServer,
};
pub use pi_coding_agent::core::mcp_runtime::McpServerConnection;
pub use pi_coding_agent::core::mcp_servers::{
    validate_mcp_server_config, McpExposure, McpScope, McpServerConfig,
};

// ============================================================================
// 读写形状
// ============================================================================

/// 管理界面用的传输配置（收发同一形状）。
///
/// 与 pi-rs `mcp_config` 的 `mcpServers` 条目一一对应，但少了两个 pi-rs 不支持的
/// 历史字段：SSE 的 `post_url`（rmcp 自行推断 POST 地址）与 `auth_token`
/// （鉴权改用 `headers`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum McpTransportConfig {
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: BTreeMap<String, String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cwd: Option<String>,
    },
    Http {
        url: String,
        #[serde(default)]
        headers: BTreeMap<String, String>,
    },
    Sse {
        url: String,
        #[serde(default)]
        headers: BTreeMap<String, String>,
    },
}

impl McpTransportConfig {
    /// 校验必填字段：stdio 需要 command，http/sse 需要 url。
    fn validate(&self) -> Result<(), McpConfigError> {
        match self {
            Self::Stdio { command, .. } if command.trim().is_empty() => {
                Err(McpConfigError::Invalid("stdio 服务器需要 command".into()))
            }
            Self::Http { url, .. } | Self::Sse { url, .. } if url.trim().is_empty() => {
                Err(McpConfigError::Invalid("HTTP/SSE 服务器需要 url".into()))
            }
            _ => Ok(()),
        }
    }
}

/// 列表视图：配置内容 + 它来自哪个作用域/文件。
#[derive(Debug, Clone, Serialize)]
pub struct McpServerView {
    pub name: String,
    pub enabled: bool,
    /// `"global"`（`{agent_dir}/mcp.json`）或 `"project"`（`{cwd}/.pi-rs/mcp.json`）。
    pub scope: &'static str,
    /// 定义该条目的配置文件路径。
    pub source: String,
    pub transport: McpTransportConfig,
    /// 生效的工具暴露方式（pi-rs 缺省 `codemode`，等价于按需加载）。
    pub exposure: McpExposure,
    /// 配置里是否显式写了 `exposure`。旧配置为 `false`，界面据此提示
    /// “默认按需加载”。
    #[serde(rename = "exposureExplicit")]
    pub exposure_explicit: bool,
}

/// 写入载荷（新增/编辑）。
///
/// 不含 `scope`/`source`：写入目标固定为全局 `mcp.json`，项目级条目只读（见
/// [`McpConfigStore::update`]）。
#[derive(Debug, Clone, Deserialize)]
pub struct McpServerPayload {
    pub name: String,
    pub enabled: bool,
    pub transport: McpTransportConfig,
    /// 工具暴露方式；新增时缺省为 `direct`，编辑时缺省保留原值。
    #[serde(default)]
    pub exposure: Option<McpExposure>,
}

/// 一次性连接测试的结果。
///
/// 连接失败属于测试结果而非配置错误，因此仍以 `Ok` 返回，让界面展示原因。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerTest {
    pub ok: bool,
    /// 握手成功后枚举到的工具数。
    pub tool_count: usize,
    /// 失败原因（成功时为 `None`）。
    pub error: Option<String>,
    /// 握手 + 工具枚举耗时。
    pub duration_ms: u64,
}

/// MCP 配置读写的错误。
#[derive(Debug, thiserror::Error)]
pub enum McpConfigError {
    /// 名字已被占用（`POST`/改名冲突）。
    #[error("MCP server already exists")]
    Exists,
    /// 名字不存在。
    #[error("MCP server not found")]
    NotFound,
    /// 条目定义在项目级文件里——项目级按名覆盖全局，写全局不会生效。
    #[error("MCP server is defined in project config: {0}")]
    ProjectScoped(String),
    /// 载荷不合法（空名、缺 command/url、名字含控制字符等）。
    #[error("invalid MCP server config: {0}")]
    Invalid(String),
    /// pi-rs 侧的文件/IO 错误。
    #[error("{0}")]
    Config(String),
}

// ============================================================================
// 配置读写
// ============================================================================

/// 两个 `mcp.json` 的读写入口。
///
/// 写入一律落在全局文件（用户决策），项目级条目在管理界面只读：项目级按名覆盖
/// 全局，若允许写全局，编辑结果会被项目条目静默盖掉。
#[derive(Debug)]
pub struct McpConfigStore {
    cwd: String,
    agent_dir: String,
    /// 串行化本进程内的写操作。
    ///
    /// pi-rs 的 `add_server`/`remove_server`/`set_enabled` 是「读文件 → 改 → 写回」，
    /// 两个并发请求会互相覆盖（丢配置）。管理界面允许同时操作多张卡片，故在此互斥。
    /// 跨进程（例如同时跑 `pi-rs mcp add`）仍可能竞争，属已知限制。
    writes: std::sync::Mutex<()>,
}

impl McpConfigStore {
    /// 显式指定位置（测试用）。
    #[must_use]
    pub fn new(cwd: impl Into<String>, agent_dir: impl Into<String>) -> Self {
        Self {
            cwd: cwd.into(),
            agent_dir: agent_dir.into(),
            writes: std::sync::Mutex::new(()),
        }
    }

    /// 按进程 cwd 与 pi-rs agent 目录构造。
    ///
    /// # Errors
    /// 取不到进程 cwd 时返回 [`McpConfigError::Config`]。
    pub fn from_process() -> Result<Self, McpConfigError> {
        let cwd = std::env::current_dir()
            .map_err(|e| McpConfigError::Config(format!("无法获取当前工作目录: {e}")))?;
        let agent_dir = pi_coding_agent::config::get_agent_dir();
        Ok(Self::new(
            cwd.to_string_lossy().to_string(),
            agent_dir.to_string_lossy().to_string(),
        ))
    }

    /// 全局 `mcp.json`（`{agent_dir}/mcp.json`）——写入目标。
    #[must_use]
    pub fn global_path(&self) -> PathBuf {
        global_config_path(Path::new(&self.agent_dir))
    }

    /// 项目级 `mcp.json`（`{cwd}/.pi-rs/mcp.json`）。
    #[must_use]
    pub fn project_path(&self) -> PathBuf {
        project_config_path(Path::new(&self.cwd))
    }

    /// pi-rs 视角的合并结果（含禁用条目）与解析错误，供 `doctor` 等需要连接/规格的场景。
    #[must_use]
    pub fn loaded(&self) -> (Vec<LoadedServer>, Vec<String>) {
        load_servers(&self.cwd, &self.agent_dir)
    }

    /// 管理界面用的列表视图与解析错误。
    #[must_use]
    pub fn servers(&self) -> (Vec<McpServerView>, Vec<String>) {
        let (loaded, errors) = self.loaded();
        (loaded.iter().map(server_view).collect(), errors)
    }

    /// 新增一个 server（写全局文件）。
    ///
    /// # Errors
    /// 名字已存在 → [`McpConfigError::Exists`]；载荷非法 → [`McpConfigError::Invalid`]。
    pub fn create(&self, payload: &McpServerPayload) -> Result<(), McpConfigError> {
        let name = validate_name(&payload.name)?;
        if self.find(&name).is_some() {
            return Err(McpConfigError::Exists);
        }
        let raw = apply_payload(None, &name, payload)?;
        self.with_write_lock(|| add_server(&self.global_path(), &name, &raw).map(|_| ()))
    }

    /// 编辑一个 server（含改名）。
    ///
    /// # Errors
    /// 不存在 → [`McpConfigError::NotFound`]；定义在项目级文件 → [`McpConfigError::ProjectScoped`]；
    /// 改名撞上已有名字 → [`McpConfigError::Exists`]。
    pub fn update(
        &self,
        current_name: &str,
        payload: &McpServerPayload,
    ) -> Result<(), McpConfigError> {
        self.require_global(current_name)?;
        let new_name = validate_name(&payload.name)?;
        if new_name != current_name && self.find(&new_name).is_some() {
            return Err(McpConfigError::Exists);
        }
        // 编辑时保留 UI 不认识的字段（exposure/timeout/oauth/auth/...）。
        let existing = raw_entry(&self.global_path(), current_name);
        let raw = apply_payload(existing.as_ref(), &new_name, payload)?;
        let path = self.global_path();
        // 改名分两步（pi-rs 没有原子重命名）。先写新条目再删旧条目：万一中途失败，
        // 留下的是「新旧并存」这种可见、可手动收拾的状态，而不是静默丢掉用户配置。
        self.with_write_lock(|| {
            add_server(&path, &new_name, &raw)?;
            if new_name != current_name {
                remove_server(&path, current_name)?;
            }
            Ok(())
        })
    }

    /// 删除一个 server（从定义它的全局文件里删）。
    ///
    /// # Errors
    /// 不存在 → [`McpConfigError::NotFound`]；定义在项目级文件 → [`McpConfigError::ProjectScoped`]。
    pub fn delete(&self, name: &str) -> Result<(), McpConfigError> {
        self.require_global(name)?;
        let path = self.global_path();
        // 前一步已确认条目存在，`false` 只可能来自并发的外部改写。
        if self.with_write_lock(|| remove_server(&path, name))? {
            Ok(())
        } else {
            Err(McpConfigError::NotFound)
        }
    }

    /// 翻转启用状态。
    ///
    /// # Errors
    /// 不存在 → [`McpConfigError::NotFound`]；定义在项目级文件 → [`McpConfigError::ProjectScoped`]。
    pub fn toggle(&self, name: &str) -> Result<(), McpConfigError> {
        let existing = self.require_global(name)?;
        let path = self.global_path();
        let target = !existing.is_enabled();
        if self.with_write_lock(|| set_enabled(&path, name, target))? {
            Ok(())
        } else {
            Err(McpConfigError::NotFound)
        }
    }

    /// 为一个已加载条目建立连接句柄（尚未连接，首次 `get_client` 才握手）。
    ///
    /// `doctor` 用它做健康检查；显式传入本 store 的 cwd/agent_dir，避免依赖进程全局状态。
    #[must_use]
    pub fn connection(&self, server: LoadedServer) -> McpServerConnection {
        McpServerConnection::with_agent_dir(server, self.cwd.clone(), self.agent_dir.clone())
    }

    /// 测试一个条目能否连接（握手 + 枚举工具）。
    ///
    /// pi-rs 的 `get_client` 自带 10s 连接超时；连接失败作为测试结果返回，
    /// 只有名字不存在才是配置错误。
    ///
    /// # Errors
    /// 名字不存在 → [`McpConfigError::NotFound`]。
    pub async fn test(&self, name: &str) -> Result<McpServerTest, McpConfigError> {
        let server = self.find(name).ok_or(McpConfigError::NotFound)?;
        let connection = self.connection(server);
        let started = std::time::Instant::now();
        let result = connection.get_client().await;
        let tool_count = connection.tools().len();
        connection.close().await;
        let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        Ok(match result {
            Ok(_) => McpServerTest {
                ok: true,
                tool_count,
                error: None,
                duration_ms,
            },
            Err(error) => McpServerTest {
                ok: false,
                tool_count: 0,
                error: Some(error),
                duration_ms,
            },
        })
    }

    /// 查一个条目（合并后的视角）。
    fn find(&self, name: &str) -> Option<LoadedServer> {
        let (servers, _) = self.loaded();
        servers.into_iter().find(|s| s.name == name)
    }

    /// 找到条目并要求它完全归全局文件管。
    ///
    /// 有项目级 override 的条目也拒绝：项目文件覆盖了 `enabled`/`exposure`，
    /// 只改全局文件不会改变实际生效状态。
    fn require_global(&self, name: &str) -> Result<LoadedServer, McpConfigError> {
        let server = self.find(name).ok_or(McpConfigError::NotFound)?;
        if server.scope == McpScope::Global && server.override_path.is_none() {
            Ok(server)
        } else {
            let blocker = server.override_path.as_ref().unwrap_or(&server.source);
            Err(McpConfigError::ProjectScoped(
                blocker.to_string_lossy().to_string(),
            ))
        }
    }

    /// 持写锁执行——锁内不得有 await（这些操作全是同步文件读写）。
    fn with_write_lock<T>(
        &self,
        f: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, McpConfigError> {
        let _guard = self
            .writes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f().map_err(McpConfigError::Config)
    }
}

/// 名字校验：非空、不含控制字符。
///
/// 字符集（字母/数字/`_`/`-`）交给 pi-rs 的 `validate_mcp_server_config` 做权威
/// 校验——它才是会话加载时真正执行的规则，避免我们放行 pi-rs 会拒绝的名字。
fn validate_name(name: &str) -> Result<String, McpConfigError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(McpConfigError::Invalid("名称不能为空".into()));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(McpConfigError::Invalid("名称不能包含控制字符".into()));
    }
    Ok(trimmed.to_string())
}

/// 载荷 → `mcpServers` 原始条目。
///
/// 形状必须与 pi-rs `mcp_servers::validate_mcp_server_config` 的解析一致：
/// - stdio 不写 `type`（`command` 服务器遇到非 `"stdio"` 的 type 会被拒绝）；
/// - http 省略 `type`（空字符串与 `"http"` 都按 streamable HTTP 解析）；
/// - sse **必须**写 `type: "sse"`，否则带 `url` 的条目会被当成 http。
///
/// `existing` 是编辑前的原始条目：除传输字段与 `enabled` 外的键（`exposure`、
/// `timeout`、`oauth`、`auth`、`toolExposure` 等 pi-rs 新字段）原样保留，避免
/// 从管理界面保存时把用户手写的配置删掉。
fn apply_payload(
    existing: Option<&serde_json::Value>,
    name: &str,
    payload: &McpServerPayload,
) -> Result<serde_json::Value, McpConfigError> {
    payload.transport.validate()?;
    let mut entry = existing
        .and_then(serde_json::Value::as_object)
        .cloned()
        .unwrap_or_default();

    // 先清空所有传输字段，避免切换类型后 `command` 与 `url` 并存。
    for key in ["type", "command", "args", "env", "cwd", "url", "headers"] {
        entry.remove(key);
    }

    if payload.enabled {
        entry.remove("enabled");
    } else {
        entry.insert("enabled".into(), serde_json::Value::Bool(false));
    }

    match &payload.transport {
        McpTransportConfig::Stdio {
            command,
            args,
            env,
            cwd,
        } => {
            entry.insert("command".into(), serde_json::Value::String(command.clone()));
            if !args.is_empty() {
                entry.insert("args".into(), serde_json::json!(args));
            }
            if !env.is_empty() {
                entry.insert("env".into(), serde_json::json!(env));
            }
            if let Some(cwd) = cwd.as_deref().filter(|c| !c.trim().is_empty()) {
                entry.insert("cwd".into(), serde_json::Value::String(cwd.to_string()));
            }
        }
        McpTransportConfig::Http { url, headers } => {
            entry.insert("url".into(), serde_json::Value::String(url.clone()));
            if !headers.is_empty() {
                entry.insert("headers".into(), serde_json::json!(headers));
            }
        }
        McpTransportConfig::Sse { url, headers } => {
            entry.insert("type".into(), serde_json::Value::String("sse".into()));
            entry.insert("url".into(), serde_json::Value::String(url.clone()));
            if !headers.is_empty() {
                entry.insert("headers".into(), serde_json::json!(headers));
            }
        }
    }

    // 本应用没有 codemode 脚本。pi-rs 的 exposure 默认是 `codemode`（映射为
    // `tool_search` 才能看到的 deferred 工具），而管理界面的语义是「配置即出现在
    // 工具列表里」。因此新增条目默认写 `direct`。
    //
    // 编辑时若载荷没带 exposure（旧客户端），保留文件里的原值——包括“未设置”，
    // 让界面如实显示 pi-rs 的默认行为，而不是悄悄改配置。
    match payload.exposure {
        Some(exposure) => {
            entry.insert(
                "exposure".into(),
                serde_json::Value::String(exposure.to_string()),
            );
        }
        None if existing.is_none() => {
            entry.insert(
                "exposure".into(),
                serde_json::Value::String(McpExposure::Direct.to_string()),
            );
        }
        None => {}
    }

    let raw = serde_json::Value::Object(entry);
    // 交给 pi-rs 做权威校验（命名规则、command/url 互斥、exposure 取值等），
    // 保证写进去的文件一定能被会话加载。
    validate_mcp_server_config(name, &raw).map_err(McpConfigError::Invalid)?;
    Ok(raw)
}

/// `LoadedServer` → 列表视图。
///
/// `env`/`headers` 的值取自**原始文件文本**而非 `LoadedServer`：pi-rs 在加载时会把
/// `${VAR}` / `!cmd` 解析成实际值，若回显解析结果，既会把密钥送到（CORS 放开、可能
/// 监听局域网的）管理接口，也会让「编辑保存」把引用替换成明文写回文件。读不到原始
/// 条目时退回解析后的值（至少不丢配置）。
fn server_view(server: &LoadedServer) -> McpServerView {
    let raw_env = || raw_map(&server.source, &server.name, "env");
    let raw_headers = || raw_map(&server.source, &server.name, "headers");
    let config = &server.config;
    let transport = if let Some(command) = &config.command {
        McpTransportConfig::Stdio {
            command: command.clone(),
            args: config.args.clone().unwrap_or_default(),
            env: raw_env().unwrap_or_else(|| {
                config
                    .env
                    .as_ref()
                    .map(|env| env.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                    .unwrap_or_default()
            }),
            cwd: config.cwd.clone(),
        }
    } else if config.kind.as_deref() == Some("sse") {
        McpTransportConfig::Sse {
            url: config.url.clone().unwrap_or_default(),
            headers: raw_headers().unwrap_or_else(|| {
                config
                    .headers
                    .as_ref()
                    .map(|h| h.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                    .unwrap_or_default()
            }),
        }
    } else {
        McpTransportConfig::Http {
            url: config.url.clone().unwrap_or_default(),
            headers: raw_headers().unwrap_or_else(|| {
                config
                    .headers
                    .as_ref()
                    .map(|h| h.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                    .unwrap_or_default()
            }),
        }
    };
    McpServerView {
        name: server.name.clone(),
        enabled: server.is_enabled(),
        // pi-rs 允许项目文件用 `enabled`/`exposure` 覆盖全局条目；此时条目仍
        // “定义”在全局，但生效状态由项目文件决定，必须按项目级只读处理。
        scope: if server.override_path.is_some() {
            McpScope::Project.as_str()
        } else {
            server.scope.as_str()
        },
        source: server
            .override_path
            .as_ref()
            .unwrap_or(&server.source)
            .to_string_lossy()
            .to_string(),
        transport,
        exposure: config.exposure(),
        exposure_explicit: config.exposure.is_some(),
    }
}

/// 从配置文件里取某个 server 的原始条目（用于编辑时保留未知字段）。
fn raw_entry(path: &Path, name: &str) -> Option<serde_json::Value> {
    let text = std::fs::read_to_string(path).ok()?;
    let root: serde_json::Value = serde_json::from_str(&text).ok()?;
    root.get("mcpServers")?.get(name).cloned()
}

/// 从配置文件里取某个 server 的原始字符串映射（`env` / `headers`）。
fn raw_map(source: &Path, name: &str, key: &str) -> Option<BTreeMap<String, String>> {
    let text = std::fs::read_to_string(source).ok()?;
    let root: serde_json::Value = serde_json::from_str(&text).ok()?;
    let map = root.get("mcpServers")?.get(name)?.get(key)?.as_object()?;
    Some(
        map.iter()
            .filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_string())))
            .collect(),
    )
}

// ============================================================================
// 一次性端点连接（CLI）
// ============================================================================

/// 远端 MCP 工具的精简视图（CLI 展示用）。
#[derive(Debug, Clone)]
pub struct McpToolInfo {
    pub name: String,
    pub description: String,
    /// 工具参数 JSON Schema。
    pub parameters: Option<serde_json::Value>,
}

/// 对单个 streamable-HTTP 端点的一次性连接。
///
/// CLI 的 `mcp list/status/call` 和 `tools doc` 只连一个自主管理的端点，
/// 不走 `mcp.json`；连接、握手、工具枚举与调用全部委托 pi-rs 的
/// [`McpServerConnection`]，这里只构造一个临时的 `LoadedServer`。
pub struct McpEndpoint {
    connection: McpServerConnection,
}

impl McpEndpoint {
    /// 连接一个 streamable-HTTP 端点（完成 initialize + tools/list）。
    ///
    /// # Errors
    /// 端点不可达或握手失败时返回 [`McpConfigError::Config`]。
    pub async fn connect_http(
        name: impl Into<String>,
        url: impl Into<String>,
    ) -> Result<Self, McpConfigError> {
        let entry = LoadedServer {
            name: name.into(),
            config: McpServerConfig {
                url: Some(url.into()),
                ..McpServerConfig::default()
            },
            source: PathBuf::new(),
            scope: McpScope::Global,
            override_path: None,
        };
        let cwd = std::env::current_dir()
            .map_err(|e| McpConfigError::Config(format!("无法获取当前工作目录: {e}")))?;
        let connection = McpServerConnection::with_agent_dir(
            entry,
            cwd.to_string_lossy().to_string(),
            pi_coding_agent::config::get_agent_dir()
                .to_string_lossy()
                .to_string(),
        );
        connection
            .get_client()
            .await
            .map_err(McpConfigError::Config)?;
        Ok(Self { connection })
    }

    /// 已枚举的工具。
    #[must_use]
    pub fn tools(&self) -> Vec<McpToolInfo> {
        self.connection
            .tools()
            .into_iter()
            .map(|tool| McpToolInfo {
                name: tool.name,
                description: tool.description.unwrap_or_default(),
                parameters: Some(serde_json::Value::Object(tool.input_schema)),
            })
            .collect()
    }

    /// 已枚举的工具数量。
    #[must_use]
    pub fn tool_count(&self) -> usize {
        self.connection.tools().len()
    }

    /// 调用一个工具，返回（内容块, 是否错误）。
    ///
    /// # Errors
    /// 连接失败或远端返回协议错误时返回 [`McpConfigError::Config`]。
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: serde_json::Map<String, serde_json::Value>,
    ) -> Result<(Vec<serde_json::Value>, bool), McpConfigError> {
        let result = self
            .connection
            .call_tool(name, arguments, None)
            .await
            .map_err(McpConfigError::Config)?;
        let content = result
            .content
            .iter()
            .filter_map(|block| serde_json::to_value(block).ok())
            .collect();
        Ok((content, result.is_error))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn store(dir: &tempfile::TempDir) -> McpConfigStore {
        McpConfigStore::new(
            dir.path().join("proj").to_string_lossy(),
            dir.path().join("agent").to_string_lossy(),
        )
    }

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn stdio_payload(name: &str) -> McpServerPayload {
        McpServerPayload {
            name: name.into(),
            enabled: true,
            transport: McpTransportConfig::Stdio {
                command: "npx".into(),
                args: vec!["-y".into(), "server".into()],
                env: BTreeMap::new(),
                cwd: None,
            },
            exposure: None,
        }
    }

    #[test]
    fn create_writes_a_stdio_entry_without_type() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);
        store.create(&stdio_payload("fs")).unwrap();

        let text = std::fs::read_to_string(store.global_path()).unwrap();
        let root: serde_json::Value = serde_json::from_str(&text).unwrap();
        let entry = &root["mcpServers"]["fs"];
        assert_eq!(entry["command"], "npx");
        assert!(entry.get("type").is_none(), "stdio 不能写 type: {entry}");
        assert!(entry.get("enabled").is_none(), "启用态不写字段");

        // 往返：pi-rs 解析回来应是同一个 stdio server。
        let (servers, errors) = store.servers();
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "fs");
        assert_eq!(servers[0].scope, "global");
        assert_eq!(servers[0].transport, stdio_payload("fs").transport);
    }

    #[test]
    fn create_writes_sse_with_type_and_http_without() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);
        store
            .create(&McpServerPayload {
                name: "legacy".into(),
                enabled: true,
                transport: McpTransportConfig::Sse {
                    url: "https://example.com/sse".into(),
                    headers: BTreeMap::new(),
                },
                exposure: None,
            })
            .unwrap();
        store
            .create(&McpServerPayload {
                name: "docs".into(),
                enabled: false,
                transport: McpTransportConfig::Http {
                    url: "https://example.com/mcp".into(),
                    headers: BTreeMap::from([("Authorization".into(), "Bearer t".into())]),
                },
                exposure: None,
            })
            .unwrap();

        let text = std::fs::read_to_string(store.global_path()).unwrap();
        let root: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(root["mcpServers"]["legacy"]["type"], "sse");
        assert!(root["mcpServers"]["docs"].get("type").is_none());
        assert_eq!(root["mcpServers"]["docs"]["enabled"], false);

        let (servers, errors) = store.servers();
        assert!(errors.is_empty(), "{errors:?}");
        // sse 未被误解析成 http。
        let legacy = servers.iter().find(|s| s.name == "legacy").unwrap();
        assert!(matches!(legacy.transport, McpTransportConfig::Sse { .. }));
        let docs = servers.iter().find(|s| s.name == "docs").unwrap();
        assert!(!docs.enabled);
        assert!(matches!(docs.transport, McpTransportConfig::Http { .. }));
    }

    /// 新条目默认 direct：pi-rs 的缺省 exposure=codemode 会让工具变成
    /// tool_search 才能看到的 deferred 工具，而本应用没有 codemode 脚本。
    #[test]
    fn create_defaults_exposure_to_direct() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);
        store.create(&stdio_payload("fs")).unwrap();

        let text = std::fs::read_to_string(store.global_path()).unwrap();
        let root: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(root["mcpServers"]["fs"]["exposure"], "direct");
    }

    /// 从管理界面保存时，pi-rs 新增/手写的字段不能被抹掉。
    #[test]
    fn update_preserves_fields_the_ui_does_not_edit() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);
        write(
            &store.global_path(),
            r#"{"mcpServers":{"docs":{
                 "url":"https://old.example/mcp",
                 "exposure":"deferred",
                 "timeout":12.5,
                 "toolExposure":{"search":"hidden"},
                 "oauth":{"clientId":"abc","scope":"read"}
               }}}"#,
        );

        store
            .update(
                "docs",
                &McpServerPayload {
                    name: "docs".into(),
                    enabled: true,
                    transport: McpTransportConfig::Http {
                        url: "https://new.example/mcp".into(),
                        headers: BTreeMap::new(),
                    },
                    exposure: None,
                },
            )
            .unwrap();

        let text = std::fs::read_to_string(store.global_path()).unwrap();
        let entry =
            &serde_json::from_str::<serde_json::Value>(&text).unwrap()["mcpServers"]["docs"];
        assert_eq!(entry["url"], "https://new.example/mcp");
        assert_eq!(
            entry["exposure"], "deferred",
            "用户设置的 exposure 必须保留"
        );
        assert_eq!(entry["timeout"], 12.5);
        assert_eq!(entry["toolExposure"]["search"], "hidden");
        assert_eq!(entry["oauth"]["clientId"], "abc");
        assert!(entry.get("command").is_none());
    }

    /// 视图必须如实反映 pi-rs 的生效值，并区分“显式设置”和“使用默认值”。
    #[test]
    fn view_reports_effective_exposure_and_explicitness() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);
        write(
            &store.global_path(),
            r#"{"mcpServers":{
                 "legacy":{"url":"https://a.example/mcp"},
                 "docs":{"url":"https://b.example/mcp","exposure":"deferred"}
               }}"#,
        );

        let (servers, errors) = store.servers();
        assert!(errors.is_empty(), "{errors:?}");
        let legacy = servers.iter().find(|s| s.name == "legacy").unwrap();
        assert_eq!(legacy.exposure, McpExposure::Codemode);
        assert!(!legacy.exposure_explicit);

        let docs = servers.iter().find(|s| s.name == "docs").unwrap();
        assert_eq!(docs.exposure, McpExposure::Deferred);
        assert!(docs.exposure_explicit);
    }

    #[test]
    fn update_sets_exposure_when_provided() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);
        store.create(&stdio_payload("fs")).unwrap();

        let mut payload = stdio_payload("fs");
        payload.exposure = Some(McpExposure::Hidden);
        store.update("fs", &payload).unwrap();

        let text = std::fs::read_to_string(store.global_path()).unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&text).unwrap()["mcpServers"]["fs"]
                ["exposure"],
            "hidden"
        );
        let view = store.servers().0.pop().unwrap();
        assert_eq!(view.exposure, McpExposure::Hidden);
        assert!(view.exposure_explicit);
    }

    #[test]
    fn project_entry_shadows_global_and_is_read_only() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);
        write(
            &store.global_path(),
            r#"{"mcpServers":{"dup":{"url":"https://global.example/mcp"}}}"#,
        );
        write(
            &store.project_path(),
            r#"{"mcpServers":{"dup":{"url":"https://project.example/mcp"}}}"#,
        );

        let (servers, errors) = store.servers();
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(servers.len(), 1, "同名只出现一次：{servers:?}");
        assert_eq!(servers[0].scope, "project");
        assert_eq!(
            servers[0].transport,
            McpTransportConfig::Http {
                url: "https://project.example/mcp".into(),
                headers: BTreeMap::new()
            }
        );

        // 写全局会被项目条目静默盖掉，故三个写操作都拒绝。
        assert!(matches!(
            store.update("dup", &stdio_payload("dup")),
            Err(McpConfigError::ProjectScoped(_))
        ));
        assert!(matches!(
            store.delete("dup"),
            Err(McpConfigError::ProjectScoped(_))
        ));
        assert!(matches!(
            store.toggle("dup"),
            Err(McpConfigError::ProjectScoped(_))
        ));
        assert!(matches!(
            store.create(&stdio_payload("dup")),
            Err(McpConfigError::Exists)
        ));
    }

    /// pi-rs 允许项目文件只写 `enabled`/`exposure` 覆盖全局条目。这种条目由项目
    /// 文件决定生效状态，管理界面必须按项目级只读展示，否则写全局会静默无效。
    #[test]
    fn project_override_of_global_entry_is_read_only() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);
        write(
            &store.global_path(),
            r#"{"mcpServers":{"docs":{"url":"https://global.example/mcp"}}}"#,
        );
        write(
            &store.project_path(),
            r#"{"mcpServers":{"docs":{"enabled":false}}}"#,
        );

        let (servers, errors) = store.servers();
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].scope, "project", "override 必须按项目级呈现");
        assert!(!servers[0].enabled);
        assert_eq!(
            servers[0].source,
            store.project_path().to_string_lossy(),
            "source 应指向真正决定生效状态的项目文件"
        );

        for result in [
            store.toggle("docs"),
            store.delete("docs"),
            store.update("docs", &stdio_payload("docs")),
        ] {
            assert!(
                matches!(result, Err(McpConfigError::ProjectScoped(_))),
                "override 条目不允许写全局: {result:?}"
            );
        }
    }

    #[test]
    fn rename_moves_the_entry() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);
        store.create(&stdio_payload("old")).unwrap();
        store.update("old", &stdio_payload("new")).unwrap();

        let (servers, errors) = store.servers();
        assert!(errors.is_empty(), "{errors:?}");
        let names: Vec<_> = servers.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["new"]);

        // 改名撞已有名 → Exists。
        store.create(&stdio_payload("other")).unwrap();
        assert!(matches!(
            store.update("new", &stdio_payload("other")),
            Err(McpConfigError::Exists)
        ));
    }

    #[test]
    fn toggle_and_delete_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);
        store.create(&stdio_payload("fs")).unwrap();

        store.toggle("fs").unwrap();
        assert!(!store.servers().0[0].enabled);
        store.toggle("fs").unwrap();
        assert!(store.servers().0[0].enabled);

        store.delete("fs").unwrap();
        assert!(store.servers().0.is_empty());
        assert!(matches!(store.delete("fs"), Err(McpConfigError::NotFound)));
        assert!(matches!(store.toggle("fs"), Err(McpConfigError::NotFound)));
        assert!(matches!(
            store.update("fs", &stdio_payload("fs")),
            Err(McpConfigError::NotFound)
        ));
    }

    #[test]
    fn missing_files_yield_no_servers_and_no_errors() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);
        let (servers, errors) = store.servers();
        assert!(servers.is_empty());
        assert!(errors.is_empty(), "{errors:?}");
    }

    #[test]
    fn invalid_entries_are_reported_and_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);
        write(
            &store.global_path(),
            r#"{"mcpServers":{"bad":{"type":"sse","command":"x"},"good":{"command":"a"}}}"#,
        );
        let (servers, errors) = store.servers();
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "good");
    }

    #[test]
    fn malformed_json_is_reported_without_panicking() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);
        write(&store.global_path(), "{ not json");
        let (servers, errors) = store.servers();
        assert!(servers.is_empty());
        assert_eq!(errors.len(), 1, "{errors:?}");
    }

    /// `${VAR}` 引用必须原样回显：pi-rs 在加载时已把它解析成实际值，若视图回显解析结果，
    /// 既会把密钥送到管理接口，也会让编辑保存把引用替换成明文写回文件。
    #[test]
    fn view_prefers_the_raw_reference_over_the_resolved_value() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.json");
        write(
            &path,
            r#"{"mcpServers":{
                 "docs":{"url":"https://e.com/mcp","headers":{"Authorization":"Bearer ${TOKEN}"}},
                 "fs":{"command":"npx","env":{"TOKEN":"${TOKEN}"}}
               }}"#,
        );

        // pi-rs 交给我们的 config 里是**解析后**的值。
        let resolved = [
            LoadedServer {
                name: "docs".into(),
                config: serde_json::from_value(serde_json::json!({
                    "url": "https://e.com/mcp",
                    "headers": {"Authorization": "Bearer s3cr3t"}
                }))
                .unwrap(),
                source: path.clone(),
                scope: McpScope::Global,
                override_path: None,
            },
            LoadedServer {
                name: "fs".into(),
                config: serde_json::from_value(serde_json::json!({
                    "command": "npx",
                    "env": {"TOKEN": "s3cr3t"}
                }))
                .unwrap(),
                source: path.clone(),
                scope: McpScope::Global,
                override_path: None,
            },
        ];

        for server in &resolved {
            let view = server_view(server);
            let serialized = serde_json::to_string(&view.transport).unwrap();
            assert!(
                serialized.contains("${TOKEN}"),
                "{}: 应回显引用原文: {serialized}",
                server.name
            );
            assert!(
                !serialized.contains("s3cr3t"),
                "{}: 解析后的密钥不得出现在视图里: {serialized}",
                server.name
            );
        }
    }

    /// 读不到原始条目（文件被删/换过）时退回解析值，至少不丢配置。
    #[test]
    fn view_falls_back_to_resolved_values_when_raw_is_unreadable() {
        let dir = tempfile::tempdir().unwrap();
        let server = LoadedServer {
            name: "docs".into(),
            config: serde_json::from_value(serde_json::json!({
                "url": "https://e.com/mcp",
                "headers": {"Authorization": "Bearer resolved"}
            }))
            .unwrap(),
            source: dir.path().join("gone.json"),
            scope: McpScope::Global,
            override_path: None,
        };
        let view = server_view(&server);
        let McpTransportConfig::Http { headers, .. } = &view.transport else {
            panic!("expected http: {view:?}");
        };
        assert_eq!(headers["Authorization"], "Bearer resolved");
    }

    #[test]
    fn payload_validation_rejects_empty_name_and_missing_endpoint() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);

        let mut empty_name = stdio_payload("");
        assert!(matches!(
            store.create(&empty_name),
            Err(McpConfigError::Invalid(_))
        ));
        empty_name.name = "  ".into();
        assert!(matches!(
            store.create(&empty_name),
            Err(McpConfigError::Invalid(_))
        ));

        let no_command = McpServerPayload {
            name: "x".into(),
            enabled: true,
            transport: McpTransportConfig::Stdio {
                command: "  ".into(),
                args: vec![],
                env: BTreeMap::new(),
                cwd: None,
            },
            exposure: None,
        };
        assert!(matches!(
            store.create(&no_command),
            Err(McpConfigError::Invalid(_))
        ));

        let no_url = McpServerPayload {
            name: "y".into(),
            enabled: true,
            transport: McpTransportConfig::Http {
                url: "".into(),
                headers: BTreeMap::new(),
            },
            exposure: None,
        };
        assert!(matches!(
            store.create(&no_url),
            Err(McpConfigError::Invalid(_))
        ));
    }

    #[test]
    fn create_failure_surfaces_as_config_error() {
        let dir = tempfile::tempdir().unwrap();
        // agent_dir 的父路径是个文件 → 建目录必然失败。
        let blocker = dir.path().join("blocker");
        std::fs::write(&blocker, "x").unwrap();
        let store = McpConfigStore::new(
            dir.path().to_string_lossy(),
            blocker.join("agent").to_string_lossy(),
        );
        assert!(matches!(
            store.create(&stdio_payload("fs")),
            Err(McpConfigError::Config(_))
        ));
    }
}
