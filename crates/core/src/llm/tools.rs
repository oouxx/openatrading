use crate::api::cls::ClsClient;
use crate::api::eastmoney::LhbClient;
use crate::api::jiuyangongshe::JiuYanSession;
use crate::api::xuangubao::XuanguBaoClient;
use crate::llm::define_tool;

// ─── XuanguBao tools ───────────────────────────────────

define_tool! {
    GetLimitUp,
    "get_limit_up",
    "获取当前涨停股票列表，包含股票代码、名称、连板天数、封板时间、炸板次数、涨停原因等详细信息",
    params: {},
    client: XuanguBaoClient,
    method: fetch_limit_up
}

define_tool! {
    GetSuperStock,
    "get_super_stock",
    "获取当前强势股股票列表，包含股票代码、名称、连板天数、封板时间、炸板次数、涨停原因等详细信息",
    params: {},
    client: XuanguBaoClient,
    method: fetch_super_stock
}

define_tool! {
    GetLimitDown,
    "get_limit_down",
    "获取当前跌停股票列表",
    params: {},
    client: XuanguBaoClient,
    method: fetch_limit_down
}

define_tool! {
    GetMarketOverview,
    "get_market_overview",
    "获取当前市场概览数据，包含涨跌家数、涨停跌停数、炸板率、市场温度等",
    params: {},
    client: XuanguBaoClient,
    method: fetch_market_overview
}

define_tool! {
    GetYesterdayLimitUp,
    "get_yesterday_limit_up",
    "获取昨日涨停股票列表，用于对比今日涨停判断晋级情况",
    params: {},
    client: XuanguBaoClient,
    method: fetch_yesterday_limit_up
}

define_tool! {
    GetDapanAbnormal,
    "get_dapan_abnormal",
    "获取大盘异动事件列表，包含异动类型、板块信息、时间戳",
    params: {},
    client: XuanguBaoClient,
    method: fetch_dapan_abnormal
}

define_tool! {
    GetRealtimeQuote,
    "get_realtime_quote",
    "获取指定股票的实时行情数据，包含价格、涨跌幅、换手率、市值、量比、振幅等",
    params: {
        symbol: String => "股票代码，如 600519.SS"
    },
    client: XuanguBaoClient,
    method: fetch_realtime
}

define_tool! {
    GetPlatesByStock,
    "get_plates_by_stock",
    "获取指定股票所属的所有板块及每个板块的涨跌幅",
    params: {
        symbol: String => "股票代码，如 600519.SS"
    },
    client: XuanguBaoClient,
    method: plates_by_any_stock
}

define_tool! {
    GetMarketTrend,
    "get_market_trend",
    "获取指定股票的日内分时走势数据，用于分析盘中价格变化趋势",
    params: {
        symbol: String => "股票代码，如 600519.SS"
    },
    client: XuanguBaoClient,
    method: fetch_market_trend
}

define_tool! {
    GetPlateAbnormal,
    "get_plate_abnormal",
    "获取板块异动事件列表，包含板块涨跌幅、关联个股、利好利空判断，默认返回30条",
    params: {
        count: String => "返回条数，默认30"
    },
    client: XuanguBaoClient,
    method: fetch_plate_abnormal_str
}

// ─── 韭研公社 tools ──────────────────────────────────────

define_tool! {
    GetFieldItems,
    "get_field_items",
    "获取韭研公社总结的涨停股，强势股数据以及涨停原因",
    params: {
        date: String => "日期，格式YYYY-MM-DD，如2025-01-20",
        pc: String => "页码，默认1"
    },
    client: JiuYanSession,
    method: fetch_field_str
}

// ─── CLS tools ─────────────────────────────────────────

define_tool! {
    GetTelegraph,
    "get_telegraph",
    "获取财联社最新电报资讯，默认返回全部类别。可指定category筛选：red(重要)、watch(关注)等",
    params: {
        category: String => "资讯类别，留空返回全部"
    },
    client: ClsClient,
    method: fetch_telegraph_latest
}

define_tool! {
    GetTelegraphAllDay,
    "get_telegraph_allday",
    "获取今日全天财联社电报资讯，自动翻页拉取全部数据",
    params: {
        category: String => "资讯类别，留空返回全部"
    },
    client: ClsClient,
    method: fetch_telegraph_allday
}

// ─── Eastmoney 龙虎榜 tools ─────────────────────────────

define_tool! {
    GetLhbDetail,
    "get_lhb_detail",
    "获取龙虎榜上榜个股详情，包含净买入额、买卖金额、涨跌幅、上榜原因、上榜后1/2/5/10日表现",
    params: {
        start_date: String => "起始日期，格式YYYYMMDD，如20250120",
        end_date: String => "结束日期，格式YYYYMMDD，如20250120"
    },
    client: LhbClient,
    method: lhb_detail
}

define_tool! {
    GetLhbCombined,
    "get_lhb_combined",
    "获取龙虎榜合并数据，包含上榜详情+买卖席位营业部明细",
    params: {
        start_date: String => "起始日期，格式YYYYMMDD，如20250120",
        end_date: String => "结束日期，格式YYYYMMDD，如20250120"
    },
    client: LhbClient,
    method: lhb_combined
}

