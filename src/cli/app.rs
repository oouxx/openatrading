use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "trending-agent")]
#[command(about = "A股市场异动追踪工具")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// 获取选股宝涨停数据
    LimitUp(CommonOpts),
    /// 获取选股宝跌停数据
    LimitDown(CommonOpts),
    /// 获取选股宝市场概览
    Overview(CommonOpts),
    /// 获取选股宝板块异动
    PlateAbnormal(CommonOpts),
    /// 获取韭研公社异动数据
    Field(CommonOpts),
    /// 获取韭研公社时间线数据
    Timeline(CommonOpts),
    /// 获取韭研公社社群文章
    Community(CommunityOpts),
    /// 运行完整报告（包含所有数据源）
    Run(RunOpts),
    /// 进入交互模式
    Agent(CommonOpts),
    /// 舆情分析
    Sentiment(CommonOpts),
    /// 技术分析
    Technical(CommonOpts),
    /// 保存淘股吧用户帖子
    SaveUser(UserOpts),
    /// 生成淘股吧子页面
    TaoGuBa(TaoGuBaOpts),
    /// 生成报告索引页
    Index(IndexOpts),
    /// 生成选股宝子页面
    #[command(name = "xuangubao")]
    XuanGuBao(XuanGuBaoOpts),
}

#[derive(clap::Args)]
pub struct CommonOpts {
    /// 启用 AI 分析（需要配置 OPENROUTER_API_KEY）
    #[arg(short, long)]
    pub ai: bool,

    /// 输出格式: markdown, html, both
    #[arg(short, long, default_value = "markdown")]
    pub format: String,

    /// 输出目录
    #[arg(short, long, default_value = "./reports")]
    pub output: String,
}

#[derive(clap::Args)]
pub struct CommunityOpts {
    #[arg(short, long, default_value = "markdown")]
    pub format: String,

    #[arg(short, long, default_value = "./reports")]
    pub output: String,

    #[arg(long)]
    pub web: bool,

    #[arg(long, default_value = "8080")]
    pub port: u16,
}

#[derive(clap::Args)]
pub struct RunOpts {
    /// 启用 AI 分析（需要配置 OPENROUTER_API_KEY）
    #[arg(short, long)]
    pub ai: bool,

    /// 输出格式: markdown, html, both
    #[arg(short, long, default_value = "markdown")]
    pub format: String,

    /// 输出目录
    #[arg(short, long, default_value = "./reports")]
    pub output: String,

    /// 启动 Web UI（默认端口 8080）
    #[arg(long)]
    pub web: bool,

    /// Web UI 端口
    #[arg(long, default_value = "8080")]
    pub port: u16,
}

#[derive(clap::Args)]
pub struct UserOpts {
    /// 淘股吧用户ID
    pub user_id: String,

    /// 输出目录
    #[arg(short, long, default_value = "./methodology/tgb")]
    pub output: String,
}

/// TaoGuBa 子页面生成选项
#[derive(clap::Args)]
pub struct TaoGuBaOpts {
    /// 用户ID列表（逗号分隔）
    #[arg(short, long)]
    pub users: String,
    /// 输出目录
    #[arg(short, long, default_value = "./dist/taoguba")]
    pub output: String,
}

#[derive(clap::Args)]
pub struct IndexOpts {
    #[arg(short, long, default_value = "./dist")]
    pub dir: String,
}

/// 生成选股宝子页面
#[derive(clap::Args)]
pub struct XuanGuBaoOpts {
    /// 输出目录
    #[arg(short, long, default_value = "./dist/xuangubao")]
    pub output: String,
}
