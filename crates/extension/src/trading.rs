//! Market data extension — A-share trading tools for the agent.
//!
//! This extension registers a set of A-share market-data tools (limit-up
//! boards, realtime quotes, K-line, dragon-tiger list, news telegraph, …)
//! and dispatches their execution to the `openatrading-api` clients via the
//! `ExtensionAPI::handle_tool_call` bridge.
//!
//! The agent runtime collects the registered [`ToolDefinition`]s through
//! `ExtensionRegistry::collect_tools` and routes tool calls whose name
//! matches a registered tool to `handle_tool_call`. The first extension that
//! returns `Some(ToolCallOutput)` wins, so each tool name below is unique.

use async_trait::async_trait;
use pi_extension_api::{
    ExtensionAPI, ExtensionContext, ExtensionEvent, ToolCallOutput, ToolDefinition, ToolRegistry,
};
use serde_json::{json, Value};

use openatrading_api::cls::ClsClient;
use openatrading_api::eastmoney::LhbClient;
use openatrading_api::jiuyangongshe::{CommunityQuery, JiuYanSession};
use openatrading_api::sina::SinaQuoteClient;
use openatrading_api::utils;
use openatrading_api::xuangubao::{KlinePeriod, XuanguBaoClient};

/// Market data extension providing A-share trading tools.
pub struct MarketDataExtension;

impl MarketDataExtension {
    pub fn new() -> Self {
        Self
    }

    /// Build the full list of tool definitions this extension provides.
    fn tool_definitions() -> Vec<ToolDefinition> {
        vec![
            limit_up_tool(),
            limit_down_tool(),
            super_stock_tool(),
            market_overview_tool(),
            yesterday_limit_up_tool(),
            dapan_abnormal_tool(),
            realtime_quote_tool(),
            plates_by_stock_tool(),
            market_trend_tool(),
            plate_abnormal_tool(),
            kline_tool(),
            telegraph_tool(),
            telegraph_allday_tool(),
            community_tool(),
            all_day_events_tool(),
            auction_quotes_tool(),
            lhb_detail_tool(),
            lhb_combined_tool(),
            lhb_active_yyb_tool(),
        ]
    }

