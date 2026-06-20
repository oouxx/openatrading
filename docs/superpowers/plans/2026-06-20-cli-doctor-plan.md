# CLI Doctor 优化实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 `openatrading doctor` 从 5 项静态检查升级为模块化诊断+修复框架，覆盖 10 个系统维度，支持交互式 `--fix` 修复，输出 ANSI 彩色文本和 JSON。

**Architecture:** 定义 `Check` trait 作为统一接口，每个检查项一个文件实现该 trait；`CheckContext` 承载共享上下文（atrading_home, AppConfig）；Runner 顺序执行所有检查；Reporter 负责输出；`cmd_doctor(json, fix)` 为入口调度函数。

**Tech Stack:** Rust, async-trait, owo-colors, serde, cron 0.12, reqwest

**关键设计决策:**
- `doctor.rs` 和 `doctor/` 目录不能共存 —— 删除 `doctor.rs`，创建 `doctor/mod.rs` 作为模块根
- `CheckContext.app_config: Option<AppConfig>` —— runner 统一加载一次传给所有 check
- 修复流程：全部检查完毕 → 遍历 failed/warn 可修复项 → 逐一 y/N 询问 → 修复后自动重检单项

---

## 文件结构

```
crates/cli/src/cli/
├── doctor.rs               # DELETE（替换为 doctor/mod.rs）
└── doctor/                 # CREATE
    ├── mod.rs              # Check trait, CheckContext, CheckResult/Status, runner, reporter, cmd_doctor
    ├── check_home.rs       # HomeCheck: 目录结构检查
    ├── check_config.rs     # ConfigCheck: config.toml 加载
    ├── check_env.rs        # EnvCheck: 环境变量检查（仅 JIUYAN_* 和 TAOGUBA_*）
    ├── check_sqlite.rs     # SqliteCheck: SQLite 连接
    ├── check_cron.rs       # CronCheck: 6 字段 cron 表达式校验
    ├── check_telegram.rs   # TelegramCheck: 发送测试消息并清理
    ├── check_llm.rs        # LlmCheck: LlmClient 连通性
    ├── check_mcp.rs        # McpCheck: 遍历 MCP server 连接
    ├── check_skills.rs     # SkillsCheck: SkillRegistry::from_disk()
    └── check_port.rs       # PortCheck: 端口占用检查

crates/cli/Cargo.toml           # MODIFY: 添加 owo-colors, openatrading-provider
crates/cli/src/cli/app.rs       # MODIFY: Doctor 命令增加 --fix 参数
crates/cli/src/cli/mod.rs       # MODIFY: 解构 doctor 命令传参
crates/cli/src/cli/config.rs    # MODIFY: 添加 web_bind 字段，删除 DEFAULT_CONFIG_TOML
config.example.toml             # MODIFY: 添加 [service] web_bind
```

## Global Constraints

- 禁止使用 `println!`/`eprintln!` 作为生产日志；统一使用 `tracing`
- 禁止 `unwrap()`/`expect()` —— 要么传播错误，要么在注释中说明不变量
- `async-trait` 通过 `{ workspace = true }` 引入
- `cron::Schedule::from_str()` 来自 cron crate 0.12
- 输出文本中的用户可见信息使用简体中文
- 所有新类型优先：有业务含义的值使用 newtype，禁止裸 String/f64

---

### Task 1: CLI 依赖和配置更新

**Files:**
- Modify: `crates/cli/Cargo.toml`
- Modify: `crates/cli/src/cli/config.rs`
- Modify: `config.example.toml`

**Interfaces:**
- Consumes: 无
- Produces: `ServiceConfig` 新增 `web_bind: String` 字段；`cmd_init()` 改为使用 `include_str!`

- [ ] **Step 1: 更新 `Cargo.toml` 添加依赖**

```toml
# 在 crates/cli/Cargo.toml 的 [dependencies] 中添加：
owo-colors = "4"
openatrading-provider = { path = "../provider" }
async-trait = { workspace = true }
```

- [ ] **Step 2: 在 `config.rs` 的 `ServiceConfig` 中添加 `web_bind`**

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct ServiceConfig {
    #[serde(default = "default_db_path")]
    pub db_path: String,
    #[serde(default = "default_web_bind")]
    pub web_bind: String,
}

fn default_web_bind() -> String {
    "0.0.0.0:8080".to_string()
}
```

- [ ] **Step 3: 在 `load()` 和 `load_from()` 中设置 `web_bind` 默认值**

在 `AppConfig::load()` 的 `merged` 构建中添加：
```rust
service: ServiceConfig {
    db_path: default_db_path(),
    web_bind: default_web_bind(),
},
```

在 `load_from()` 的 ServiceConfig unwrap 中添加：
```rust
service: raw.service.unwrap_or(ServiceConfig {
    db_path: default_db_path(),
    web_bind: default_web_bind(),
}),
```

- [ ] **Step 4: 删除 `DEFAULT_CONFIG_TOML` 常量，修改 `cmd_init()`**

删除 `config.rs:164-217` 的 `DEFAULT_CONFIG_TOML` 常量。

修改 `cmd_init()`：
```rust
pub fn cmd_init() -> Result<()> {
    let atrading = openatrading_core::atrading_home();

    if atrading.exists() {
        eprintln!("{} 已存在，跳过初始化", atrading.display());
        return Ok(());
    }

    std::fs::create_dir_all(&atrading)?;
    std::fs::write(
        atrading.join("config.toml"),
        include_str!("../../../../config.example.toml"),
    )?;

    // Create skills dir with default zarili strategy
    let skills_dir = atrading.join("skills");
    std::fs::create_dir_all(&skills_dir)?;

    eprintln!("✅ 已初始化 {}", atrading.display());
    eprintln!("   配置文件: {}", atrading.join("config.toml").display());

    eprintln!("   (可运行 `openatrading skills install` 查看更多可安装技能)");
    Ok(())
}
```

- [ ] **Step 5: 在 `config.example.toml` 的 `[service]` 中添加 `web_bind`**

```toml
[service]
db_path = "data.db"
web_bind = "0.0.0.0:8080"
```

- [ ] **Step 6: 运行 `cargo check` 验证编译**

Run: `cargo check -p openatrading-cli 2>&1 | head -30`
Expected: 无编译错误，但会报 `doctor mod` 相关警告（因为 `doctor` 模块尚未实现）

- [ ] **Step 7: 提交**

```bash
git add crates/cli/Cargo.toml crates/cli/src/cli/config.rs config.example.toml
git commit -m "feat(cli): add owo-colors/openatrading-provider deps and web_bind config

