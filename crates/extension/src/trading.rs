//! Market data extension — A-share trading tools for the agent.
//!
//! This extension registers a set of A-share market-data tools (limit-up
//! boards, realtime quotes, K-line, dragon-tiger list, news telegraph, …)
//! and dispatches their execution to the `openatrading-api` clients via the
//! `HookHandler::handle_tool_call` bridge.
//!
//! The agent runtime collects the registered [`ToolDefinition`]s through
//! `ExtensionRegistry::collect_tools` and routes tool calls whose name
//! matches a registered tool to `handle_tool_call`. The first extension that
//! returns `Some(ToolCallOutput)` wins, so each tool name below is unique.

use anyhow::Context;
use async_trait::async_trait;
use pi_extension_api::{
    ExtensionContext, HookHandler, ToolCallOutput, ToolDefinition, ToolRegistry,
};
use serde_json::{json, Value};

use openatrading_api::cls::ClsClient;
use openatrading_api::eastmoney::{LhbClient, ReportCategory};
use openatrading_api::sina::SinaQuoteClient;
use openatrading_api::xuangubao::{KlinePeriod, PlateRankType, XuanguBaoClient};

/// Market data extension providing A-share market-data tools.
///
/// 交易不在 agent 侧：模拟盘改为用户在 Web/桌面端手动记录
/// （见 `openatrading_core::trade`），因此本扩展无状态，只提供行情/资讯工具。
pub struct MarketDataExtension;

