use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "trending-agent")]
#[command(about = "A股市场异动追踪工具")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// 启用 AI 分析（需要配置 OPENROUTER_API_KEY）
    #[arg(short, long)]
    pub ai: bool,

    /// 输出目录
    #[arg(short, long, default_value = "./reports")]
    pub output: String,
}

#[derive(Subcommand)]
pub enum Commands {
    /// 获取选股宝涨停数据
    LimitUp,
    /// 获取选股宝跌停数据
    LimitDown,
    /// 获取选股宝市场概览
    Overview,
    /// 获取选股宝板块异动
    PlateAbnormal,
    /// 获取韭研公社异动数据
    Field,
    /// 获取韭研公社时间线数据
    Timeline,
    /// 获取韭研公社社群文章
    Community,
    /// 运行完整报告（包含所有数据源）
    Run,
}