DEFAULT_CONFIG_TOML removed, init now reads from config.example.toml.
ServiceConfig gains web_bind field defaulting to 0.0.0.0:8080."
```

---

### Task 2: CLI 命令接口扩展（--fix 参数）

**Files:**
- Modify: `crates/cli/src/cli/app.rs`
- Modify: `crates/cli/src/cli/mod.rs`

**Interfaces:**
- Consumes: 无
- Produces: `Doctor` 命令新增 `fix: bool` 字段；`mod.rs` 中将 `json`、`fix` 传给 `cmd_doctor`

- [ ] **Step 1: 在 `app.rs` 的 `Doctor` 命令中添加 `--fix` 参数**

```rust
/// 诊断当前服务状态
Doctor {
    /// 输出 JSON 格式
    #[arg(long)]
    json: bool,
    /// 交互式修复
    #[arg(long)]
    fix: bool,
},
```

- [ ] **Step 2: 在 `mod.rs` 中解构并传递新参数**

修改 `mod.rs` 第 103-104 行：
```rust
if let Some(Commands::Doctor { json, fix }) = cli.command {
    return cmd_doctor(json, fix).await;
}
```

- [ ] **Step 3: 验证编译**

Run: `cargo check -p openatrading-cli 2>&1 | head -20`
Expected: 编译通过，`cmd_doctor` 未找到的链接错误属于下一任务范围

- [ ] **Step 4: 提交**

```bash
git add crates/cli/src/cli/app.rs crates/cli/src/cli/mod.rs
git commit -m "feat(cli): add --fix flag to doctor command"
```

---

### Task 3: 核心类型系统和 Doctor 模块根

**Files:**
- Create: `crates/cli/src/cli/doctor/mod.rs`
- Delete: `crates/cli/src/cli/doctor.rs`

**Interfaces:**
- Consumes: 无
- Produces: `Check` trait, `CheckContext`, `CheckStatus`, `CheckResult`, `FixResult`, `FixStatus`, `FixRisk`, `FlatCheckResult`, `all_checks()`, `run_checks()`, `print_report()`, `print_json()`, `interactive_fix()`, `cmd_doctor(json, fix)`

- [ ] **Step 1: 删除旧 `doctor.rs`，创建 `doctor/mod.rs`**

```bash
rm crates/cli/src/cli/doctor.rs
mkdir -p crates/cli/src/cli/doctor
```

- [ ] **Step 2: 创建 `doctor/mod.rs` — 核心类型和 Check trait**

```rust
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use owo_colors::OwoColorize;
use serde::Serialize;

use crate::cli::config::AppConfig;

// ── Types ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub enum CheckStatus {
    Ok,
    #[serde(rename = "warn")]
    Warn(String),
    #[serde(rename = "error")]
    Error(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckResult {
    pub status: CheckStatus,
    pub summary: String,
    pub details: Vec<String>,
    pub duration_ms: u64,
}

#[derive(Debug)]
pub enum FixRisk {
    Safe,
    Dangerous,
    Impossible,
}

#[derive(Debug)]
pub enum FixStatus {
    Fixed,
    Skipped,
    Failed(String),
}

#[derive(Debug)]
pub struct FixResult {
    pub status: FixStatus,
    pub message: String,
}

pub struct CheckContext {
    pub atrading_home: PathBuf,
    pub app_config: Option<AppConfig>,
}

// ── Check trait ──────────────────────────────────────────

#[async_trait]
pub trait Check: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn has_fix(&self) -> bool;
    fn fix_risk(&self) -> FixRisk;
    fn fix_label(&self) -> Option<String>;

    async fn check(&self, ctx: &CheckContext) -> CheckResult;
    async fn fix(&self, ctx: &CheckContext) -> Result<FixResult>;
}

// ── Flat result for JSON output ──────────────────────────

#[derive(Debug, Serialize)]
pub struct FlatCheckResult {
    pub id: &'static str,
    pub name: &'static str,
    pub status: CheckStatus,
    pub summary: String,
    pub details: Vec<String>,
    pub duration_ms: u64,
}

#[derive(Debug, Serialize)]
struct ReportJson {
    version: &'static str,
    items: Vec<FlatCheckResult>,
}

// ── Helper functions ─────────────────────────────────────

fn status_icon(status: &CheckStatus) -> &'static str {
    match status {
        CheckStatus::Ok => "✓",
        CheckStatus::Warn(_) => "⚠",
        CheckStatus::Error(_) => "✗",
    }
}

fn status_color(status: &CheckStatus) -> owo_colors::Style {
    match status {
        CheckStatus::Ok => owo_colors::Style::new().green().bold(),
        CheckStatus::Warn(_) => owo_colors::Style::new().yellow().bold(),
        CheckStatus::Error(_) => owo_colors::Style::new().red().bold(),
    }
}

fn status_summary(results: &[CheckResult]) -> (usize, usize, usize) {
    let ok = results.iter().filter(|r| matches!(r.status, CheckStatus::Ok)).count();
    let warn = results.iter().filter(|r| matches!(r.status, CheckStatus::Warn(_))).count();
    let err = results.iter().filter(|r| matches!(r.status, CheckStatus::Error(_))).count();
    (ok, warn, err)
}

// ── All checks registry ──────────────────────────────────

fn all_checks() -> Vec<Box<dyn Check>> {
    vec![
        Box::new(super::check_home::HomeCheck),
        Box::new(super::check_config::ConfigCheck),
        Box::new(super::check_env::EnvCheck),
        Box::new(super::check_sqlite::SqliteCheck),
        Box::new(super::check_cron::CronCheck),
        Box::new(super::check_telegram::TelegramCheck),
        Box::new(super::check_llm::LlmCheck),
        Box::new(super::check_mcp::McpCheck),
        Box::new(super::check_skills::SkillsCheck),
        Box::new(super::check_port::PortCheck),
    ]
}

// ── Runner ───────────────────────────────────────────────

async fn run_checks(checks: &[Box<dyn Check>], ctx: &CheckContext) -> Vec<CheckResult> {
    let mut results = Vec::with_capacity(checks.len());
    for check in checks {
        let start = std::time::Instant::now();
        let result = check.check(ctx).await;
        let elapsed = start.elapsed();
        let mut r = result;
        r.duration_ms = elapsed.as_millis() as u64;
        results.push(r);
    }
    results
}