impl MarketDataExtension {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Market-data tool definitions.
    fn market_definitions() -> Vec<ToolDefinition> {
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
            plate_rankings_tool(),
            plate_detail_tool(),
            kline_tool(),
            market_indicators_tool(),
            market_pcp_distribution_tool(),
            fund_flow_tool(),
            stock_data_tool(),
            stock_labels_tool(),
            security_type_tool(),
            telegraph_tool(),
            telegraph_allday_tool(),
            all_day_events_tool(),
            auction_quotes_tool(),
            lhb_detail_tool(),
            lhb_combined_tool(),
            lhb_active_yyb_tool(),
            reports_tool(),
            stock_reports_tool(),
            report_detail_tool(),
        ]
    }

    /// Dispatch a tool call to the matching handler.
    ///
    /// Returns `Some(output)` when `tool_name` is handled by this extension,
    /// `None` otherwise (so other extensions / built-ins can handle it).
    async fn dispatch(&self, tool_name: &str, params: Value) -> Option<ToolCallOutput> {
        let result: Result<serde_json::Value, String> = match tool_name {
            "get_limit_up" => {
                call(|| async { XuanguBaoClient::new().fetch_limit_up().await }).await
            }
            "get_limit_down" => {
                call(|| async { XuanguBaoClient::new().fetch_limit_down().await }).await
            }
            "get_super_stock" => {
                call(|| async { XuanguBaoClient::new().fetch_super_stock().await }).await
            }
            "get_market_overview" => {
                call(|| async { XuanguBaoClient::new().fetch_market_overview().await }).await
            }
            "get_yesterday_limit_up" => {
                call(|| async { XuanguBaoClient::new().fetch_yesterday_limit_up().await }).await
            }
            "get_dapan_abnormal" => {
                call(|| async { XuanguBaoClient::new().fetch_dapan_abnormal().await }).await
            }
            "get_realtime_quote" => {
                let symbol = str_arg(&params, "symbol");
                call(|| async move { XuanguBaoClient::new().fetch_realtime(&symbol).await }).await
            }
            "get_plates_by_stock" => {
                let symbol = str_arg(&params, "symbol");
                call(|| async move { XuanguBaoClient::new().plates_by_any_stock(&symbol).await })
                    .await
            }
            "get_market_trend" => {
                let symbol = str_arg(&params, "symbol");
                call(|| async move { XuanguBaoClient::new().fetch_market_trend(&symbol).await })
                    .await
            }
            "get_plate_abnormal" => {
                let count = str_arg(&params, "count");
                call(|| async move {
                    XuanguBaoClient::new()
                        .fetch_plate_abnormal_str(&count)
                        .await
                })
                .await
            }
            "get_plate_rankings" => {
                let rank_type = parse_rank_type(&str_arg(&params, "type"));
                let limit = parse_usize_arg(&params, "limit", 10, 50);
                match (rank_type, limit) {
                    (Ok(rank_type), Ok(limit)) => {
                        call(|| async move {
                            XuanguBaoClient::new()
                                .fetch_plate_rankings(rank_type, limit)
                                .await
                        })
                        .await
                    }
                    (Err(error), _) | (_, Err(error)) => Err(error),
                }
            }
            "get_plate_detail" => {
                let plate_id = parse_i64_arg(&params, "plate_id");
                match plate_id {
                    Ok(plate_id) if plate_id > 0 => call(|| async move {
                        XuanguBaoClient::new().fetch_plate_detail(plate_id).await
                    })
                    .await,
                    Ok(_) => Err("plate_id 必须大于 0".to_string()),
                    Err(error) => Err(error),
                }
            }
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
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(100)
                    .min(500) as u32;
                call(|| async move {
                    XuanguBaoClient::new()
                        .fetch_kline(&symbol, period, count)
                        .await
                })
                .await
            }
            "get_market_indicators" => {
                call(|| async { XuanguBaoClient::new().fetch_market_indicator_line().await }).await
            }
            "get_market_pcp_distribution" => {
                call(|| async { XuanguBaoClient::new().fetch_market_pcp_distribution().await })
                    .await
            }
            "get_fund_flow" => {
                let codes = parse_codes_arg(&params, "codes");
                let day_count = parse_usize_arg(&params, "day_count", 5, 30);
                match (codes, day_count) {
                    (Ok(codes), Ok(day_count)) => {
                        call(|| async move {
                            let refs = codes.iter().map(String::as_str).collect::<Vec<_>>();
                            let data = XuanguBaoClient::new()
                                .fetch_fund_flow_batch(&refs, day_count)
                                .await?;
                            Ok::<_, anyhow::Error>(
                                data.into_iter()
                                    .collect::<std::collections::BTreeMap<_, _>>(),
                            )
                        })
                        .await
                    }
                    (Err(error), _) | (_, Err(error)) => Err(error),
                }
            }
            "get_stock_data" => {
                let codes = parse_codes_arg(&params, "codes");
                match codes {
                    Ok(codes) => {
                        call(|| async move {
                            let refs = codes.iter().map(String::as_str).collect::<Vec<_>>();
                            let data = XuanguBaoClient::new().fetch_stock_data(&refs).await?;
                            Ok::<_, anyhow::Error>(
                                data.into_iter()
                                    .collect::<std::collections::BTreeMap<_, _>>(),
                            )
                        })
                        .await
                    }
                    Err(error) => Err(error),
                }
            }
            "get_stock_labels" => {
                let codes = parse_codes_arg(&params, "codes");
                match codes {
                    Ok(codes) => {
                        call(|| async move {
                            let refs = codes.iter().map(String::as_str).collect::<Vec<_>>();
                            let data = XuanguBaoClient::new().fetch_stock_labels(&refs).await?;
                            Ok::<_, anyhow::Error>(
                                data.into_iter()
                                    .collect::<std::collections::BTreeMap<_, _>>(),
                            )
                        })
                        .await
                    }
                    Err(error) => Err(error),
                }
            }
            "get_security_type" => {
                let symbol = str_arg(&params, "symbol");
                call(|| async move { XuanguBaoClient::new().fetch_security_type(&symbol).await })
                    .await
            }
            "get_telegraph" => {
                let category = str_arg(&params, "category");
                let cat = if category.is_empty() {
                    ""
                } else {
                    category.as_str()
                };
                call(|| async move { ClsClient::new().fetch_telegraph(cat, None, 20).await }).await
            }
            "get_telegraph_allday" => {
                call(|| async { ClsClient::new().fetch_telegraph("", None, 100).await }).await
            }
            "get_all_day_events" => {
                call(|| async { XuanguBaoClient::new().fetch_all_day_events(0.03).await }).await
            }
            "get_auction_quotes" => {
                let codes_str = str_arg(&params, "codes");
                let codes: Vec<&str> = codes_str
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .collect();
                if codes.is_empty() {
                    Err("codes 不能为空".to_string())
                } else {
                    match SinaQuoteClient::new() {
                        Ok(client) => {
                            call(|| async move { client.fetch_quotes(&codes).await }).await
                        }
                        Err(e) => Err(e.to_string()),
                    }
                }
            }
            "get_lhb_detail" => {
                let start_date = str_arg(&params, "start_date");
                let end_date = str_arg(&params, "end_date");
                call(|| async move { LhbClient::new().lhb_detail(&start_date, &end_date).await })
                    .await
            }
            "get_lhb_combined" => {
                let start_date = str_arg(&params, "start_date");
                let end_date = str_arg(&params, "end_date");
                call(|| async move { LhbClient::new().lhb_combined(&start_date, &end_date).await })
                    .await
            }
            "get_lhb_active_yyb" => {
                let start_date = str_arg(&params, "start_date");
                let end_date = str_arg(&params, "end_date");
                call(|| async move {
                    LhbClient::new()
                        .lhb_active_yyb(&start_date, &end_date)
                        .await
                })
                .await
            }
            "get_reports" => {
                let category = str_arg(&params, "category");
                let page = params
                    .get("page")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(1) as u32;
                let size = params
                    .get("size")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(20) as u32;
                let cat = match category.as_str() {
                    "macresearch" => ReportCategory::MacroResearch,
                    "brokerreport" => ReportCategory::BrokerReport,
                    "industry" => ReportCategory::Industry,
                    "stock" => ReportCategory::Stock,
                    _ => ReportCategory::StrategyReport,
                };
                call(|| async move { LhbClient::new().reports(cat, page, size).await }).await
            }
            "get_stock_reports" => {
                let page = params
                    .get("page")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(1) as u32;
                let size = params
                    .get("size")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(20) as u32;
                call(|| async move { LhbClient::new().stock_reports(page, size).await }).await
            }
            "get_report_detail" => {
                let info_code = str_arg(&params, "info_code");
                let category = str_arg(&params, "category");
                let cat = match category.as_str() {
                    "macresearch" => ReportCategory::MacroResearch,
                    "brokerreport" => ReportCategory::BrokerReport,
                    "industry" => ReportCategory::Industry,
                    "stock" => ReportCategory::Stock,
                    _ => ReportCategory::StrategyReport,
                };
                call(|| async move { LhbClient::new().report_detail(&info_code, cat).await }).await
            }
            _ => return None,
        };

        Some(match result {
            Ok(value) => ToolCallOutput {
                content: vec![
                    json!({ "type": "text", "text": serde_json::to_string_pretty(&value).unwrap_or_default() }),
                ],
                details: None,
                is_error: false,
                terminate: None,
            },
            Err(err) => {
                tracing::warn!(tool = tool_name, error = %err, "market tool failed");
                ToolCallOutput {
                    content: vec![json!({ "type": "text", "text": err })],
                    details: None,
                    is_error: true,
                    terminate: None,
                }
            }
        })
    }
}

