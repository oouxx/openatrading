//! 首次运行初始化：创建数据目录与默认配置模板，并把品牌根绑定给 pi-rs。

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::atrading_home;

/// pi-rs agent 目录在品牌根下的子目录名（pi-rs 自己的约定，见其 `get_agent_dir`）。
///
/// 公开给 doctor 等需要断言"品牌根布局"的地方，避免同一个名字在多处硬编码。
pub const AGENT_DIR_NAME: &str = "agent";

/// 幂等初始化 `.atrading` 数据目录。
///
/// 以 `config.toml` 是否存在为准（而非目录是否存在）：`Gateway::init` 会先
/// 创建目录和 `data.db`，若按目录判断会导致 config.toml 永不生成。
/// 已存在时跳过，绝不覆盖用户配置。
///
/// 返回 `true` 表示本次创建了配置模板（首次运行）。
pub fn ensure_initialized() -> Result<bool> {
    ensure_initialized_at(&atrading_home())
}

fn ensure_initialized_at(home: &Path) -> Result<bool> {
    let config_path = home.join("config.toml");
    let created = if !config_path.exists() {
        std::fs::create_dir_all(home)?;
        std::fs::write(&config_path, include_str!("../../../config.example.toml"))?;
        true
    } else {
        false
    };
    // roles 目录始终确保存在（幂等）
    std::fs::create_dir_all(home.join("roles"))?;
    // agent 目录（pi-rs 的家）也在品牌根内，保持"一个品牌根"的布局。
    std::fs::create_dir_all(home.join(AGENT_DIR_NAME))?;
    Ok(created)
}

/// 把品牌根绑定为 pi-rs 的家，返回实际生效的 agent 目录。
///
/// **必须在进程入口、任何 pi-rs 调用之前调用**（理由见下），且只调用一次。
///
/// 为什么不"传参就够了"：pi-rs 内部有大量路径是直接读全局解析（`bash` 工具的
/// `{agent_dir}/bin` PATH、skills、prompt 模板、session 默认目录、扩展 runtime
/// handle 等），并不接受参数。只给 `CreateAgentSessionOptions.agent_dir` 传参，
/// 会让"我们传的 agent 目录"与"pi-rs 内部解析出的 agent 目录"分成两个地方——
/// 这正是要避免的双根漂移。绑一次 `PI_RS_HOME`，两条路径自动同源。
///
/// 覆盖优先级（尊重用户显式设置）：`PI_CODING_AGENT_DIR` > `PI_RS_HOME` > 本函数绑定。
/// 因此 `.env` / 环境变量里显式指定过 pi-rs 目录时，这里什么都不做。
///
/// 绑定只作用于本进程，不导出到父 shell：同一台机器上独立安装的 pi-rs 应用仍用
/// `~/.pi-rs`，两个应用各持一份 agent 目录（会话/登录/技能互不影响），这也是不让
/// openatrading 去动 `~/.pi-rs` 的原因。
///
/// 本函数会写进程环境变量：这是 pi-rs 提供的唯一重定位机制，且调用点在任何线程与
/// async runtime 起来之前（单线程阶段），不存在并发读写。
#[allow(clippy::disallowed_methods)] // 见上：唯一的重定位机制，且早于多线程
pub fn bind_pi_rs_home() -> PathBuf {
    let brand_root = atrading_home();
    let agent_dir = brand_root.join(AGENT_DIR_NAME);

    let already_set = ["PI_CODING_AGENT_DIR", "PI_RS_HOME"]
        .iter()
        .any(|k| std::env::var_os(k).is_some_and(|v| !v.is_empty()));

    // 注意：本函数在本进程的日志订阅者装好**之前**执行（入口最早处），因此只能用
    // eprintln —— log!/tracing 在这里会被丢弃。与 [env]/[session-factory] 同风格。
    if already_set {
        eprintln!(
            "[pi-rs] agent 目录（由环境变量指定）: {}",
            pi_coding_agent::config::get_agent_dir().display()
        );
    } else {
        std::env::set_var("PI_RS_HOME", &brand_root);
        eprintln!("[pi-rs] agent 目录: {}", agent_dir.display());
    }

    agent_dir
}

/// 当前生效的 pi-rs agent 目录（品牌根内嵌 `agent/`）。
///
/// 给 web/cli 打印"配置到底在哪"用——它们不便直接依赖 `pi-coding-agent`。
#[must_use]
pub fn agent_dir() -> PathBuf {
    atrading_home().join(AGENT_DIR_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_initialized_creates_config_once_and_is_idempotent() {
        let tmp = std::env::temp_dir().join(format!("atrading-init-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let home = tmp.join(".atrading");

        assert!(ensure_initialized_at(&home).unwrap_or_else(|e| panic!("init: {e}")));
        assert!(home.join("config.toml").exists(), "config.toml 应被创建");
        assert!(home.join("roles").is_dir(), "roles 目录应被创建");

        // 幂等：再次调用不覆盖已有配置
        let content = std::fs::read_to_string(home.join("config.toml"))
            .unwrap_or_else(|e| panic!("read: {e}"));
        assert!(
            !ensure_initialized_at(&home).unwrap_or_else(|e| panic!("init again: {e}")),
            "第二次调用不应再创建"
        );
        assert_eq!(
            std::fs::read_to_string(home.join("config.toml"))
                .unwrap_or_else(|e| panic!("read2: {e}")),
            content,
            "已有配置不应被覆盖"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
