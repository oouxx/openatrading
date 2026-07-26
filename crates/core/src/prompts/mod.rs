use crate::memory::MarketSnapshot;
use openatrading_api::xuangubao::MarketOverview;
use std::fmt::Write;

// ── Default system prompts ─────────────────────────

/// Default system prompt when no role-specific strategy is selected.
pub const DEFAULT_CHAT_SYSTEM_PROMPT: &str =
    "你是一个A股短线交易AI助手。可以调用工具获取实时市场数据来回答用户问题。";

// ── Market context ─────────────────────────────────

/// Format market overview data into a structured context block.
pub fn build_market_context(overview: &MarketOverview, date: &str) -> String {
    format!(
        "## 当前市场数据\n\
         - 日期：{date}\n\
         - 上涨家数：{rise}\n\
         - 下跌家数：{fall}\n\
         - 涨停家数：{limit_up}\n\
         - 跌停家数：{limit_down}\n\
         - 炸板率：{bomb:.1}%\n\
         - 市场温度：{temp:.1}\n",
        rise = overview.rise_count,
        fall = overview.fall_count,
        limit_up = overview.limit_up_count,
        limit_down = overview.limit_down_count,
        bomb = overview.bomb_rate * 100.0,
        temp = overview.market_temperature,
    )
}

pub const MARKET_CONTEXT_ERROR: &str = "## 市场数据获取失败\n";

// ── Decision ───────────────────────────────────────

/// Build the user message for the cron decision path.
pub fn build_decision_user_message(market_context: &str, display_name: &str) -> String {
    format!(
        "{market_context}\n\n请以【{display_name}】身份，根据策略体系分析当前市场状态和持仓情况。你可以调用 get_current_positions 查询持仓，调用 get_historical_context 查询历史数据。给出今日复盘和明日操作建议。\n\n\
        ⚠️ 调用 submit_trading_decision 时，每条交易指令的 `symbol` 必须是 Yahoo Finance 风格带市场后缀的代码：\n\
        - 沪市（6 开头）→ `.SS`，例如 600519.SS、688981.SS\n\
        - 深市（0 或 3 开头）→ `.SZ`，例如 000001.SZ、002008.SZ、300750.SZ\n\
        - 北交所（4/8/9 开头）→ `.BJ`，例如 873169.BJ\n\
        禁止输出裸 6 位代码（如 002008），否则后续行情拉取会失败。"
    )
}

/// Aggressive-mode rules appended to the user message.
///
/// Enforces: must be fully invested daily, every decision must produce concrete
/// buy/sell/hold instructions. No "wait and see" or "观望" allowed.
pub const AGGRESSIVE_MODE_USER_PROMPT: &str = "\n\n⚠️ 当前为**激进模式**，必须严格遵守：\n\
1. 每天必须满仓，资金不可闲置。\n\
2. 必须输出具体的 buy/sell/hold 指令，禁止\"观望\"。\n\
3. 有可用资金必须买入最强标的。\n\
4. 持仓走弱必须果断卖出换股。\n\
5. 单票不超过总资金30%。\n\
6. 每次至少一条买入或卖出指令。";

// ── Web chat ───────────────────────────────────────

/// Market data snapshot used to build the web chat user message.
#[derive(Debug, Clone)]
pub struct WebChatMarketData<'a> {
    pub date: &'a str,
    pub rise: u32,
    pub fall: u32,
    pub limit_up: u32,
    pub limit_down: u32,
    pub bomb_rate: f64,
    pub risk_level: &'a str,
    pub summary: &'a str,
    pub field_items: &'a str,
    pub super_stocks: &'a str,
}

/// Build the user message for the web chat endpoint (richer format with risk analysis).
pub fn build_web_chat_user_message(data: &WebChatMarketData<'_>) -> String {
    let ratio = f64::from(data.rise) / f64::from(data.fall.max(1));
    format!(
        r"## 当前市场数据
- 日期：{date}
- 上涨家数：{rise} 家
- 下跌家数：{fall} 家
- 涨停家数：{limit_up} 只
- 跌停家数：{limit_down} 只
- 炸板率：{bomb_rate:.1}%
- 涨跌比：{ratio:.2}
- 风险等级：{risk_level}
- 市场摘要：{summary}
{field_items}{super_stocks}
请在回答时结合以上实时市场数据，帮助用户理解当前市场状态。",
        date = data.date,
        rise = data.rise,
        fall = data.fall,
        limit_up = data.limit_up,
        limit_down = data.limit_down,
        bomb_rate = data.bomb_rate,
        ratio = ratio,
        risk_level = data.risk_level,
        summary = data.summary,
        field_items = data.field_items,
        super_stocks = data.super_stocks,
    )
}

// ── Replay analysis ─────────────────────────────────

/// Build the replay analysis system prompt for the web SSE endpoint.
pub fn build_replay_analysis_prompt(date: &str, data_table: &str) -> String {
    format!(
        r"你是A股短线复盘专家。以下是{date}全天5分钟级别行情快照数据：

{data_table}

请基于以上数据输出结构化复盘报告，严格遵循以下格式：

## 一、情绪节奏
- 开盘情绪（09:30-10:00）：情绪温度变化趋势，多空比特征
- 盘中变化（10:00-14:00）：关键转折点及时间
- 尾盘走势（14:00-15:00）：收盘前情绪动向
- 全天趋势判断：单边多/单边空/震荡/反转

## 二、主线板块
- 开盘主线（前30分钟最强板块）
- 盘中切换（如有新板块崛起，时间及原因）
- 尾盘赢家（全天累计涨幅最大板块）
- 主线持续性评价

## 三、关键节点
- 列出2-3个情绪突变时间点，分析可能原因

## 四、轮动特征
- 资金流向判断
- 高低切换迹象

## 五、复盘结论
- 日内最优策略
- 次日关注方向
- 主要风险提示

格式要求：纯 Markdown，禁止 emoji/字符画/Unicode 图表，数据引用用数字。"
    )
}

// ── Extraction ─────────────────────────────────────

pub const EXTRACTION_SYSTEM_PROMPT: &str =
    "Extract the trading decision data from the analysis text below. The analysis contains market analysis and trade instructions that need to be structured.\n\n\
    IMPORTANT: For each trade instruction's `symbol` field, use Yahoo Finance style with market suffix:\n\
    - Shanghai (codes starting with 6): append `.SS` (e.g. 600519.SS, 688981.SS)\n\
    - Shenzhen (codes starting with 0 or 3): append `.SZ` (e.g. 000001.SZ, 002008.SZ, 300750.SZ)\n\
    - Beijing (codes starting with 4, 8, or 9): append `.BJ` (e.g. 873169.BJ)\n\
    Never emit a bare 6-digit code. If the source text mentions a bare code, infer the suffix from the leading digit.";

// ── Historical context ────────────────────────────

/// Build the historical context text shown to the LLM as tool output.
pub fn build_historical_context(days: u32, snapshots: &[MarketSnapshot]) -> String {
    if snapshots.is_empty() {
        return "暂无历史数据".to_string();
    }
    let mut ctx = format!("## 近{days}日市场数据快照 ({}条记录)\n\n", snapshots.len());
    for s in snapshots {
        let _ = writeln!(
            ctx,
            "- {} | {} | {}",
            s.captured_at, s.data_source, &s.raw_json
        );
    }
    ctx
}