    /// Dispatch a tool call to the matching handler.
    ///
    /// Returns `Some(output)` when `tool_name` is handled by this extension,
    /// `None` otherwise (so other extensions / built-ins can handle it).
    async fn dispatch(&self, tool_name: &str, params: Value) -> Option<ToolCallOutput> {
        let result: Result<String, String> = match tool_name {
            "get_limit_up" => {
                call(|| async { XuanguBaoClient::new().fetch_limit_up().await.map(to_json) }).await
            },
            "get_limit_down" => {
                call(|| async { XuanguBaoClient::new().fetch_limit_down().await.map(to_json) })
                    .await
            },
            "get_super_stock" => {
                call(|| async {
                    XuanguBaoClient::new()
                        .fetch_super_stock()
                        .await
                        .map(to_json)
                })
                .await
            },
            "get_market_overview" => {
                call(|| async {
                    XuanguBaoClient::new()
                        .fetch_market_overview()
                        .await
                        .map(to_json)
                })
                .await
            },
            "get_yesterday_limit_up" => {
                call(|| async {
                    XuanguBaoClient::new()
                        .fetch_yesterday_limit_up()
                        .await
                        .map(to_json)
                })
                .await
            },
            "get_dapan_abnormal" => {
                call(|| async {
                    XuanguBaoClient::new()
                        .fetch_dapan_abnormal()
                        .await
                        .map(to_json)
                })
                .await
            },
            "get_realtime_quote" => {
                let symbol = str_arg(&params, "symbol");
                call(|| async move {
                    XuanguBaoClient::new()
                        .fetch_realtime(&symbol)
                        .await
                        .map(to_json)
                })
                .await
            },
            "get_plates_by_stock" => {
                let symbol = str_arg(&params, "symbol");
                call(|| async move {
                    XuanguBaoClient::new()
                        .plates_by_any_stock(&symbol)
                        .await
                        .map(to_json)
                })
                .await
            },
            "get_market_trend" => {
                let symbol = str_arg(&params, "symbol");
                call(|| async move {
                    XuanguBaoClient::new()
                        .fetch_market_trend(&symbol)
                        .await
                        .map(to_json)
                })
                .await
            },
            "get_plate_abnormal" => {
                let count = str_arg(&params, "count");
                call(|| async move {
                    XuanguBaoClient::new()
                        .fetch_plate_abnormal_str(&count)
                        .await
                        .map(to_json)
                })
                .await
            },
            "get_kline" => {
                let symbol = str_arg(&params, "symbol");
                let period = match str_arg(&params, "period").as_str() {
                    "Minute" => KlinePeriod::Minute,
                    "Week" => KlinePeriod::Week,
                    "Month" => KlinePeriod::Month,
                    _ => KlinePeriod::Day,
                };
                let count = params
                    .get("count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(100)
                    .min(500) as u32;
                call(|| async move {
                    XuanguBaoClient::new()
                        .fetch_kline(&symbol, period, count)
                        .await
                        .map(to_json)
                })
                .await
            },
            "get_telegraph" => {
                let category = str_arg(&params, "category");
                let cat = if category.is_empty() {
                    ""
                } else {
                    category.as_str()
                };
                call(|| async move {
                    ClsClient::new()
                        .fetch_telegraph(cat, None, 20)
                        .await
                        .map(to_json)
                })
                .await
            },
            "get_telegraph_allday" => {
                call(|| async {
                    ClsClient::new()
                        .fetch_telegraph("", None, 100)
                        .await
                        .map(to_json)
                })
                .await
            },
            "get_community" => {
                let category_id = str_arg(&params, "category_id");
                let article_type = params
                    .get("article_type")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i32;
                let page = params.get("page").and_then(|v| v.as_i64()).unwrap_or(1) as i32;
                let page_size = params
                    .get("page_size")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(20)
                    .min(50) as i32;
                let query = CommunityQuery {
                    category_id,
                    article_type,
                    page,
                    page_size,
                };
                call(|| async move {
                    JiuYanSession::new()
                        .fetch_community(query)
                        .await
                        .map(to_json)
                })
                .await
            },
            "get_all_day_events" => {
                call(|| async {
                    XuanguBaoClient::new()
                        .fetch_all_day_events(0.03)
                        .await
                        .map(to_json)
                })
                .await
            },
            "get_auction_quotes" => {
                let codes_str = str_arg(&params, "codes");
                let codes: Vec<&str> = codes_str
                    .split(',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .collect();
                if codes.is_empty() {
                    Err("codes 不能为空".to_string())
                } else {
                    match SinaQuoteClient::new() {
                        Ok(client) => {
                            call(|| async move { client.fetch_quotes(&codes).await.map(to_json) })
                                .await
                        },
                        Err(e) => Err(e.to_string()),
                    }
                }
            },
            "get_lhb_detail" => {
                let start_date = str_arg(&params, "start_date");
                let end_date = str_arg(&params, "end_date");
                call(|| async move {
                    LhbClient::new()
                        .lhb_detail(&start_date, &end_date)
                        .await
                        .map(to_json)
                })
                .await
            },
            "get_lhb_combined" => {
                let start_date = str_arg(&params, "start_date");
                let end_date = str_arg(&params, "end_date");
                call(|| async move {
                    LhbClient::new()
                        .lhb_combined(&start_date, &end_date)
                        .await
                        .map(to_json)
                })
                .await
            },
            "get_lhb_active_yyb" => {
                let start_date = str_arg(&params, "start_date");
                let end_date = str_arg(&params, "end_date");
                call(|| async move {
                    LhbClient::new()
                        .lhb_active_yyb(&start_date, &end_date)
                        .await
                        .map(to_json)
                })
                .await
            },
            _ => return None,
        };

        Some(match result {
            Ok(text) => ToolCallOutput {
                content: vec![json!({ "type": "text", "text": text })],
                details: None,
                is_error: false,
            },
            Err(err) => {
                tracing::warn!(tool = tool_name, error = %err, "market tool failed");
                ToolCallOutput {
                    content: vec![json!({ "type": "text", "text": err })],
                    details: None,
                    is_error: true,
                }
            },
        })
    }
}

impl Default for MarketDataExtension {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExtensionAPI for MarketDataExtension {
    fn name(&self) -> &'static str {
        "openatrading-market"
    }

    fn register_tools(&self, registry: &mut ToolRegistry) {
        for def in Self::tool_definitions() {
            registry.register(def);
        }
    }

    async fn on_event(
        &self,
        _event: &ExtensionEvent,
        _ctx: &ExtensionContext,
    ) -> Option<pi_extension_api::EventResult> {
        None
    }

    async fn handle_tool_call(
        &self,
        tool_name: &str,
        params: Value,
        _ctx: &ExtensionContext,
    ) -> Option<ToolCallOutput> {
        self.dispatch(tool_name, params).await
    }
}

// ─── Tool definition builders ───────────────────────────────────────────────

fn limit_up_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_limit_up".into(),
        label: Some("涨停板".into()),
        description: "获取今日涨停股票完整列表。每只股票包含：代码(symbol)、名称(stock_chi_name)、现价、涨跌幅、\
            连板天数(limit_up_days)、首次/最后封板时间、炸板次数、封单量比、换手率、量比、涨停原因(surge_reason)等。\
            用法：与 get_yesterday_limit_up 对比得出连板晋级率，构建连板天梯，计算炸板率。".into(),
        prompt_guidelines: Some(vec![
            "盘前/盘中调用以判断市场连板强度与封板质量".into(),
            "结合 get_yesterday_limit_up 计算晋级率（>50% 强势，<30% 退潮）".into(),
        ]),
        parameters: Some(empty_schema()),
        ..Default::default()
    }
}

