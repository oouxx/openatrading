# Role Module Refactor Design

## Overview

重构 core role 模块，从硬编码枚举改为配置文件驱动，角色可扩展，策略独立管理。

## 1. Directory Structure

```
~/.atrading/                       # 用户 HOME，全局最低优先级（init 命令创建）
├── config.toml
└── strategies/
    ├── zarili/
    │   ├── base.md                # 内置种子策略
    │   └── supplement.md          # 用户补充（空文件占位）
    ├── trader/
    │   ├── base.md
    │   └── supplement.md
    ├── quant/
    │   ├── base.md
    │   └── supplement.md
    └── risk_officer/
        ├── base.md
        └── supplement.md

项目根/.atrading/                   # 项目中优先级
├── config.toml
└── strategies/                     # 用户可在项目级添加自定义角色
    └── my-style/
        ├── base.md
        └── supplement.md

项目根/config.toml                   # 最高优先级
```

## 2. config.toml Format

```toml
# ── Trending Agent 配置 ──────────────────────────────────
# 优先级：项目根 config.toml > 项目根/.atrading/config.toml > ~/.atrading/config.toml
# 同名角色：高优先级完整覆盖低优先级

default_role = "zarili"

[[roles]]
name = "zarili"
display_name = "zarili量价派"
dir = "strategies/zarili"
merge_mode = "supplement"     # supplement | override | base_only

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
```

### merge_mode 语义

| Mode | base.md | supplement.md | 结果 |
|------|---------|---------------|------|
| `base_only` | 使用 | 忽略 | = base |
| `supplement` | 使用 | 拼接到后面 | = base + "\n\n" + supplement |
| `override` | 忽略 | 使用 | = supplement |

- `dir` 相对路径相对于所属 `.atrading/` 目录解析
- 同名角色：高优先级配置完整覆盖低优先级（不深度合并字段）

## 3. Config Loading (crates/cli)

### Three-layer merge

```
AppConfig::load(project_root):
  1. ~/.atrading/config.toml        (低)
  2. 项目根/.atrading/config.toml    (中)
  3. 项目根/config.toml             (高)
  
  Merge: 逐层加载，同名角色完整覆盖，default_role 逐层覆盖
```

### Module: `crates/cli/src/cli/config.rs`

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub default_role: Option<String>,
    pub roles: Vec<RoleConfig>,
}

impl AppConfig {
    pub fn load(project_root: &Path) -> Result<Self>;
    pub fn find_role(&self, name: &str) -> Option<&RoleConfig>;
    pub fn load_strategy(&self, role: &RoleConfig, source_dir: &Path) -> Result<String>;
}
```

- `load_strategy()` 按 merge_mode 从 `dir/base.md` 和 `dir/supplement.md` 组装策略文本
- Strategy dir 路径解析：相对 `dir` 基于找到该角色的配置文件的 `.atrading/` 父目录

## 4. Core Module Refactor (crates/core)

### `crates/core/src/roles/mod.rs`

```rust
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

/// 内置策略常量 — Web 端二进制嵌入，CLI init 种子文件
pub const BUILTIN_STRATEGIES: &[(&str, &str, &str)] = &[
    ("zarili", "zarili量价派", include_str!("../../strategies/zarili/base.md")),
    ("trader", "游资操盘手", include_str!("../../strategies/trader/base.md")),
    ("quant", "量化分析师", include_str!("../../strategies/quant/base.md")),
    ("risk_officer", "风控官", include_str!("../../strategies/risk_officer/base.md")),
];

pub fn get_builtin_strategy(name: &str) -> Option<&'static str>;
```

### `crates/core/src/roles/prompt.rs`

```rust
pub struct Prompt {
    pub system: String,   // 角色策略文本
    pub user: String,     // 市场数据 JSON + 分析指令
}

pub fn build_prompt(strategy_text: &str, market_data_json: &str, display_name: &str) -> Prompt;
```

**Prompt 分离规则（CLI + Web 统一）：**
- **system** = 策略文本（base.md ± supplement.md）
- **user** = 市场数据 JSON + 分析指令

### 删除

- `crates/core/src/roles/analyst.rs` — 硬编码的 `Role` 枚举和 `persona()` 全部移除

### 新增内置策略种子文件

```
crates/core/strategies/
├── zarili/base.md
├── trader/base.md
├── quant/base.md
└── risk_officer/base.md
```

内容从当前 `analyst.rs` 的 `persona()` 抽取为纯 markdown。

## 5. CLI Parameters

```bash
openatrading init                              # 初始化 ~/.atrading/
openatrading --role zarili --ai                # 指定角色运行 AI 分析
openatrading --list-roles                      # 列出所有可用角色
openatrading --ai                              # 不指定 --role，用 default_role
```

## 6. Web Server

Web 端暂不接入 config.toml，使用二进制嵌入的内置策略：

- `build_system_prompt()` 中的市场数据移到 user message
- system prompt = `get_builtin_strategy("zarili")` 的策略文本
- 后续版本可接入 role 参数

## 7. Data Flow

### CLI

```
config.toml → AppConfig::load() → RoleConfig
                 ↓
RoleConfig.dir/ + merge_mode → load_strategy() → strategy_text
                 ↓
fetch market data → serde_json::to_string() → market_data_json
                 ↓
build_prompt(strategy_text, market_data_json, display_name) → Prompt
                 ↓
LlmClient::chat(system=Prompt.system, user=Prompt.user)
```

### Web

```
get_builtin_strategy("zarili") → strategy_text
fetch market data → market_data_json
build_prompt(strategy_text, market_data_json, "zarili量价派") → Prompt
```

## 8. Implementation Order

1. 创建 `crates/core/strategies/*/base.md` — 从 analyst.rs 抽取策略文本
2. 重构 `crates/core/src/roles/` — RoleConfig, MergeMode, BUILTIN_STRATEGIES, build_prompt()
3. 新增 `crates/cli/src/cli/config.rs` — AppConfig 加载/合并/角色查找
4. 更新 CLI `app.rs` — 新增 --role, --list-roles, init 子命令
5. 更新 CLI `mod.rs` — 接入 config 驱动角色选择
6. 更新 Web `main.rs` — system prompt 改用内置策略，市场数据移到 user
7. 删除 `analyst.rs`