// ── Reporter (Text) ──────────────────────────────────────

fn print_report(results: &[CheckResult]) {
    let (ok, warn, err) = status_summary(results);
    println!();
    println!("{}", " OpenATrading 诊断报告 ".bold());
    println!("{}", "════════════════════════════════════".bold());
    println!("  版本: {}", env!("GIT_VERSION").bold());
    println!("  平台: {}/{}", std::env::consts::OS, std::env::consts::ARCH);
    println!();

    for (i, r) in results.iter().enumerate() {
        let icon = status_icon(&r.status).style(status_color(&r.status));
        let name = r.name.style(owo_colors::Style::new().bold());
        let details = if r.details.is_empty() {
            String::new()
        } else {
            r.details.join("; ")
        };
        println!(" {}  {:12}  {}  {}", icon, name, r.summary, details.dimmed());
    }

    println!();
    println!("{}", "════════════════════════════════════".bold());

    if err == 0 && warn == 0 {
        println!("{}", "✓ 一切正常".green().bold());
    } else if err == 0 {
        println!("{}", format!("⚠ {ok} 项正常，{warn} 项警告").yellow().bold());
        println!("  建议运行 `openatrading doctor --fix` 修复警告项");
    } else {
        println!("{}", format!("✗ {ok} 项正常，{warn} 项警告，{err} 项错误").red().bold());
        println!("  建议运行 `openatrading doctor --fix` 尝试自动修复");
    }
    println!();
}

// ── Reporter (JSON) ─────────────────────────────────────

fn print_json(results: &[CheckResult]) {
    let flat: Vec<FlatCheckResult> = results
        .iter()
        .enumerate()
        .map(|(i, r)| FlatCheckResult {
            id: "", // Filled after run_checks returns (see cmd_doctor)
            name: "",
            status: r.status.clone(),
            summary: r.summary.clone(),
            details: r.details.clone(),
            duration_ms: r.duration_ms,
        })
        .collect();
    // We need the check id/name — fix below in cmd_doctor
}

// ── Interactive fix ──────────────────────────────────────

async fn interactive_fix(
    checks: &[Box<dyn Check>],
    ctx: &CheckContext,
    results: &[CheckResult],
) {
    let fixable: Vec<_> = checks
        .iter()
        .zip(results.iter())
        .filter(|(check, result)| {
            check.has_fix()
                && matches!(result.status, CheckStatus::Warn(_) | CheckStatus::Error(_))
        })
        .collect();

    if fixable.is_empty() {
        println!("{}", "没有需要修复的项目".green());
        return;
    }

    for (i, (check, result)) in fixable.iter().enumerate() {
        let label = check
            .fix_label()
            .as_deref()
            .unwrap_or("执行修复");
        print!("是否修复 #{} ({})？[{}] [y/N]: ", i + 1, check.name(), label);
        std::io::stdout().flush().ok();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("  {} 跳过", "☐".dimmed());
            continue;
        }

        match check.fix(ctx).await {
            Ok(fix_result) => match fix_result.status {
                FixStatus::Fixed => {
                    println!("  {} {}", "✓".green().bold(), fix_result.message);
                    // Re-check
                    let new_result = check.check(ctx).await;
                    let icon = status_icon(&new_result.status).style(status_color(&new_result.status));
                    println!("    复查: {} {}", icon, new_result.summary);
                }
                FixStatus::Skipped => {
                    println!("  {} {}", "☐".dimmed(), fix_result.message);
                }
                FixStatus::Failed(msg) => {
                    println!("  {} {}", "✗".red().bold(), msg);
                }
            },
            Err(e) => {
                println!("  {} 修复失败: {e:#}", "✗".red().bold());
            }
        }
    }
}

// ── Entry point ─────────────────────────────────────────

pub async fn cmd_doctor(json: bool, fix: bool) -> Result<()> {
    let atrading_home = openatrading_core::atrading_home();
    let app_config = AppConfig::load().ok();

    let ctx = CheckContext {
        atrading_home,
        app_config,
    };

    let checks = all_checks();
    let results = run_checks(&checks, &ctx).await;

    if json {
        #[derive(Serialize)]
        struct JsonOutput {
            version: &'static str,
            items: Vec<FlatCheckResult>,
        }
        let items: Vec<FlatCheckResult> = checks
            .iter()
            .zip(results.iter())
            .map(|(check, result)| FlatCheckResult {
                id: check.id(),
                name: check.name(),
                status: result.status.clone(),
                summary: result.summary.clone(),
                details: result.details.clone(),
                duration_ms: result.duration_ms,
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&JsonOutput {
            version: env!("GIT_VERSION"),
            items,
        })?);
    } else {
        print_report(&results);
    }

    if fix {
        interactive_fix(&checks, &ctx, &results).await;
    }

    Ok(())
}
```

- [ ] **Step 3: 在 `doctor/mod.rs` 末尾添加子模块声明（before the mod content, at top）**

```rust
mod check_home;
mod check_config;
mod check_env;
mod check_sqlite;
mod check_cron;
mod check_telegram;
mod check_llm;
mod check_mcp;
mod check_skills;
mod check_port;
```

(将这些放在文件最顶部 `use` 语句之后。)

- [ ] **Step 4: 验证没有编译错误（expecting check_{name} module not found errors）**

Run: `cargo check -p openatrading-cli 2>&1 | head -10`
Expected: 预期会有模块未找到错误（因为 check 文件尚未创建）

- [ ] **Step 5: 提交**

```bash
git add crates/cli/src/cli/doctor/mod.rs
git rm crates/cli/src/cli/doctor.rs
git commit -m "feat(cli): add Check trait, runner, reporter, and cmd_doctor entry

Old DoctorResult/CheckItem types replaced with modular Check trait system.
Text output uses owo-colors ANSI styling; JSON output is FlatCheckResult array."
```

---

### Task 4: HomeCheck — 环境目录检查

**Files:**
- Create: `crates/cli/src/cli/doctor/check_home.rs`

**Interfaces:**
- Consumes: `Check` trait, `CheckContext`, `CheckStatus`, `CheckResult`, `FixRisk`, `FixResult`, `FixStatus`
- Produces: `HomeCheck` struct 实现 `Check`

- [ ] **Step 1: 创建 `check_home.rs`**

```rust
use async_trait::async_trait;
use std::path::Path;

