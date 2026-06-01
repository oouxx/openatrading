# Role Module Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 重构 core role 模块，从硬编码枚举改为 config.toml 驱动，角色可扩展，策略独立管理。

**Architecture:** 抽取角色策略为 markdown 文件（核心内置策略嵌入二进制，用户策略从 `.atrading/` 读取）；CLI 负责三层配置合并，core 只保留纯数据结构 + prompt 构建。Prompt 统一为 system=策略、user=市场数据+指令。

**Tech Stack:** Rust (tokio, serde, toml, clap, dirs)

---

### Task 1: 创建内置策略 base.md 文件

**Files:**
- Create: `crates/core/strategies/zarili/base.md`
- Create: `crates/core/strategies/trader/base.md`
- Create: `crates/core/strategies/quant/base.md`
- Create: `crates/core/strategies/risk_officer/base.md`

- [ ] **Step 1: 创建 trader/base.md**

从 `crates/core/src/roles/analyst.rs` Role::Trader 的 persona() 抽取为纯 markdown：

Write `crates/core/strategies/trader/base.md`:
```markdown
你是一名专注A股短线的游资操盘手，有15年打板经验。
擅长从涨停情绪、连板高度、封板资金研判龙头股。
语言直接，像老手盘中复盘，不废话。
```

- [ ] **Step 2: 创建 quant/base.md**

Write `crates/core/strategies/quant/base.md`:
```markdown
你是量化分析师，只相信数据和统计规律。
关注涨跌比、换手率分布、行业集中度、情绪指标。
用数字说话，给出客观判断。
```

- [ ] **Step 3: 创建 risk_officer/base.md**

Write `crates/core/strategies/risk_officer/base.md`:
```markdown
你是风险控制官，专注识别市场风险和泡沫信号。
偏保守，提醒潜在风险。关注炸板率、跌停数量、尾盘异动。
不鼓励追高，提醒止损。
```

- [ ] **Step 4: 创建 zarili/base.md**

从 analyst.rs Role::Zarili 的 persona() 抽取（去掉 Rust 字符串续行符 `\`）：

Write `crates/core/strategies/zarili/base.md`:
```markdown
你是zarili，A股量价低吸派实战交易者，方法论蒸馏自其30篇实盘复盘文章。

核心信仰：一切围绕量能。量价关系是一切判断的根基，股价是车、成交量是油，油门不松车速不止。

【择时体系】
1.指数定方向：短线先择时后选股。5日线是短期生命线，跌破5日线且5日线向下则空仓，站上5日线且5日线拐头向上才试错。
2.量能定级别：市场放量加仓、缩量降仓。2万亿以上量能支撑大科技主升，1.6-1.7万亿则回归小票轮动。
3.昨成交20指数：统计前日成交前20个股走势，是容量票盈亏效应的先行指标，比指数慢一拍但更真实。
4.股指期货打提前量：股指弱于指数是风险信号，股指强于指数是机会信号。

【选股体系】
1.成交金额前排：只做市场合力大票，不碰单一资金能撼动的小票。同一板块成交前20出现多只个股=大资金涌入=行情级别大。
2.大马拉小车：同等成交金额下，选自由流通市值小的，换手率高、股性活跃。
3.极限量：主升期间出现上市以来最大堆量（非一两日放量），极其罕见的成交量=极其罕见的大行情。
4.均线黏合+爆量突破：短中长期均线黏合后放量涨停启动，突破强度看堆量程度和启动强度（涨停>大阳>中小阳）。
5.百日新高板块：新高个股越多=板块越是突破走势，围绕新高板块选股。

【买卖体系】
1.只低吸不追高：超过3个点浑身不舒服，绿盘低吸是首选姿势。分歧充分、持续堆量才是心头宝。
2.板块分歧低吸：板块大分歧日是买点，一致加速日是卖点。分歧出妖股、一致走不远。
3.趋势持股：强趋势股看5日线，慢趋势股看20日线或5周线。趋势不破一股不让。
4.先看指数再看板块最后选股：指数好→情绪好→热门题材好，炒作的本质是指数。

