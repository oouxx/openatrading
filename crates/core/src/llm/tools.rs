use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use openatrading_agent::pi_ai_types::ContentBlock;
use openatrading_agent::types::{AgentTool, AgentToolResult, DynTool};

use crate::api::cls::ClsClient;
use crate::api::eastmoney::LhbClient;
use crate::api::jiuyangongshe::JiuYanSession;
use crate::api::xuangubao::{KlinePeriod, XuanguBaoClient};
use crate::memory::MemoryStore;
use crate::trade::TradeManager;

type ExecuteFn = Arc<
    dyn Fn(
            String,
            serde_json::Value,
            Option<tokio::sync::watch::Receiver<bool>>,
            Option<Arc<dyn Fn(AgentToolResult<serde_json::Value>) + Send + Sync>>,
        ) -> Pin<
            Box<
                dyn Future<
                        Output = Result<
                            AgentToolResult<serde_json::Value>,
                            Box<dyn Error + Send + Sync>,
                        >,
                    > + Send,
            >,
        > + Send
        + Sync,
>;

fn empty_schema() -> serde_json::Value {
    serde_json::json!({"type": "object", "properties": {}, "required": []})
}

fn str_schema(params: &[(&str, &str)]) -> serde_json::Value {
    let properties: serde_json::Map<String, serde_json::Value> = params
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

fn str_arg(args: &serde_json::Value, name: &str) -> String {
    args.get(name)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn text_result<T: serde::Serialize>(value: &T) -> AgentToolResult<serde_json::Value> {
    AgentToolResult {
        content: vec![ContentBlock::text(
            serde_json::to_string(value).unwrap_or_default(),
        )],
        details: serde_json::Value::Null,
        terminate: None,
    }
}

fn to_box_err(e: impl std::fmt::Display) -> Box<dyn Error + Send + Sync> {
    e.to_string().into()
}

fn tool(name: &str, description: &str, schema: serde_json::Value, exec: ExecuteFn) -> Arc<DynTool> {
    Arc::new(AgentTool {
        name: name.into(),
        description: description.into(),
        label: name.into(),
        parameters_schema: schema,
        execution_mode: None,
        prepare_arguments: None,
        execute: exec,
    })
}

// ─── XuanguBao tools ───────────────────────────────────

pub fn get_limit_up() -> Arc<DynTool> {
    tool(
        "get_limit_up",
        "获取当前涨停股票列表，包含股票代码、名称、连板天数、封板时间、炸板次数、涨停原因等详细信息",
        empty_schema(),
        Arc::new(|_id, _args, _signal, _progress| {
            Box::pin(async move {
                let result = XuanguBaoClient::new().fetch_limit_up().await.map_err(to_box_err)?;
                Ok(text_result(&result))
            })
        }),
    )
}

pub fn get_super_stock() -> Arc<DynTool> {
    tool(
        "get_super_stock",
        "获取当前强势股股票列表，包含股票代码、名称、连板天数、封板时间、炸板次数、涨停原因等详细信息",
        empty_schema(),
        Arc::new(|_id, _args, _signal, _progress| {
            Box::pin(async move {
                let result = XuanguBaoClient::new().fetch_super_stock().await.map_err(to_box_err)?;
                Ok(text_result(&result))
            })
        }),
    )
}

pub fn get_limit_down() -> Arc<DynTool> {
    tool(
        "get_limit_down",
        "获取当前跌停股票列表",
        empty_schema(),
        Arc::new(|_id, _args, _signal, _progress| {
            Box::pin(async move {
                let result = XuanguBaoClient::new()
                    .fetch_limit_down()
                    .await
                    .map_err(to_box_err)?;
                Ok(text_result(&result))
            })
        }),
    )
}

pub fn get_market_overview() -> Arc<DynTool> {
    tool(
        "get_market_overview",
        "获取当前市场概览数据，包含涨跌家数、涨停跌停数、炸板率、市场温度等",
        empty_schema(),
        Arc::new(|_id, _args, _signal, _progress| {
            Box::pin(async move {
                let result = XuanguBaoClient::new()
                    .fetch_market_overview()
                    .await
                    .map_err(to_box_err)?;
                Ok(text_result(&result))
            })
        }),
    )
}

pub fn get_yesterday_limit_up() -> Arc<DynTool> {
    tool(
        "get_yesterday_limit_up",
        "获取昨日涨停股票列表，用于对比今日涨停判断晋级情况",
        empty_schema(),
        Arc::new(|_id, _args, _signal, _progress| {
            Box::pin(async move {
                let result = XuanguBaoClient::new()
                    .fetch_yesterday_limit_up()
                    .await
                    .map_err(to_box_err)?;
                Ok(text_result(&result))
            })
        }),
    )
}

pub fn get_dapan_abnormal() -> Arc<DynTool> {
    tool(
        "get_dapan_abnormal",
        "获取大盘异动事件列表，包含异动类型、板块信息、时间戳",
        empty_schema(),
        Arc::new(|_id, _args, _signal, _progress| {
            Box::pin(async move {
                let result = XuanguBaoClient::new()
                    .fetch_dapan_abnormal()
                    .await
                    .map_err(to_box_err)?;
                Ok(text_result(&result))
            })
        }),
    )
}

pub fn get_realtime_quote() -> Arc<DynTool> {
    tool(
        "get_realtime_quote",
        "获取指定股票的实时行情数据，包含价格、涨跌幅、换手率、市值、量比、振幅等",
        str_schema(&[("symbol", "股票代码，如 600519.SS")]),
        Arc::new(|_id, args, _signal, _progress| {
            Box::pin(async move {
                let symbol = str_arg(&args, "symbol");
                let result = XuanguBaoClient::new()
                    .fetch_realtime(&symbol)
                    .await
                    .map_err(to_box_err)?;
                Ok(text_result(&result))
            })
        }),
    )
}

pub fn get_plates_by_stock() -> Arc<DynTool> {
    tool(
        "get_plates_by_stock",
        "获取指定股票所属的所有板块及每个板块的涨跌幅",
        str_schema(&[("symbol", "股票代码，如 600519.SS")]),
        Arc::new(|_id, args, _signal, _progress| {
            Box::pin(async move {
                let symbol = str_arg(&args, "symbol");
                let result = XuanguBaoClient::new()
                    .plates_by_any_stock(&symbol)
                    .await
                    .map_err(to_box_err)?;
                Ok(text_result(&result))
            })
        }),
    )
}

pub fn get_market_trend() -> Arc<DynTool> {
    tool(
        "get_market_trend",
        "获取指定股票的日内分时走势数据，用于分析盘中价格变化趋势",
        str_schema(&[("symbol", "股票代码，如 600519.SS")]),
        Arc::new(|_id, args, _signal, _progress| {
            Box::pin(async move {
                let symbol = str_arg(&args, "symbol");
                let result = XuanguBaoClient::new()
                    .fetch_market_trend(&symbol)
                    .await
                    .map_err(to_box_err)?;
                Ok(text_result(&result))
            })
        }),
    )
}

pub fn get_plate_abnormal() -> Arc<DynTool> {
    tool(
        "get_plate_abnormal",
        "获取板块异动事件列表，包含板块涨跌幅、关联个股、利好利空判断，默认返回30条",
        str_schema(&[("count", "返回条数，默认30")]),
        Arc::new(|_id, args, _signal, _progress| {
            Box::pin(async move {
                let count = str_arg(&args, "count");
                let result = XuanguBaoClient::new()
                    .fetch_plate_abnormal_str(&count)
                    .await
                    .map_err(to_box_err)?;
                Ok(text_result(&result))
            })
        }),
    )
}

// ─── K线工具 ────────────────────────────────────────────

fn parse_period(s: &str) -> Result<KlinePeriod, &'static str> {
    match s {
        "minute" => Ok(KlinePeriod::Minute),
        "day" => Ok(KlinePeriod::Day),
        "week" => Ok(KlinePeriod::Week),
        "month" => Ok(KlinePeriod::Month),
        _ => Err("period 必须是 minute/day/week/month 之一"),
    }
}

pub fn get_kline() -> Arc<DynTool> {
    tool(
        "get_kline",
        "获取指定股票的K线数据，支持分钟/日/周/月线，返回开盘价、收盘价、最高价、最低价、成交量、涨跌幅",
        str_schema(&[
            ("symbol", "股票代码，如 600519.SS"),
            ("period", "K线周期：minute(分钟线) / day(日线) / week(周线) / month(月线)"),
            ("count", "返回K线条数，如 20"),
        ]),
        Arc::new(|_id, args, _signal, _progress| {
            Box::pin(async move {
                let symbol = str_arg(&args, "symbol");
                let period = parse_period(&str_arg(&args, "period")).map_err(to_box_err)?;
                let count: u32 = args
                    .get("count")
                    .and_then(|v| v.as_str())
                    .unwrap_or("20")
                    .parse()
                    .map_err(|_| to_box_err("count 必须是数字"))?;
                let result = XuanguBaoClient::new()
                    .fetch_kline(&symbol, period, count)
                    .await
                    .map_err(to_box_err)?;
                Ok(text_result(&result))
            })
        }),
    )
}

// ─── 韭研公社 tools ──────────────────────────────────────

pub fn get_field_items() -> Arc<DynTool> {
    tool(
        "get_field_items",
        "获取韭研公社总结的涨停股，强势股数据以及涨停原因",
        str_schema(&[
            ("date", "日期，格式YYYY-MM-DD，如2025-01-20"),
            ("pc", "页码，默认1"),
        ]),
        Arc::new(|_id, args, _signal, _progress| {
            Box::pin(async move {
                let date = str_arg(&args, "date");
                let pc = str_arg(&args, "pc");
                let result = JiuYanSession::new()
                    .fetch_field_str(&date, &pc)
                    .await
                    .map_err(to_box_err)?;
                Ok(text_result(&result))
            })
        }),
    )
}

// ─── CLS tools ─────────────────────────────────────────

pub fn get_telegraph() -> Arc<DynTool> {
    tool(
        "get_telegraph",
        "获取财联社最新电报资讯，默认返回全部类别。可指定category筛选：red(重要)、watch(关注)等",
        str_schema(&[("category", "资讯类别，留空返回全部")]),
        Arc::new(|_id, args, _signal, _progress| {
            Box::pin(async move {
                let category = str_arg(&args, "category");
                let result = ClsClient::new()
                    .fetch_telegraph_latest(&category)
                    .await
                    .map_err(to_box_err)?;
                Ok(text_result(&result))
            })
        }),
    )
}

pub fn get_telegraph_allday() -> Arc<DynTool> {
    tool(
        "get_telegraph_allday",
        "获取今日全天财联社电报资讯，自动翻页拉取全部数据",
        str_schema(&[("category", "资讯类别，留空返回全部")]),
        Arc::new(|_id, args, _signal, _progress| {
            Box::pin(async move {
                let category = str_arg(&args, "category");
                let result = ClsClient::new()
                    .fetch_telegraph_allday(&category)
                    .await
                    .map_err(to_box_err)?;
                Ok(text_result(&result))
            })
        }),
    )
}

// ─── Eastmoney 龙虎榜 tools ─────────────────────────────

pub fn get_lhb_detail() -> Arc<DynTool> {
    tool(
        "get_lhb_detail",
        "获取龙虎榜上榜个股详情，包含净买入额、买卖金额、涨跌幅、上榜原因、上榜后1/2/5/10日表现",
        str_schema(&[
            ("start_date", "起始日期，格式YYYYMMDD，如20250120"),
            ("end_date", "结束日期，格式YYYYMMDD，如20250120"),
        ]),
        Arc::new(|_id, args, _signal, _progress| {
            Box::pin(async move {
                let start_date = str_arg(&args, "start_date");
                let end_date = str_arg(&args, "end_date");
                let result = LhbClient::new()
                    .lhb_detail(&start_date, &end_date)
                    .await
                    .map_err(to_box_err)?;
                Ok(text_result(&result))
            })
        }),
    )
}

pub fn get_lhb_combined() -> Arc<DynTool> {
    tool(
        "get_lhb_combined",
        "获取龙虎榜合并数据，包含上榜详情+买卖席位营业部明细",
        str_schema(&[
            ("start_date", "起始日期，格式YYYYMMDD，如20250120"),
            ("end_date", "结束日期，格式YYYYMMDD，如20250120"),
        ]),
        Arc::new(|_id, args, _signal, _progress| {
            Box::pin(async move {
                let start_date = str_arg(&args, "start_date");
                let end_date = str_arg(&args, "end_date");
                let result = LhbClient::new()
                    .lhb_combined(&start_date, &end_date)
                    .await
                    .map_err(to_box_err)?;
                Ok(text_result(&result))
            })
        }),
    )
}

pub fn get_lhb_active_yyb() -> Arc<DynTool> {
    tool(
        "get_lhb_active_yyb",
        "获取近期活跃营业部排名，按净买入额排序",
        str_schema(&[
            ("start_date", "起始日期，格式YYYYMMDD，如20250120"),
            ("end_date", "结束日期，格式YYYYMMDD，如20250120"),
        ]),
        Arc::new(|_id, args, _signal, _progress| {
            Box::pin(async move {
                let start_date = str_arg(&args, "start_date");
                let end_date = str_arg(&args, "end_date");
                let result = LhbClient::new()
                    .lhb_active_yyb(&start_date, &end_date)
                    .await
                    .map_err(to_box_err)?;
                Ok(text_result(&result))
            })
        }),
    )
}

// ─── Tests ──────────────────────────────────────────────

// ============================================================================
// Tool factories: TradeManager & Memory
// ============================================================================

/// Create a tool that queries current positions for a given role.
pub fn create_positions_tool(
    trade: std::sync::Arc<dyn TradeManager>,
    role_name: String,
) -> std::sync::Arc<DynTool> {
    tool(
        "get_current_positions",
        "查询当前持仓信息，包含股票代码、名称、成本价格、持仓数量、当前价格等。用于分析当前仓位情况。",
        empty_schema(),
        std::sync::Arc::new(move |_id, _args, _signal, _progress| {
            let trade = trade.clone();
            let role_name = role_name.clone();
            Box::pin(async move {
                let positions = trade.format_positions(&role_name).map_err(to_box_err)?;
                Ok(text_result(&positions))
            })
        }),
    )
}

/// Create a tool that queries historical context data from memory.
pub fn create_historical_context_tool(
    memory: std::sync::Arc<dyn MemoryStore>,
) -> std::sync::Arc<DynTool> {
    tool(
        "get_historical_context",
        "查询近N日市场历史数据，用于对比分析。参数：days（可选，默认5天）",
        serde_json::json!({"type":"object","properties":{"days":{"type":"integer","description":"查询天数，默认5","default":5}},"required":[]}),
        std::sync::Arc::new(move |_id, args, _signal, _progress| {
            let memory = memory.clone();
            Box::pin(async move {
                let days: u32 = args.get("days").and_then(|v| v.as_u64()).unwrap_or(5) as u32;
                let ctx = memory
                    .get_historical_context(days)
                    .unwrap_or_else(|_| "暂无历史数据".to_string());
                Ok(text_result(&ctx))
            })
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_tool_names() {
        assert_eq!(get_limit_up().name, "get_limit_up");
        assert_eq!(get_limit_down().name, "get_limit_down");
        assert_eq!(get_super_stock().name, "get_super_stock");
        assert_eq!(get_market_overview().name, "get_market_overview");
        assert_eq!(get_yesterday_limit_up().name, "get_yesterday_limit_up");
        assert_eq!(get_dapan_abnormal().name, "get_dapan_abnormal");
        assert_eq!(get_realtime_quote().name, "get_realtime_quote");
        assert_eq!(get_plates_by_stock().name, "get_plates_by_stock");
        assert_eq!(get_market_trend().name, "get_market_trend");
        assert_eq!(get_plate_abnormal().name, "get_plate_abnormal");
        assert_eq!(get_telegraph().name, "get_telegraph");
        assert_eq!(get_telegraph_allday().name, "get_telegraph_allday");
        assert_eq!(get_field_items().name, "get_field_items");
        assert_eq!(get_lhb_detail().name, "get_lhb_detail");
        assert_eq!(get_lhb_combined().name, "get_lhb_combined");
        assert_eq!(get_lhb_active_yyb().name, "get_lhb_active_yyb");
        assert_eq!(get_kline().name, "get_kline");
    }

    #[test]
    fn test_all_tool_schemas_are_valid() {
        let tools: Vec<Arc<DynTool>> = vec![
            get_limit_up(),
            get_limit_down(),
            get_super_stock(),
            get_market_overview(),
            get_yesterday_limit_up(),
            get_dapan_abnormal(),
            get_realtime_quote(),
            get_plates_by_stock(),
            get_market_trend(),
            get_plate_abnormal(),
            get_telegraph(),
            get_telegraph_allday(),
            get_field_items(),
            get_lhb_detail(),
            get_lhb_combined(),
            get_lhb_active_yyb(),
            get_kline(),
        ];
        for tool in &tools {
            assert_eq!(
                tool.parameters_schema["type"], "object",
                "{} schema type mismatch",
                tool.name
            );
            assert!(
                !tool.description.is_empty(),
                "{} description empty",
                tool.name
            );
        }
        assert_eq!(tools.len(), 17);
    }

    #[test]
    fn test_realtime_quote_has_symbol_param() {
        let tool = get_realtime_quote();
        assert!(tool.parameters_schema["properties"]["symbol"].is_object());
        assert!(tool.parameters_schema["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("symbol")));
    }

    #[test]
    fn test_plate_abnormal_has_count_param() {
        let tool = get_plate_abnormal();
        assert!(tool.parameters_schema["properties"]["count"].is_object());
    }

    #[test]
    fn test_lhb_detail_has_date_params() {
        let tool = get_lhb_detail();
        assert!(tool.parameters_schema["properties"]["start_date"].is_object());
        assert!(tool.parameters_schema["properties"]["end_date"].is_object());
    }
}
