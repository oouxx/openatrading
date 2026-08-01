use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "openatrading")]
#[command(about = "A股市场异动追踪工具 — 命令行接口")]
#[command(long_about = r#"
openatrading 是一个 A 股市场异动追踪工具。
支持多种数据源的获取和 AI 分析，输出 JSON / Markdown / HTML 格式。

用法:
  openatrading                           # 默认获取全量数据，输出 Markdown 到 stdout
  openatrading --json                    # 输出 JSON 到 stdout
  openatrading --html --output report.html   # 输出 HTML 到文件
  openatrading --data limit_up,overview  # 只获取涨停和市场概览
  openatrading --data cls_telegraph --ai # 获取财联社电报并 AI 分析
  openatrading init                          # 初始化 ~/.atrading/ 配置目录
  openatrading --list-roles                  # 列出所有可用角色

环境变量:
  LLM_API_KEY       LLM API 密钥 (DeepSeek 或 OpenAI-compatible)
  LLM_MODEL         LLM 模型名称 (默认 deepseek-chat)
  LLM_API_ENDPOINT  LLM API 端点 (默认 DeepSeek)
  JIUYAN_PHONE      韭研公社登录手机号
"#)]
pub struct Cli {
    /// 显示版本号
    #[arg(short = 'V', long)]
    pub version: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// 初始化 ~/.atrading/ 配置目录（含内置策略种子文件）
    Init,
    /// Gateway 生命周期管理 (systemd)
    Gateway {
        #[command(subcommand)]
        action: GatewayAction,
    },
    /// 从 GitHub Releases 更新到最新版本
    Update {
        /// 仅检查更新，不下载
        #[arg(long)]
        check: bool,
    },
    /// 持仓管理
    Trade {
        #[command(subcommand)]
        action: TradeAction,
    },
    /// 卸载 openatrading（服务、配置、二进制）
    Uninstall,
    /// 诊断当前服务状态
    Doctor {
        /// 输出 JSON 格式
        #[arg(long)]
        json: bool,
        /// 交互式修复
        #[arg(long)]
        fix: bool,
        /// 跳过需要联网的检查（Telegram / LLM / MCP）
        #[arg(long)]
        skip_network: bool,
    },
    /// 技能管理（安装/列出/卸载）
    Skills {
        #[command(subcommand)]
        action: SkillAction,
    },
    /// 生成工具参考文档（markdown）
    Tools {
        #[command(subcommand)]
        action: ToolsAction,
    },
    /// MCP 服务器管理（列出工具/状态检查/调用）
    Mcp {
        #[command(subcommand)]
        action: McpAction,
    },
    /// 查看/修改配置
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// 通道管理（列出/检查）
    Channels {
        #[command(subcommand)]
        action: ChannelsAction,
    },
}

#[derive(Subcommand)]
pub enum ChannelsAction {
    /// 列出所有通道的编译状态
    List,
}

#[derive(Subcommand)]
pub enum ToolsAction {
    /// 生成所有可用工具的 markdown 参考文档
    Doc {
        /// MCP 端点（可选，用于发现 MCP 工具）
        #[arg(long, default_value = "http://localhost:8081/mcp")]
        endpoint: String,
    },
}

#[derive(Subcommand)]
pub enum McpAction {
    /// 列出 MCP 服务端所有已注册工具
    List {
        /// MCP 端点，默认 <http://localhost:8081/mcp>
        #[arg(long, default_value = "http://localhost:8081/mcp")]
        endpoint: String,
    },
    /// 检查 MCP 服务器连通性
    Status {
        /// MCP 端点，默认 <http://localhost:8081/mcp>
        #[arg(long, default_value = "http://localhost:8081/mcp")]
        endpoint: String,
    },
    /// 直接调用一个 MCP 工具
    Call {
        /// 工具名称
        name: String,
        /// JSON 参数（可选）
        #[arg(default_value = "{}")]
        args: String,
        /// MCP 端点，默认 <http://localhost:8081/mcp>
        #[arg(long, default_value = "http://localhost:8081/mcp")]
        endpoint: String,
    },
    /// 启动 MCP 服务端，对外暴露行情工具
    Serve {
        #[arg(long, value_enum, default_value_t = McpTransport::Http)]
        transport: McpTransport,
        #[arg(long, default_value = "0.0.0.0:8082")]
        bind: String,
    },
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum McpTransport {
    Http,
    Stdio,
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// 查看所有配置
    Show,
    /// 查看 Social 通知配置
    Social,
    /// 修改配置项
    Set {
        #[command(subcommand)]
        action: ConfigSetter,
    },
}

#[derive(Subcommand)]
pub enum ConfigSetter {
    /// 修改 social `notify_channel`
    NotifyChannel { value: String },
    /// 修改 Telegram `bot_token`
    TelegramToken { value: String },
    /// 修改 Telegram `chat_id`
    TelegramChatId { value: String },
    /// 修改 Discord `bot_token`
    DiscordToken { value: String },
    /// 修改 Discord `channel_id`
    DiscordChannelId { value: String },
}

#[derive(Subcommand)]
pub enum SkillAction {
    /// 列出已安装的技能
    List,
    /// 安装一个技能（文件路径、URL 或直接粘贴内容）
    Install {
        /// 技能名称（用于卸载时指定）
        #[arg(long)]
        name: String,
        /// 文件路径、URL 或 SKILL.md 内容
        source: String,
        /// 从 URL 下载
        #[arg(long)]
        url: bool,
    },
    /// 卸载已安装的技能
    Uninstall {
        /// 技能名称
        name: String,
    },
}

#[derive(Subcommand)]
pub enum GatewayAction {
    /// 安装 systemd 服务
    Install,
    /// 卸载 systemd 服务
    Uninstall,
    /// 启动服务
    Start,
    /// 停止服务
    Stop,
    /// 查看服务状态
    Status,
    /// 前台运行（调试用）
    Run {
        /// 仅运行 cron scheduler
        #[arg(long)]
        cron_only: bool,
        /// 仅运行 web server
        #[arg(long)]
        web_only: bool,
        /// 不启动 MCP server（默认随 gateway 启动）
        #[arg(long)]
        no_mcp: bool,
    },
}

#[derive(Subcommand)]
pub enum TradeAction {
    /// 添加持仓
    Add {
        #[arg(long)]
        role: String,
        #[arg(long)]
        symbol: String,
        #[arg(long)]
        price: f64,
        #[arg(long)]
        shares: i64,
    },
    /// 列出持仓
    List {
        #[arg(long)]
        role: Option<String>,
    },
    /// 平仓
    Close {
        id: i64,
        #[arg(long)]
        price: f64,
    },
}