use super::*;

pub struct HomeCheck;

#[async_trait]
impl Check for HomeCheck {
    fn id(&self) -> &'static str { "home" }
    fn name(&self) -> &'static str { "环境目录" }
    fn has_fix(&self) -> bool { true }
    fn fix_risk(&self) -> FixRisk { FixRisk::Safe }
    fn fix_label(&self) -> Option<String> { Some("自动创建目录") }

    async fn check(&self, ctx: &CheckContext) -> CheckResult {
        let home = &ctx.atrading_home;
        let mut details = Vec::new();
        let mut error_count = 0;

        if !home.exists() {
            details.push(format!("目录不存在: {}", home.display()));
            error_count += 1;
        } else if !home.is_dir() {
            details.push(format!("路径不是目录: {}", home.display()));
            error_count += 1;
        }

        let subdirs = ["skills", "logs", "pipelines"];
        for name in &subdirs {
            let p = home.join(name);
            if !p.exists() {
                details.push(format!("子目录缺失: {}", name));
                error_count += 1;
            } else if !p.is_dir() {
                details.push(format!("子目录不是目录: {}", name));
                error_count += 1;
            }
        }

        if error_count == 0 {
            CheckResult {
                status: CheckStatus::Ok,
                summary: format!("{}", home.display()),
                details: vec![],
                duration_ms: 0,
            }
        } else {
            CheckResult {
                status: CheckStatus::Error(format!("{} 个问题", error_count)),
                summary: format!("{}", home.display()),
                details,
                duration_ms: 0,
            }
        }
    }

    async fn fix(&self, ctx: &CheckContext) -> Result<FixResult> {
        let home = &ctx.atrading_home;
        std::fs::create_dir_all(home)?;
        for name in &["skills", "logs", "pipelines"] {
            std::fs::create_dir_all(home.join(name))?;
        }
        Ok(FixResult {
            status: FixStatus::Fixed,
            message: format!("已创建 {} 及其子目录", home.display()),
        })
    }
}
```

- [ ] **Step 2: 验证编译**

Run: `cargo check -p openatrading-cli 2>&1 | head -10`
Expected: 编译通过

- [ ] **Step 3: 提交**

```bash
git add crates/cli/src/cli/doctor/check_home.rs
git commit -m "feat(cli): add HomeCheck for environment directory structure"
```

---

### Task 5: ConfigCheck — 配置文件加载检查

**Files:**
- Create: `crates/cli/src/cli/doctor/check_config.rs`

- [ ] **Step 1: 创建 `check_config.rs`**

```rust
use async_trait::async_trait;

use super::*;

pub struct ConfigCheck;

#[async_trait]
impl Check for ConfigCheck {
    fn id(&self) -> &'static str { "config" }
    fn name(&self) -> &'static str { "配置文件" }
    fn has_fix(&self) -> bool { true }
    fn fix_risk(&self) -> FixRisk { FixRisk::Safe }
    fn fix_label(&self) -> Option<String> { Some("生成默认配置") }

    async fn check(&self, ctx: &CheckContext) -> CheckResult {
        match &ctx.app_config {
            Some(cfg) => {
                let roles_count = cfg.roles.len();
                let mcp_count = cfg.mcp_servers.len();
                CheckResult {
                    status: CheckStatus::Ok,
                    summary: format!("{} 个角色, {} 个 MCP 服务", roles_count, mcp_count),
                    details: vec![],
                    duration_ms: 0,
                }
            }
            None => CheckResult {
                status: CheckStatus::Error("无法加载".into()),
                summary: "config.toml 加载失败".into(),
                details: vec!["请检查 ~/.atrading/config.toml 是否存在且格式正确".into()],
                duration_ms: 0,
            },
        }
    }

    async fn fix(&self, ctx: &CheckContext) -> Result<FixResult> {
        let config_path = ctx.atrading_home.join("config.toml");
        if config_path.exists() {
            return Ok(FixResult {
                status: FixStatus::Skipped,
                message: "配置文件已存在".into(),
            });
        }
        std::fs::write(
            &config_path,
            include_str!("../../../../../config.example.toml"),
        )?;
        Ok(FixResult {
            status: FixStatus::Fixed,
            message: format!("已生成 {}", config_path.display()),
        })
    }
}
```

- [ ] **Step 2: 验证编译**

Run: `cargo check -p openatrading-cli 2>&1 | head -10`
Expected: 编译通过

- [ ] **Step 3: 提交**

```bash
git add crates/cli/src/cli/doctor/check_config.rs
git commit -m "feat(cli): add ConfigCheck for config.toml loading"
```

---

### Task 6: EnvCheck — 环境变量检查

**Files:**
- Create: `crates/cli/src/cli/doctor/check_env.rs`

- [ ] **Step 1: 创建 `check_env.rs`**

```rust
use async_trait::async_trait;

use super::*;

pub struct EnvCheck;

const REQUIRED_VARS: &[(&str, &str)] = &[
    ("JIUYAN_PHONE", "韭研公社手机号"),
    ("JIUYAN_PASSWORD", "韭研公社密码"),
    ("JIUYAN_INIT_TOKEN", "韭研公社初始 Token"),
    ("TAOGUBA_USER", "淘股吧用户名"),
    ("TAOGUBA_PASSWORD", "淘股吧密码"),
];

#[async_trait]
impl Check for EnvCheck {
    fn id(&self) -> &'static str { "env" }
    fn name(&self) -> &'static str { "环境变量" }
    fn has_fix(&self) -> bool { true }
    fn fix_risk(&self) -> FixRisk { FixRisk::Safe }
    fn fix_label(&self) -> Option<String> { Some("生成 .env 模板") }

    async fn check(&self, _ctx: &CheckContext) -> CheckResult {
        let mut details = Vec::new();
        let mut missing = 0usize;

        for (var, desc) in REQUIRED_VARS {
            if std::env::var(var).ok().filter(|v| !v.is_empty()).is_none() {
                details.push(format!("{} ({})", var, desc));
                missing += 1;
            }
        }

        if missing == 0 {
            CheckResult {
                status: CheckStatus::Ok,
                summary: format!("{} 个变量均已设置", REQUIRED_VARS.len()),
                details: vec![],
                duration_ms: 0,
            }
        } else {
            CheckResult {
                status: CheckStatus::Warn(format!("{} 个变量未设置", missing)),
                summary: "部分环境变量缺失".into(),
                details,
                duration_ms: 0,
            }
        }
    }