define_tool! {
    GetLhbActiveYyb,
    "get_lhb_active_yyb",
    "获取近期活跃营业部排名，按净买入额排序",
    params: {
        start_date: String => "起始日期，格式YYYYMMDD，如20250120",
        end_date: String => "结束日期，格式YYYYMMDD，如20250120"
    },
    client: LhbClient,
    method: lhb_active_yyb
}

// ─── Tests ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::ToolSet;

    #[test]
    fn test_all_tool_names() {
        assert_eq!(GetLimitUp::to_agent_tool().name, "get_limit_up");
        assert_eq!(GetLimitDown::to_agent_tool().name, "get_limit_down");
        assert_eq!(GetSuperStock::to_agent_tool().name, "get_super_stock");
        assert_eq!(
            GetMarketOverview::to_agent_tool().name,
            "get_market_overview"
        );
        assert_eq!(
            GetYesterdayLimitUp::to_agent_tool().name,
            "get_yesterday_limit_up"
        );
        assert_eq!(GetDapanAbnormal::to_agent_tool().name, "get_dapan_abnormal");
        assert_eq!(GetRealtimeQuote::to_agent_tool().name, "get_realtime_quote");
        assert_eq!(
            GetPlatesByStock::to_agent_tool().name,
            "get_plates_by_stock"
        );
        assert_eq!(GetMarketTrend::to_agent_tool().name, "get_market_trend");
        assert_eq!(GetPlateAbnormal::to_agent_tool().name, "get_plate_abnormal");
        assert_eq!(GetTelegraph::to_agent_tool().name, "get_telegraph");
        assert_eq!(
            GetTelegraphAllDay::to_agent_tool().name,
            "get_telegraph_allday"
        );
        assert_eq!(GetFieldItems::to_agent_tool().name, "get_field_items");
        assert_eq!(GetLhbDetail::to_agent_tool().name, "get_lhb_detail");
        assert_eq!(GetLhbCombined::to_agent_tool().name, "get_lhb_combined");
        assert_eq!(GetLhbActiveYyb::to_agent_tool().name, "get_lhb_active_yyb");
    }

    #[test]
    fn test_all_tool_schemas() {
        let tools = vec![
            GetLimitUp::to_agent_tool(),
            GetLimitDown::to_agent_tool(),
            GetSuperStock::to_agent_tool(),
            GetMarketOverview::to_agent_tool(),
            GetYesterdayLimitUp::to_agent_tool(),
            GetDapanAbnormal::to_agent_tool(),
            GetRealtimeQuote::to_agent_tool(),
            GetPlatesByStock::to_agent_tool(),
            GetMarketTrend::to_agent_tool(),
            GetPlateAbnormal::to_agent_tool(),
            GetTelegraph::to_agent_tool(),
            GetLhbDetail::to_agent_tool(),
            GetLhbCombined::to_agent_tool(),
            GetLhbActiveYyb::to_agent_tool(),
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
    }

    #[test]
    fn test_realtime_quote_has_symbol_param() {
        let tool = GetRealtimeQuote::to_agent_tool();
        assert!(tool.parameters_schema["properties"]["symbol"].is_object());
        assert!(tool.parameters_schema["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("symbol")));
    }

    #[test]
    fn test_plate_abnormal_has_count_param() {
        let tool = GetPlateAbnormal::to_agent_tool();
        assert!(tool.parameters_schema["properties"]["count"].is_object());
    }

    #[test]
    fn test_lhb_detail_has_date_params() {
        let tool = GetLhbDetail::to_agent_tool();
        assert!(tool.parameters_schema["properties"]["start_date"].is_object());
        assert!(tool.parameters_schema["properties"]["end_date"].is_object());
    }

    #[tokio::test]
    #[ignore]
    async fn test_toolset_with_all_tools() {
        use std::sync::Arc;

        let tools = Arc::new(
            ToolSet::new()
                .with_tool(GetLimitUp::to_agent_tool())
                .with_tool(GetLimitDown::to_agent_tool())
                .with_tool(GetSuperStock::to_agent_tool())
                .with_tool(GetMarketOverview::to_agent_tool())
                .with_tool(GetYesterdayLimitUp::to_agent_tool())
                .with_tool(GetDapanAbnormal::to_agent_tool())
                .with_tool(GetRealtimeQuote::to_agent_tool())
                .with_tool(GetPlatesByStock::to_agent_tool())
                .with_tool(GetMarketTrend::to_agent_tool())
                .with_tool(GetPlateAbnormal::to_agent_tool())
                .with_tool(GetTelegraph::to_agent_tool())
                .with_tool(GetLhbDetail::to_agent_tool())
                .with_tool(GetLhbCombined::to_agent_tool())
                .with_tool(GetLhbActiveYyb::to_agent_tool()),
        );

        assert_eq!(tools.to_openai_tools().len(), 14);

        let result = tools.call("get_limit_up", &serde_json::json!({})).await;
        assert!(result.is_ok());

        let result = tools
            .call(
                "get_realtime_quote",
                &serde_json::json!({"symbol": "600519.SS"}),
            )
            .await;
        assert!(result.is_ok());
    }
}