fn limit_down_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_limit_down".into(),
        label: Some("跌停板".into()),
        description:
            "获取今日跌停股票列表。包含代码、名称、连跌天数(limit_down_days)、跌停封单数据等。\
            用法：判断市场亏钱效应与恐慌程度——跌停数/昨日涨停数 = 亏钱效应比率。"
                .into(),
        prompt_guidelines: Some(vec!["用于识别退潮与系统性风险信号".into()]),
        parameters: Some(empty_schema()),
        ..Default::default()
    }
}

fn super_stock_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_super_stock".into(),
        label: Some("强势股".into()),
        description: "获取强势股列表（未涨停但走势强劲的个股）。数据结构与涨停列表相同。\
            用法：结合 get_realtime_quote 判断是否有封板潜力（换手、量比、涨幅）。"
            .into(),
        prompt_guidelines: Some(vec!["用于发现潜在涨停候选与盘中异动".into()]),
        parameters: Some(empty_schema()),
        ..Default::default()
    }
}

fn market_overview_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_market_overview".into(),
        label: Some("市场全景".into()),
        description:
            "获取市场全景概览：上涨家数、下跌家数、涨停家数、跌停家数、炸板率、市场温度(0~100)、\
            昨日涨停今日平均涨幅。用法：首先调用，快速判断市场整体强弱——温度>60且炸板率<0.2为强势。"
                .into(),
        prompt_guidelines: Some(vec!["每次市场分析的第一步，用于定调整体情绪".into()]),
        parameters: Some(empty_schema()),
        ..Default::default()
    }
}

fn yesterday_limit_up_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_yesterday_limit_up".into(),
        label: Some("昨日涨停".into()),
        description:
            "获取昨日涨停股票列表及今日表现。用法：与今日涨停对比计算连板晋级率、构建连板天梯、\
            判断亏钱效应与溢价率。"
                .into(),
        prompt_guidelines: Some(vec!["用于连板梯队分析与晋级率计算".into()]),
        parameters: Some(empty_schema()),
        ..Default::default()
    }
}

fn dapan_abnormal_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_dapan_abnormal".into(),
        label: Some("大盘异动".into()),
        description: "获取大盘异动事件（全市场级别的异常波动）。包含异动类型、触发板块、时间戳。\
            用法：检测系统性风险——大盘异动频发 = 市场不稳定，应降低仓位或回避。"
            .into(),
        prompt_guidelines: Some(vec!["用于识别系统性风险".into()]),
        parameters: Some(empty_schema()),
        ..Default::default()
    }
}

fn realtime_quote_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_realtime_quote".into(),
        label: Some("实时行情".into()),
        description: "获取单只股票的实时行情：现价、涨跌幅、开高低、振幅、换手率、量比、市值、市盈率、市净率。\
            用法：查看涨停候选股的实时盘口——换手率适中(5-15%)+量比>2 = 活跃。".into(),
        prompt_guidelines: Some(vec!["分析个股盘口强弱时使用".into()]),
        parameters: Some(str_schema(&[(
            "symbol",
            "股票代码，如 600519.SS",
        )])),
        ..Default::default()
    }
}