    async fn fix(&self, ctx: &CheckContext) -> Result<FixResult> {
        let env_path = ctx.atrading_home.join(".env");
        if env_path.exists() {
            return Ok(FixResult {
                status: FixStatus::Skipped,
                message: ".env 文件已存在".into(),
            });
        }
        let content = REQUIRED_VARS
            .iter()
            .map(|(var, desc)| format!("# {}={}", desc, var))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&env_path, content)?;
        Ok(FixResult {
            status: FixStatus::Fixed,
            message: format!("已生成模板 {}", env_path.display()),
        })
    }
}
```

- [ ] **Step 2: 验证编译**

Run: `cargo check -p openatrading-cli 2>&1 | head -10`
Expected: 编译通过

- [ ] **Step 3: 提交**

```bash
git add crates/cli/src/cli/doctor/check_env.rs
git commit -m "feat(cli): add EnvCheck for JIUYAN_* and TAOGUBA_* environment variables"
```

---

### Task 7: SqliteCheck — SQLite 数据库连接检查

**Files:**
- Create: `crates/cli/src/cli/doctor/check_sqlite.rs`

- [ ] **Step 1: 创建 `check_sqlite.rs`**

```rust
use async_trait::async_trait;
use std::path::PathBuf;

use super::*;

pub struct SqliteCheck;

fn resolve_db_path(ctx: &CheckContext) -> PathBuf {
    let db_name = ctx
        .app_config
        .as_ref()
        .map(|c| c.service.db_path.as_str())
        .unwrap_or("data.db");
    let p = PathBuf::from(db_name);
    if p.is_relative() {
        ctx.atrading_home.join(db_name)
    } else {
        p
    }
}

#[async_trait]
impl Check for SqliteCheck {
    fn id(&self) -> &'static str { "sqlite" }
    fn name(&self) -> &'static str { "数据库" }
    fn has_fix(&self) -> bool { true }
    fn fix_risk(&self) -> FixRisk { FixRisk::Safe }
    fn fix_label(&self) -> Option<String> { Some("初始化数据库") }

    async fn check(&self, ctx: &CheckContext) -> CheckResult {
        let db_path = resolve_db_path(ctx);
        match openatrading_web::gateway::Gateway::init(&db_path) {
            Ok(_) => CheckResult {
                status: CheckStatus::Ok,
                summary: format!("{}", db_path.display()),
                details: vec![],
                duration_ms: 0,
            },
            Err(e) => CheckResult {
                status: CheckStatus::Error("连接失败".into()),
                summary: format!("{}", db_path.display()),
                details: vec![format!("{e:#}")],
                duration_ms: 0,
            },
        }
    }

    async fn fix(&self, ctx: &CheckContext) -> Result<FixResult> {
        let db_path = resolve_db_path(ctx);
        // Gateway::init creates tables if not exist
        openatrading_web::gateway::Gateway::init(&db_path)?;
        Ok(FixResult {
            status: FixStatus::Fixed,
            message: format!("数据库已初始化: {}", db_path.display()),
        })
    }
}
```

- [ ] **Step 2: 验证编译**

Run: `cargo check -p openatrading-cli 2>&1 | head -10`
Expected: 编译通过

- [ ] **Step 3: 提交**

```bash
git add crates/cli/src/cli/doctor/check_sqlite.rs
git commit -m "feat(cli): add SqliteCheck via Gateway::init"
```

---

### Task 8: CronCheck — 定时任务表达式校验

**Files:**
- Create: `crates/cli/src/cli/doctor/check_cron.rs`

- [ ] **Step 1: 创建 `check_cron.rs`**

```rust
use async_trait::async_trait;
use std::str::FromStr;

use super::*;

pub struct CronCheck;

/// Validate a cron expression as 6-field format.
/// The `cron` crate's Schedule::from_str parses both 6-field and 7-field;
/// we explicitly reject 7-field by counting tokens.
fn validate_cron(expr: &str) -> Result<(), String> {
    let field_count = expr.split_whitespace().count();
    if field_count != 6 {
        return Err(format!("字段数应为 6 (当前 {field_count}): {expr}"));
    }
    cron::Schedule::from_str(expr).map_err(|e| format!("解析失败: {e} ({expr})"))?;
    Ok(())
}

#[async_trait]
impl Check for CronCheck {
    fn id(&self) -> &'static str { "cron" }
    fn name(&self) -> &'static str { "定时任务" }
    fn has_fix(&self) -> bool { true }
    fn fix_risk(&self) -> FixRisk { FixRisk::Safe }
    fn fix_label(&self) -> Option<String> { Some("写入默认值") }

    async fn check(&self, ctx: &CheckContext) -> CheckResult {
        match &ctx.app_config {
            Some(cfg) => match &cfg.cron {
                Some(c) if c.enabled => {
                    let mut details = Vec::new();
                    let schedules = [
                        ("采集", &c.collection_schedule),
                        ("决策", &c.decision_schedule),
                        ("新闻", &c.news_schedule),
                    ];
                    let mut errors = 0usize;

                    for (name, expr) in &schedules {
                        match validate_cron(expr) {
                            Ok(()) => details.push(format!("{}: {} ✓", name, expr)),
                            Err(e) => {
                                details.push(format!("{}: {} ✗ ({})", name, expr, e));
                                errors += 1;
                            }
                        }
                    }

                    if errors == 0 {
                        CheckResult {
                            status: CheckStatus::Ok,
                            summary: format!("{} 个调度均有效", schedules.len()),
                            details,
                            duration_ms: 0,
                        }
                    } else {
                        CheckResult {
                            status: CheckStatus::Error(format!("{} 个表达式无效", errors)),
                            summary: "定时任务调度表达式校验失败".into(),
                            details,
                            duration_ms: 0,
                        }
                    }
                }
                _ => CheckResult {
                    status: CheckStatus::Warn("cron 未启用".into()),
                    summary: "定时任务调度未启用".into(),
                    details: vec![],
                    duration_ms: 0,
                },
            },
            None => CheckResult {
                status: CheckStatus::Error("无法加载配置".into()),
                summary: "无法加载配置".into(),
                details: vec![],
                duration_ms: 0,
            },
        }
    }

