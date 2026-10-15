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
