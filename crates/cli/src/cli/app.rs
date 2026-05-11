use clap::Parser;

#[derive(Parser)]
#[command(name = "trending-agent")]
#[command(about = "A股市场异动追踪工具 — 命令行接口")]
#[command(long_about = r#"
trending-agent 是一个 A 股市场异动追踪工具。
支持多种数据源的获取和 AI 分析，输出 JSON / Markdown / HTML 格式。

用法:
  trending-agent                           # 默认获取全量数据，输出 Markdown 到 stdout
  trending-agent --json                    # 输出 JSON 到 stdout
  trending-agent --html --output report.html  # 输出 HTML 到文件
  trending-agent --data limit_up,overview  # 只获取涨停和市场概览
  trending-agent --data cls_telegraph --ai # 获取财联社电报并 AI 分析

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
}