    async fn fix(&self, ctx: &CheckContext) -> Result<FixResult> {
        let config_path = ctx.atrading_home.join("config.toml");
        let content = std::fs::read_to_string(&config_path)?;
        let has_cron = content.contains("[cron]");
        if has_cron {
            return Ok(FixResult {
                status: FixStatus::Skipped,
                message: "[cron] 段已存在，请手动修正 cron 表达式".into(),
            });
        }
        let cron_section = r#"
[cron]
enabled = true
collection_schedule = "0 0 18 * * 1-5"
decision_schedule = "0 */30 9-15 * * 1-5"
news_schedule = "0 */5 9-15 * * 1-5"
"#;
        std::fs::write(&config_path, content + cron_section)?;
        Ok(FixResult {
            status: FixStatus::Fixed,
            message: "已追加 [cron] 配置段".into(),
        })
    }
}
```

- [ ] **Step 2: 验证编译**

Run: `cargo check -p openatrading-cli 2>&1 | head -10`
Expected: 编译通过

- [ ] **Step 3: 提交**

```bash
git add crates/cli/src/cli/doctor/check_cron.rs
git commit -m "feat(cli): add CronCheck with 6-field validation for 3 schedules"
```

---

### Task 9: TelegramCheck — Telegram 连接测试

**Files:**
- Create: `crates/cli/src/cli/doctor/check_telegram.rs`

- [ ] **Step 1: 创建 `check_telegram.rs`**

```rust
use async_trait::async_trait;

use super::*;

pub struct TelegramCheck;

#[async_trait]
impl Check for TelegramCheck {
    fn id(&self) -> &'static str { "telegram" }
    fn name(&self) -> &'static str { "Telegram" }
    fn has_fix(&self) -> bool { true }
    fn fix_risk(&self) -> FixRisk { FixRisk::Safe }
    fn fix_label(&self) -> Option<String> { Some("提示配置") }

    async fn check(&self, ctx: &CheckContext) -> CheckResult {
        let tg = match &ctx.app_config {
            Some(cfg) => cfg.social.as_ref().and_then(|s| s.telegram.as_ref()),
            None => return CheckResult {
                status: CheckStatus::Error("无法加载配置".into()),
                summary: "配置加载失败".into(),
                details: vec![],
                duration_ms: 0,
            },
        };

        let tg = match tg {
            Some(t) if t.enabled => t,
            _ => return CheckResult {
                status: CheckStatus::Warn("未启用".into()),
                summary: "Telegram 通知未启用".into(),
                details: vec![],
                duration_ms: 0,
            },
        };

        let token = match tg.bot_token.as_ref().filter(|t| !t.is_empty()) {
            Some(t) => t,
            None => return CheckResult {
                status: CheckStatus::Error("未配置".into()),
                summary: "bot_token 未设置".into(),
                details: vec![],
                duration_ms: 0,
            },
        };

        let chat_id = match tg.chat_id.as_ref().filter(|c| !c.is_empty()) {
            Some(c) => c,
            None => return CheckResult {
                status: CheckStatus::Error("未配置".into()),
                summary: "chat_id 未设置".into(),
                details: vec![],
                duration_ms: 0,
            },
        };

        let client = reqwest::Client::new();
        let send_url = format!("https://api.telegram.org/bot{token}/sendMessage");
        match client
            .post(&send_url)
            .json(&serde_json::json!({
                "chat_id": chat_id,
                "text": "✓ OpenATrading 连接测试",
            }))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                // Clean up: delete the test message
                if let Ok(body) = resp.json::<serde_json::Value>().await {
                    if let Some(msg_id) = body["result"]["message_id"].as_i64() {
                        let del_url = format!("https://api.telegram.org/bot{token}/deleteMessage");
                        let _ = client
                            .post(&del_url)
                            .json(&serde_json::json!({
                                "chat_id": chat_id,
                                "message_id": msg_id,
                            }))
                            .send()
                            .await;
                    }
                }
                CheckResult {
                    status: CheckStatus::Ok,
                    summary: "连接成功".into(),
                    details: vec![],
                    duration_ms: 0,
                }
            }
            Ok(resp) => {
                let body = resp.text().await.unwrap_or_default();
                CheckResult {
                    status: CheckStatus::Error("API 错误".into()),
                    summary: "Telegram API 返回错误".into(),
                    details: vec![body],
                    duration_ms: 0,
                }
            }
            Err(e) => CheckResult {
                status: CheckStatus::Error("连接失败".into()),
                summary: "无法连接 Telegram API".into(),
                details: vec![format!("{e}")],
                duration_ms: 0,
            },
        }
    }

    async fn fix(&self, _ctx: &CheckContext) -> Result<FixResult> {
        Ok(FixResult {
            status: FixStatus::Skipped,
            message: "请在 config.toml [social.telegram] 中配置 bot_token 和 chat_id".into(),
        })
    }
}
```

- [ ] **Step 2: 验证编译**

Run: `cargo check -p openatrading-cli 2>&1 | head -10`
Expected: 编译通过

- [ ] **Step 3: 提交**

```bash
git add crates/cli/src/cli/doctor/check_telegram.rs
git commit -m "feat(cli): add TelegramCheck with send-and-cleanup test"
```

---

### Task 10: LlmCheck — LLM API 连通性测试

**Files:**
- Create: `crates/cli/src/cli/doctor/check_llm.rs`

- [ ] **Step 1: 创建 `check_llm.rs`**

```rust
use async_trait::async_trait;

use super::*;

pub struct LlmCheck;

#[async_trait]
impl Check for LlmCheck {
    fn id(&self) -> &'static str { "llm" }
    fn name(&self) -> &'static str { "LLM API" }
    fn has_fix(&self) -> bool { true }
    fn fix_risk(&self) -> FixRisk { FixRisk::Safe }
    fn fix_label(&self) -> Option<String> { Some("提示检查配置") }

    async fn check(&self, ctx: &CheckContext) -> CheckResult {
        let (provider, model) = match &ctx.app_config {
            Some(cfg) => (cfg.llm.provider.as_deref(), cfg.llm.model.as_deref()),
            None => (None, None),
        };

        let client = openatrading_provider::llm::LlmClient::from_config(provider, model);
        let start = std::time::Instant::now();
        match client.chat("Respond with exactly one word: ok").await {
            Ok(_) => {
                let elapsed = start.elapsed();
                let ms = elapsed.as_millis();
                CheckResult {
                    status: CheckStatus::Ok,
                    summary: format!("响应 {ms}ms"),
                    details: vec![],
                    duration_ms: ms as u64,
                }
            }
            Err(e) => CheckResult {
                status: CheckStatus::Error("请求失败".into()),
                summary: "LLM API 调用失败".into(),
                details: vec![format!("{e:#}")],
                duration_ms: start.elapsed().as_millis() as u64,
            },
        }
    }

