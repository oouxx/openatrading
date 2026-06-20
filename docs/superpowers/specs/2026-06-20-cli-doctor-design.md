# CLI Doctor 优化设计

## 目标

将 `openatrading doctor` 从 5 项静态检查升级为模块化诊断+修复框架，覆盖更多系统维度，支持交互式逐一修复，输出更清晰的信息。

## 架构

### Check trait

```rust
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

pub enum FixRisk { Safe, Dangerous, Impossible }
```

### CheckContext

```rust
pub struct CheckContext {
    pub atrading_home: PathBuf,
    pub app_config: Option<AppConfig>,
}
```

- `atrading_home` 始终可用
- `app_config` 由 runner 加载一次传给所有 check

### CheckResult

```rust
pub enum CheckStatus {
    Ok,
    Warn(String),
    Error(String),
}

pub struct CheckResult {
    pub status: CheckStatus,
    pub summary: String,       // 单行摘要
    pub details: Vec<String>,  // 多行详细信息
    pub duration: Duration,
}
```

### FixResult

```rust
pub struct FixResult {
    pub status: FixStatus,
    pub message: String,
}

pub enum FixStatus {
    Fixed,
    Skipped,
    Failed(String),
}
```

### 检查项列表（10 项）

| id | 名称 | 检查内容 | 修复 |
|---|---|---|---|
| home | 环境目录 | AI_HOME 是否存在、可写，skills/ logs/ pipelines/ 子目录 | 自动创建，从 config.example.toml 复制（含 service.web_bind） |
| config | 配置文件 | config.toml 加载 | 生成默认（询问） |
| env | 环境变量 | 检查 JIUYAN_PHONE / PASSWORD / INIT_TOKEN（代码实际读取的变量） | 生成模板（询问） |
| sqlite | 数据库 | SQLite 连接（通过 Gateway::init） | 自动初始化 |
| cron | 定时任务 | 校验 3 个 cron 表达式（含 news_schedule），6 字段格式 | 写入默认（询问） |
| telegram | Telegram | 发送测试消息并自动清理 | 提示配置 bot_token/chat_id |
| llm | LLM API | 复用 LlmClient，向配置的 provider/model 发轻量请求，测响应时间 | 提示检查 API key/网络 |
| mcp | MCP 服务器 | 遍历 MCP server，逐个尝试连接，输出传输类型和地址 | 无 |
| skills | 技能 | SkillRegistry::from_disk() 列出已安装技能+描述 | 无 |
| port | 端口 | 检查 web_bind 和 mcp_server.bind 端口是否被占用 | 提示占用进程 |

### 特殊项输出示例

```
✅  MCP Server   3 个 (2 启用, 1 禁用)
    akshare               streamable-http → http://akshare-mcp:8081/mcp  ✅
    sequential-thinking   stdio → npx -y @anthropic/sequential-thinking  ✅
    postgres              streamable-http → http://localhost:5433/mcp     ❌ 连接超时

✅  Skills       4 个已安装
    首板策略                 只做首板涨停
    排板策略                 跟随主力资金排板
    龙虎榜追踪               龙虎榜席位分析
    A股热点分析              板块热度和涨停原因分析
```

### Runner

Runner 为 `Vec<Box<dyn Check>>` 顺序执行，静态注册：

```rust
fn all_checks() -> Vec<Box<dyn Check>> {
    vec![
        Box::new(HomeCheck),
        Box::new(ConfigCheck),
        Box::new(EnvCheck),
        Box::new(SqliteCheck),
        Box::new(CronCheck),
        Box::new(TelegramCheck),
        Box::new(LlmCheck),
        Box::new(McpCheck),
        Box::new(SkillsCheck),
        Box::new(PortCheck),
    ]
}
```

`--fix` 流程：全部检查完毕输出完整报告 → 遍历 failed/warn 的可修复项 → 逐一询问 → 修复后自动重新 check。

### Reporter

- Text 模式使用 `owo-colors` crate 输出 ANSI 颜色
- 每项检查完成后立即输出单行结果，用户可实时看到进度
- `--json` 输出展平为 `Vec<FlatCheckResult>`

```
OpenATrading 诊断报告 (v{version})
════════════════════════════════
  AI_HOME: {path}
  平台: {os}/{arch}

{check_icon}  {check_name:<12} {summary}
...每项一行...

════════════════════════════════
{summary_line}
```

### 交互式修复

`doctor --fix` 模式下，遍历 failed/warn 且可修复的项，逐项询问：

```
是否修复 #1 ({name})？[{fix_label}] [y/N] →
```

修复后自动重新 check 该单项，输出新状态。

## 文件结构

```
crates/cli/src/cli/
├── doctor.rs           # 入口，保持 cmd_doctor(json, fix)
└── doctor/
    ├── mod.rs          # Check trait, CheckContext, CheckResult, runner, reporter
    ├── check_home.rs
    ├── check_config.rs
    ├── check_env.rs
    ├── check_sqlite.rs
    ├── check_cron.rs
    ├── check_telegram.rs
    ├── check_llm.rs
    ├── check_mcp.rs
    ├── check_skills.rs
    └── check_port.rs
```

## 接口不变

`cmd_doctor` 签名保持 `pub async fn cmd_doctor(json: bool, fix: bool) -> Result<()>`，新增 `--fix` 参数。原有 `--json` 输出向后兼容，`DoctorResult` 扩展新字段。
