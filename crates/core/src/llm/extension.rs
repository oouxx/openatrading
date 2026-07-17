//! Trading tool extension — wraps market data tools as pi-coding-agent extensions.
//!
//! Tools are registered via `ExtensionAPI::register_tools()` and executed
//! via `ExtensionAPI::handle_tool_call()`. The extension registry is passed
//! to `create_agent_session()` through `CreateAgentSessionOptions.extension_registry`.

use async_trait::async_trait;
use pi_coding_agent::core::extensions::{
    ExtensionAPI, ExtensionContext, ExtensionRegistry, ToolCallOutput, ToolDefinition, ToolRegistry,
};
use serde_json::Value;

// ── TradingToolExtension ────────────────────────────────────────────────

/// Extension that provides A-share market data tools to the agent.
pub struct TradingToolExtension;

impl Default for TradingToolExtension {
    fn default() -> Self {
        Self::new()
    }
}

impl TradingToolExtension {
    pub fn new() -> Self {
        Self
    }

    /// Create an ExtensionRegistry with this extension registered.
    pub fn create_registry() -> ExtensionRegistry {
        let mut registry = ExtensionRegistry::new();
        registry.register(Box::new(Self::new()));
        registry
    }
}

#[async_trait]
impl ExtensionAPI for TradingToolExtension {
    fn name(&self) -> &'static str {
        "openatrading-market"
    }

    fn register_tools(&self, registry: &mut ToolRegistry) {
        // ── 涨停 & 强势股 ──
        registry.register(tool_def(
            "get_limit_up",
            "获取今日涨停股票完整列表。每只股票包含：代码(symbol)、名称(stock_chi_name)、现价、涨跌幅、\
            连板天数(limit_up_days)、今日首次封板时间(first_limit_up, Unix秒)、最后封板时间(last_limit_up)、\
            炸板次数(break_limit_up_times)、首次炸板时间(first_break_limit_up)、最后炸板时间(last_break_limit_up)、\
            昨日连板天数(yesterday_limit_up_days)、封单量比(buy_lock_volume_ratio)、卖单量比(sell_lock_volume_ratio)、\
            换手率(turnover_ratio)、量比(volume_bias_ratio)、M天N板数据(m_days_n_boards_boards/days)、\
            涨停原因(surge_reason)等。",
            empty_schema(),
        ));
        registry.register(tool_def(
            "get_super_stock",
            "获取强势股列表（未涨停但走势强劲的个股）。用于发现潜在涨停候选、\
            识别盘中异动但尚未封板的股票。",
            empty_schema(),
        ));
        registry.register(tool_def(
            "get_limit_down",
            "获取今日跌停股票列表。包含代码、名称、连跌天数(limit_down_days)、\
            跌停封单数据等。用于判断市场亏钱效应、恐慌程度。",
            empty_schema(),
        ));

        // ── 市场全景 ──
        registry.register(tool_def(
            "get_market_overview",
            "获取市场全景概览：上涨家数(rise_count)、下跌家数(fall_count)、\
            涨停家数(limit_up_count)、跌停家数(limit_down_count)、\
            炸板率(bomb_rate, 0~1)、市场温度(market_temperature, 0~100)、\
            昨日涨停今日平均涨幅(yesterday_limit_up_avg_pcp)。",
            empty_schema(),
        ));
        registry.register(tool_def(
            "get_yesterday_limit_up",
            "获取昨日涨停股票列表及今日表现。每只包含：昨日连板天数(yesterday_limit_up_days)、\
            今日是否继续涨停、今日涨跌幅等。",
            empty_schema(),
        ));
        registry.register(tool_def(
            "get_dapan_abnormal",
            "获取大盘异动事件（全市场级别的异常波动）。包含异动类型、触发板块、时间戳。",
            empty_schema(),
        ));

        // ── 实时行情 ──
        registry.register(tool_def(
            "get_realtime_quote",
            "获取单只股票的实时行情：现价(last_px)、涨跌幅(px_change_rate)、开盘价(open_px)、\
            最高价(high_px)、最低价(low_px)、振幅(amplitude)、换手率(turnover_ratio)、\
            量比(volume_ratio)、市值(market_value)、流通市值(circulation_value)、\
            成交额(turnover_value)、市盈率(pe_rate/dyn_pe)、市净率(dyn_pb_rate)。",
            str_schema(&[("symbol", "股票代码，如 600519.SS")]),
        ));
        registry.register(tool_def(
            "get_plates_by_stock",
            "获取指定股票所属的所有概念/行业板块，及每个板块的当前涨跌幅。",
            str_schema(&[("symbol", "股票代码，如 600519.SS")]),
        ));
        registry.register(tool_def(
            "get_market_trend",
            "获取个股分时走势数据（日内价格+成交量序列）。",
            str_schema(&[("symbol", "股票代码，如 600519.SS")]),
        ));

        // ── 板块异动 ──
        registry.register(tool_def(
            "get_plate_abnormal",
            "获取板块异动事件：板块涨跌幅、异动类型、关联个股、利好/利空方向判断。",
            str_schema(&[("count", "返回条数，默认30")]),
        ));

        // ── K线 ──
        registry.register(tool_def(
            "get_kline",
            "获取个股历史K线数据（OHLCV）。支持日线、周线、月线、分钟线。\
            每根K线包含：时间戳(tick_at)、开盘价(open_px)、收盘价(close_px)、\
            最高价(high_px)、最低价(low_px)、成交量(turnover_volume)、涨跌幅(px_change_rate)。",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "symbol": {"type": "string", "description": "股票代码，如 600519.SS"},
                    "period": {"type": "string", "description": "K线周期：Minute(分钟), Day(日线), Week(周线), Month(月线)", "enum": ["Minute", "Day", "Week", "Month"]},
                    "count": {"type": "integer", "description": "返回K线数量，默认100"}
                },
                "required": ["symbol"]
            }),
        ));

        // ── 新闻 & 电报 ──
        registry.register(tool_def(
            "get_field_items",
            "获取韭研公社今日异动板块数据。包含板块名称、异动信号、关联个股、趋势判断等。",
            empty_schema(),
        ));
        registry.register(tool_def(
            "get_telegraph",
            "获取财联社分类电报（最新N条）。参数 category 可选值：\
            \"\" (全部), \"stock\" (股市), \"macro\" (宏观), \"industry\" (行业), \"company\" (公司)。",
            str_schema(&[("category", "分类，可选 '' 'stock' 'macro' 'industry' 'company'")]),
        ));
        registry.register(tool_def(
            "get_telegraph_allday",
            "获取财联社全天电报（所有分类，不限条数）。盘后复盘时获取全天重要消息。",
            empty_schema(),
        ));

        // ── 社群文章 ──
        registry.register(tool_def(
            "get_community",
            "获取韭研公社社群文章动态。包含文章标题、内容概要、作者信息(nickname)、\
            关联股票(stock_list, 含代码和名称)、点赞数、评论数、收藏数、转发数、发布时间。",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "category_id": {"type": "string", "description": "文章分类ID，默认空字符串获取所有分类"},
                    "article_type": {"type": "integer", "description": "文章类型：0=所有类型、1=观点、2=资讯，默认0"},
                    "page": {"type": "integer", "description": "页码，从1开始，默认1"},
                    "page_size": {"type": "integer", "description": "每页返回条数，默认20，最大50"}
                },
                "required": []
            }),
        ));

        // ── 龙虎榜 ──
        registry.register(tool_def(
            "get_lhb_detail",
            "获取龙虎榜明细数据。包含上榜股票、买入/卖出营业部、买卖金额、净买入等。",
            str_schema(&[
                ("start_date", "起始日期，如 20260615"),
                ("end_date", "结束日期，如 20260615"),
            ]),
        ));
        registry.register(tool_def(
            "get_lhb_combined",
            "获取龙虎榜合并数据（按股票汇总买卖席位）。比明细更简洁，\
            直接显示每只股票的总买入额、总卖出额、净买入额、上榜营业部数量。",
            str_schema(&[
                ("start_date", "起始日期，如 20260615"),
                ("end_date", "结束日期，如 20260615"),
            ]),
        ));
        registry.register(tool_def(
            "get_lhb_active_yyb",
            "获取近期活跃营业部排名。包含营业部名称、上榜次数、买入总额、\
            卖出总额、跟投胜率等。",
            str_schema(&[
                ("start_date", "起始日期，如 20260615"),
                ("end_date", "结束日期，如 20260615"),
            ]),
        ));

        // ── 全量异动 ──
        registry.register(tool_def(
            "get_all_day_events",
            "获取今日盘中全量异动事件（板块异动+个股异动），按时间倒序排列。",
            empty_schema(),
        ));

        // ── 竞价数据 ──
        registry.register(tool_def(
            "get_auction_quotes",
            "获取指定股票的集合竞价实时数据（9:15-9:25）。每只股票包含：代码(code)、名称(name)、\
            前收盘价(prev_close)、竞价撮合价(current_price)、涨跌幅(change_pct)、\
            买一价(bid1_price)、买一量(bid1_volume, 手)、卖一价(ask1_price)、卖一量(ask1_volume, 手)、\
            累计撮合量(volume, 手)、累计成交额(amount)、数据时间(time)。",
            str_schema(&[("codes", "逗号分隔的股票代码，如 000001,600519")]),
        ));
    }

    async fn handle_tool_call(
        &self,
        _tool_name: &str,
        _params: Value,
        _ctx: &ExtensionContext,
    ) -> Option<ToolCallOutput> {
        // All trading tools are stateless — they create fresh API clients per call.
        // The actual execution is handled by the DynTool instances in the agent's tool list.
        // This handler is a fallback for when the extension system intercepts the call.
        None
    }
}

// ── Helpers ────────────────────────────────────────────────────────────

fn tool_def(name: &str, description: &str, parameters: Value) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        label: None,
        description: description.to_string(),
        prompt_snippet: None,
        prompt_guidelines: None,
        parameters: Some(parameters),
        render_shell: None,
        execution_mode: None,
    }
}

fn empty_schema() -> Value {
    serde_json::json!({"type": "object", "properties": {}, "required": []})
}

fn str_schema(params: &[(&str, &str)]) -> Value {
    let properties: serde_json::Map<String, Value> = params
        .iter()
        .map(|(name, desc)| {
            (
                (*name).to_string(),
                serde_json::json!({"type": "string", "description": desc}),
            )
        })
        .collect();
    let required: Vec<String> = params.iter().map(|(n, _)| (*n).to_string()).collect();
    serde_json::json!({
        "type": "object",
        "properties": properties,
        "required": required,
    })
}
