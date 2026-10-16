use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "openatrading")]
#[command(about = "A股市场异动追踪工具 — 命令行接口")]
#[command(long_about = r#"
openatrading 是一个 A 股短线行情与舆情工作台。

用法:
  openatrading init                  # 初始化 ~/.atrading/ 配置目录
  openatrading gateway run           # 前台启动 Web + Cron + MCP
  openatrading doctor                # 诊断环境与服务状态
  openatrading skills list           # 查看已安装技能
  openatrading mcp list              # 列出 MCP 端点上的工具
  openatrading mcp serve             # 对外暴露行情工具（MCP server）
  openatrading config show           # 查看当前配置

模拟盘交易不走 CLI：在 Web / 桌面端面板里手动记录买卖，agent 只做行情与舆情分析。

环境变量（也可写在 ~/.atrading/.env，按全局 → 项目两级加载）:
  JIUYAN_PHONE       韭研公社登录手机号
  JIUYAN_PASSWORD    韭研公社密码
  OPENATRADING_HOME  自定义品牌根目录（默认 ~/.atrading）
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
    /// 初始化 ~/.atrading/ 配置目录
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
        /// 跳过需要联网的检查（Telegram / MCP）
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