fn plates_by_stock_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_plates_by_stock".into(),
        label: Some("个股所属板块".into()),
        description: "获取指定股票所属的所有概念/行业板块，及每个板块的当前涨跌幅。\
            用法：判断涨停股的板块效应——所属板块涨幅>2%且有其他涨停股 = 板块助攻。"
            .into(),
        prompt_guidelines: Some(vec!["判断板块效应与主线持续性".into()]),
        parameters: Some(str_schema(&[("symbol", "股票代码，如 600519.SS")])),
        ..Default::default()
    }
}

fn market_trend_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_market_trend".into(),
        label: Some("分时走势".into()),
        description: "获取个股分时走势数据（日内价格+成交量序列）。\
            用法：分析盘中走势形态——均价线上方运行 = 强势；多次回踩均价线不破 = 有支撑。"
            .into(),
        prompt_guidelines: Some(vec!["分析日内走势形态时使用".into()]),
        parameters: Some(str_schema(&[("symbol", "股票代码，如 600519.SS")])),
        ..Default::default()
    }
}

fn plate_abnormal_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_plate_abnormal".into(),
        label: Some("板块异动".into()),
        description: "获取板块异动事件：板块涨跌幅、异动类型、关联个股、利好/利空方向判断。\
            用法：发现资金攻击方向——连续多条同一板块异动 = 资金抱团，可能形成主线。"
            .into(),
        prompt_guidelines: Some(vec!["用于发现资金主线与轮动节奏".into()]),
        parameters: Some(str_schema(&[("count", "返回条数，默认30")])),
        ..Default::default()
    }
}

fn kline_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_kline".into(),
        label: Some("K线".into()),
        description: "获取个股历史K线数据（OHLCV）。支持日线、周线、月线、分钟线。\
            每根K线包含：时间戳、开高低收、成交量、涨跌幅。用法：查看技术形态——突破前高、均线支撑、量价配合。".into(),
        prompt_guidelines: Some(vec!["技术形态分析时使用，周期参数用枚举值".into()]),
        parameters: Some(str_schema(&[
            ("symbol", "股票代码，如 600519.SS"),
            (
                "period",
                "K线周期：Minute(分钟), Day(日线), Week(周线), Month(月线)",
            ),
            ("count", "返回K线数量，默认100"),
        ])),
        ..Default::default()
    }
}

fn telegraph_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_telegraph".into(),
        label: Some("财联社电报".into()),
        description:
            "获取财联社分类电报（最新N条）。category 可选值：\"\"(全部)、\"stock\"(股市)、\
            \"macro\"(宏观)、\"industry\"(行业)、\"company\"(公司)。用法：扫描突发新闻与政策消息。"
                .into(),
        prompt_guidelines: Some(vec!["捕捉突发新闻与事件驱动".into()]),
        parameters: Some(str_schema(&[(
            "category",
            "分类，可选 '' 'stock' 'macro' 'industry' 'company'",
        )])),
        ..Default::default()
    }
}

fn telegraph_allday_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_telegraph_allday".into(),
        label: Some("全天电报".into()),
        description: "获取财联社全天电报（所有分类，不限条数）。用法：盘后复盘时获取全天重要消息，按时间线梳理事件脉络。".into(),
        prompt_guidelines: Some(vec!["盘后复盘梳理全天事件".into()]),
        parameters: Some(empty_schema()),
        ..Default::default()
    }
}

fn community_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_community".into(),
        label: Some("韭研公社".into()),
        description: "获取韭研公社社群文章动态。包含标题、内容概要、作者、关联股票、点赞/评论/收藏/转发数、发布时间。\
            用法：捕捉社群对特定板块/热点的讨论热度与潜在热点标的。".into(),
        prompt_guidelines: Some(vec!["衡量社群情绪与热点讨论度".into()]),
        parameters: Some(str_schema(&[
            ("category_id", "文章分类ID，默认空字符串获取所有分类"),
            (
                "article_type",
                "文章类型：0=所有类型、1=观点、2=资讯，默认0",
            ),
            ("page", "页码，从1开始，默认1"),
            ("page_size", "每页返回条数，默认20，最大50"),
        ])),
        ..Default::default()
    }
}