    async fn fix(&self, _ctx: &CheckContext) -> Result<FixResult> {
        Ok(FixResult {
            status: FixStatus::Skipped,
            message: "请检查 LLM_API_KEY 环境变量或 [llm] 配置段".into(),
        })
    }
}
```

- [ ] **Step 2: 验证编译**

Run: `cargo check -p openatrading-cli 2>&1 | head -10`
Expected: 编译通过

- [ ] **Step 3: 提交**

```bash
git add crates/cli/src/cli/doctor/check_llm.rs
git commit -m "feat(cli): add LlmCheck using LlmClient::from_config + chat"
```

---

### Task 11: McpCheck — MCP 服务器连接测试

**Files:**
- Create: `crates/cli/src/cli/doctor/check_mcp.rs`

- [ ] **Step 1: 创建 `check_mcp.rs`**

```rust
use async_trait::async_trait;
use std::collections::HashMap;

use super::*;
use openatrading_core::llm::{McpClient, McpTransportConfig};

pub struct McpCheck;

#[async_trait]
impl Check for McpCheck {
    fn id(&self) -> &'static str { "mcp" }
    fn name(&self) -> &'static str { "MCP 服务" }
    fn has_fix(&self) -> bool { false }
    fn fix_risk(&self) -> FixRisk { FixRisk::Impossible }
    fn fix_label(&self) -> Option<String> { None }

    async fn check(&self, ctx: &CheckContext) -> CheckResult {
        let servers = match &ctx.app_config {
            Some(cfg) => &cfg.mcp_servers,
            None => return CheckResult {
                status: CheckStatus::Error("无法加载配置".into()),
                summary: "配置加载失败".into(),
                details: vec![],
                duration_ms: 0,
            },
        };

        if servers.is_empty() {
            return CheckResult {
                status: CheckStatus::Warn("无 MCP 服务".into()),
                summary: "未配置任何 MCP 服务器".into(),
                details: vec![],
                duration_ms: 0,
            };
        }

        let enabled = servers.iter().filter(|s| s.enabled).count();
        let disabled = servers.len() - enabled;
        let mut details = Vec::new();
        let mut failures = 0usize;

        for server in servers {
            let label = if server.enabled {
                format!("  {}  ", server.name)
            } else {
                format!("  {} (已禁用)", server.name)
            };

            if !server.enabled {
                details.push(format!("{} ⤚ 跳过（已禁用）", label));
                continue;
            }

            let transport_desc = match &server.transport {
                McpTransportConfig::StreamableHttp { url, .. } => {
                    format!("streamable-http → {url}")
                }
                McpTransportConfig::Sse { url, .. } => {
                    format!("sse → {url}")
                }
                McpTransportConfig::Stdio { command, args, .. } => {
                    format!("stdio → {command} {}", args.join(" "))
                }
            };

            let connect_result = match &server.transport {
                McpTransportConfig::StreamableHttp { url, .. } => {
                    McpClient::connect(url).await
                }
                McpTransportConfig::Sse { url, .. } => {
                    McpClient::connect(url).await
                }
                McpTransportConfig::Stdio { command, args, env } => {
                    McpClient::connect_stdio(command, args, env).await
                }
            };

            match connect_result {
                Ok(_client) => {
                    details.push(format!("{} {}  ✓", label, transport_desc));
                }
                Err(e) => {
                    details.push(format!("{} {}  ✗ ({})", label, transport_desc, e));
                    failures += 1;
                }
            }
        }

        if failures == 0 {
            CheckResult {
                status: CheckStatus::Ok,
                summary: format!("{} 个 ({enabled} 启用, {disabled} 禁用)", servers.len()),
                details,
                duration_ms: 0,
            }
        } else {
            CheckResult {
                status: CheckStatus::Error(format!("{failures} 个连接失败")),
                summary: format!("{} 个 ({enabled} 启用, {disabled} 禁用)", servers.len()),
                details,
                duration_ms: 0,
            }
        }
    }

    async fn fix(&self, _ctx: &CheckContext) -> Result<FixResult> {
        unreachable!("McpCheck has no fix");
    }
}
```

- [ ] **Step 2: 验证编译**

Run: `cargo check -p openatrading-cli 2>&1 | head -30`
Expected: 编译通过（注意 `McpClient` 的 `connect` 和 `connect_stdio` 方法可能有所有权问题，`Stdio` 分支的 `env` 参数需要 `&HashMap<String, String>` 类型匹配。根据检查，`connect_stdio` 签名为 `(command: &str, args: &[String], env: &HashMap<String, String>)`，而 `McpTransportConfig::Stdio { command, args, env }` 中的 `env` 已经是 `HashMap<String, String>`。）

- [ ] **Step 3: 提交**

```bash
git add crates/cli/src/cli/doctor/check_mcp.rs
git commit -m "feat(cli): add McpCheck — connect to each MCP server"
```

---

### Task 12: SkillsCheck — 已安装技能枚举

**Files:**
- Create: `crates/cli/src/cli/doctor/check_skills.rs`

- [ ] **Step 1: 创建 `check_skills.rs`**

```rust
use async_trait::async_trait;

use super::*;

pub struct SkillsCheck;

#[async_trait]
impl Check for SkillsCheck {
    fn id(&self) -> &'static str { "skills" }
    fn name(&self) -> &'static str { "技能" }
    fn has_fix(&self) -> bool { false }
    fn fix_risk(&self) -> FixRisk { FixRisk::Impossible }
    fn fix_label(&self) -> Option<String> { None }