【风控体系】
1.不做难题：市场难做就不做，好机会一眼馋不用动脑筋。
2.合法合规是底线：监管不允许的不碰，主动进监管的标的全部取关。
3.仓位跟着量能走：量能足满仓干，量能缩半仓守，跌破关键量能线就休息。
4.强势股补跌是见底信号：最弱的止跌+最强的补跌=行情内生见底。

语言风格：直接、不废话、像老手盘中复盘，用数据和逻辑说话。
```

- [ ] **Step 5: Commit**

```bash
git add crates/core/strategies/
git commit -m "feat: extract builtin role strategies to base.md files"
```

---

### Task 2: 重构 core/roles 模块

**Files:**
- Modify: `crates/core/src/roles/mod.rs`
- Create: `crates/core/src/roles/prompt.rs`

- [ ] **Step 1: 重写 roles/mod.rs — 定义 RoleConfig, MergeMode, BUILTIN_STRATEGIES**

Write `crates/core/src/roles/mod.rs`:
```rust
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct RoleConfig {
    pub name: String,
    pub display_name: String,
    pub dir: String,
    pub merge_mode: MergeMode,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeMode {
    Supplement,
    Override,
    BaseOnly,
}

pub mod prompt;

pub const BUILTIN_STRATEGIES: &[(&str, &str, &str)] = &[
    ("zarili", "zarili量价派", include_str!("../../strategies/zarili/base.md")),
    ("trader", "游资操盘手", include_str!("../../strategies/trader/base.md")),
    ("quant", "量化分析师", include_str!("../../strategies/quant/base.md")),
    ("risk_officer", "风控官", include_str!("../../strategies/risk_officer/base.md")),
];

pub fn get_builtin_strategy(name: &str) -> Option<&'static str> {
    BUILTIN_STRATEGIES
        .iter()
        .find(|(n, _, _)| *n == name)
        .map(|(_, _, content)| *content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_builtin_strategy_exists() {
        let s = get_builtin_strategy("trader");
        assert!(s.is_some());
        assert!(s.unwrap().contains("游资操盘手"));
    }

    #[test]
    fn test_get_builtin_strategy_not_found() {
        assert!(get_builtin_strategy("nonexistent").is_none());
    }

    #[test]
    fn test_merge_mode_deserialize() {
        let config: RoleConfig = toml::from_str(
            r#"name = "test"
display_name = "测试"
dir = "strategies/test"
merge_mode = "supplement"
"#,
        )
        .unwrap();
        assert!(matches!(config.merge_mode, MergeMode::Supplement));
    }
}
```

- [ ] **Step 2: 创建 prompt.rs**

Write `crates/core/src/roles/prompt.rs`:
```rust
pub struct Prompt {
    pub system: String,
    pub user: String,
}

/// 构建 LLM prompt
/// - `strategy_text`: 角色策略文本（base.md ± supplement.md 拼装后的结果）
/// - `market_data_json`: 市场数据，JSON 字符串
/// - `display_name`: 角色中文名
pub fn build_prompt(strategy_text: &str, market_data_json: &str, display_name: &str) -> Prompt {
    Prompt {
        system: strategy_text.to_string(),
        user: format!(
            r#"## 今日市场数据

```json
{market_data}
```

请以【{name}】身份，用简洁专业的语言分析：
1. 今日市场情绪定性（偏强/震荡/偏弱）
2. 值得关注的机会或风险
3. 明日操作建议

控制在150字以内，直接给结论。"#,
            market_data = market_data_json,
            name = display_name,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_prompt_structure() {
        let prompt = build_prompt("策略正文", r#"{"rise": 3000}"#, "测试角色");
        assert_eq!(prompt.system, "策略正文");
        assert!(prompt.user.contains("测试角色"));
        assert!(prompt.user.contains(r#""rise": 3000"#));
    }
}
```

- [ ] **Step 3: 更新 lib.rs 导出**

`crates/core/src/lib.rs` 中 `pub mod roles;` 不变，roles 子模块已更新。

- [ ] **Step 4: 运行测试**

```bash
cargo test -p openatrading-core
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/roles/ crates/core/src/lib.rs
git commit -m "refactor(core): RoleConfig + MergeMode + BUILTIN_STRATEGIES + prompt module"
```

---

### Task 3: 新增 CLI config 模块

**Files:**
- Create: `crates/cli/src/cli/config.rs`
- Modify: `crates/cli/Cargo.toml`
- Modify: `crates/cli/src/cli/mod.rs`

- [ ] **Step 1: 添加依赖**

Edit `crates/cli/Cargo.toml` — add `toml` and `dirs`:

```toml
[dependencies]
# ... existing deps ...
toml = "0.8"
dirs = "6"
```

- [ ] **Step 2: 创建 config.rs**

Write `crates/cli/src/cli/config.rs`:
```rust
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use serde::Deserialize;
use openatrading_core::roles::{RoleConfig, MergeMode, BUILTIN_STRATEGIES};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub default_role: Option<String>,
    pub roles: Vec<RoleEntry>,
}

#[derive(Debug, Clone)]
pub struct RoleEntry {
    pub config: RoleConfig,
    /// resolved absolute path for `dir`
    pub strategy_base: PathBuf,
}

#[derive(Debug, Deserialize)]
struct RawConfig {
    default_role: Option<String>,
    #[serde(default)]
    roles: Vec<RoleConfig>,
}

const DEFAULT_CONFIG_TOML: &str = r##"# ── Trending Agent 配置 ──────────────────────────────────
# 优先级：项目根 config.toml > 项目根/.atrading/config.toml > ~/.atrading/config.toml
# 同名角色：高优先级完整覆盖低优先级

default_role = "zarili"

[[roles]]
name = "zarili"
display_name = "zarili量价派"
dir = "strategies/zarili"
merge_mode = "supplement"

[[roles]]
name = "trader"
display_name = "游资操盘手"
dir = "strategies/trader"
merge_mode = "base_only"

[[roles]]
name = "quant"
display_name = "量化分析师"
dir = "strategies/quant"
merge_mode = "supplement"

[[roles]]
name = "risk_officer"
display_name = "风控官"
dir = "strategies/risk_officer"
merge_mode = "supplement"
"##;

impl AppConfig {
    /// 按三层优先级加载并合并配置
    pub fn load(project_root: &Path) -> Result<Self> {
        let home = dirs::home_dir().context("无法获取 HOME 目录")?;
        let home_atrading = home.join(".atrading");
        let project_atrading = project_root.join(".atrading");

        let mut merged = AppConfig {
            default_role: None,
            roles: Vec::new(),
        };

        // Layer 1 (lowest): ~/.atrading/config.toml
        if let Some(cfg) = Self::load_from(&home_atrading.join("config.toml"), &home_atrading) {
            merged.merge(cfg);
        }

        // Layer 2 (medium): <project>/.atrading/config.toml
        if let Some(cfg) =
            Self::load_from(&project_atrading.join("config.toml"), &project_atrading)
        {
            merged.merge(cfg);
        }

        // Layer 3 (highest): <project>/config.toml
        // 其 dir 相对路径基于项目 .atrading/ 解析
        if let Some(cfg) = Self::load_from(&project_root.join("config.toml"), &project_atrading) {
            merged.merge(cfg);
        }

        Ok(merged)
    }

    fn load_from(config_path: &Path, atrading_dir: &Path) -> Option<AppConfig> {
        let content = std::fs::read_to_string(config_path).ok()?;
        let raw: RawConfig = toml::from_str(&content).ok()?;
        let roles = raw
            .roles
            .into_iter()
            .map(|r| {
                let strategy_base = atrading_dir.join(&r.dir);
                RoleEntry {
                    config: r,
                    strategy_base,
                }
            })
            .collect();
        Some(AppConfig {
            default_role: raw.default_role,
            roles,
        })
    }

    fn merge(&mut self, other: AppConfig) {
        if other.default_role.is_some() {
            self.default_role = other.default_role;
        }
        for entry in other.roles {
            self.roles.retain(|e| e.config.name != entry.config.name);
            self.roles.push(entry);
        }
    }

    pub fn find_role(&self, name: &str) -> Option<&RoleEntry> {
        self.roles.iter().find(|e| e.config.name == name)
    }

    /// 按 merge_mode 从 strategy_base/base.md 和 supplement.md 组装策略文本
    pub fn load_strategy(role: &RoleEntry) -> Result<String> {
        let base_path = role.strategy_base.join("base.md");
        let supplement_path = role.strategy_base.join("supplement.md");

        match role.config.merge_mode {
            MergeMode::BaseOnly => {
                let base = std::fs::read_to_string(&base_path)
                    .with_context(|| format!("无法读取 base.md: {}", base_path.display()))?;
                Ok(base)
            }
            MergeMode::Supplement => {
                let base = std::fs::read_to_string(&base_path)
                    .with_context(|| format!("无法读取 base.md: {}", base_path.display()))?;
                let supplement = std::fs::read_to_string(&supplement_path).unwrap_or_default();
                let trimmed = supplement.trim();
                if trimmed.is_empty() {
                    Ok(base)
                } else {
                    Ok(format!("{base}\n\n{supplement}"))
                }
            }
            MergeMode::Override => {
                let supplement =
                    std::fs::read_to_string(&supplement_path).with_context(|| {
                        format!("无法读取 supplement.md: {}", supplement_path.display())
                    })?;
                Ok(supplement)
            }
        }
    }

    /// 列出所有可用角色
    pub fn list_roles(&self) {
        println!("{:<20} {:<20} {:<30}", "name", "display_name", "dir");
        println!("{}", "-".repeat(72));
        for entry in &self.roles {
            println!(
                "{:<20} {:<20} {:<30}",
                entry.config.name, entry.config.display_name, entry.config.dir
            );
        }
        if let Some(ref default) = self.default_role {
            println!("\ndefault_role: {default}");
        }
    }
}

/// `openatrading init` — 在 HOME 目录创建 ~/.atrading/
pub fn cmd_init() -> Result<()> {
    let home = dirs::home_dir().context("无法获取 HOME 目录")?;
    let atrading = home.join(".atrading");

    if atrading.exists() {
        eprintln!("~/.atrading/ 已存在，跳过初始化");
        return Ok(());
    }

    // 写入 config.toml
    std::fs::create_dir_all(&atrading)?;
    std::fs::write(atrading.join("config.toml"), DEFAULT_CONFIG_TOML)?;

    // 写入内置策略 base.md 和空 supplement.md
    for (name, _display, content) in BUILTIN_STRATEGIES {
        let role_dir = atrading.join("strategies").join(name);
        std::fs::create_dir_all(&role_dir)?;
        std::fs::write(role_dir.join("base.md"), content)?;
        std::fs::write(
            role_dir.join("supplement.md"),
            "# 在此文件补充或覆盖 base.md 中的策略\n",
        )?;
    }

    eprintln!("✅ 已初始化 ~/.atrading/");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use openatrading_core::roles::MergeMode;

    #[test]
    fn test_merge_role_override() {
        let role_a = RoleConfig {
            name: "trader".into(),
            display_name: "A".into(),
            dir: "strategies/a".into(),
            merge_mode: MergeMode::BaseOnly,
        };
        let role_b = RoleConfig {
            name: "trader".into(),
            display_name: "B".into(),
            dir: "strategies/b".into(),
            merge_mode: MergeMode::Supplement,
        };

        let mut cfg = AppConfig {
            default_role: None,
            roles: vec![RoleEntry {
                config: role_a,
                strategy_base: PathBuf::from("/tmp/a"),
            }],
        };
        cfg.merge(AppConfig {
            default_role: Some("trader".into()),
            roles: vec![RoleEntry {
                config: role_b,
                strategy_base: PathBuf::from("/tmp/b"),
            }],
        });

        assert_eq!(cfg.default_role.as_deref(), Some("trader"));
        assert_eq!(cfg.roles.len(), 1);
        assert_eq!(cfg.roles[0].config.display_name, "B");
    }
}
```

- [ ] **Step 3: 更新 cli/mod.rs 声明模块**

Edit `crates/cli/src/cli/mod.rs` — add `pub mod config;` after line 1:

```rust
pub mod app;
pub mod config;
```

- [ ] **Step 4: 运行测试**

```bash
cargo test -p openatrading-cli
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/cli/Cargo.toml crates/cli/src/cli/config.rs crates/cli/src/cli/mod.rs
git commit -m "feat(cli): add AppConfig with three-layer config merge + init command"
```

---

### Task 4: 更新 CLI app.rs — 新增参数

**Files:**
- Modify: `crates/cli/src/cli/app.rs`

- [ ] **Step 1: 添加 --role, --list-roles 参数和 init 子命令**

Read the current file, then replace the struct definition.

Edit `crates/cli/src/cli/app.rs` — replace the `Cli` struct (lines 23-59) with:

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "openatrading")]
#[command(about = "A股市场异动追踪工具 — 命令行接口")]
#[command(long_about = r#"
openatrading 是一个 A 股市场异动追踪工具。
支持多种数据源的获取和 AI 分析，输出 JSON / Markdown / HTML 格式。

用法:
  openatrading                               # 默认获取全量数据，输出 Markdown 到 stdout
  openatrading --json                        # 输出 JSON 到 stdout
  openatrading --html --output report.html   # 输出 HTML 到文件
  openatrading --data limit_up,overview      # 只获取涨停和市场概览
  openatrading --data cls_telegraph --ai     # 获取财联社电报并 AI 分析
  openatrading init                          # 初始化 ~/.atrading/ 配置目录
  openatrading --role zarili --ai            # 指定角色运行 AI 分析
  openatrading --list-roles                  # 列出所有可用角色

环境变量:
  LLM_API_KEY       LLM API 密钥 (DeepSeek 或 OpenAI-compatible)
  LLM_MODEL         LLM 模型名称 (默认 deepseek-chat)
  LLM_API_ENDPOINT  LLM API 端点 (默认 DeepSeek)
  JIUYAN_PHONE      韭研公社登录手机号
"#)]
pub struct Cli {
    /// 输出 JSON 格式
    #[arg(long, group = "format")]
    pub json: bool,

    /// 输出 Markdown 格式（默认）
    #[arg(long, group = "format")]
    pub markdown: bool,

    /// 输出 HTML 格式
    #[arg(long, group = "format")]
    pub html: bool,

    /// 输出到文件（默认 stdout）
    #[arg(short, long)]
    pub output: Option<String>,

    /// 数据源，逗号分隔
    /// 可选: limit_up, limit_down, super_stock, field_item, overview,
    ///       plate_abnormal, timeline, community, sentiment, technical,
    ///       cls_telegraph, dapan_abnormal, market_trend
    /// 默认: 全部
    #[arg(short, long, value_delimiter = ',')]
    pub data: Option<Vec<String>>,

    /// 启用 AI 分析
    #[arg(long)]
    pub ai: bool,

    /// 淘股吧用户 ID（community 数据源使用）
    #[arg(long)]
    pub tgb_user: Option<String>,

    /// 财联社电报分类（cls_telegraph 数据源使用）
    #[arg(long)]
    pub cls_category: Option<String>,

    /// 指定角色名（对应 config.toml 中 [[roles]].name）
    #[arg(long)]
    pub role: Option<String>,

    /// 列出所有可用角色
    #[arg(long)]
    pub list_roles: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// 初始化 ~/.atrading/ 配置目录（含内置策略种子文件）
    Init,
}
```

- [ ] **Step 2: 编译验证**

```bash
cargo check -p openatrading-cli
```

Expected: compiles without errors.

- [ ] **Step 3: Commit**

```bash
git add crates/cli/src/cli/app.rs
git commit -m "feat(cli): add --role, --list-roles params and init subcommand"
```

---

### Task 5: 更新 CLI mod.rs — 接入角色配置

**Files:**
- Modify: `crates/cli/src/cli/mod.rs`

- [ ] **Step 1: 添加 AiAnalysis 字段到 OutputData 并重写 AI 分析部分**

Replace the entire `run()` function's AI analysis section and add config-driven role selection.

Edit `crates/cli/src/cli/mod.rs`:

Add imports (after existing imports):
```rust
use openatrading_core::roles::prompt::build_prompt;
use crate::cli::config::{AppConfig, cmd_init};
```

Add field to `OutputData`:
```rust
#[derive(Serialize)]
struct OutputData {
    // ... existing fields ...
    #[serde(skip_serializing_if = "Option::is_none")]
    ai_analysis: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    role_name: Option<String>,
}
```

Replace the `run()` function with the new version that handles init/list-roles/role:

In `pub async fn run()` — add at the top, after `let cli = Cli::parse();`:

```rust
// Handle `init` subcommand
if let Some(Commands::Init) = cli.command {
    return cmd_init();
}

// Load config
let project_root = std::env::current_dir()?;
let app_config = AppConfig::load(&project_root)?;

// Handle --list-roles
if cli.list_roles {
    app_config.list_roles();
    return Ok(());
}
```

Replace the existing AI analysis block (lines 158-175) with:

```rust
// ── AI Analysis ─────────────────────────────────────

if cli.ai {
    let role_name = cli
        .role
        .or_else(|| app_config.default_role.clone())
        .unwrap_or_else(|| "zarili".to_string());

    let strategy_text = match app_config.find_role(&role_name) {
        Some(entry) => {
            output.role_name = Some(entry.config.display_name.clone());
            AppConfig::load_strategy(entry)?
        }
        None => {
            eprintln!("⚠️ 未找到角色 '{role_name}'，使用默认策略");
            output.role_name = Some("默认分析".to_string());
            openatrading_core::roles::get_builtin_strategy("zarili")
                .unwrap_or("")
                .to_string()
        }
    };

    let api_key = std::env::var("LLM_API_KEY")
        .or_else(|_| std::env::var("DEEPSEEK_API_KEY"))
        .or_else(|_| std::env::var("OPENROUTER_API_KEY"))
        .ok();

    if let Some(key) = api_key {
        let llm = LlmClient::new(key);
        let market_data_json = serde_json::to_string(&output)?;
        let display = output
            .role_name
            .as_deref()
            .unwrap_or("AI助手");
        let prompt = build_prompt(&strategy_text, &market_data_json, display);

        let messages = serde_json::json!([
            {"role": "system", "content": prompt.system},
            {"role": "user", "content": prompt.user}
        ])
        .to_string();

        if let Ok(analysis) = llm.chat_with_messages(&messages).await {
            output.ai_analysis = Some(analysis);
        }
    } else {
        eprintln!("⚠️ 未配置 LLM_API_KEY，跳过 AI 分析");
    }
}
```

- [ ] **Step 2: 为 LlmClient 添加 chat_with_messages 方法**

Edit `crates/core/src/llm/openrouter.rs` — add a new method to `impl LlmClient`:

```rust
/// 非流式发送 messages JSON，返回完整回复文本
pub async fn chat_with_messages(&self, messages_json: &str) -> Result<String> {
    let body = serde_json::json!({
        "model": self.model,
        "messages": serde_json::from_str::<serde_json::Value>(messages_json).unwrap_or_default(),
        "max_tokens": 4096,
        "temperature": 0.7,
        "stream": false,
    })
    .to_string();

    let resp: ChatResponse = self
        .client
        .post(&self.api_endpoint)
        .bearer_auth(&self.api_key)
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await?
        .json()
        .await?;

    Ok(resp.choices[0].message.content.clone())
}
```

- [ ] **Step 3: 编译验证**

```bash
cargo check -p openatrading-cli
```

Expected: compiles without errors.

- [ ] **Step 4: Commit**

```bash
git add crates/cli/src/cli/mod.rs crates/core/src/llm/openrouter.rs
git commit -m "feat(cli): wire role config into AI analysis with system/user prompt split"
```

---

### Task 6: 更新 Web main.rs — system=策略 user=市场数据

**Files:**
- Modify: `crates/web/src/main.rs`

- [ ] **Step 1: 修改 build_system_prompt → 拆分 system/user**

Read `crates/web/src/main.rs` lines 333-382.

Replace the `build_system_prompt` function with two functions and update `ai_chat`:

Replace function (lines 333-382):
```rust
fn build_system_prompt(strategy_text: &str) -> String {
    strategy_text.to_string()
}

fn build_user_message(
    overview: &OverviewResponse,
    field_items: Option<&str>,
    super_stocks: Option<&str>,
) -> String {
    let field_section = field_items
        .map(|s| format!("\n## 韭研公社异动数据\n{s}\n"))
        .unwrap_or_default();

    let super_section = super_stocks
        .map(|s| format!("\n## 强势股数据（未涨停但强）\n{s}\n"))
        .unwrap_or_default();

    format!(
        r#"## 当前市场数据
- 日期：{date}
- 上涨家数：{rise} 家
- 下跌家数：{fall} 家
- 涨停家数：{limit_up} 只
- 跌停家数：{limit_down} 只
- 炸板率：{bomb_rate:.1}%
- 涨跌比：{ratio}
- 风险等级：{risk_level}
- 市场摘要：{summary}
{field_items}{super_stocks}
请在回答时结合以上实时市场数据，帮助用户理解当前市场状态。"#,
        date = overview.date,
        rise = overview.rise_count,
        fall = overview.fall_count,
        limit_up = overview.limit_up_count,
        limit_down = overview.limit_down_count,
        bomb_rate = overview.bomb_rate,
        ratio = format!(
            "{:.2}",
            overview.rise_count as f64 / overview.fall_count.max(1) as f64
        ),
        risk_level = overview.risk_level,
        summary = overview.decision_summary,
        field_items = field_section,
        super_stocks = super_section,
    )
}
```

Then in `ai_chat` (lines 384-418), replace the system prompt building block:

Replace lines 399-408:
```rust
    // Web 端暂使用内置 zarili 策略（二进制嵌入）
    let strategy_text = openatrading_core::roles::get_builtin_strategy("zarili")
        .unwrap_or("你是一个A股短线交易AI助手。");
    let system_prompt = build_system_prompt(strategy_text);
    let user_message = build_user_message(&overview, field_items.as_deref(), super_stocks.as_deref());

    // Build messages array: system + user
    let prompt = serde_json::json!([
        {"role": "system", "content": system_prompt},
        {"role": "user", "content": format!("{}\n\n用户提问：{}", user_message, req.message)}
    ])
    .to_string();
```

- [ ] **Step 2: 编译验证**

```bash
cargo check -p openatrading-web
```

Expected: compiles without errors.

- [ ] **Step 3: Commit**

```bash
git add crates/web/src/main.rs
git commit -m "refactor(web): split system prompt to strategy-only, market data to user message"
```

---

### Task 7: 删除旧 analyst.rs

**Files:**
- Delete: `crates/core/src/roles/analyst.rs`

- [ ] **Step 1: 删除文件**

```bash
rm crates/core/src/roles/analyst.rs
```

- [ ] **Step 2: 全量编译验证**

```bash
cargo check --workspace
```

Expected: all crates compile without errors (analyst.rs is no longer referenced).

- [ ] **Step 3: 运行全部测试**

```bash
cargo test --workspace
```

Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/roles/analyst.rs
git commit -m "refactor(core): remove hardcoded analyst.rs, replaced by config-driven roles"
```
