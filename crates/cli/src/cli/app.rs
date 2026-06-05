use clap::{Parser, Subcommand};

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
    /// 诊断当前服务状态
    Doctor {
        /// 输出 JSON 格式
        #[arg(long)]
        json: bool,
    },
    /// 启动 AI 对话终端界面
    Chat,
    /// 手动触发一次所有角色的每日决策并推送通知
    Decision {
        /// 仅输出到终端，不推送通知
        #[arg(long)]
        dry_run: bool,
        /// 仅对指定角色执行决策（可多次使用）
        #[arg(long)]
        role: Option<String>,
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