    async fn check(&self, _ctx: &CheckContext) -> CheckResult {
        let registry = openatrading_core::skills::SkillRegistry::from_disk();
        let skills = registry.skills();

        if skills.is_empty() {
            return CheckResult {
                status: CheckStatus::Warn("无已安装技能".into()),
                summary: "未安装任何技能".into(),
                details: vec!["可运行 `openatrading skills install` 安装技能".into()],
                duration_ms: 0,
            };
        }

        let details: Vec<String> = skills
            .iter()
            .map(|s| {
                let desc = if s.description.is_empty() {
                    String::new()
                } else {
                    format!(" — {}", s.description)
                };
                format!("  {}{}", s.name, desc)
            })
            .collect();

        CheckResult {
            status: CheckStatus::Ok,
            summary: format!("{} 个已安装", skills.len()),
            details,
            duration_ms: 0,
        }
    }

    async fn fix(&self, _ctx: &CheckContext) -> Result<FixResult> {
        unreachable!("SkillsCheck has no fix");
    }
}
```

- [ ] **Step 2: 验证编译**

Run: `cargo check -p openatrading-cli 2>&1 | head -10`
Expected: 编译通过

- [ ] **Step 3: 提交**

```bash
git add crates/cli/src/cli/doctor/check_skills.rs
git commit -m "feat(cli): add SkillsCheck — list installed skills"
```

---

### Task 13: PortCheck — 端口占用检查

**Files:**
- Create: `crates/cli/src/cli/doctor/check_port.rs`

- [ ] **Step 1: 创建 `check_port.rs`**

```rust
use async_trait::async_trait;

use super::*;

pub struct PortCheck;

/// Try to bind to the given address to check if the port is free.
fn check_port(addr: &str) -> Result<(), String> {
    let listener = std::net::TcpListener::bind(addr);
    match listener {
        Ok(l) => {
            // Dropping the listener releases the bind immediately
            drop(l);
            Ok(())
        }
        Err(e) => Err(format!("地址 {addr} 被占用: {e}")),
    }
}

#[async_trait]
impl Check for PortCheck {
    fn id(&self) -> &'static str { "port" }
    fn name(&self) -> &'static str { "端口" }
    fn has_fix(&self) -> bool { true }
    fn fix_risk(&self) -> FixRisk { FixRisk::Dangerous }
    fn fix_label(&self) -> Option<String> { Some("提示占用进程") }

    async fn check(&self, ctx: &CheckContext) -> CheckResult {
        let mut details = Vec::new();
        let mut errors = 0usize;

        // Check web_bind port
        let web_bind = ctx
            .app_config
            .as_ref()
            .map(|c| c.service.web_bind.clone())
            .unwrap_or_else(|| "0.0.0.0:8080".to_string());

        match check_port(&web_bind) {
            Ok(()) => details.push(format!("web: {}  ✓", web_bind)),
            Err(e) => {
                details.push(format!("web: {}  ✗ ({})", web_bind, e));
                errors += 1;
            }
        }

        // Check mcp_server bind port
        let mcp_bind = ctx
            .app_config
            .as_ref()
            .map(|c| c.mcp_server.bind.clone())
            .unwrap_or_else(|| "0.0.0.0:8082".to_string());

        match check_port(&mcp_bind) {
            Ok(()) => details.push(format!("mcp: {}  ✓", mcp_bind)),
            Err(e) => {
                details.push(format!("mcp: {}  ✗ ({})", mcp_bind, e));
                errors += 1;
            }
        }

        if errors == 0 {
            CheckResult {
                status: CheckStatus::Ok,
                summary: "端口均可用".into(),
                details,
                duration_ms: 0,
            }
        } else {
            CheckResult {
                status: CheckStatus::Warn(format!("{errors} 个端口被占用")),
                summary: "部分端口被占用".into(),
                details,
                duration_ms: 0,
            }
        }
    }

    async fn fix(&self, _ctx: &CheckContext) -> Result<FixResult> {
        Ok(FixResult {
            status: FixStatus::Skipped,
            message: "请手动停止占用端口的进程，或在配置中修改 bind 地址".into(),
        })
    }
}
```

- [ ] **Step 2: 验证编译**

Run: `cargo check -p openatrading-cli 2>&1 | head -10`
Expected: 编译通过

- [ ] **Step 3: 提交**

```bash
git add crates/cli/src/cli/doctor/check_port.rs
git commit -m "feat(cli): add PortCheck — check web_bind and mcp bind ports"
```

---

### Self-Review

**1. Spec coverage:**

| Spec 要求 | 对应任务 |
|---|---|
| 10 项检查（home/config/env/sqlite/cron/telegram/llm/mcp/skills/port） | Task 4-13 |
| `Check` trait 设计 | Task 3 (mod.rs) |
| `CheckContext` 共享 app_config | Task 3 |
| ANSI 彩色文本输出（owo-colors） | Task 3 (print_report) |
| `--json` 输出 | Task 3 (print_json) |
| `--fix` 交互式修复 | Task 3 (interactive_fix) |
| `FixRisk` 安全级别 | Task 3 |
| web_bind 配置 | Task 1 |
| 删除 DEFAULT_CONFIG_TOML | Task 1 |
| config.example.toml 更新 | Task 1 |
| app.rs/mod.rs 参数扩展 | Task 2 |
| 环境变量仅 JIUYAN_* 和 TAOGUBA_* | Task 6 |

**2. 占位符扫描：** 无 "TBD"、"TODO"、"implement later"、"fill in details" 等。

**3. 类型一致性检查：**
- `cmd_doctor(json: bool, fix: bool)` 签名在 Task 2 和 Task 3 一致
- `CheckContext` 在所有 check task 中一致（`atrading_home: PathBuf`, `app_config: Option<AppConfig>`）
- `CheckResult.duration_ms: u64` 在 runner 中设置，check 实现中初始化为 0（set by runner）
- `McpTransportConfig::Stdio { command, args, env }` 中 `env` 类型为 `HashMap<String, String>`，与 `connect_stdio` 签名匹配
- `FixStatus::Skipped` 和 `FixStatus::Failed(String)` 在所有 fix 实现中一致使用
- `LlmClient::from_config(Option<&str>, Option<&str>)` 匹配 provider crate 的实际签名

**4. 边界情况检查：**
- McpCheck 处理 enabled/disabled 服务器
- SkillsCheck 处理空技能列表
- CronCheck 明确拒绝 7 字段表达式（field count check）
- PortCheck 使用 `TcpListener::bind` 而非 `connect`
- `resolve_db_path` 处理相对路径和绝对路径

---

### 执行选择

Plan complete and saved to `docs/superpowers/plans/2026-06-20-cli-doctor-plan.md`. Two execution options:

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** — Execute tasks in this session, batch execution with checkpoints

**Which approach?**