impl Default for MarketDataExtension {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HookHandler for MarketDataExtension {
    fn name(&self) -> &'static str {
        "openatrading-market"
    }

    fn register_tools(&self, registry: &mut ToolRegistry) {
        for def in Self::market_definitions() {
            registry.register(&def.name.clone(), def);
        }
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

fn plate_rankings_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_plate_rankings".into(),
        label: Some("板块涨跌榜".into()),
        description: "获取板块涨幅榜和跌幅榜，返回资金流、上涨/下跌/平盘家数、涨停数、核心涨幅排名及变化、领涨股和领跌股。type 可选 0=全部、1=概念、2=行业、3=风格。".into(),
        prompt_guidelines: Some(vec![
            "分析市场主线、板块轮动与资金聚焦方向时优先使用".into(),
            "涨幅榜和跌幅榜应分别解读，不要把 0% 板块混入任一侧".into(),
        ]),
        parameters: Some(mixed_str_schema(
            &[],
            &[
                ("type", "板块分类：0=全部（默认）、1=概念、2=行业、3=风格"),
                ("limit", "每侧返回数量，默认10，最大50"),
            ],
        )),
        ..Default::default()
    }
}

fn plate_detail_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_plate_detail".into(),
        label: Some("板块详情".into()),
        description: "获取板块详情、涨跌统计、产业链和完整成分股。返回字段包含板块描述、资金流、上涨/下跌/平盘家数、核心股数量、成分股入选原因及产业链归属。".into(),
        prompt_guidelines: Some(vec!["从板块排行下钻到成分股时使用".into()]),
        parameters: Some(str_schema(&[("plate_id", "板块 ID，如 22114510")])),
        ..Default::default()
    }
}

fn market_indicators_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_market_indicators".into(),
        label: Some("市场指标序列".into()),
        description:
            "获取市场涨跌家数、涨跌停、炸板家数/比例、市场情绪温度和昨日涨停平均涨幅的分时序列。"
                .into(),
        prompt_guidelines: Some(vec!["分析盘中市场宽度和情绪变化时使用".into()]),
        parameters: Some(empty_schema()),
        ..Default::default()
    }
}

fn market_pcp_distribution_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_market_pcp_distribution".into(),
        label: Some("涨跌幅分布".into()),
        description: "获取全市场涨跌幅档位分布、涨跌停数量、ST 涨跌停、停牌和总股票数。".into(),
        prompt_guidelines: Some(vec!["判断市场赚钱效应和涨跌幅结构时使用".into()]),
        parameters: Some(empty_schema()),
        ..Default::default()
    }
}