fn all_day_events_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_all_day_events".into(),
        label: Some("全天异动".into()),
        description: "获取今日盘中全量异动事件（板块异动+个股异动），按时间倒序排列。\
            用法：盘后/盘中复盘时梳理全天异动时间线，结合涨停数据分析「异动→封板」转化率。"
            .into(),
        prompt_guidelines: Some(vec!["复盘全天异动时间线".into()]),
        parameters: Some(empty_schema()),
        ..Default::default()
    }
}

fn auction_quotes_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_auction_quotes".into(),
        label: Some("集合竞价".into()),
        description: "获取指定股票的集合竞价实时数据（9:15-9:25）。每只股票包含：代码、名称、前收盘价、\
            竞价撮合价、涨跌幅、买卖一价量、累计撮合量额、数据时间。用法：竞价期判断开盘方向，支持批量查询。".into(),
        prompt_guidelines: Some(vec!["竞价期(9:15-9:25)调用判断开盘方向".into()]),
        parameters: Some(str_schema(&[(
            "codes",
            "逗号分隔的股票代码，如 000001,600519",
        )])),
        ..Default::default()
    }
}

fn lhb_detail_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_lhb_detail".into(),
        label: Some("龙虎榜明细".into()),
        description: "获取龙虎榜明细数据。包含上榜股票、买入/卖出营业部、买卖金额、净买入等。\
            用法：判断主力动向——知名游资买入 = 短线看多信号。日期格式 YYYYMMDD。"
            .into(),
        prompt_guidelines: Some(vec!["分析主力与游资动向".into()]),
        parameters: Some(str_schema(&[
            ("start_date", "起始日期，如 20260615"),
            ("end_date", "结束日期，如 20260615"),
        ])),
        ..Default::default()
    }
}

fn lhb_combined_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_lhb_combined".into(),
        label: Some("龙虎榜合并".into()),
        description:
            "获取龙虎榜合并数据（按股票汇总买卖席位）。直接显示每只股票的总买入额、总卖出额、\
            净买入额、上榜营业部数量。用法：快速扫描当日龙虎榜——净买入前10、知名席位动向。"
                .into(),
        prompt_guidelines: Some(vec!["快速扫描龙虎榜净买入排名".into()]),
        parameters: Some(str_schema(&[
            ("start_date", "起始日期，如 20260615"),
            ("end_date", "结束日期，如 20260615"),
        ])),
        ..Default::default()
    }
}

fn lhb_active_yyb_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_lhb_active_yyb".into(),
        label: Some("活跃营业部".into()),
        description:
            "获取近期活跃营业部排名。包含营业部名称、上榜次数、买入总额、卖出总额、跟投胜率等。\
            用法：识别当前活跃的游资席位——跟投胜率高的席位 = 正向跟随信号。日期格式 YYYYMMDD。"
                .into(),
        prompt_guidelines: Some(vec!["识别活跃游资席位与跟随信号".into()]),
        parameters: Some(str_schema(&[
            ("start_date", "起始日期，如 20260615"),
            ("end_date", "结束日期，如 20260615"),
        ])),
        ..Default::default()
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Run an async API call, converting success into a JSON string and errors
/// into a display string (so the tool result carries the error text).
async fn call<F, Fut, T, E>(f: F) -> Result<String, String>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    T: serde::Serialize,
    E: std::fmt::Display,
{
    f().await.map(to_json).map_err(|e| e.to_string())
}

/// Serialize an API result to a compact JSON string for the tool output.
fn to_json<T: serde::Serialize>(value: T) -> String {
    serde_json::to_string(&value).unwrap_or_else(|_| "[]".to_string())
}

fn str_arg(params: &Value, name: &str) -> String {
    params
        .get(name)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn empty_schema() -> Value {
    json!({ "type": "object", "properties": {}, "required": [] })
}

fn str_schema(params: &[(&str, &str)]) -> Value {
    let properties: serde_json::Map<String, Value> = params
        .iter()
        .map(|(name, desc)| {
            (
                (*name).to_string(),
                json!({ "type": "string", "description": desc }),
            )
        })
        .collect();
    let required: Vec<String> = params.iter().map(|(n, _)| (*n).to_string()).collect();
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
    })
}

// Keep the date helper referenced so the extension can be extended with
// date-scoped tools without an unused-import warning.
#[allow(dead_code)]
fn today_beijing() -> String {
    utils::beijing_now().format("%Y-%m-%d").to_string()
}
