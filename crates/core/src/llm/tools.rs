use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use openatrading_agent::pi_ai_types::ContentBlock;
use openatrading_agent::types::{AgentToolResult, DynTool};

use crate::api::cls::ClsClient;
use crate::api::eastmoney::LhbClient;
use crate::api::jiuyangongshe::JiuYanSession;
use crate::api::xuangubao::{KlinePeriod, XuanguBaoClient};
use crate::memory::sqlite::MemoryStore;
use crate::trade::sqlite::TradeManager;

pub(crate) type ExecuteFn = Arc<
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

pub(crate) fn empty_schema() -> serde_json::Value {
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

pub(crate) fn text_result<T: serde::Serialize>(value: &T) -> AgentToolResult<serde_json::Value> {
    AgentToolResult {
        content: vec![ContentBlock::text(
            serde_json::to_string(value).unwrap_or_default(),
        )],
        details: serde_json::Value::Null,
        terminate: None,
    }
}

pub(crate) fn to_box_err(e: impl std::fmt::Display) -> Box<dyn Error + Send + Sync> {
    e.to_string().into()
}

pub(crate) fn tool(
    name: &str,
    description: &str,
    parameters_schema: serde_json::Value,
    execute: ExecuteFn,
) -> Arc<DynTool> {
    Arc::new(DynTool {
        name: name.to_string(),
        description: description.to_string(),
        parameters_schema,
        label: String::new(),
        execution_mode: None,
        prepare_arguments: None,
        execute,
    })
}

// ─── 涨停 & 强势股 ─────────────────────────────────────

pub fn get_limit_up() -> Arc<DynTool> {
    tool(
        "get_limit_up",
        "获取今日涨停股票完整列表。每只股票包含：代码(symbol)、名称(stock_chi_name)、现价、涨跌幅、\
        连板天数(limit_up_days)、今日首次封板时间(first_limit_up, Unix秒)、最后封板时间(last_limit_up)、\
        炸板次数(break_limit_up_times)、首次炸板时间(first_break_limit_up)、最后炸板时间(last_break_limit_up)、\
        昨日连板天数(yesterday_limit_up_days)、封单量比(buy_lock_volume_ratio)、卖单量比(sell_lock_volume_ratio)、\
        换手率(turnover_ratio)、量比(volume_bias_ratio)、M天N板数据(m_days_n_boards_boards/days)、\
        涨停原因(surge_reason)等。\
        用法：与 get_yesterday_limit_up 对比得出连板晋级率（昨日X板今日Y板的比例），\
        连板天梯（各梯队涨停数分布），炸板率 = 炸板次数>0 的股票占比。\
        换手率+封单量比可判断封板质量（高换手低封单 = 弱势封板）。",
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
        "获取强势股列表（未涨停但走势强劲的个股）。用于发现潜在涨停候选、\
        识别盘中异动但尚未封板的股票。数据结构与涨停列表相同。\
        用法：结合 realtime_quote 判断是否有封板潜力（换手、量比、涨幅）。",
        empty_schema(),
        Arc::new(|_id, _args, _signal, _progress| {
            Box::pin(async move {
                let result = XuanguBaoClient::new()
                    .fetch_super_stock()
                    .await
                    .map_err(to_box_err)?;
                Ok(text_result(&result))
            })
        }),
    )
}

pub fn get_limit_down() -> Arc<DynTool> {
    tool(
        "get_limit_down",
        "获取今日跌停股票列表。包含代码、名称、连跌天数(limit_down_days)、\
        跌停封单数据等。用于判断市场亏钱效应、恐慌程度。\
        用法：跌停数/昨日涨停数 = 亏钱效应比率；跌停家数增加 = 情绪退潮信号。",
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

// ─── 市场全景 ─────────────────────────────────────────

pub fn get_market_overview() -> Arc<DynTool> {
    tool(
        "get_market_overview",
        "获取市场全景概览：上涨家数(rise_count)、下跌家数(fall_count)、\
        涨停家数(limit_up_count)、跌停家数(limit_down_count)、\
        炸板率(bomb_rate, 0~1)、市场温度(market_temperature, 0~100)、\
        昨日涨停今日平均涨幅(yesterday_limit_up_avg_pcp)。\
        用法：首先调用，快速判断市场整体强弱——温度>60且炸板率<0.2为强势；\
        温度<40且炸板率>0.3为弱势。涨跌比<1且跌停>10为退潮。",
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
        "获取昨日涨停股票列表及今日表现。每只包含：昨日连板天数(yesterday_limit_up_days)、\
        今日是否继续涨停、今日涨跌幅等。\
        用法：核心用途——(1)与今日涨停对比计算连板晋级率（昨日X板→今日Y板的比例），\
        构建连板天梯；(2)昨日涨停今日下跌比例 = 亏钱效应；(3)昨日涨停平均涨幅 = 溢价率。\
        晋级率>50%为强势，<30%为退潮。",
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
        "获取大盘异动事件（全市场级别的异常波动）。包含异动类型、触发板块、时间戳。\
        用法：检测系统性风险——大盘异动频发 = 市场不稳定，应降低仓位或回避。",
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

// ─── 实时行情 ─────────────────────────────────────────

pub fn get_realtime_quote() -> Arc<DynTool> {
    tool(
        "get_realtime_quote",
        "获取单只股票的实时行情：现价(last_px)、涨跌幅(px_change_rate)、开盘价(open_px)、\
        最高价(high_px)、最低价(low_px)、振幅(amplitude)、换手率(turnover_ratio)、\
        量比(volume_ratio)、市值(market_value)、流通市值(circulation_value)、\
        成交额(turnover_value)、市盈率(pe_rate/dyn_pe)、市净率(dyn_pb_rate)。\
        用法：查看涨停候选股的实时盘口——换手率适中(5-15%)+量比>2 = 活跃；\
        振幅过大(>15%)+高位 = 分歧加大。",
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
        "获取指定股票所属的所有概念/行业板块，及每个板块的当前涨跌幅。\
        用法：判断涨停股的板块效应——所属板块涨幅>2%且有其他涨停股 = 板块助攻；\
        个股涨停但板块不跟 = 独立行情，持续性和空间有限。",
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
        "获取个股分时走势数据（日内价格+成交量序列）。\
        用法：分析盘中走势形态——均价线上方运行 = 强势；\
        多次冲击涨停未封 = 抛压大；尾盘跳水 = 次日低开概率高。",
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

// ─── 板块异动 ─────────────────────────────────────────

pub fn get_plate_abnormal() -> Arc<DynTool> {
    tool(
        "get_plate_abnormal",
        "获取板块异动事件：板块涨跌幅、异动类型、关联个股、利好/利空方向判断。\
        用法：发现资金攻击方向——连续多条同一板块异动 = 资金抱团，可能形成主线；\
        异动板块切换频繁 = 轮动快，不适合追高。",
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

// ─── K线 ─────────────────────────────────────────────

pub fn get_kline() -> Arc<DynTool> {
    tool(
        "get_kline",
        "获取个股历史K线数据（OHLCV）。支持日线、周线、月线、分钟线。\
        每根K线包含：时间戳(tick_at)、开盘价(open_px)、收盘价(close_px)、\
        最高价(high_px)、最低价(low_px)、成交量(turnover_volume)、涨跌幅(px_change_rate)。\
        用法：查看技术形态——突破前高、均线支撑、量价配合等。周期参数用枚举值。",
        str_schema(&[
            ("symbol", "股票代码，如 600519.SS"),
            (
                "period",
                "K线周期：Minute(分钟), Day(日线), Week(周线), Month(月线)",
            ),
            ("count", "返回K线数量，默认100"),
        ]),
        Arc::new(|_id, args, _signal, _progress| {
            Box::pin(async move {
                let symbol = str_arg(&args, "symbol");
                let period = match str_arg(&args, "period").as_str() {
                    "Minute" => KlinePeriod::Minute,
                    "Week" => KlinePeriod::Week,
                    "Month" => KlinePeriod::Month,
                    _ => KlinePeriod::Day,
                };
                let count = args
                    .get("count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(100)
                    .min(500);
                let result = XuanguBaoClient::new()
                    .fetch_kline(&symbol, period, count as u32)
                    .await
                    .map_err(to_box_err)?;
                Ok(text_result(&result))
            })
        }),
    )
}

// ─── 新闻 & 电报 ──────────────────────────────────────

pub fn get_field_items() -> Arc<DynTool> {
    tool(
        "get_field_items",
        "获取韭研公社今日异动板块数据。包含板块名称、异动信号、关联个股、\
        趋势判断等。用于社群情绪和事件驱动的辅助参考。",
        empty_schema(),
        Arc::new(|_id, _args, _signal, _progress| {
            Box::pin(async move {
                let date = crate::utils::time::beijing_now()
                    .format("%Y-%m-%d")
                    .to_string();
                let result = JiuYanSession::new()
                    .fetch_field_str(&date, "1")
                    .await
                    .map_err(to_box_err)?;
                Ok(text_result(&result))
            })
        }),
    )
}

pub fn get_telegraph() -> Arc<DynTool> {
    tool(
        "get_telegraph",
        "获取财联社分类电报（最新N条）。参数 category 可选值：\
        \"\" (全部), \"stock\" (股市), \"macro\" (宏观), \"industry\" (行业), \"company\" (公司)。\
        用法：扫描突发新闻——政策消息(宏观)、行业利好(行业)、个股公告(公司)。",
        str_schema(&[(
            "category",
            "分类，可选 '' 'stock' 'macro' 'industry' 'company'",
        )]),
        Arc::new(|_id, args, _signal, _progress| {
            Box::pin(async move {
                let category = str_arg(&args, "category");
                let cat = if category.is_empty() { "" } else { &category };
                let result = ClsClient::new()
                    .fetch_telegraph(cat, None, 20)
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
        "获取财联社全天电报（所有分类，不限条数）。\
        用法：盘后复盘时获取全天重要消息，按时间线梳理事件脉络。",
        empty_schema(),
        Arc::new(|_id, _args, _signal, _progress| {
            Box::pin(async move {
                let result = ClsClient::new()
                    .fetch_telegraph("", None, 100)
                    .await
                    .map_err(to_box_err)?;
                Ok(text_result(&result))
            })
        }),
    )
}

// ─── 龙虎榜 ──────────────────────────────────────────

pub fn get_lhb_detail() -> Arc<DynTool> {
    tool(
        "get_lhb_detail",
        "获取龙虎榜明细数据。包含上榜股票、买入/卖出营业部、买卖金额、净买入等。\
        用法：判断主力动向——知名游资买入 = 短线看多信号；机构席位大额买入 = 中线看好；\
        净买入金额大的股票次日溢价概率高。日期格式 YYYYMMDD。",
        str_schema(&[
            ("start_date", "起始日期，如 20260615"),
            ("end_date", "结束日期，如 20260615"),
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
        "获取龙虎榜合并数据（按股票汇总买卖席位）。比明细更简洁，\
        直接显示每只股票的总买入额、总卖出额、净买入额、上榜营业部数量。\
        用法：快速扫描当日龙虎榜——净买入前10、知名席位动向。",
        str_schema(&[
            ("start_date", "起始日期，如 20260615"),
            ("end_date", "结束日期，如 20260615"),
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
        "获取近期活跃营业部排名。包含营业部名称、上榜次数、买入总额、\
        卖出总额、跟投胜率等。\
        用法：识别当前活跃的游资席位——跟投胜率高的席位 = 正向跟随信号。\
        日期格式 YYYYMMDD。",
        str_schema(&[
            ("start_date", "起始日期，如 20260615"),
            ("end_date", "结束日期，如 20260615"),
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

// ─── 全量异动 ────────────────────────────────────────

pub fn get_all_day_events() -> Arc<DynTool> {
    tool(
        "get_all_day_events",
        "获取今日盘中全量异动事件（板块异动+个股异动），按时间倒序排列。\
        每条包含事件类型、触发标的、发生时间戳等。\
        用法：盘后/盘中复盘时梳理全天异动时间线——\
        哪个时间点集中异动、哪个板块异动频次最高、异动与涨停的时间关系。\
        结合 limit_up 数据可分析「异动→封板」的转化率。",
        empty_schema(),
        Arc::new(|_id, _args, _signal, _progress| {
            Box::pin(async move {
                let result = XuanguBaoClient::new()
                    .fetch_all_day_events()
                    .await
                    .map_err(to_box_err)?;
                Ok(text_result(&result))
            })
        }),
    )
}

// ─── 持仓 & 记忆（per-role 注入）─────────────────────────

/// Create a tool that queries current positions for a given role.
pub fn create_positions_tool(trade: Arc<TradeManager>, role_name: String) -> Arc<DynTool> {
    tool(
        "get_current_positions",
        "查询当前角色的持仓列表。返回股票代码、名称、成本价、持仓数量、\
        当前价格、浮动盈亏、买入理由、持仓占比等。\
        用法：做决策前先查持仓，避免重复买入已持仓股票，\
        根据持仓盈亏决定是否止盈/止损。",
        empty_schema(),
        Arc::new(move |_id, _args, _signal, _progress| {
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
pub fn create_historical_context_tool(memory: Arc<MemoryStore>) -> Arc<DynTool> {
    tool(
        "get_historical_context",
        "查询近N日市场历史快照数据（默认5天）。包含每日市场概览、\
        涨停数据、板块异动等历史记录。\
        用法：对比历史同期数据判断趋势变化——\
        今日温度 vs 近5日均温 = 情绪方向；连续缩量涨停 = 加速赶顶。",
        serde_json::json!({"type":"object","properties":{"days":{"type":"integer","description":"查询天数，默认5","default":5}},"required":[]}),
        Arc::new(move |_id, args, _signal, _progress| {
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

// ─── 竞价数据（新浪）───────────────────────────────────

pub fn get_auction_quotes() -> Arc<DynTool> {
    tool(
        "get_auction_quotes",
        "获取指定股票的集合竞价实时数据（9:15-9:25）。每只股票包含：代码(code)、名称(name)、\
        前收盘价(prev_close)、竞价撮合价(current_price)、涨跌幅(change_pct)、\
        买一价(bid1_price)、买一量(bid1_volume, 手)、卖一价(ask1_price)、卖一量(ask1_volume, 手)、\
        累计撮合量(volume, 手)、累计成交额(amount)、数据时间(time)。\
        用法：竞价期(9:15-9:25)调用，判断开盘方向——竞价涨幅>3%且买一量>卖一量 = 强势开盘；\
        竞价跌幅>3% = 弱势；涨停竞价放量 = 可能开盘封板。支持批量查询多个代码。\
        参数：codes(逗号分隔的股票代码列表，如 '000001,600519')。",
        str_schema(&[("codes", "逗号分隔的股票代码，如 000001,600519")]),
        Arc::new(|_id, args, _signal, _progress| {
            Box::pin(async move {
                let codes_str = str_arg(&args, "codes");
                let codes: Vec<&str> = codes_str
                    .split(',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .collect();
                if codes.is_empty() {
                    return Err("codes 不能为空".to_string().into());
                }
                let client = openatrading_api::sina::SinaQuoteClient::new().map_err(to_box_err)?;
                let result = client.fetch_quotes(&codes).await.map_err(to_box_err)?;
                Ok(text_result(&result))
            })
        }),
    )
}

// ─── 元技能：生成新策略 ───────────────────────────────

pub fn create_meta_skill_tool() -> Arc<DynTool> {
    tool(
        "create_skill",
        "元技能：根据市场观察和分析，生成并安装一个新的交易策略技能(SKILL.md)。\
        当你在决策过程中发现新的可复用的交易模式、特定市场环境下的有效策略、\
        或值得固化下来的交易规则时，调用此工具将其沉淀为技能，供后续决策复用。\
        参数：skill_name(技能名称，英文短名如 'breakout-pullback')、\
        content(策略内容，Markdown格式，包含策略规则、适用条件、买卖信号等)。",
        serde_json::json!({
            "type": "object",
            "properties": {
                "skill_name": {"type": "string", "description": "技能名称，英文短名"},
                "content": {"type": "string", "description": "策略内容，Markdown格式"}
            },
            "required": ["skill_name", "content"]
        }),
        Arc::new(|_id, args, _signal, _progress| {
            Box::pin(async move {
                let name = args
                    .get("skill_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unnamed");
                let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
                if content.is_empty() {
                    return Err("content 不能为空".to_string().into());
                }
                let skill = crate::skills::SkillRegistry::parse_skill_from_md(content)
                    .map(|mut s| {
                        s.name = name.to_string();
                        s
                    })
                    .ok_or_else(|| to_box_err("content 为空"))?;
                let path = crate::skills::SkillRegistry::install_skill(&skill)
                    .map_err(|e| -> Box<dyn Error + Send + Sync> { e.to_string().into() })?;
                Ok(AgentToolResult {
                    content: vec![ContentBlock::text(format!(
                        "技能已安装: {} ({})",
                        skill.name,
                        path.display()
                    ))],
                    details: serde_json::Value::Null,
                    terminate: None,
                })
            })
        }),
    )
}

// ============================================================================
// ToolRegistry — categorized tool access
// ============================================================================

/// Stateless registry of all available tools, grouped by purpose.
/// Tools are created once on construction and reused.
pub struct ToolRegistry {
    market: Vec<Arc<DynTool>>,
    mcp: Vec<Arc<DynTool>>,
    decision: Option<Arc<DynTool>>,
    meta_skill: Arc<DynTool>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            market: vec![
                get_limit_up(),
                get_limit_down(),
                get_market_overview(),
                get_super_stock(),
                get_yesterday_limit_up(),
                get_dapan_abnormal(),
                get_realtime_quote(),
                get_plates_by_stock(),
                get_market_trend(),
                get_plate_abnormal(),
                get_kline(),
                get_telegraph(),
                get_telegraph_allday(),
                get_all_day_events(),
                get_auction_quotes(),
            ],
            decision: Some(create_decision_tool()),
            mcp: Vec::new(),
            meta_skill: create_meta_skill_tool(),
        }
    }

    /// All market-analysis tools (limit-up, overview, kline, plates, …).
    pub fn market_tools(&self) -> &[Arc<DynTool>] {
        &self.market
    }

    /// Decision extraction tool (`submit_trading_decision`).
    pub fn decision_tool(&self) -> Option<&Arc<DynTool>> {
        self.decision.as_ref()
    }

    /// Meta-skill tool for generating new strategies.
    pub fn meta_skill_tool(&self) -> &Arc<DynTool> {
        &self.meta_skill
    }

    /// Register an MCP tool.
    pub fn add_mcp(&mut self, tool: Arc<DynTool>) {
        self.mcp.push(tool);
    }

    pub fn mcp_tools(&self) -> &[Arc<DynTool>] {
        &self.mcp
    }

    /// Market tools + meta skill + decision tool, ready for an agent_loop call.
    pub fn all_tools(&self) -> Vec<Arc<DynTool>> {
        let mut t = self.market.clone();
        t.push(self.meta_skill.clone());
        t.extend(self.mcp.clone());
        if let Some(d) = &self.decision {
            t.push(d.clone());
        }
        t
    }
}

// ============================================================================
// Decision extraction tool (embedded schema — no second LLM call)
// ============================================================================

/// Creates a "passive" tool whose arguments **are** the parsed `DecisionOutput`.
/// The tool does nothing except receive structured data; `terminate: true`
/// stops the agent loop immediately after the call.
pub fn create_decision_tool() -> Arc<DynTool> {
    let schema = schemars::schema_for!(crate::roles::prompt::DecisionOutput);
    let schema_value = serde_json::to_value(&schema).unwrap_or_default();

    tool(
        "submit_trading_decision",
        "提交交易决策。调用此工具表示已完成分析，输出结构化的分析报告和交易指令列表。\
        每个交易指令包含：action(Buy/Sell/Hold)、symbol(股票代码)、\
        name(股票名称)、shares(股数，100的整数倍)、reason(决策理由)。\
        ⚠️ symbol 必须使用 Yahoo Finance 风格带市场后缀：\
        沪市主板/科创板用 .SS（如 600519.SS、688981.SS），\
        深市主板/中小板/创业板用 .SZ（如 000001.SZ、002008.SZ、300750.SZ），\
        北交所用 .BJ（如 873169.BJ）。\
        禁止输出裸 6 位代码（如 002008），否则后续行情拉取会失败。\
        注意：必须基于实际工具返回的数据做决策，不得凭空猜测。",
        schema_value,
        Arc::new(|_id, _args, _signal, _progress| {
            Box::pin(async move {
                Ok(AgentToolResult {
                    content: vec![ContentBlock::text("决策已接收")],
                    details: serde_json::Value::Null,
                    terminate: Some(true),
                })
            })
        }),
    )
}

// ─── Tests ──────────────────────────────────────────────

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
        assert_eq!(get_all_day_events().name, "get_all_day_events");
        assert_eq!(get_auction_quotes().name, "get_auction_quotes");
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
            get_all_day_events(),
            get_auction_quotes(),
            create_meta_skill_tool(),
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
        assert_eq!(tools.len(), 20);
    }

    #[test]
    fn test_realtime_quote_has_symbol_param() {
        let tool = get_realtime_quote();
        assert!(tool.parameters_schema["properties"]["symbol"].is_object());
        assert!(tool.parameters_schema["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::Value::String("symbol".into())));
    }

    #[test]
    fn test_lhb_detail_has_date_params() {
        let tool = get_lhb_detail();
        assert!(tool.parameters_schema["properties"]["start_date"].is_object());
        assert!(tool.parameters_schema["properties"]["end_date"].is_object());
    }

    #[test]
    fn test_plate_abnormal_has_count_param() {
        let tool = get_plate_abnormal();
        assert!(tool.parameters_schema["properties"]["count"].is_object());
    }
}