fn fund_flow_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_fund_flow".into(),
        label: Some("资金流向".into()),
        description: "批量获取股票最近若干交易日资金流。上游中间指标列为厂商定义，工具按原顺序返回，不臆造主力/大单等列名。".into(),
        prompt_guidelines: Some(vec!["分析个股连续资金流变化时使用".into()]),
        parameters: Some(mixed_str_schema(
            &[("codes", "逗号分隔的股票代码，如 000001.SZ,600519.SS")],
            &[("day_count", "返回交易日数量，默认5，最大30")],
        )),
        ..Default::default()
    }
}

fn stock_data_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_stock_data".into(),
        label: Some("股票指标".into()),
        description:
            "批量获取股票价格、涨跌幅、换手率、流通/总市值、PE、连板天数和最新涨停时间等指标。"
                .into(),
        prompt_guidelines: Some(vec!["补充连板、市值和涨停时间等个股指标时使用".into()]),
        parameters: Some(str_schema(&[(
            "codes",
            "逗号分隔的股票代码，如 000001.SZ,600519.SS",
        )])),
        ..Default::default()
    }
}

fn stock_labels_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_stock_labels".into(),
        label: Some("股票标签".into()),
        description: "批量获取股票标签，包含标签名称、类型、说明和 effect 方向编码。".into(),
        prompt_guidelines: Some(vec!["识别白马、ST、破净、龙虎榜等个股特征时使用".into()]),
        parameters: Some(str_schema(&[(
            "codes",
            "逗号分隔的股票代码，如 000001.SZ,600519.SS",
        )])),
        ..Default::default()
    }
}

fn security_type_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_security_type".into(),
        label: Some("证券类型".into()),
        description: "获取证券的市场类型、证券类型和子类型元数据，例如股票、指数、ETF。".into(),
        prompt_guidelines: Some(vec!["需要确认证券类别时使用".into()]),
        parameters: Some(str_schema(&[("symbol", "证券代码，如 000001.SS")])),
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

fn reports_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_reports".into(),
        label: Some("研究报告".into()),
        description: "获取东方财富研究报告列表。支持分类：strategyreport(策略报告)、macresearch(宏观研究)、\
            brokerreport(券商晨报)、industry(行业研报)、stock(个股研报)。\
            返回报告标题、作者、机构、发布日期等。注意：个股研报请用 get_stock_reports（含盈利预测）。".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "category": {
                    "type": "string",
                    "description": "研报分类：strategyreport/macresearch/brokerreport/industry/stock",
                    "default": "strategyreport"
                },
                "page": {
                    "type": "integer",
                    "description": "页码，从1开始",
                    "default": 1
                },
                "size": {
                    "type": "integer",
                    "description": "每页条数",
                    "default": 20
                }
            },
            "required": []
        })),
        ..Default::default()
    }
}

fn stock_reports_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_stock_reports".into(),
        label: Some("个股研报".into()),
        description: "获取东方财富个股研报列表（含盈利预测）。返回股票名称、代码、机构、评级、            盈利预测(EPS/PE)、评级变动等。用法：分析机构对特定个股的观点变化，            结合盈利预测判断业绩趋势。".into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "page": {
                    "type": "integer",
                    "description": "页码，从1开始",
                    "default": 1
                },
                "size": {
                    "type": "integer",
                    "description": "每页条数",
                    "default": 20
                }
            },
            "required": []
        })),
        ..Default::default()
    }
}

fn report_detail_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_report_detail".into(),
        label: Some("研报详情".into()),
        description: "获取东方财富研报详情，返回 PDF 下载链接和纯文本正文摘要（description）。\
            需要提供 info_code（从 get_reports/get_stock_reports 返回的 encodeUrl 或 infoCode）\
            和 category（研报分类）。注意：industry 分类传 infoCode，其余非个股分类传 encodeUrl。"
            .into(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "info_code": {
                    "type": "string",
                    "description": "研报 infoCode（个股研报）或 encodeUrl（非个股研报）"
                },
                "category": {
                    "type": "string",
                    "description": "研报分类：strategyreport/macresearch/brokerreport/industry/stock"
                }
            },
            "required": ["info_code", "category"]
        })),
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
async fn call<F, Fut, T, E>(f: F) -> Result<serde_json::Value, String>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    T: serde::Serialize,
    E: std::fmt::Display,
{
    f().await
        .map(|v| serde_json::to_value(v).unwrap_or(serde_json::Value::Null))
        .map_err(|e| e.to_string())
}

