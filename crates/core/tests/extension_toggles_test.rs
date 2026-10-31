//! 内置扩展开关（pi-rs `settings.json` 的 `extensionsEnabled`）集成测试。
//!
//! 走真实链路：`settings.json` → 会话工厂过滤工具 → 活跃会话 actor 立即
//! 应用过滤。
//!
//! 整个文件只有一个测试函数：`PI_CODING_AGENT_DIR` 是进程级环境变量，
//! 并行执行的不同测试会互相踩踏，串行执行才能保证隔离。

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::disallowed_methods)]

use openatrading_core::extensions::{
    load_extension_toggles, save_extension_toggles, BUILTIN_EXTENSIONS,
};
use openatrading_core::llm::{create_factory, FactoryOptions, SessionPool};

/// 内置扩展各自的关键工具名（pi-rs v1.87.10 移除了 goal；剩余扩展无斜杠命令）。
const SUBAGENT_TOOLS: [&str; 1] = ["subagent"];

fn has_any_tool(tools: &[String], candidates: &[&str]) -> bool {
    candidates
        .iter()
        .any(|tool| tools.iter().any(|name| name == tool))
}

/// 把 `toggles` 落盘到隔离出来的 `settings.json`。
fn persist(toggles: &openatrading_core::extensions::ExtensionToggles) {
    save_extension_toggles(toggles).expect("写入 settings.json 失败");
}

#[tokio::test]
async fn builtin_extension_toggles_filter_tools() {
    // ── 隔离环境：settings.json / sessions 都落在临时目录 ──
    let home = tempfile::tempdir().expect("创建临时目录失败");
    let agent_dir = home.path().join("agent");
    std::fs::create_dir_all(&agent_dir).expect("创建 agent 目录失败");
    let agent_dir = agent_dir.to_string_lossy().to_string();
    std::env::set_var("PI_CODING_AGENT_DIR", &agent_dir);
    let cwd = home.path().to_string_lossy().to_string();

    // ── Happy path：没有 settings.json 时内置扩展全部启用 ──
    let factory = create_factory(FactoryOptions::new(&agent_dir));
    let pool = SessionPool::new(factory);

    let session_id = pool.create_session(&cwd).await.expect("创建会话失败");
    let handle = pool.get_session(&session_id).await.expect("会话不在池中");
    let baseline = handle.active_tool_names().await.expect("读取工具列表失败");
    for ext in BUILTIN_EXTENSIONS {
        assert!(
            has_any_tool(&baseline, ext.tools),
            "默认应启用 {}，实际工具: {baseline:?}",
            ext.id
        );
    }

    // ── 停用 subagent：新会话不再获得 subagent 工具，其它扩展不受影响 ──
    let mut toggles = load_extension_toggles();
    assert!(toggles.set("subagent", false), "subagent 应可切换");
    persist(&toggles);

    let second_id = pool.create_session(&cwd).await.expect("创建会话失败");
    let second = pool.get_session(&second_id).await.expect("会话不在池中");
    let second_tools = second.active_tool_names().await.expect("读取工具列表失败");
    assert!(
        !has_any_tool(&second_tools, &SUBAGENT_TOOLS),
        "停用后不应再有 subagent 工具: {second_tools:?}"
    );
    assert!(
        has_any_tool(&second_tools, &["web_search"]),
        "停用 subagent 不应影响其它扩展: {second_tools:?}"
    );

    // ── 活跃会话立即跟随设置变化（无需重启进程） ──
    let disabled_tools = toggles.disabled_tools();
    let disabled_commands = toggles.disabled_commands();
    let updated = pool
        .apply_extension_toggles(&disabled_tools, &disabled_commands)
        .await;
    assert_eq!(updated, 2, "池中两个会话都应被更新");

    let live_tools = handle.active_tool_names().await.expect("读取工具列表失败");
    assert!(
        !has_any_tool(&live_tools, &SUBAGENT_TOOLS),
        "活跃会话应立即失去 subagent 工具: {live_tools:?}"
    );

    // pi-rs v1.87.10 的内置扩展不再注册斜杠命令，因此这里没有端到端的
    // 「停用命令被拒绝」用例；命令名解析与拒绝规则由
    // `agent::session::extensions` 的单元测试覆盖。

    // ── 重新启用：活跃会话恢复到创建时的工具基线 ──
    assert!(toggles.set("subagent", true), "subagent 应可切换");
    persist(&toggles);
    pool.apply_extension_toggles(&toggles.disabled_tools(), &toggles.disabled_commands())
        .await;

    let restored = handle.active_tool_names().await.expect("读取工具列表失败");
    assert_eq!(restored, baseline, "重新启用后应恢复创建时的工具基线");

    // ── 边界：`exclude_tools` 排除的工具不会被扩展开关“顺手”恢复 ──
    let mut toggles = load_extension_toggles();
    assert!(toggles.set("web_search", false));
    persist(&toggles);
    let readonly_id = pool
        .create_session_with_tools(&cwd, &["bash"])
        .await
        .expect("创建只读会话失败");
    let readonly = pool.get_session(&readonly_id).await.expect("会话不在池中");
    let readonly_baseline = readonly
        .active_tool_names()
        .await
        .expect("读取工具列表失败");
    assert!(
        !readonly_baseline.iter().any(|name| name == "bash"),
        "exclude_tools 应生效: {readonly_baseline:?}"
    );

    assert!(toggles.set("web_search", true));
    persist(&toggles);
    pool.apply_extension_toggles(&toggles.disabled_tools(), &toggles.disabled_commands())
        .await;

    let readonly_restored = readonly
        .active_tool_names()
        .await
        .expect("读取工具列表失败");
    assert!(
        !readonly_restored.iter().any(|name| name == "bash"),
        "重新启用扩展不能把 exclude_tools 排除的工具加回来: {readonly_restored:?}"
    );
    assert!(
        has_any_tool(&readonly_restored, &["web_search", "web_fetch"]),
        "重新启用 web_search 应恢复它的工具: {readonly_restored:?}"
    );
}