fn str_arg(params: &Value, name: &str) -> String {
    params
        .get(name)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn parse_rank_type(value: &str) -> Result<PlateRankType, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "0" | "all" | "全部" => Ok(PlateRankType::All),
        "1" | "concept" | "概念" => Ok(PlateRankType::Concept),
        "2" | "industry" | "行业" => Ok(PlateRankType::Industry),
        "3" | "style" | "风格" => Ok(PlateRankType::Style),
        other => Err(format!(
            "非法板块分类 {other}，可选 0=全部、1=概念、2=行业、3=风格"
        )),
    }
}

fn parse_usize_arg(
    params: &Value,
    name: &str,
    default: usize,
    max: usize,
) -> Result<usize, String> {
    let Some(value) = params.get(name) else {
        return Ok(default);
    };
    let parsed = value
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .or_else(|| value.as_str()?.trim().parse::<usize>().ok())
        .with_context(|| format!("{name} 必须是非负整数"))
        .map_err(|error| error.to_string())?;
    if parsed == 0 || parsed > max {
        return Err(format!("{name} 必须在 1..={max}"));
    }
    Ok(parsed)
}

fn parse_i64_arg(params: &Value, name: &str) -> Result<i64, String> {
    let value = params
        .get(name)
        .with_context(|| format!("缺少 {name}"))
        .map_err(|error| error.to_string())?;
    value
        .as_i64()
        .or_else(|| value.as_str()?.trim().parse::<i64>().ok())
        .with_context(|| format!("{name} 必须是整数"))
        .map_err(|error| error.to_string())
}

fn parse_codes_arg(params: &Value, name: &str) -> Result<Vec<String>, String> {
    let raw = str_arg(params, name);
    let mut seen = std::collections::HashSet::new();
    let codes: Vec<String> = raw
        .split(',')
        .map(str::trim)
        .filter(|code| !code.is_empty())
        .filter(|code| seen.insert((*code).to_string()))
        .map(str::to_string)
        .collect();
    if codes.is_empty() {
        return Err(format!("{name} 不能为空"));
    }
    Ok(codes)
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

fn mixed_str_schema(required: &[(&str, &str)], optional: &[(&str, &str)]) -> Value {
    let properties: serde_json::Map<String, Value> = required
        .iter()
        .chain(optional)
        .map(|(name, desc)| {
            (
                (*name).to_string(),
                json!({ "type": "string", "description": desc }),
            )
        })
        .collect();
    let required: Vec<String> = required
        .iter()
        .map(|(name, _)| (*name).to_string())
        .collect();
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plate_rank_categories_and_limits() {
        assert_eq!(
            parse_rank_type("0").unwrap_or(PlateRankType::All),
            PlateRankType::All
        );
        assert_eq!(
            parse_rank_type("概念").unwrap_or(PlateRankType::All),
            PlateRankType::Concept
        );
        assert_eq!(
            parse_rank_type("2").unwrap_or(PlateRankType::All),
            PlateRankType::Industry
        );
        assert_eq!(
            parse_rank_type("style").unwrap_or(PlateRankType::All),
            PlateRankType::Style
        );
        assert!(parse_rank_type("4").is_err());
        assert_eq!(parse_usize_arg(&json!({}), "limit", 10, 50), Ok(10));
        assert!(parse_usize_arg(&json!({"limit": "0"}), "limit", 10, 50).is_err());
        assert!(parse_usize_arg(&json!({"limit": "51"}), "limit", 10, 50).is_err());
    }

    #[test]
    fn parses_and_deduplicates_code_lists() {
        let codes = parse_codes_arg(
            &json!({"codes": " 600519.SS,000001.SZ,600519.SS "}),
            "codes",
        )
        .unwrap_or_else(|error| panic!("codes should parse: {error}"));
        assert_eq!(codes, vec!["600519.SS", "000001.SZ"]);
        assert!(parse_codes_arg(&json!({"codes": " , "}), "codes").is_err());
    }

    #[test]
    fn market_tools_include_rank_detail_and_capital_flow() {
        let tools = MarketDataExtension::market_definitions();
        let names: Vec<&str> = tools.iter().map(|tool| tool.name.as_str()).collect();
        assert_eq!(tools.len(), 29);
        for expected in [
            "get_plate_rankings",
            "get_plate_detail",
            "get_market_indicators",
            "get_market_pcp_distribution",
            "get_fund_flow",
            "get_stock_data",
            "get_stock_labels",
            "get_security_type",
        ] {
            assert!(names.contains(&expected), "missing tool: {expected}");
        }
        assert!(!names.contains(&"get_community"));
    }
}
