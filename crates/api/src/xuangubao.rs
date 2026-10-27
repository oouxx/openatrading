use anyhow::{Context, Result};
use chrono::{Datelike, FixedOffset};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, time::Duration};

use crate::utils::to_yahoo_symbol;

/// UTC+8 Beijing time — all A-share market clock calculations should use this.
fn beijing_tz() -> FixedOffset {
    FixedOffset::east_opt(8 * 3600).unwrap_or_else(|| panic!("8*3600 is a valid offset"))
}

/// Convert f64 to i64, rounding to nearest integer.
/// Timestamps from the API are always whole numbers.
fn f64_to_i64(v: f64) -> i64 {
    v as i64
}

/// 板块事件取板块涨幅，个股事件取个股涨幅。
fn event_pcp(event: &PlateOrStockAbnormalEvent) -> Option<f64> {
    event
        .plate_abnormal_event_data
        .as_ref()
        .and_then(|data| data.pcp)
        .or_else(|| {
            event
                .stock_abnormal_event_data
                .as_ref()
                .and_then(|data| data.pcp)
        })
}

/// 按事件 id 去重，保留第一次出现的事件。
///
/// 上游在同一秒可能重复返回同一个事件，且两条记录之间夹着别的事件；
/// 这类重复不是相邻的，不能用 `Vec::dedup_by_key`。
fn dedup_events_by_id(events: &mut Vec<PlateOrStockAbnormalEvent>) {
    let mut seen = std::collections::HashSet::with_capacity(events.len());
    events.retain(|event| seen.insert(event.id));
}

const BASE_URL: &str = "https://flash-api.xuangubao.com.cn/api";
const MARKET_BASE_URL: &str = "https://api-ddc-wscn.xuangubao.com.cn";
const PLATE_INDEX_TYPE: &str = "1";
const PLATE_INDEX_DATA_TYPE: &str = "2";

/// 前端按 200 个标的拆分资金流批量请求，避免 URL 过长和单次响应过大。
const FUND_FLOW_BATCH_SIZE: usize = 200;
/// 前端按 200 个标的拆分股票指标批量请求。
const STOCK_DATA_BATCH_SIZE: usize = 200;
/// 前端按 200 个标的拆分股票标签批量请求。
const STOCK_LABEL_BATCH_SIZE: usize = 200;
const PLATE_SUMMARY_BATCH_SIZE: usize = 200;
const FUND_FLOW_METRIC_START: usize = 2;
const FUND_FLOW_METRIC_END: usize = 19;
const FUND_FLOW_ROW_LEN: usize = 20;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlateReason {
    #[serde(default)]
    pub plate_id: i64,
    pub plate_name: String,
    #[serde(default)]
    pub plate_reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SurgeReason {
    pub symbol: String,
    pub stock_reason: String,
    pub related_plates: Vec<PlateReason>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LimitTimelineItem {
    pub timestamp: i64,
    pub status: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LimitTimeline {
    pub items: Vec<LimitTimelineItem>,
}

// ─── 板块异动数据 ───────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RelatedStock {
    pub symbol: String,
    pub name: String,
    #[serde(default)]
    pub mtm: f64,
    #[serde(default)]
    pub pcp: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct PlateAbnormalEventData {
    #[serde(default)]
    pub plate_id: Option<i64>,
    #[serde(default)]
    pub plate_name: Option<String>,
    #[serde(default)]
    pub pcp: Option<f64>,
    #[serde(default)]
    pub related_stocks: Vec<RelatedStock>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct StockAbnormalEventData {
    pub symbol: Option<String>,
    pub name: Option<String>,
    pub pcp: Option<f64>,
    pub mtm: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlateOrStockAbnormalEvent {
    pub id: i64,
    pub target: String,
    pub event_type: i32,
    pub event_timestamp: i64,
    #[serde(default)]
    pub stock_abnormal_event_data: Option<StockAbnormalEventData>,
    #[serde(default)]
    pub plate_abnormal_event_data: Option<PlateAbnormalEventData>,
    pub good_or_bad: i32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DapanAbnormal {
    #[serde(alias = "Id")]
    pub id: String,
    #[serde(alias = "Title")]
    pub title: String,
    #[serde(alias = "BkYiDongType")]
    pub bk_yi_dong_type: i32,
    #[serde(alias = "BkjInfo")]
    pub bkj_info: BkjInfo,
    #[serde(alias = "CreatedAt")]
    pub created_at: i64,
    #[serde(alias = "UpdatedAt")]
    pub updated_at: i64,
}
// 板块信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BkjInfo {
    #[serde(alias = "Id")]
    pub id: String,
    #[serde(alias = "Name")]
    pub name: String,
}

/*
      "BkYiDongType": 1,
      "BkjInfo": {
        "Id": "56_044_969",
        "Name": "数字人民币"
      },
      "CreatedAt": 1_775_797_961,
      "UpdatedAt": 1_775_798_032


*/
#[derive(Debug, Deserialize, Clone)]
pub struct ApiResponse<T> {
    pub code: i32,
    pub message: String,
    pub data: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stock {
    pub symbol: String,
    pub stock_chi_name: String,
    pub price: f64,
    pub change_percent: f64,
    pub limit_up_days: u32,
    pub first_limit_up: i64,
    pub last_limit_up: i64,
    pub break_limit_up_times: u32,
    pub first_break_limit_up: i64,
    pub last_break_limit_up: i64,
    pub limit_down_days: u32,
    pub first_limit_down: i64,
    pub last_limit_down: i64,
    pub break_limit_down_times: u32,
    pub first_break_limit_down: i64,
    pub last_break_limit_down: i64,
    pub yesterday_first_limit_up: i64,
    pub yesterday_last_limit_up: i64,
    pub yesterday_break_limit_up_times: u32,
    pub yesterday_limit_up_days: u32,
    pub yesterday_limit_down_days: u32,
    pub non_restricted_capital: f64,
    pub total_capital: f64,
    pub turnover_ratio: f64,
    pub volume_bias_ratio: f64,
    pub buy_lock_volume_ratio: f64,
    pub sell_lock_volume_ratio: f64,
    pub stock_type: i32,
    pub is_new_stock: bool,
    pub issue_price: f64,
    pub listed_date: i64,
    pub surge_reason: Option<SurgeReason>,
    #[serde(default)]
    pub limit_timeline: Option<LimitTimeline>,
    pub mtm: f64,
    pub m_days_n_boards_boards: u32,
    pub m_days_n_boards_days: u32,
    pub nearly_new_acc_pcp: f64,
    pub nearly_new_break_days: u32,
    pub new_stock_acc_pcp: f64,
    pub new_stock_break_limit_up: u32,
    pub new_stock_limit_up_days: u32,
    pub new_stock_limit_up_price_before_broken: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketOverview {
    pub limit_up_count: u32,
    pub limit_down_count: u32,
    pub rise_count: u32,
    pub fall_count: u32,
    pub bomb_rate: f64,
    pub market_temperature: f64,
    pub yesterday_limit_up_avg_pcp: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketOverviewDay {
    pub date: String,
    pub rise_count: u32,
    pub fall_count: u32,
    pub limit_up_count: u32,
    pub limit_down_count: u32,
    pub bomb_rate: f64,
    pub market_temperature: f64,
}

/// 单个证券的市场类型元数据。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityTypeMetadata {
    pub market_type: String,
    pub securities_type: String,
    #[serde(default)]
    pub sub_type: String,
}

/// 市场指标分时序列中的一个时点。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketIndicatorPoint {
    pub timestamp: i64,
    #[serde(default)]
    pub rise_count: Option<u32>,
    #[serde(default)]
    pub fall_count: Option<u32>,
    #[serde(default)]
    pub limit_up_count: Option<u32>,
    #[serde(default)]
    pub limit_down_count: Option<u32>,
    #[serde(default)]
    pub limit_up_broken_count: Option<u32>,
    #[serde(default)]
    pub limit_up_broken_ratio: Option<f64>,
    #[serde(default)]
    pub market_temperature: Option<f64>,
    #[serde(default)]
    pub yesterday_limit_up_avg_pcp: Option<f64>,
}

/// 全市场涨跌幅分布。
///
/// `buckets` 的键是涨跌幅档位编码（例如 `10`、`0`、`-10`），值是股票数量。
/// 上游没有公开档位与百分比的换算规则，因此这里保留原始编码。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MarketPcpDistribution {
    pub buckets: BTreeMap<i32, u32>,
    pub limit_up_count: u32,
    pub limit_down_count: u32,
    pub st_limit_up_count: u32,
    pub st_limit_down_count: u32,
    pub halt_count: u32,
    pub total_count: u32,
    pub timestamp: i64,
}

/// 题材摘要。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlateSummary {
    pub plate_id: i64,
    pub plate_name: String,
    #[serde(default)]
    pub core_avg_pcp: Option<f64>,
    #[serde(default)]
    pub fund_flow: Option<f64>,
    #[serde(default)]
    pub rise_count: u32,
    #[serde(default)]
    pub fall_count: u32,
    #[serde(default)]
    pub stay_count: u32,
    #[serde(default)]
    pub limit_up_count: u32,
    #[serde(default)]
    pub core_avg_pcp_rank: Option<i32>,
    #[serde(default)]
    pub core_avg_pcp_rank_change: Option<i32>,
    #[serde(default)]
    pub top_n_stocks: PlateLeaderStocks,
    #[serde(default)]
    pub bottom_n_stocks: PlateLeaderStocks,
    #[serde(default, rename = "jinrilongtou")]
    pub today_leaders: Option<Vec<String>>,
    #[serde(default, rename = "renqilongtou")]
    pub popular_leaders: Option<Vec<String>>,
    #[serde(default, rename = "hangyelongtou")]
    pub industry_leaders: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlateLeaderStocks {
    #[serde(default)]
    pub items: Vec<PlateLeaderStock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlateLeaderStock {
    #[serde(default)]
    pub symbol: String,
    #[serde(default)]
    pub stock_chi_name: String,
    #[serde(default)]
    pub change_percent: Option<f64>,
    #[serde(default)]
    pub price_change: Option<f64>,
}

/// 按涨幅分组的板块排行。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlateRankings {
    pub gainers: Vec<PlateSummary>,
    pub losers: Vec<PlateSummary>,
}

/// 题材涨跌榜使用的上游类型编码。
///
/// 文档只确认前端使用 `1`、`2`，没有给出稳定的“上涨/下跌”语义，因此不要自行推断。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlateIncdecType {
    One,
    Two,
}

impl PlateIncdecType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::One => "1",
            Self::Two => "2",
        }
    }
}

/// 板块涨跌幅榜分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlateRankType {
    #[default]
    All,
    Concept,
    Industry,
    Style,
}

impl PlateRankType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::All => "0",
            Self::Concept => "1",
            Self::Industry => "2",
            Self::Style => "3",
        }
    }
}

/// 题材涨跌榜中的代表股。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlateMoverStock {
    pub symbol: String,
    pub name: String,
    #[serde(default)]
    pub mtm: Option<f64>,
}

/// 题材涨跌榜条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlateMover {
    pub plate_id: i64,
    pub plate_name: String,
    pub pcp: f64,
    #[serde(default)]
    pub stocks: Vec<PlateMoverStock>,
}

/// 题材指数历史 K 线。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlateIndexCandle {
    #[serde(rename = "date_time")]
    pub timestamp: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
}

/// 题材指数分时点。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlateIndexPoint {
    #[serde(rename = "time")]
    pub timestamp: i64,
    pub index: f64,
}

/// 题材指数分时序列。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlateIndexRealtime {
    pub prev_close: f64,
    pub items: Vec<PlateIndexPoint>,
}

/// 题材成分股。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlateStock {
    pub symbol: String,
    #[serde(default)]
    pub core_flag: Option<i32>,
    #[serde(default)]
    pub desc: String,
    #[serde(default)]
    pub desc_url: Option<String>,
    #[serde(default)]
    pub industrial_chain_id: Option<i64>,
    #[serde(default)]
    pub is_new: bool,
    #[serde(default)]
    pub need_pay: bool,
}

/// 题材产业链。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlateIndustrialChain {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub order: i32,
    #[serde(default)]
    pub stocks: Vec<String>,
    #[serde(default)]
    pub stock_premium_infos: Vec<PlateStock>,
}

/// 题材常见问题。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlateQuestionAnswer {
    #[serde(default)]
    pub question: String,
    #[serde(default)]
    pub html_answer: String,
}

/// 题材子集。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlateSubset {
    pub plate_id: i64,
    pub plate_name: String,
}

/// 题材详情及成分股。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlateDetail {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub desc: String,
    #[serde(default)]
    pub avg_pcp: Option<f64>,
    #[serde(default)]
    pub core_avg_pcp: Option<f64>,
    /// 上游返回的题材资金流数值；公开文档没有给出单位。
    #[serde(default)]
    pub fund_flow: Option<f64>,
    #[serde(default)]
    pub rise_count: u32,
    #[serde(default)]
    pub fall_count: u32,
    #[serde(default)]
    pub stay_count: u32,
    #[serde(default)]
    pub core_stocks_count: u32,
    #[serde(default)]
    pub stocks: Vec<PlateStock>,
    #[serde(default)]
    pub industrial_chains: Vec<PlateIndustrialChain>,
    #[serde(default)]
    pub allow_comment: bool,
    #[serde(default)]
    pub create_name: String,
    #[serde(default)]
    pub display_after_at: i64,
    #[serde(default)]
    pub hang_ye_long_tou_stocks_count: u32,
    #[serde(default)]
    pub has_north: i32,
    #[serde(default)]
    pub hide: bool,
    #[serde(default)]
    pub is_fav: bool,
    #[serde(default)]
    pub need_pay: bool,
    #[serde(default)]
    pub premium_desc: String,
    #[serde(default)]
    pub question_answers: Vec<PlateQuestionAnswer>,
    #[serde(default)]
    pub seo_desc: String,
    #[serde(default)]
    pub seo_keywords: String,
    #[serde(default)]
    pub seo_title: String,
    #[serde(default)]
    pub sub_sets: Option<Vec<PlateSubset>>,
    #[serde(default)]
    pub subject_id: i64,
    #[serde(default)]
    pub update_name: String,
    #[serde(default)]
    pub whether_display_plate_stock_portfolio: bool,
}

/// 单个标的单个交易日的资金流。
///
/// 上游返回 20 列紧凑数组。文档只稳定定义了标的、交易日、中间指标区和末列时间戳；
/// `metrics` 按响应原顺序保存第 2..18 列，避免把厂商内部列序误标成业务含义。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundFlowRow {
    pub symbol: String,
    pub trade_date: String,
    pub metrics: Vec<f64>,
    pub timestamp: i64,
}

/// 选股宝股票指标快照，用于补充涨停天数、最新涨停时间等字段。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StockMetrics {
    pub symbol: String,
    #[serde(default)]
    pub stock_chi_name: String,
    #[serde(default)]
    pub change_percent: f64,
    #[serde(default)]
    pub price: f64,
    #[serde(default)]
    pub turnover_ratio: f64,
    #[serde(default)]
    pub non_restricted_capital: f64,
    #[serde(default)]
    pub total_capital: f64,
    #[serde(default)]
    pub per: f64,
    #[serde(default)]
    pub limit_up_days: u32,
    #[serde(default)]
    pub last_limit_up: i64,
    #[serde(default)]
    pub limit_status: i32,
    #[serde(default)]
    pub nearly_new_acc_pcp: f64,
}

/// 选股宝股票标签。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StockLabel {
    #[serde(default)]
    pub code: String,
    /// 该字段在上游部分记录中存放股票名称，而不是证券代码。
    #[serde(default)]
    pub symbol: String,
    #[serde(default)]
    pub label_name: String,
    #[serde(default)]
    pub label_type: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub effect: i32,
}

/// 选股宝 API `fields` 请求参数中的单个字段。
///
/// 通过 `ApiField::join(&[variant, ...])` 生成逗号分隔字符串，
/// 每个 endpoint 在其方法中显式声明所需的字段列表。
#[derive(Debug, Clone, Copy)]
pub enum ApiField {
    /// 上涨家数
    RiseCount,
    /// 下跌家数
    FallCount,
    /// 涨停家数
    LimitUpCount,
    /// 跌停家数
    LimitDownCount,
    /// 炸板率
    LimitUpBrokenRatio,
    /// 炸板家数
    LimitUpBrokenCount,
    /// 市场情绪温度
    MarketTemperature,
    /// 昨日涨停股今日平均涨幅
    YesterdayLimitUpAvgPcp,
    /// 时间戳（分时/K线）
    TickAt,
    /// 时间戳（市场指标）
    Timestamp,
    /// 最新价（分时/K线）
    ClosePx,
    /// 开盘价
    OpenPx,
    /// 最高价
    HighPx,
    /// 最低价
    LowPx,
    /// 成交量
    TurnoverVolume,
    /// 涨跌幅
    PxChangeRate,
    /// 产品名称
    ProdName,
    /// 最新价（实时）
    LastPx,
    /// 涨跌额
    PxChange,
    /// 股票代码
    Symbol,
    /// 交易状态
    TradeStatus,
    /// 更新时间
    UpdateTime,
    /// 昨收价
    PreclosePx,
    /// 振幅
    Amplitude,
    /// 换手率
    TurnoverRatio,
    /// 市盈率
    PeRate,
    /// 动态市盈率
    DynPe,
    /// 市净率
    DynPbRate,
    /// 总市值
    MarketValue,
    /// 流通市值
    CirculationValue,
    /// 成交额
    TurnoverValue,
    /// 量比
    VolumeRatio,
    /// 流通股本
    CirculationShares,
    /// 总股本
    TotalShares,
    /// 板块核心涨幅
    CoreAvgPcp,
    /// 板块名称
    PlateName,
}

impl ApiField {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::RiseCount => "rise_count",
            Self::FallCount => "fall_count",
            Self::LimitUpCount => "limit_up_count",
            Self::LimitDownCount => "limit_down_count",
            Self::LimitUpBrokenRatio => "limit_up_broken_ratio",
            Self::LimitUpBrokenCount => "limit_up_broken_count",
            Self::MarketTemperature => "market_temperature",
            Self::YesterdayLimitUpAvgPcp => "yesterday_limit_up_avg_pcp",
            Self::TickAt => "tick_at",
            Self::Timestamp => "timestamp",
            Self::ClosePx => "close_px",
            Self::OpenPx => "open_px",
            Self::HighPx => "high_px",
            Self::LowPx => "low_px",
            Self::TurnoverVolume => "turnover_volume",
            Self::PxChangeRate => "px_change_rate",
            Self::ProdName => "prod_name",
            Self::LastPx => "last_px",
            Self::PxChange => "px_change",
            Self::Symbol => "symbol",
            Self::TradeStatus => "trade_status",
            Self::UpdateTime => "update_time",
            Self::PreclosePx => "preclose_px",
            Self::Amplitude => "amplitude",
            Self::TurnoverRatio => "turnover_ratio",
            Self::PeRate => "pe_rate",
            Self::DynPe => "dyn_pe",
            Self::DynPbRate => "dyn_pb_rate",
            Self::MarketValue => "market_value",
            Self::CirculationValue => "circulation_value",
            Self::TurnoverValue => "turnover_value",
            Self::VolumeRatio => "volume_ratio",
            Self::CirculationShares => "circulation_shares",
            Self::TotalShares => "total_shares",
            Self::CoreAvgPcp => "core_avg_pcp",
            Self::PlateName => "plate_name",
        }
    }

    /// 将多个字段拼接为逗号分隔的字符串（即 `fields=...` 参数值）。
    pub fn join(fields: &[Self]) -> String {
        fields
            .iter()
            .map(Self::as_str)
            .collect::<Vec<_>>()
            .join(",")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketTrend {
    pub symbol: String,
    pub candles: Vec<KlineCandle>,
}

/// Parse the trend endpoint response using the field order declared by the API.
///
/// The trend endpoint returns `data.fields` plus parallel `data.candle.<symbol>.lines`
/// arrays. Parsing by field name avoids depending on a fixed column order.
fn parse_market_trend(symbol: &str, resp: &serde_json::Value) -> Result<MarketTrend> {
    let candle_data = resp["data"]["candle"]
        .get(symbol)
        .with_context(|| format!("missing trend data for {symbol}"))?;
    let lines = candle_data["lines"]
        .as_array()
        .with_context(|| format!("missing trend lines for {symbol}"))?;
    let fields: Vec<String> = resp["data"]["fields"]
        .as_array()
        .context("missing trend fields array")?
        .iter()
        .filter_map(|value| value.as_str().map(String::from))
        .collect();

    let candles: Vec<KlineCandle> = lines
        .iter()
        .filter_map(|line| {
            let arr = line.as_array()?;
            let get = |name: &str| -> Option<f64> {
                let idx = fields.iter().position(|field| field == name)?;
                arr.get(idx)?.as_f64()
            };
            Some(KlineCandle {
                time: get("tick_at").map(f64_to_i64)?,
                open: get("open_px"),
                close: get("close_px")?,
                high: get("high_px"),
                low: get("low_px"),
                volume: get("turnover_volume"),
                change_rate: get("px_change_rate"),
            })
        })
        .collect();

    Ok(MarketTrend {
        symbol: symbol.to_string(),
        candles,
    })
}

fn ensure_api_success(resp: &serde_json::Value, operation: &str) -> Result<()> {
    let code = resp
        .get("code")
        .and_then(serde_json::Value::as_i64)
        .with_context(|| format!("{operation}响应缺少业务状态码"))?;
    if code != 20000 {
        let message = resp
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("上游未提供错误说明");
        anyhow::bail!("{operation}失败: {message}");
    }
    Ok(())
}

fn parse_security_type_metadata(
    symbol: &str,
    resp: &serde_json::Value,
) -> Result<SecurityTypeMetadata> {
    ensure_api_success(resp, "证券类型查询")?;
    let value = resp["data"]
        .get(symbol)
        .with_context(|| format!("证券类型响应缺少 {symbol}"))?;
    serde_json::from_value(value.clone()).context("证券类型字段格式异常")
}

fn parse_market_indicators(resp: &serde_json::Value) -> Result<Vec<MarketIndicatorPoint>> {
    ensure_api_success(resp, "市场指标查询")?;
    let data = resp["data"]
        .as_array()
        .context("市场指标响应缺少 data 数组")?;
    data.iter()
        .map(|value| serde_json::from_value(value.clone()).context("市场指标字段格式异常"))
        .collect()
}

fn json_u32(value: Option<&serde_json::Value>, field: &str) -> Result<u32> {
    let number = value
        .with_context(|| format!("涨跌幅分布缺少 {field}"))?
        .as_u64()
        .with_context(|| format!("涨跌幅分布 {field} 不是非负整数"))?;
    u32::try_from(number).with_context(|| format!("涨跌幅分布 {field} 超出范围"))
}

fn json_i64(value: Option<&serde_json::Value>, field: &str) -> Result<i64> {
    value
        .with_context(|| format!("涨跌幅分布缺少 {field}"))?
        .as_i64()
        .with_context(|| format!("涨跌幅分布 {field} 不是整数"))
}

fn parse_market_pcp_distribution(resp: &serde_json::Value) -> Result<MarketPcpDistribution> {
    ensure_api_success(resp, "涨跌幅分布查询")?;
    let data = resp["data"]
        .as_object()
        .context("涨跌幅分布响应缺少 data 对象")?;

    let mut distribution = MarketPcpDistribution {
        limit_up_count: json_u32(data.get("limit_up_count"), "limit_up_count")?,
        limit_down_count: json_u32(data.get("limit_down_count"), "limit_down_count")?,
        st_limit_up_count: json_u32(data.get("st_limit_up_count"), "st_limit_up_count")?,
        st_limit_down_count: json_u32(data.get("st_limit_down_count"), "st_limit_down_count")?,
        halt_count: json_u32(data.get("halt_count"), "halt_count")?,
        total_count: json_u32(data.get("total_count"), "total_count")?,
        timestamp: json_i64(data.get("ts"), "ts")?,
        ..MarketPcpDistribution::default()
    };

    for (bucket, value) in data {
        let Ok(bucket) = bucket.parse::<i32>() else {
            continue;
        };
        let count = value
            .as_u64()
            .with_context(|| format!("涨跌幅档位 {bucket} 的股票数不是非负整数"))?;
        distribution.buckets.insert(
            bucket,
            u32::try_from(count)
                .with_context(|| format!("涨跌幅档位 {bucket} 的股票数超出范围"))?,
        );
    }

    Ok(distribution)
}

fn parse_plate_summaries(resp: &serde_json::Value) -> Result<Vec<PlateSummary>> {
    ensure_api_success(resp, "题材摘要查询")?;
    let data = resp["data"]
        .as_object()
        .context("题材摘要响应缺少 data 对象")?;
    data.values()
        .map(|value| serde_json::from_value(value.clone()).context("题材摘要字段格式异常"))
        .collect()
}

fn parse_plate_rank_ids(resp: &serde_json::Value) -> Result<Vec<i64>> {
    ensure_api_success(resp, "板块排行榜查询")?;
    resp["data"]
        .as_array()
        .context("板块排行榜响应缺少 data 数组")?
        .iter()
        .map(|value| value.as_i64().context("板块排行榜 ID 不是整数"))
        .collect()
}

fn parse_plate_incdec(resp: &serde_json::Value) -> Result<Vec<PlateMover>> {
    ensure_api_success(resp, "题材涨跌榜查询")?;
    let items = resp["data"]["items"]
        .as_array()
        .context("题材涨跌榜响应缺少 data.items 数组")?;
    items
        .iter()
        .map(|value| serde_json::from_value(value.clone()).context("题材涨跌榜字段格式异常"))
        .collect()
}

fn parse_plate_index_history(resp: &serde_json::Value) -> Result<Vec<PlateIndexCandle>> {
    ensure_api_success(resp, "题材指数历史查询")?;
    let data = resp["data"]
        .as_array()
        .context("题材指数历史响应缺少 data 数组")?;
    data.iter()
        .map(|value| serde_json::from_value(value.clone()).context("题材指数历史字段格式异常"))
        .collect()
}

fn parse_plate_index_realtime(resp: &serde_json::Value) -> Result<PlateIndexRealtime> {
    ensure_api_success(resp, "题材指数分时查询")?;
    let data = resp["data"]
        .as_object()
        .context("题材指数分时响应缺少 data 对象")?;
    let prev_close = data
        .get("prev_close_idx")
        .and_then(serde_json::Value::as_f64)
        .context("题材指数分时响应缺少 prev_close_idx")?;
    let items = data
        .get("items")
        .and_then(serde_json::Value::as_array)
        .context("题材指数分时响应缺少 data.items 数组")?
        .iter()
        .map(|value| serde_json::from_value(value.clone()).context("题材指数分时字段格式异常"))
        .collect::<Result<Vec<_>>>()?;
    Ok(PlateIndexRealtime { prev_close, items })
}

fn parse_plate_detail(resp: &serde_json::Value) -> Result<PlateDetail> {
    ensure_api_success(resp, "题材详情查询")?;
    let data = resp.get("data").context("题材详情响应缺少 data")?;
    serde_json::from_value(data.clone()).context("题材详情字段格式异常")
}

fn parse_fund_flow_number(value: &serde_json::Value, column: usize) -> Result<f64> {
    value
        .as_f64()
        .with_context(|| format!("资金流第 {column} 列不是数值"))
}

fn parse_fund_flow_row(value: &serde_json::Value) -> Result<FundFlowRow> {
    let row = value.as_array().context("资金流行不是数组")?;
    anyhow::ensure!(
        row.len() == FUND_FLOW_ROW_LEN,
        "资金流行列数异常: 期望 {FUND_FLOW_ROW_LEN}，实际 {}",
        row.len()
    );

    let symbol = row[0].as_str().context("资金流第 0 列不是证券代码")?;
    let trade_date = row[1].as_str().context("资金流第 1 列不是交易日期")?;
    chrono::NaiveDate::parse_from_str(trade_date, "%Y-%m-%d")
        .with_context(|| format!("资金流交易日期格式异常: {trade_date}"))?;

    let metrics = row[FUND_FLOW_METRIC_START..FUND_FLOW_METRIC_END]
        .iter()
        .enumerate()
        .map(|(offset, value)| parse_fund_flow_number(value, offset + FUND_FLOW_METRIC_START))
        .collect::<Result<Vec<_>>>()?;
    let timestamp = row[FUND_FLOW_METRIC_END]
        .as_i64()
        .context("资金流末列不是 Unix 秒时间戳")?;

    Ok(FundFlowRow {
        symbol: symbol.to_string(),
        trade_date: trade_date.to_string(),
        metrics,
        timestamp,
    })
}

fn parse_fund_flow_batch(
    resp: &serde_json::Value,
) -> Result<ahash::AHashMap<String, Vec<FundFlowRow>>> {
    ensure_api_success(resp, "资金流查询")?;
    let data = resp["data"]
        .as_object()
        .context("资金流响应缺少 data 对象")?;
    let mut result = ahash::AHashMap::with_capacity(data.len());

    for (key, series) in data {
        let rows = series["fund_flow"]
            .as_array()
            .with_context(|| format!("资金流响应缺少 {key}.fund_flow 数组"))?
            .iter()
            .map(parse_fund_flow_row)
            .collect::<Result<Vec<_>>>()?;
        for row in &rows {
            anyhow::ensure!(
                row.symbol == *key,
                "资金流响应键 {key} 与行内证券代码 {} 不一致",
                row.symbol
            );
        }
        result.insert(key.clone(), rows);
    }

    Ok(result)
}

fn parse_stock_data(resp: &serde_json::Value) -> Result<ahash::AHashMap<String, StockMetrics>> {
    ensure_api_success(resp, "股票指标查询")?;
    let data = resp["data"]
        .as_object()
        .context("股票指标响应缺少 data 对象")?;
    data.iter()
        .map(|(symbol, value)| {
            let metrics: StockMetrics = serde_json::from_value(value.clone())
                .with_context(|| format!("股票指标 {symbol} 字段格式异常"))?;
            Ok((symbol.clone(), metrics))
        })
        .collect()
}

fn parse_stock_labels(
    resp: &serde_json::Value,
) -> Result<ahash::AHashMap<String, Vec<StockLabel>>> {
    ensure_api_success(resp, "股票标签查询")?;
    let data = resp["data"]
        .as_object()
        .context("股票标签响应缺少 data 对象")?;
    data.iter()
        .map(|(symbol, value)| {
            if value.is_null() {
                return Ok((symbol.clone(), Vec::new()));
            }
            let labels = value
                .as_array()
                .with_context(|| format!("股票标签 {symbol} 不是数组"))?
                .iter()
                .map(|label| {
                    serde_json::from_value(label.clone())
                        .with_context(|| format!("股票标签 {symbol} 字段格式异常"))
                })
                .collect::<Result<Vec<_>>>()?;
            Ok((symbol.clone(), labels))
        })
        .collect()
}

#[derive(Clone)]
pub struct XuanguBaoClient {
    client: reqwest::Client,
    base_url: String,
    market_base_url: String,
}

impl Default for XuanguBaoClient {
    fn default() -> Self {
        Self::new()
    }
}

impl XuanguBaoClient {
    pub fn new() -> Self {
        Self::with_urls(BASE_URL.to_string(), MARKET_BASE_URL.to_string())
    }

    pub fn with_base_url(base_url: String) -> Self {
        Self::with_urls(base_url, MARKET_BASE_URL.to_string())
    }

    fn with_urls(base_url: String, market_base_url: String) -> Self {
        let client = Self::build_client();
        Self {
            client,
            base_url,
            market_base_url,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn build_client() -> reqwest::Client {
        reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap_or_else(|e| panic!("failed to build reqwest client: {e}"))
    }

    #[cfg(target_arch = "wasm32")]
    fn build_client() -> reqwest::Client {
        reqwest::Client::new()
    }

    async fn fetch_pool(&self, pool_name: &str) -> Result<Vec<Stock>> {
        let url = format!("{}/pool/detail?pool_name={pool_name}", self.base_url);

        let resp: ApiResponse<Vec<Stock>> = self.client.get(&url).send().await?.json().await?;

        anyhow::ensure!(resp.code == 20000, "API 返回错误码: {}", resp.code);
        Ok(resp.data)
    }

    /// Fetch limit-up stocks.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn fetch_limit_up(&self) -> Result<Vec<Stock>> {
        self.fetch_pool("limit_up").await
    }

    /// Fetch limit-down stocks.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn fetch_limit_down(&self) -> Result<Vec<Stock>> {
        self.fetch_pool("limit_down").await
    }

    /// Fetch yesterday's limit-up stocks.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn fetch_yesterday_limit_up(&self) -> Result<Vec<Stock>> {
        self.fetch_pool("yesterday_limit_up").await
    }
    /// Fetch super stocks.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn fetch_super_stock(&self) -> Result<Vec<Stock>> {
        self.fetch_pool("super_stock").await
    }

    /// Fetch market overview data.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn fetch_market_overview(&self) -> Result<MarketOverview> {
        let fields = ApiField::join(&[
            ApiField::RiseCount,
            ApiField::FallCount,
            ApiField::LimitUpCount,
            ApiField::LimitDownCount,
            ApiField::LimitUpBrokenRatio,
            ApiField::MarketTemperature,
            ApiField::YesterdayLimitUpAvgPcp,
        ]);
        let url = format!("{}/market_indicator/line?fields={fields}", self.base_url);

        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;

        let latest = &resp["data"]
            .as_array()
            .and_then(|a| a.last())
            .cloned()
            .unwrap_or_default();

        Ok(MarketOverview {
            rise_count: u32::try_from(latest["rise_count"].as_u64().unwrap_or(0)).unwrap_or(0),
            fall_count: u32::try_from(latest["fall_count"].as_u64().unwrap_or(0)).unwrap_or(0),
            limit_up_count: u32::try_from(latest["limit_up_count"].as_u64().unwrap_or(0))
                .unwrap_or(0),
            limit_down_count: u32::try_from(latest["limit_down_count"].as_u64().unwrap_or(0))
                .unwrap_or(0),
            bomb_rate: latest["limit_up_broken_ratio"].as_f64().unwrap_or(0.0),
            market_temperature: latest["market_temperature"].as_f64().unwrap_or(50.0) as f64,
            yesterday_limit_up_avg_pcp: latest["yesterday_limit_up_avg_pcp"].as_f64().unwrap_or(0.0)
                as f64,
        })
    }

    /// 获取市场指标分时序列（涨跌家数、涨跌停、炸板、情绪温度等）。
    ///
    /// # Errors
    /// 请求失败、业务状态码异常或响应字段不符合约定时返回错误。
    pub async fn fetch_market_indicator_line(&self) -> Result<Vec<MarketIndicatorPoint>> {
        let fields = ApiField::join(&[
            ApiField::Timestamp,
            ApiField::RiseCount,
            ApiField::FallCount,
            ApiField::LimitUpCount,
            ApiField::LimitDownCount,
            ApiField::LimitUpBrokenCount,
            ApiField::LimitUpBrokenRatio,
            ApiField::MarketTemperature,
            ApiField::YesterdayLimitUpAvgPcp,
        ]);
        let resp: serde_json::Value = self
            .client
            .get(format!("{}/market_indicator/line", self.base_url))
            .query(&[("fields", fields)])
            .send()
            .await
            .context("市场指标请求失败")?
            .json()
            .await
            .context("市场指标响应不是合法 JSON")?;
        parse_market_indicators(&resp)
    }

    /// 获取全市场涨跌幅分布。
    ///
    /// # Errors
    /// 请求失败、业务状态码异常或响应字段不符合约定时返回错误。
    pub async fn fetch_market_pcp_distribution(&self) -> Result<MarketPcpDistribution> {
        let resp: serde_json::Value = self
            .client
            .get(format!(
                "{}/market_indicator/pcp_distribution",
                self.base_url
            ))
            .send()
            .await
            .context("涨跌幅分布请求失败")?
            .json()
            .await
            .context("涨跌幅分布响应不是合法 JSON")?;
        parse_market_pcp_distribution(&resp)
    }

    /// Fetch intraday market overview for a specific date, returns the last (most recent) snapshot.
    /// Generate the last `n` trading day dates (YYYY-MM-DD), skipping weekends.
    /// Generate the last `n` trading day dates (YYYY-MM-DD), skipping weekends.
    ///
    /// # Panics
    /// Panics if date underflow occurs (should never happen with current dates).
    pub fn last_n_trading_days(n: usize) -> Vec<String> {
        let mut dates = Vec::with_capacity(n);
        let mut current = chrono::Utc::now().with_timezone(&beijing_tz()).date_naive();
        // If today is a weekend, start from Friday
        while dates.len() < n {
            match current.weekday() {
                chrono::Weekday::Sat | chrono::Weekday::Sun => {
                    current = current.pred_opt().unwrap_or_else(|| {
                        panic!("date underflow - should not happen with current dates")
                    });
                }
                _ => {
                    dates.push(current.format("%Y-%m-%d").to_string());
                    current = current.pred_opt().unwrap_or_else(|| {
                        panic!("date underflow - should not happen with current dates")
                    });
                }
            }
        }
        dates
    }

    /// Fetch market overview for a specific date.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn fetch_market_overview_for_date(
        &self,
        date: &str,
    ) -> Result<Option<MarketOverviewDay>> {
        let fields = ApiField::join(&[
            ApiField::RiseCount,
            ApiField::FallCount,
            ApiField::LimitUpCount,
            ApiField::LimitDownCount,
            ApiField::LimitUpBrokenRatio,
            ApiField::MarketTemperature,
        ]);
        let url = format!(
            "{}/market_indicator/line?fields={fields}&date={date}",
            self.base_url,
        );

        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        let arr = resp["data"].as_array().cloned().unwrap_or_default();

        // Take the last data point (most recent timestamp) from the intraday array
        let last = match arr.last() {
            Some(v) => v.clone(),
            None => return Ok(None),
        };

        Ok(Some(MarketOverviewDay {
            date: date.to_string(),
            rise_count: u32::try_from(last["rise_count"].as_u64().unwrap_or(0)).unwrap_or(0),
            fall_count: u32::try_from(last["fall_count"].as_u64().unwrap_or(0)).unwrap_or(0),
            limit_up_count: u32::try_from(last["limit_up_count"].as_u64().unwrap_or(0))
                .unwrap_or(0),
            limit_down_count: u32::try_from(last["limit_down_count"].as_u64().unwrap_or(0))
                .unwrap_or(0),
            bomb_rate: last["limit_up_broken_ratio"].as_f64().unwrap_or(0.0),
            market_temperature: last["market_temperature"].as_f64().unwrap_or(50.0),
        }))
    }
    /// Fetch market trend data for a symbol.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn fetch_market_trend(&self, symbol: &str) -> Result<MarketTrend> {
        // 与实时行情/K 线保持一致：允许 600519、sh600519、600519.SH 等输入，
        // 统一成接口实际使用的 600519.SS 形式。
        let symbol = to_yahoo_symbol(symbol)?;
        let url = format!(
            "{}/market/trend?fields={}&prod_code={symbol}",
            self.market_base_url,
            ApiField::join(&[
                ApiField::TickAt,
                ApiField::ClosePx,
                ApiField::OpenPx,
                ApiField::HighPx,
                ApiField::LowPx,
                ApiField::TurnoverVolume,
                ApiField::PxChangeRate,
            ]),
        );
        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        anyhow::ensure!(
            resp["code"] == 20000,
            "Trend API error: {}",
            resp["message"]
        );
        parse_market_trend(&symbol, &resp)
    }
    /// Fetch real-time market data for multiple symbols.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn fetch_market_real(&self, symbols: &[&str]) -> Result<Vec<IndexQuote>> {
        let joined = symbols.join(",");
        let fields = ApiField::join(&[
            ApiField::ProdName,
            ApiField::LastPx,
            ApiField::PxChange,
            ApiField::PxChangeRate,
            ApiField::Symbol,
            ApiField::TradeStatus,
        ]);
        let url = format!(
            "{}/market/real?fields={fields}&prod_code={joined}",
            self.market_base_url,
        );

        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        anyhow::ensure!(
            resp["code"] == 20000,
            "Market Real API error: {}",
            resp["message"]
        );

        let field_names: Vec<String> = resp["data"]["fields"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let snapshot = resp["data"]["snapshot"]
            .as_object()
            .context("missing snapshot")?;

        let mut results = Vec::new();
        for (symbol, values) in snapshot {
            let arr = values.as_array().context("snapshot value not array")?;
            let get_f64 = |name: &str| -> f64 {
                field_names
                    .iter()
                    .position(|f| f == name)
                    .and_then(|i| arr.get(i))
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0)
            };
            let get_str = |name: &str| -> String {
                field_names
                    .iter()
                    .position(|f| f == name)
                    .and_then(|i| arr.get(i))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            };
            results.push(IndexQuote {
                symbol: symbol.clone(),
                name: get_str("prod_name"),
                last_px: get_f64("last_px"),
                px_change: get_f64("px_change"),
                px_change_rate: get_f64("px_change_rate"),
                trade_status: get_str("trade_status"),
            });
        }

        Ok(results)
    }

    /// 获取证券的市场类型、证券类型和子类型元数据。
    ///
    /// # Errors
    /// 证券代码非法、请求失败、业务状态码异常或响应字段不符合约定时返回错误。
    pub async fn fetch_security_type(&self, symbol: &str) -> Result<SecurityTypeMetadata> {
        let symbol = to_yahoo_symbol(symbol)?;
        let resp: serde_json::Value = self
            .client
            .get(format!("{}/market/info/type", self.market_base_url))
            .query(&[("prod_code", symbol.as_str())])
            .send()
            .await
            .context("证券类型请求失败")?
            .json()
            .await
            .context("证券类型响应不是合法 JSON")?;
        parse_security_type_metadata(&symbol, &resp)
    }

    /// 批量获取题材摘要。
    ///
    /// # Errors
    /// 题材列表为空、请求失败、业务状态码异常或响应字段不符合约定时返回错误。
    pub async fn fetch_plate_summaries(&self, plate_ids: &[i64]) -> Result<Vec<PlateSummary>> {
        anyhow::ensure!(!plate_ids.is_empty(), "至少需要一个题材 ID");
        let mut result = Vec::with_capacity(plate_ids.len());
        for chunk in plate_ids.chunks(PLATE_SUMMARY_BATCH_SIZE) {
            result.extend(self.fetch_plate_summaries_chunk(chunk).await?);
        }
        Ok(result)
    }

    async fn fetch_plate_summaries_chunk(&self, plate_ids: &[i64]) -> Result<Vec<PlateSummary>> {
        let plates = plate_ids
            .iter()
            .map(i64::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let fields = "plate_id,plate_name,fund_flow,rise_count,fall_count,stay_count,limit_up_count,core_avg_pcp,core_avg_pcp_rank,core_avg_pcp_rank_change,top_n_stocks,bottom_n_stocks";
        let resp: serde_json::Value = self
            .client
            .get(format!("{}/plate/data", self.base_url))
            .query(&[("plates", plates.as_str()), ("fields", fields)])
            .send()
            .await
            .context("题材摘要请求失败")?
            .json()
            .await
            .context("题材摘要响应不是合法 JSON")?;
        parse_plate_summaries(&resp)
    }

    /// 获取指定分类的板块涨跌幅排名 ID。
    ///
    /// # Errors
    /// 请求失败、业务状态码异常或响应字段不符合约定时返回错误。
    pub async fn fetch_plate_rank_ids(&self, rank_type: PlateRankType) -> Result<Vec<i64>> {
        let resp: serde_json::Value = self
            .client
            .get(format!("{}/plate/rank", self.base_url))
            .query(&[("field", "core_avg_pcp"), ("type", rank_type.as_str())])
            .send()
            .await
            .context("板块排行榜请求失败")?
            .json()
            .await
            .context("板块排行榜响应不是合法 JSON")?;
        parse_plate_rank_ids(&resp)
    }

    /// 获取指定分类的板块涨跌榜，正涨幅和负涨幅分别排序。
    ///
    /// # Errors
    /// `limit` 为 0、排行榜或板块摘要请求失败时返回错误。
    pub async fn fetch_plate_rankings(
        &self,
        rank_type: PlateRankType,
        limit: usize,
    ) -> Result<PlateRankings> {
        anyhow::ensure!(limit > 0, "板块排行数量必须大于 0");
        let ids = self.fetch_plate_rank_ids(rank_type).await?;
        let summaries = self.fetch_plate_summaries(&ids).await?;
        let mut seen = std::collections::HashSet::with_capacity(summaries.len());
        let mut ranked: Vec<PlateSummary> = summaries
            .into_iter()
            .filter(|summary| summary.core_avg_pcp.is_some())
            .filter(|summary| seen.insert(summary.plate_id))
            .collect();
        ranked.sort_by(|left, right| {
            right
                .core_avg_pcp
                .unwrap_or_default()
                .total_cmp(&left.core_avg_pcp.unwrap_or_default())
                .then_with(|| left.plate_id.cmp(&right.plate_id))
        });
        let gainers = ranked
            .iter()
            .filter(|summary| summary.core_avg_pcp.is_some_and(|pcp| pcp > 0.0))
            .take(limit)
            .cloned()
            .collect();
        let mut losers: Vec<PlateSummary> = ranked
            .into_iter()
            .filter(|summary| summary.core_avg_pcp.is_some_and(|pcp| pcp < 0.0))
            .collect();
        losers.sort_by(|left, right| {
            left.core_avg_pcp
                .unwrap_or_default()
                .total_cmp(&right.core_avg_pcp.unwrap_or_default())
                .then_with(|| left.plate_id.cmp(&right.plate_id))
        });
        losers.truncate(limit);
        Ok(PlateRankings { gainers, losers })
    }

    /// 获取题材涨跌榜。
    ///
    /// `move_type` 的原生语义未公开确认，只保留文档观测到的编码。
    ///
    /// # Errors
    /// `limit` 为 0、请求失败、业务状态码异常或响应字段不符合约定时返回错误。
    pub async fn fetch_plate_incdec(
        &self,
        move_type: PlateIncdecType,
        limit: Option<usize>,
    ) -> Result<Vec<PlateMover>> {
        if let Some(limit) = limit {
            anyhow::ensure!(limit > 0, "题材涨跌榜数量必须大于 0");
        }
        let mut request = self
            .client
            .get(format!("{}/plate/incdec", self.base_url))
            .query(&[("move_type", move_type.as_str())]);
        if let Some(limit) = limit {
            request = request.query(&[("limit", limit)]);
        }
        let resp: serde_json::Value = request
            .send()
            .await
            .context("题材涨跌榜请求失败")?
            .json()
            .await
            .context("题材涨跌榜响应不是合法 JSON")?;
        parse_plate_incdec(&resp)
    }

    /// 获取题材指数历史 K 线。
    ///
    /// # Errors
    /// `data_count` 为 0、请求失败、业务状态码异常或响应字段不符合约定时返回错误。
    pub async fn fetch_plate_index_history(
        &self,
        plate_id: i64,
        data_count: usize,
        end_time: Option<i64>,
    ) -> Result<Vec<PlateIndexCandle>> {
        anyhow::ensure!(plate_id > 0, "题材 ID 必须大于 0");
        anyhow::ensure!(data_count > 0, "题材指数 K 线数量必须大于 0");
        let plate_id = plate_id.to_string();
        let data_count = data_count.to_string();
        let mut request = self
            .client
            .get(format!("{}/plate/index_history", self.base_url))
            .query(&[
                ("plate_id", plate_id.as_str()),
                ("index_type", PLATE_INDEX_TYPE),
                ("data_count", data_count.as_str()),
            ]);
        if let Some(end_time) = end_time {
            request = request.query(&[("end_time", end_time)]);
        }
        let resp: serde_json::Value = request
            .send()
            .await
            .context("题材指数历史请求失败")?
            .json()
            .await
            .context("题材指数历史响应不是合法 JSON")?;
        parse_plate_index_history(&resp)
    }

    /// 获取题材指数分时序列。
    ///
    /// # Errors
    /// 题材 ID 非正、请求失败、业务状态码异常或响应字段不符合约定时返回错误。
    pub async fn fetch_plate_index_realtime(&self, plate_id: i64) -> Result<PlateIndexRealtime> {
        anyhow::ensure!(plate_id > 0, "题材 ID 必须大于 0");
        let plate_id = plate_id.to_string();
        let resp: serde_json::Value = self
            .client
            .get(format!("{}/plate/index_realtime", self.base_url))
            .query(&[
                ("plate_id", plate_id.as_str()),
                ("index_type", PLATE_INDEX_TYPE),
                ("data_type", PLATE_INDEX_DATA_TYPE),
            ])
            .send()
            .await
            .context("题材指数分时请求失败")?
            .json()
            .await
            .context("题材指数分时响应不是合法 JSON")?;
        parse_plate_index_realtime(&resp)
    }

    /// 获取题材详情、产业链与成分股。
    ///
    /// 该接口单次响应可能达到数百 KB；调用方应缓存结果。
    ///
    /// # Errors
    /// 题材 ID 非正、请求失败、业务状态码异常或响应字段不符合约定时返回错误。
    pub async fn fetch_plate_detail(&self, plate_id: i64) -> Result<PlateDetail> {
        anyhow::ensure!(plate_id > 0, "题材 ID 必须大于 0");
        let plate_id = plate_id.to_string();
        let resp: serde_json::Value = self
            .client
            .get(format!("{}/plate/plate_set", self.base_url))
            .query(&[("id", plate_id.as_str())])
            .send()
            .await
            .context("题材详情请求失败")?
            .json()
            .await
            .context("题材详情响应不是合法 JSON")?;
        parse_plate_detail(&resp)
    }

    /// Fetch plate abnormal events.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn fetch_plate_abnormal(&self, count: i32) -> Result<Vec<PlateOrStockAbnormalEvent>> {
        let types =
            "10001,10005,10003,10007,10002,10006,10004,10008,10012,10014,10009,10010,11000,11001";
        let url = format!(
            "{}/event/history?count={}&types={}",
            self.base_url, count, types
        );

        let resp: ApiResponse<Vec<PlateOrStockAbnormalEvent>> =
            self.client.get(&url).send().await?.json().await?;

        anyhow::ensure!(resp.code == 20000, "API 返回错误码: {}", resp.code);
        Ok(resp.data)
    }

    /// `fetch_plate_abnormal` with String count — for LLM tool use
    /// Fetch plate abnormal events as raw JSON.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn fetch_plate_abnormal_str(&self, count: &str) -> Result<serde_json::Value> {
        let n: i32 = count.parse().unwrap_or(30);
        let data = self.fetch_plate_abnormal(n).await?;
        Ok(serde_json::to_value(&data).unwrap_or_default())
    }

    /// Cursor-paginate to fetch ALL events for the latest trading day (09:30–15:00).
    /// Returns events sorted by `event_timestamp` ascending.
    /// Fetch a page of abnormal events.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn fetch_events_page(
        &self,
        timestamp: i64,
        count: u32,
        types: &str,
    ) -> Result<Vec<PlateOrStockAbnormalEvent>> {
        let url = format!(
            "{}/event/history?timestamp={timestamp}&count={count}&types={types}",
            self.base_url
        );
        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        let code = resp["code"].as_i64().unwrap_or(0);
        anyhow::ensure!(code == 20000, "API 返回错误码: {code}");
        let data = resp["data"].as_array().cloned().unwrap_or_default();
        let events: Vec<PlateOrStockAbnormalEvent> = data
            .iter()
            .filter_map(|v| serde_json::from_value(v.clone()).ok())
            .collect();
        Ok(events)
    }

    /// 拉取指定交易日 09:30–15:00 的异动事件。
    ///
    /// 盘中调用时结束时间取当前时间，收盘后取 15:00。`min_pcp` 只做粗过滤，
    /// 建议传一个较低阈值（例如 0.005），把细粒度数据留给回放页面做二次筛选。
    ///
    /// # Errors
    /// 日期非法、HTTP 请求失败或响应无法解析时返回错误。
    pub async fn fetch_events_for_date(&self, date: &str, min_pcp: f64) -> Result<DayEventBatch> {
        let close_ts = Self::trade_date_close_ts(date)?;
        let open_ts = close_ts - 5 * 3600 - 30 * 60;
        let end_ts = Self::effective_event_end_ts(date, close_ts);
        if end_ts <= open_ts {
            return Ok(DayEventBatch {
                events: Vec::new(),
                window_start: open_ts,
                window_end: end_ts,
                reached_open: true,
            });
        }

        let mut all_events: Vec<PlateOrStockAbnormalEvent> = Vec::new();
        let page_size: u32 = 1000;
        let mut cursor = end_ts + 1;
        let mut reached_open = false;

        loop {
            let page = self
                .fetch_events_page(cursor, page_size, "11001,11000")
                .await?;
            if page.is_empty() {
                // 没有更早的事件 —— 分页已经走到当天起点。
                reached_open = true;
                break;
            }
            // API 返回 cursor 之前的事件（新→旧）；用最小时间戳作为下一页游标。
            let earliest = page
                .iter()
                .map(|e| e.event_timestamp)
                .min()
                .unwrap_or(cursor);
            all_events.extend(
                page.into_iter()
                    .filter(|e| e.event_timestamp >= open_ts && e.event_timestamp <= end_ts),
            );
            if earliest <= open_ts {
                reached_open = true;
                break;
            }
            if earliest >= cursor {
                // 上游返回了不在游标之前的数据，避免死循环。
                break;
            }
            cursor = earliest;
        }

        all_events.sort_by_key(|e| e.event_timestamp);
        dedup_events_by_id(&mut all_events);
        all_events.retain(|e| event_pcp(e).is_some_and(|pcp| pcp.abs() > min_pcp));

        tracing::debug!(
            date,
            count = all_events.len(),
            reached_open,
            "xuangubao::fetch_events_for_date done"
        );
        Ok(DayEventBatch {
            events: all_events,
            window_start: open_ts,
            window_end: end_ts,
            reached_open,
        })
    }

    /// Fetch all abnormal events for the latest completed trading day.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn fetch_all_day_events(
        &self,
        min_pcp: f64,
    ) -> Result<Vec<PlateOrStockAbnormalEvent>> {
        let date = Self::latest_completed_trade_date();
        Ok(self.fetch_events_for_date(&date, min_pcp).await?.events)
    }

    /// 最近一个已收盘的交易日（YYYY-MM-DD）。
    ///
    /// 只跳过周末，不含节假日日历；节假日会取到空数据，调用方需自行回退。
    pub fn latest_completed_trade_date() -> String {
        let now = chrono::Utc::now().with_timezone(&beijing_tz());
        let close = chrono::NaiveTime::from_hms_opt(15, 0, 0)
            .unwrap_or_else(|| panic!("15:00:00 is a valid time"));
        let mut date = now.date_naive();
        if now.time() < close {
            date = date.pred_opt().unwrap_or(date);
        }
        while matches!(date.weekday(), chrono::Weekday::Sat | chrono::Weekday::Sun) {
            date = date.pred_opt().unwrap_or(date);
        }
        date.format("%Y-%m-%d").to_string()
    }

    /// 指定交易日 15:00（北京时间）的 Unix 时间戳。
    ///
    /// # Errors
    /// 日期不是 `YYYY-MM-DD` 时返回错误。
    pub fn trade_date_close_ts(date: &str) -> Result<i64> {
        let naive = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .with_context(|| format!("非法交易日: {date}"))?
            .and_hms_opt(15, 0, 0)
            .with_context(|| format!("非法收盘时间: {date}"))?;
        Ok(naive.and_local_timezone(beijing_tz()).single().map_or_else(
            || panic!("fixed offset always resolves to a single local time"),
            |dt| dt.timestamp(),
        ))
    }

    /// 当天盘中只回放到当前时刻，收盘后回放到 15:00。
    fn effective_event_end_ts(date: &str, close_ts: i64) -> i64 {
        let now = chrono::Utc::now().with_timezone(&beijing_tz());
        if date == now.format("%Y-%m-%d").to_string() {
            now.timestamp().min(close_ts)
        } else {
            close_ts
        }
    }

    /// Unix timestamp for 15:00 on the latest completed trading day (Beijing time).
    /// If today's market hasn't closed yet, returns the previous trading day's close.
    ///
    /// # Panics
    /// Panics if the computed date is invalid (should never happen).
    pub fn latest_trading_day_close_ts() -> i64 {
        let date = Self::latest_completed_trade_date();
        Self::trade_date_close_ts(&date)
            .unwrap_or_else(|e| panic!("latest trading day close timestamp: {e}"))
    }

    // ---大盘异动---------------------------------------
    /// Fetch dapan (大盘) abnormal events.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn fetch_dapan_abnormal(&self) -> Result<Vec<DapanAbnormal>> {
        let url =
            "https://api.xuangubao.com.cn/api/messages/todayDaPanYiDong?headmark=0".to_string();

        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;

        // API returns { Messages: [...], NextHeadMark: "..." } or { messages: [...] }
        let messages = resp["Messages"]
            .as_array()
            .or_else(|| resp["messages"].as_array())
            .cloned()
            .unwrap_or_default();

        let result: Vec<DapanAbnormal> = messages
            .iter()
            .filter_map(|v| serde_json::from_value(v.clone()).ok())
            .collect();

        Ok(result)
    }

    // ─── K线数据 ───────────────────────────────────────

    /// Fetch K-line data for a symbol.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the symbol is invalid, or the response cannot be parsed.
    pub async fn fetch_kline(
        &self,
        symbol: &str,
        period: KlinePeriod,
        count: u32,
    ) -> Result<KlineData> {
        let symbol = to_yahoo_symbol(symbol)?;
        let url = format!(
            "{}/market/kline?fields={}&prod_code={symbol}&period_type={period_sec}&tick_count={count}&adjust_price_type=forward",
            self.market_base_url,
            ApiField::join(&[
                ApiField::TickAt,
                ApiField::OpenPx,
                ApiField::ClosePx,
                ApiField::HighPx,
                ApiField::LowPx,
                ApiField::TurnoverVolume,
                ApiField::PxChangeRate,
            ]),
            period_sec = period.as_seconds()
        );

        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        anyhow::ensure!(
            resp["code"] == 20000,
            "Kline API error: {}",
            resp["message"]
        );

        let candle_data = &resp["data"]["candle"][&symbol];
        let lines = candle_data["lines"]
            .as_array()
            .context("missing kline lines array")?;

        // API 返回的字段顺序以 data.fields 为准，需要动态映射
        let fields: Vec<String> = resp["data"]["fields"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let candles: Vec<KlineCandle> = lines
            .iter()
            .filter_map(|line| {
                let arr = line.as_array()?;
                let get = |name: &str| -> Option<f64> {
                    let idx = fields.iter().position(|f| f == name)?;
                    arr.get(idx)?.as_f64()
                };
                Some(KlineCandle {
                    time: get("tick_at").map(f64_to_i64)?,
                    open: get("open_px"),
                    close: get("close_px")?,
                    high: get("high_px"),
                    low: get("low_px"),
                    volume: get("turnover_volume"),
                    change_rate: get("px_change_rate"),
                })
            })
            .collect();

        Ok(KlineData {
            symbol: symbol.clone(),
            period: period.as_str().to_string(),
            candles,
        })
    }

    /// Fetch kline for multiple symbols in a single API call.
    /// Returns a map keyed by normalized (yahoo) symbol.
    /// Fetch K-line data for multiple symbols.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, any symbol is invalid, or the response cannot be parsed.
    pub async fn fetch_kline_batch(
        &self,
        symbols: &[String],
        period: KlinePeriod,
        count: u32,
    ) -> Result<ahash::AHashMap<String, KlineData>> {
        let yahoo_symbols: Vec<String> = symbols
            .iter()
            .map(|s| to_yahoo_symbol(s))
            .collect::<Result<Vec<_>>>()?;

        let prod_codes: Vec<&str> = yahoo_symbols
            .iter()
            .map(std::string::String::as_str)
            .collect();
        let url = format!(
            "{}/market/kline?fields={}&prod_code={}&period_type={period_sec}&tick_count={count}&adjust_price_type=forward",
            self.market_base_url,
            ApiField::join(&[
                ApiField::TickAt,
                ApiField::OpenPx,
                ApiField::ClosePx,
                ApiField::HighPx,
                ApiField::LowPx,
                ApiField::TurnoverVolume,
                ApiField::PxChangeRate,
            ]),
            prod_codes.join(","),
            period_sec = period.as_seconds()
        );

        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        anyhow::ensure!(
            resp["code"] == 20000,
            "Kline API error: {}",
            resp["message"]
        );

        let fields: Vec<String> = resp["data"]["fields"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let candle_data = &resp["data"]["candle"];
        let mut result = ahash::AHashMap::new();

        for yahoo_sym in &yahoo_symbols {
            let lines = candle_data[yahoo_sym]["lines"]
                .as_array()
                .cloned()
                .unwrap_or_default();

            let candles: Vec<KlineCandle> = lines
                .iter()
                .filter_map(|line| {
                    let arr = line.as_array()?;
                    let get = |name: &str| -> Option<f64> {
                        let idx = fields.iter().position(|f| f == name)?;
                        arr.get(idx)?.as_f64()
                    };
                    Some(KlineCandle {
                        time: get("tick_at").map(f64_to_i64)?,
                        open: get("open_px"),
                        close: get("close_px")?,
                        high: get("high_px"),
                        low: get("low_px"),
                        volume: get("turnover_volume"),
                        change_rate: get("px_change_rate"),
                    })
                })
                .collect();

            result.insert(
                yahoo_sym.clone(),
                KlineData {
                    symbol: yahoo_sym.clone(),
                    period: period.as_str().to_string(),
                    candles,
                },
            );
        }

        Ok(result)
    }

    /// 批量获取股票指标（连板数、最新涨停时间、市值、换手率等）。
    ///
    /// # Errors
    /// 标的列表为空、证券代码非法、请求失败或响应字段不符合约定时返回错误。
    pub async fn fetch_stock_data(
        &self,
        symbols: &[&str],
    ) -> Result<ahash::AHashMap<String, StockMetrics>> {
        anyhow::ensure!(!symbols.is_empty(), "至少需要一个证券代码");
        let normalized = symbols
            .iter()
            .map(|symbol| to_yahoo_symbol(symbol))
            .collect::<Result<Vec<_>>>()?;
        let mut result = ahash::AHashMap::with_capacity(normalized.len());
        for chunk in normalized.chunks(STOCK_DATA_BATCH_SIZE) {
            let mut partial = self.fetch_stock_data_chunk(chunk).await?;
            result.extend(partial.drain());
        }
        Ok(result)
    }

    async fn fetch_stock_data_chunk(
        &self,
        symbols: &[String],
    ) -> Result<ahash::AHashMap<String, StockMetrics>> {
        let symbols = symbols.join(",");
        let fields = "symbol,stock_chi_name,change_percent,price,turnover_ratio,non_restricted_capital,total_capital,per,limit_up_days,last_limit_up,limit_status,nearly_new_acc_pcp";
        let resp: serde_json::Value = self
            .client
            .get(format!("{}/stock/data", self.base_url))
            .query(&[
                ("symbols", symbols.as_str()),
                ("fields", fields),
                ("strict", "true"),
            ])
            .send()
            .await
            .context("股票指标请求失败")?
            .json()
            .await
            .context("股票指标响应不是合法 JSON")?;
        parse_stock_data(&resp)
    }

    /// 批量获取股票标签。
    ///
    /// # Errors
    /// 标的列表为空、证券代码非法、请求失败或响应字段不符合约定时返回错误。
    pub async fn fetch_stock_labels(
        &self,
        symbols: &[&str],
    ) -> Result<ahash::AHashMap<String, Vec<StockLabel>>> {
        anyhow::ensure!(!symbols.is_empty(), "至少需要一个证券代码");
        let normalized = symbols
            .iter()
            .map(|symbol| to_yahoo_symbol(symbol))
            .collect::<Result<Vec<_>>>()?;
        let mut result = ahash::AHashMap::with_capacity(normalized.len());
        for chunk in normalized.chunks(STOCK_LABEL_BATCH_SIZE) {
            let symbols = chunk.join(",");
            let resp: serde_json::Value = self
                .client
                .get(format!("{}/stock_label/labels", self.base_url))
                .query(&[("symbols", symbols.as_str())])
                .send()
                .await
                .context("股票标签请求失败")?
                .json()
                .await
                .context("股票标签响应不是合法 JSON")?;
            let mut partial = parse_stock_labels(&resp)?;
            result.extend(partial.drain());
        }
        Ok(result)
    }

    /// 批量获取证券最近若干交易日的资金流。
    ///
    /// 返回映射的键是规范化后的证券代码；每行中间指标保存在
    /// [`FundFlowRow::metrics`]，不依赖未公开的列语义。超过 200 个标的时按
    /// 前端同样的批次大小串行请求，任一批失败则整体返回错误。
    ///
    /// # Errors
    /// 标的列表为空、`day_count` 为 0、证券代码非法、请求失败或响应字段
    /// 不符合约定时返回错误。
    pub async fn fetch_fund_flow_batch(
        &self,
        symbols: &[&str],
        day_count: usize,
    ) -> Result<ahash::AHashMap<String, Vec<FundFlowRow>>> {
        anyhow::ensure!(!symbols.is_empty(), "至少需要一个证券代码");
        anyhow::ensure!(day_count > 0, "资金流交易日数量必须大于 0");
        let normalized = symbols
            .iter()
            .map(|symbol| to_yahoo_symbol(symbol))
            .collect::<Result<Vec<_>>>()?;
        let mut result = ahash::AHashMap::with_capacity(normalized.len());
        for chunk in normalized.chunks(FUND_FLOW_BATCH_SIZE) {
            let mut partial = self.fetch_fund_flow_chunk(chunk, day_count).await?;
            result.extend(partial.drain());
        }
        Ok(result)
    }

    async fn fetch_fund_flow_chunk(
        &self,
        symbols: &[String],
        day_count: usize,
    ) -> Result<ahash::AHashMap<String, Vec<FundFlowRow>>> {
        let prod_codes = symbols.join(",");
        let day_count = day_count.to_string();
        let resp: serde_json::Value = self
            .client
            .get(format!("{}/market/fundflow/batch", self.market_base_url))
            .query(&[
                ("prod_codes", prod_codes.as_str()),
                ("day_count", day_count.as_str()),
            ])
            .send()
            .await
            .context("资金流请求失败")?
            .json()
            .await
            .context("资金流响应不是合法 JSON")?;
        parse_fund_flow_batch(&resp)
    }

    /// Fetch real-time quote for a single symbol.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the symbol is invalid, or the response cannot be parsed.
    pub async fn fetch_realtime(&self, symbol: &str) -> Result<RealtimeQuote> {
        let symbol = to_yahoo_symbol(symbol)?;
        let url = format!(
            "{}/market/real?fields={}&prod_code={}",
            self.market_base_url,
            ApiField::join(&[
                ApiField::ProdName,
                ApiField::TradeStatus,
                ApiField::UpdateTime,
                ApiField::LastPx,
                ApiField::PxChange,
                ApiField::PxChangeRate,
                ApiField::PreclosePx,
                ApiField::OpenPx,
                ApiField::HighPx,
                ApiField::LowPx,
                ApiField::Amplitude,
                ApiField::TurnoverRatio,
                ApiField::PeRate,
                ApiField::DynPe,
                ApiField::DynPbRate,
                ApiField::MarketValue,
                ApiField::CirculationValue,
                ApiField::TurnoverVolume,
                ApiField::TurnoverValue,
                ApiField::VolumeRatio,
                ApiField::CirculationShares,
                ApiField::TotalShares,
            ]),
            symbol
        );

        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        anyhow::ensure!(
            resp["code"] == 20000,
            "Realtime API error: {}",
            resp["message"]
        );

        let fields: Vec<String> = resp["data"]["fields"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let snap = &resp["data"]["snapshot"][&symbol];
        if snap.is_null() {
            anyhow::bail!("symbol '{symbol}' not found in realtime snapshot");
        }
        let arr = snap.as_array().context("missing snapshot array")?;

        let field_pos = |name: &str| -> Option<usize> { fields.iter().position(|f| f == name) };

        let get_f64 = |name: &str| -> Option<f64> {
            field_pos(name)
                .and_then(|i| arr.get(i))
                .and_then(serde_json::Value::as_f64)
        };
        let get_str = |name: &str| -> Option<&str> {
            field_pos(name)
                .and_then(|i| arr.get(i))
                .and_then(|v| v.as_str())
        };
        let get_i64 = |name: &str| -> Option<i64> {
            field_pos(name)
                .and_then(|i| arr.get(i))
                .and_then(serde_json::Value::as_i64)
        };

        Ok(RealtimeQuote {
            symbol: symbol.clone(),
            name: get_str("prod_name").unwrap_or("").to_string(),
            trade_status: get_str("trade_status")
                .context("missing trade_status in realtime response")?
                .to_string(),
            update_time: get_i64("update_time")
                .context("missing update_time in realtime response")?,
            last_px: get_f64("last_px").context("missing last_px in realtime response")?,
            px_change: get_f64("px_change").unwrap_or(0.0),
            px_change_rate: get_f64("px_change_rate").unwrap_or(0.0),
            preclose_px: get_f64("preclose_px")
                .context("missing preclose_px in realtime response")?,
            open_px: get_f64("open_px").unwrap_or(0.0),
            high_px: get_f64("high_px").unwrap_or(0.0),
            low_px: get_f64("low_px").unwrap_or(0.0),
            amplitude: get_f64("amplitude").unwrap_or(0.0),
            turnover_ratio: get_f64("turnover_ratio").unwrap_or(0.0),
            pe_rate: get_f64("pe_rate").unwrap_or(0.0),
            dyn_pe: get_f64("dyn_pe").unwrap_or(0.0),
            dyn_pb_rate: get_f64("dyn_pb_rate").unwrap_or(0.0),
            market_value: get_f64("market_value").unwrap_or(0.0),
            circulation_value: get_f64("circulation_value").unwrap_or(0.0),
            turnover_volume: get_f64("turnover_volume").unwrap_or(0.0),
            turnover_value: get_f64("turnover_value").unwrap_or(0.0),
            volume_ratio: get_f64("volume_ratio").unwrap_or(0.0),
            circulation_shares: get_f64("circulation_shares").unwrap_or(0.0),
            total_shares: get_f64("total_shares").unwrap_or(0.0),
        })
    }

    /// Fetch realtime quotes for multiple symbols in a single API call.
    /// Returns a map keyed by normalized (yahoo) symbol.
    /// Fetch real-time quotes for multiple symbols.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, any symbol is invalid, or the response cannot be parsed.
    pub async fn fetch_realtime_batch(
        &self,
        symbols: &[String],
    ) -> Result<ahash::AHashMap<String, RealtimeQuote>> {
        let yahoo_symbols: Vec<String> = symbols
            .iter()
            .map(|s| to_yahoo_symbol(s))
            .collect::<Result<Vec<_>>>()?;

        let prod_codes: Vec<&str> = yahoo_symbols
            .iter()
            .map(std::string::String::as_str)
            .collect();
        let url = format!(
            "{}/market/real?fields={}&prod_code={}",
            self.market_base_url,
            ApiField::join(&[
                ApiField::ProdName,
                ApiField::TradeStatus,
                ApiField::UpdateTime,
                ApiField::LastPx,
                ApiField::PxChange,
                ApiField::PxChangeRate,
                ApiField::PreclosePx,
                ApiField::OpenPx,
                ApiField::HighPx,
                ApiField::LowPx,
                ApiField::Amplitude,
                ApiField::TurnoverRatio,
                ApiField::PeRate,
                ApiField::DynPe,
                ApiField::DynPbRate,
                ApiField::MarketValue,
                ApiField::CirculationValue,
                ApiField::TurnoverVolume,
                ApiField::TurnoverValue,
                ApiField::VolumeRatio,
                ApiField::CirculationShares,
                ApiField::TotalShares,
            ]),
            prod_codes.join(","),
        );

        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        anyhow::ensure!(
            resp["code"] == 20000,
            "Realtime API error: {}",
            resp["message"]
        );

        let fields: Vec<String> = resp["data"]["fields"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let field_pos = |name: &str| -> Option<usize> { fields.iter().position(|f| f == name) };
        let get_f64 = |arr: &[serde_json::Value], name: &str| -> Option<f64> {
            field_pos(name)
                .and_then(|i| arr.get(i))
                .and_then(serde_json::Value::as_f64)
        };
        let get_str_for_arr = |arr: &[serde_json::Value], name: &str| -> Option<String> {
            let idx = fields.iter().position(|f| f == name)?;
            arr.get(idx).and_then(|v| v.as_str()).map(String::from)
        };
        let get_i64 = |arr: &[serde_json::Value], name: &str| -> Option<i64> {
            field_pos(name)
                .and_then(|i| arr.get(i))
                .and_then(serde_json::Value::as_i64)
        };

        let snapshots = &resp["data"]["snapshot"];
        let mut result = ahash::AHashMap::new();
        for yahoo_sym in &yahoo_symbols {
            let snap = &snapshots[yahoo_sym];
            if snap.is_null() {
                continue;
            }
            if let Some(arr) = snap.as_array() {
                result.insert(
                    yahoo_sym.clone(),
                    RealtimeQuote {
                        symbol: yahoo_sym.clone(),
                        name: get_str_for_arr(arr, "prod_name").unwrap_or_default(),
                        trade_status: get_str_for_arr(arr, "trade_status").unwrap_or_default(),
                        update_time: get_i64(arr, "update_time").unwrap_or(0),
                        last_px: get_f64(arr, "last_px").unwrap_or(0.0),
                        px_change: get_f64(arr, "px_change").unwrap_or(0.0),
                        px_change_rate: get_f64(arr, "px_change_rate").unwrap_or(0.0),
                        preclose_px: get_f64(arr, "preclose_px").unwrap_or(0.0),
                        open_px: get_f64(arr, "open_px").unwrap_or(0.0),
                        high_px: get_f64(arr, "high_px").unwrap_or(0.0),
                        low_px: get_f64(arr, "low_px").unwrap_or(0.0),
                        amplitude: get_f64(arr, "amplitude").unwrap_or(0.0),
                        turnover_ratio: get_f64(arr, "turnover_ratio").unwrap_or(0.0),
                        pe_rate: get_f64(arr, "pe_rate").unwrap_or(0.0),
                        dyn_pe: get_f64(arr, "dyn_pe").unwrap_or(0.0),
                        dyn_pb_rate: get_f64(arr, "dyn_pb_rate").unwrap_or(0.0),
                        market_value: get_f64(arr, "market_value").unwrap_or(0.0),
                        circulation_value: get_f64(arr, "circulation_value").unwrap_or(0.0),
                        turnover_volume: get_f64(arr, "turnover_volume").unwrap_or(0.0),
                        turnover_value: get_f64(arr, "turnover_value").unwrap_or(0.0),
                        volume_ratio: get_f64(arr, "volume_ratio").unwrap_or(0.0),
                        circulation_shares: get_f64(arr, "circulation_shares").unwrap_or(0.0),
                        total_shares: get_f64(arr, "total_shares").unwrap_or(0.0),
                    },
                );
            }
        }

        Ok(result)
    }
    /// Fetch plates that contain the given stock.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    /// Fetch plates that contain the given stock.
    ///
    /// # Panics
    /// Panics if the `plate_id` value does not fit in `i32`.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn plates_by_any_stock(&self, symbol: &str) -> Result<Vec<PlateAndReason>> {
        let url = format!(
            "{}/stage2/plates_by_any_stock?symbol={symbol}&fields={}",
            self.base_url,
            ApiField::join(&[ApiField::CoreAvgPcp, ApiField::PlateName]),
        );

        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        anyhow::ensure!(
            resp["code"] == 20000,
            "Plates API error: {}",
            resp["message"]
        );
        let plates = resp["data"]
            .as_object()
            .context("missing plates data")?
            .values()
            .filter_map(|v| {
                let obj = v.as_object()?;
                Some(PlateAndReason {
                    core_avg_pcp: obj.get("core_avg_pcp")?.as_f64()?,
                    plate_id: i32::try_from(obj.get("plate_id")?.as_i64()?)
                        .unwrap_or_else(|e| panic!("plate_id should fit in i32: {e}")),
                    plate_name: obj.get("plate_name")?.as_str()?.to_string(),
                    reason: obj
                        .get("reason")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string(),
                })
            })
            .collect();

        Ok(plates)
    }
}

// ─── K线数据结构 ──────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub enum KlinePeriod {
    Minute,
    Day,
    Week,
    Month,
}

impl KlinePeriod {
    pub const fn as_seconds(&self) -> u32 {
        match self {
            Self::Minute => 60,
            Self::Day => 86400,
            Self::Week => 604_800,
            Self::Month => 2_592_000,
        }
    }

    pub const fn as_str(&self) -> &str {
        match self {
            Self::Minute => "minute",
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KlineCandle {
    pub time: i64,
    #[serde(default)]
    pub open: Option<f64>,
    pub close: f64,
    #[serde(default)]
    pub high: Option<f64>,
    #[serde(default)]
    pub low: Option<f64>,
    #[serde(default)]
    pub volume: Option<f64>,
    #[serde(default)]
    pub change_rate: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KlineData {
    pub symbol: String,
    pub period: String,
    pub candles: Vec<KlineCandle>,
}

// fields=prod_name(0),trade_status(1),update_time(2),last_px(3),px_change(4),
//   px_change_rate(5),preclose_px(6),open_px(7),high_px(8),low_px(9),
//   amplitude(10),turnover_ratio(11),pe_rate(12),dyn_pe(13),dyn_pb_rate(14),
//   market_value(15),circulation_value(16),turnover_volume(17),turnover_value(18),
//   volume_ratio(19),circulation_shares(20),total_shares(21)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeQuote {
    pub symbol: String,
    /// 0: `prod_name`
    pub name: String,
    /// 3: `last_px`
    pub last_px: f64,
    /// 4: `px_change`
    pub px_change: f64,
    /// 5: `px_change_rate`
    pub px_change_rate: f64,
    /// 7: `open_px`
    pub open_px: f64,
    /// 8: `high_px`
    pub high_px: f64,
    /// 9: `low_px`
    pub low_px: f64,
    /// 6: `preclose_px`
    pub preclose_px: f64,
    /// 10: amplitude
    pub amplitude: f64,
    /// 11: `turnover_ratio`
    pub turnover_ratio: f64,
    /// 17: `turnover_volume`
    pub turnover_volume: f64,
    /// 18: `turnover_value`
    pub turnover_value: f64,
    /// 15: `market_value`
    pub market_value: f64,
    /// 16: `circulation_value`
    pub circulation_value: f64,
    /// 12: `pe_rate`
    pub pe_rate: f64,
    /// 13: `dyn_pe`
    pub dyn_pe: f64,
    /// 14: `dyn_pb_rate`
    pub dyn_pb_rate: f64,
    /// 19: `volume_ratio`
    pub volume_ratio: f64,
    /// 20: `circulation_shares`
    pub circulation_shares: f64,
    /// 21: `total_shares`
    pub total_shares: f64,
    /// 1: `trade_status`
    pub trade_status: String,
    /// 2: `update_time`
    pub update_time: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexQuote {
    pub symbol: String,
    pub name: String,
    pub last_px: f64,
    pub px_change: f64,
    pub px_change_rate: f64,
    pub trade_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlateAndReason {
    pub core_avg_pcp: f64,
    pub plate_id: i32,
    pub plate_name: String,
    #[serde(default)]
    pub reason: String,
}

// ─── 行情回放 ────────────────────────────────────────────

/// 某一天的完整事件批次。`reached_open` 表示分页已经覆盖到 09:30 或
/// 确认没有更早的事件，收盘对账时用它判断数据是否完整。
#[derive(Debug, Clone)]
pub struct DayEventBatch {
    pub events: Vec<PlateOrStockAbnormalEvent>,
    pub window_start: i64,
    pub window_end: i64,
    pub reached_open: bool,
}

/// Lightweight event for the replay frontend — strips unused `PlateAbnormalEvent` fields.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReplayEvent {
    pub id: i64,
    pub event_type: i32,
    pub event_timestamp: i64,
    pub good_or_bad: i32,
    /// 事件原始目标（板块名或股票代码），作为展示名的兜底。
    #[serde(default)]
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plate_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stock_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pcp: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_stocks: Vec<RelatedStock>,
}

impl From<&PlateOrStockAbnormalEvent> for ReplayEvent {
    fn from(ev: &PlateOrStockAbnormalEvent) -> Self {
        // 上游对不适用的字段会返回 `{}`，反序列化后是 `Some(默认值)`。
        // 不能只看 `is_some()`，否则空的板块载荷会吞掉真正的个股载荷。
        let plate = ev.plate_abnormal_event_data.as_ref().filter(|plate| {
            plate.plate_name.is_some() || plate.pcp.is_some() || !plate.related_stocks.is_empty()
        });
        let stock = ev
            .stock_abnormal_event_data
            .as_ref()
            .filter(|stock| stock.name.is_some() || stock.symbol.is_some() || stock.pcp.is_some());
        let plate_name = plate.and_then(|plate| plate.plate_name.clone());
        let stock_name = stock.and_then(|stock| stock.name.clone());
        let symbol = stock.and_then(|stock| stock.symbol.clone());
        // 少数板块事件只带关联股票、不带自身 pcp，这时用个股 pcp 兜底。
        let pcp = plate
            .and_then(|plate| plate.pcp)
            .or_else(|| stock.and_then(|stock| stock.pcp));
        let related_stocks = plate
            .map(|plate| plate.related_stocks.clone())
            .unwrap_or_default();
        Self {
            id: ev.id,
            event_type: ev.event_type,
            event_timestamp: ev.event_timestamp,
            good_or_bad: ev.good_or_bad,
            target: ev.target.clone(),
            plate_name,
            stock_name,
            symbol,
            pcp,
            related_stocks,
        }
    }
}

/// Paginated replay response.
#[derive(Debug, Serialize)]
pub struct ReplayPage {
    pub date: String,
    pub market_phase: String,
    pub events: Vec<ReplayEvent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<i64>,
}

/// Aggregated view of a 5‑minute window.
#[derive(Debug, Serialize, Deserialize)]
pub struct TimeSnapshot {
    pub timestamp: i64,
    pub time_label: String,
    pub emotion_score: f64,
    pub bull_count: u32,
    pub bear_count: u32,
    pub top_plates: Vec<TopPlate>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TopPlate {
    pub plate_name: String,
    pub pcp: f64,
    pub good_or_bad: i32,
    pub related_stocks: Vec<RelatedStock>,
}

struct PlateAccumulator {
    pcp: f64,
    good_or_bad: i32,
    related_stocks: Vec<RelatedStock>,
}

/// Slice events into 5‑minute buckets across 09:30–15:00, computing
/// cumulative emotion scores and top plates per bucket.
/// Build replay snapshots from abnormal events, grouped into 5-minute buckets.
///
/// # Panics
/// Panics if `events` is empty (caller should check before calling).
pub fn build_replay_snapshots(events: &[PlateOrStockAbnormalEvent]) -> Vec<TimeSnapshot> {
    if events.is_empty() {
        return vec![];
    }

    let start_ts = events
        .first()
        .unwrap_or_else(|| panic!("events should not be empty"))
        .event_timestamp;
    let end_ts = events
        .last()
        .unwrap_or_else(|| panic!("events should not be empty"))
        .event_timestamp;
    let bucket_secs: i64 = 5 * 60;

    let first_bucket = (start_ts / bucket_secs) * bucket_secs;
    let last_bucket = (end_ts / bucket_secs) * bucket_secs;

    let mut snapshots: Vec<TimeSnapshot> = Vec::new();
    let mut bull_acc = 0u32;
    let mut bear_acc = 0u32;
    let mut plate_map: ahash::AHashMap<String, PlateAccumulator> = ahash::AHashMap::new();
    let mut ev_idx = 0usize;

    let mut bucket_start = first_bucket;
    while bucket_start <= last_bucket {
        let bucket_end = bucket_start + bucket_secs;

        let mut bucket_bull = 0u32;
        let mut bucket_bear = 0u32;
        while ev_idx < events.len() && events[ev_idx].event_timestamp < bucket_end {
            let ev = &events[ev_idx];
            match ev.good_or_bad {
                1 => bucket_bull += 1,
                2 => bucket_bear += 1,
                _ => {}
            }
            if let Some(ref pd) = ev.plate_abnormal_event_data {
                if let (Some(ref name), Some(pcp)) = (&pd.plate_name, pd.pcp) {
                    let acc = plate_map
                        .entry(name.clone())
                        .or_insert_with(|| PlateAccumulator {
                            pcp,
                            good_or_bad: ev.good_or_bad,
                            related_stocks: pd.related_stocks.clone(),
                        });
                    if pcp.abs() > acc.pcp.abs() {
                        acc.pcp = pcp;
                        acc.good_or_bad = ev.good_or_bad;
                    }
                    if acc.related_stocks.len() < pd.related_stocks.len() {
                        acc.related_stocks.clone_from(&pd.related_stocks);
                    }
                }
            }
            ev_idx += 1;
        }

        bull_acc += bucket_bull;
        bear_acc += bucket_bear;

        let emotion = if bull_acc + bear_acc > 0 {
            f64::from(bull_acc) / f64::from(bull_acc + bear_acc) * 100.0
        } else {
            50.0
        };

        let mut top: Vec<TopPlate> = plate_map
            .drain()
            .map(|(plate_name, acc)| TopPlate {
                plate_name,
                pcp: acc.pcp,
                good_or_bad: acc.good_or_bad,
                related_stocks: acc.related_stocks,
            })
            .collect();
        top.sort_by(|a, b| {
            b.pcp
                .abs()
                .partial_cmp(&a.pcp.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        top.truncate(5);

        let time_label = unix_to_hhmm(bucket_start);

        snapshots.push(TimeSnapshot {
            timestamp: bucket_start,
            time_label,
            emotion_score: (emotion * 10.0).round() / 10.0,
            bull_count: bull_acc,
            bear_count: bear_acc,
            top_plates: top,
        });

        bucket_start = bucket_end;
    }

    snapshots
}

fn unix_to_hhmm(ts: i64) -> String {
    // 时间戳是 UTC，展示统一用北京时间。
    let secs_since_midnight = (ts + 8 * 3600).rem_euclid(86400);
    let h = secs_since_midnight / 3600;
    let m = (secs_since_midnight % 3600) / 60;
    format!("{h:02}:{m:02}")
}

// ─── 指数走势（三指数并发） ──────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IndexTrendData {
    pub sh: Vec<IndexTick>,
    pub sz: Vec<IndexTick>,
    pub cyb: Vec<IndexTick>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IndexTick {
    pub tick_at: i64,
    pub close_px: f64,
}

/// Fetch index trends for major indices (SH, SZ, CYB).
///
/// # Errors
/// Returns an error if any of the HTTP requests fail.
pub async fn fetch_index_trends(client: &XuanguBaoClient) -> Result<IndexTrendData> {
    let (sh, sz, cyb) = tokio::try_join!(
        client.fetch_index_trend("000001.SS"),
        client.fetch_index_trend("399001.SZ"),
        client.fetch_index_trend("399006.SZ"),
    )?;
    Ok(IndexTrendData { sh, sz, cyb })
}

impl XuanguBaoClient {
    async fn fetch_index_trend(&self, prod_code: &str) -> Result<Vec<IndexTick>> {
        let url = format!(
            "{}/market/trend?fields={}&prod_code={prod_code}",
            self.market_base_url,
            ApiField::join(&[ApiField::TickAt, ApiField::ClosePx])
        );
        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        anyhow::ensure!(
            resp["code"] == 20000,
            "Trend API error: {}",
            resp["message"]
        );
        let candle = &resp["data"]["candle"][prod_code];
        let fields: Vec<String> = resp["data"]["fields"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let tick_at_idx = fields.iter().position(|f| f == "tick_at");
        let close_idx = fields.iter().position(|f| f == "close_px");
        let lines = candle["lines"].as_array().cloned().unwrap_or_default();
        let ticks: Vec<IndexTick> = lines
            .iter()
            .filter_map(|line| {
                let arr = line.as_array()?;
                let tick_at = tick_at_idx.and_then(|i| arr.get(i)?.as_f64().map(f64_to_i64))?;
                let close_px = close_idx.and_then(|i| arr.get(i)?.as_f64())?;
                Some(IndexTick { tick_at, close_px })
            })
            .collect();
        Ok(ticks)
    }

    /// Fetch news stream from xuangubao.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn fetch_news_stream(
        &self,
        tailmark: Option<i64>,
        msg_id_mark: Option<&str>,
    ) -> Result<(Vec<NewsItem>, Option<i64>, Option<String>)> {
        let mut url = String::from(
            "https://api.xuangubao.com.cn/api/pc/msgs?subjids=9,723,10,35,469&limit=50",
        );
        if let Some(tm) = tailmark {
            use std::fmt::Write;
            writeln!(url, "&tailmark={tm}").ok();
        }
        if let Some(mid) = msg_id_mark {
            use std::fmt::Write;
            writeln!(url, "&msgIdMark={mid}").ok();
        }
        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        let items = resp["NewMsgs"].as_array().cloned().unwrap_or_default();
        // Debug: log first and last item timestamps
        if let (Some(first), Some(last)) = (items.first(), items.last()) {
            tracing::debug!(
                first_ts = ?first["CreatedAtInSec"].as_i64(),
                last_ts = ?last["CreatedAtInSec"].as_i64(),
                first_id = ?first["Id"].as_str(),
                "xuangubao::fetch_news_stream raw"
            );
        }
        let result: Vec<NewsItem> = items
            .iter()
            .filter_map(|v| {
                let id = v["Id"].as_str()?.parse::<i64>().ok()?;
                let title = v["Title"].as_str()?.to_string();
                let summary = v["Summary"].as_str().unwrap_or("").to_string();
                let created_at_in_sec = v["CreatedAtInSec"].as_i64()?;
                let updated_at_in_sec = v["UpdatedAtInSec"].as_i64().unwrap_or(created_at_in_sec);
                Some(NewsItem {
                    id,
                    created_at_in_sec,
                    updated_at_in_sec,
                    title,
                    content: String::new(),
                    summary,
                })
            })
            .collect();
        let next_tail = resp["TailMark"]
            .as_str()
            .and_then(|s| s.parse::<i64>().ok());
        let next_msg = resp["TailMsgId"]
            .as_str()
            .map(std::string::ToString::to_string);
        tracing::debug!(
            ?tailmark,
            count = result.len(),
            ?next_tail,
            "xuangubao::fetch_news_stream"
        );
        Ok((result, next_tail, next_msg))
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct NewsItem {
    pub id: i64,
    pub created_at_in_sec: i64,
    pub updated_at_in_sec: i64,
    pub title: String,
    #[serde(default)]
    pub content: String,
    pub summary: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fmt::Display,
        io::{Read, Write},
        net::TcpListener,
        thread::{self, JoinHandle},
    };

    fn ok_or_panic<T, E: Display>(result: std::result::Result<T, E>, context: &str) -> T {
        result.unwrap_or_else(|error| panic!("{context}: {error}"))
    }

    fn error_or_panic<T, E>(result: std::result::Result<T, E>, context: &str) -> E {
        match result {
            Ok(_) => panic!("{context}: expected an error"),
            Err(error) => error,
        }
    }

    fn serve_json(body: serde_json::Value) -> (String, JoinHandle<String>) {
        let listener = ok_or_panic(TcpListener::bind("127.0.0.1:0"), "bind test server");
        let address = ok_or_panic(listener.local_addr(), "test server address");
        let handle = thread::spawn(move || {
            let (mut stream, _) = ok_or_panic(listener.accept(), "accept test request");
            ok_or_panic(
                stream.set_read_timeout(Some(Duration::from_secs(2))),
                "set read timeout",
            );
            let mut request = [0_u8; 16 * 1024];
            let read = ok_or_panic(stream.read(&mut request), "read test request");
            let request = String::from_utf8_lossy(&request[..read]).to_string();
            let body = body.to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            ok_or_panic(stream.write_all(response.as_bytes()), "write test response");
            request
        });
        (format!("http://{address}"), handle)
    }

    fn fund_flow_row(symbol: &str, date: &str, timestamp: i64) -> Vec<serde_json::Value> {
        let mut row = vec![serde_json::json!(symbol), serde_json::json!(date)];
        row.extend((0..17).map(|index| serde_json::json!(index as f64 + 0.5)));
        row.push(serde_json::json!(timestamp));
        row
    }

    #[test]
    fn parses_market_trend_ohlcv_fields() {
        let response = serde_json::json!({
            "code": 20000,
            "data": {
                "fields": [
                    "tick_at",
                    "close_px",
                    "open_px",
                    "high_px",
                    "low_px",
                    "turnover_volume",
                    "px_change_rate"
                ],
                "candle": {
                    "600519.SS": {
                        "lines": [[
                            1792546200.0,
                            1331.88,
                            1341.0,
                            1342.0,
                            1327.88,
                            28170.0,
                            -1.3439
                        ]]
                    }
                }
            }
        });

        let trend = parse_market_trend("600519.SS", &response)
            .unwrap_or_else(|error| panic!("trend should parse: {error}"));
        assert_eq!(trend.symbol, "600519.SS");
        assert_eq!(trend.candles.len(), 1);
        let candle = &trend.candles[0];
        assert_eq!(candle.time, 1_792_546_200);
        assert_eq!(candle.close, 1331.88);
        assert_eq!(candle.open, Some(1341.0));
        assert_eq!(candle.high, Some(1342.0));
        assert_eq!(candle.low, Some(1327.88));
        assert_eq!(candle.volume, Some(28170.0));
        assert_eq!(candle.change_rate, Some(-1.3439));
    }

    #[test]
    fn replay_event_keeps_stock_fields() {
        let event = PlateOrStockAbnormalEvent {
            id: 7,
            target: "300750.SZ".to_string(),
            event_type: 11_001,
            event_timestamp: 1_792_546_200,
            stock_abnormal_event_data: Some(StockAbnormalEventData {
                symbol: Some("300750.SZ".to_string()),
                name: Some("宁德时代".to_string()),
                pcp: Some(-0.018),
                mtm: Some(1.0),
            }),
            plate_abnormal_event_data: Some(PlateAbnormalEventData::default()),
            good_or_bad: 2,
        };
        let replay = ReplayEvent::from(&event);
        assert_eq!(replay.stock_name.as_deref(), Some("宁德时代"));
        assert_eq!(replay.symbol.as_deref(), Some("300750.SZ"));
        assert_eq!(replay.pcp, Some(-0.018));
        assert!(replay.plate_name.is_none());
    }

    #[test]
    fn replay_event_uses_stock_pcp_when_plate_pcp_missing() {
        let event = PlateOrStockAbnormalEvent {
            id: 8,
            target: "64".to_string(),
            event_type: 11_000,
            event_timestamp: 1_792_546_200,
            stock_abnormal_event_data: Some(StockAbnormalEventData {
                symbol: Some("300750.SZ".to_string()),
                name: Some("宁德时代".to_string()),
                pcp: Some(0.021),
                mtm: Some(1.0),
            }),
            plate_abnormal_event_data: Some(PlateAbnormalEventData {
                plate_name: Some("机器人".to_string()),
                related_stocks: vec![RelatedStock {
                    symbol: "300750.SZ".to_string(),
                    name: "宁德时代".to_string(),
                    mtm: 1.0,
                    pcp: 0.021,
                }],
                ..PlateAbnormalEventData::default()
            }),
            good_or_bad: 1,
        };
        let replay = ReplayEvent::from(&event);
        assert_eq!(replay.plate_name.as_deref(), Some("机器人"));
        assert_eq!(replay.pcp, Some(0.021));
        assert_eq!(replay.related_stocks.len(), 1);
    }

    #[test]
    fn replay_duplicate_ids_are_removed_even_when_not_adjacent() {
        let make = |id| PlateOrStockAbnormalEvent {
            id,
            target: id.to_string(),
            event_type: 11_000,
            event_timestamp: 1_792_546_200,
            stock_abnormal_event_data: None,
            plate_abnormal_event_data: None,
            good_or_bad: 1,
        };
        let mut events = vec![make(1), make(2), make(1)];
        dedup_events_by_id(&mut events);
        assert_eq!(
            events.iter().map(|event| event.id).collect::<Vec<_>>(),
            vec![1, 2]
        );
    }

    #[test]
    fn market_trend_missing_symbol_is_an_error() {
        let response = serde_json::json!({
            "code": 20000,
            "data": { "fields": [], "candle": {} }
        });
        assert!(parse_market_trend("600519.SS", &response).is_err());
    }

    #[test]
    fn flash_api_uses_documented_host() {
        assert_eq!(
            BASE_URL, "https://flash-api.xuangubao.com.cn/api",
            "短域名 flash-api.xuangubao.cn 对 plate/incdec 返回空 data，不能作为统一 base URL"
        );
    }

    #[tokio::test]
    async fn fund_flow_batch_round_trips_documented_compact_rows() {
        let response = serde_json::json!({
            "code": 20000,
            "message": "OK",
            "data": {
                "000008.SZ": {
                    "fund_flow": [
                        fund_flow_row("000008.SZ", "2026-10-27", 1_793_059_200_i64),
                        fund_flow_row("000008.SZ", "2026-10-26", 1_792_972_800_i64)
                    ]
                },
                "600000.SS": {
                    "fund_flow": [
                        fund_flow_row("600000.SS", "2026-10-27", 1_793_059_200_i64)
                    ]
                }
            }
        });
        let (base_url, server) = serve_json(response);
        let client = XuanguBaoClient::with_urls(format!("{base_url}/api"), base_url);

        let flows = ok_or_panic(
            client
                .fetch_fund_flow_batch(&["000008.SZ", "600000.SS"], 5)
                .await,
            "fund flow should parse",
        );
        let request = server
            .join()
            .unwrap_or_else(|_| panic!("test server should finish"));

        assert!(request.starts_with("GET /market/fundflow/batch?"));
        assert!(request.contains("prod_codes=000008.SZ%2C600000.SS"));
        assert!(request.contains("day_count=5"));
        assert_eq!(flows["000008.SZ"].len(), 2);
        assert_eq!(flows["000008.SZ"][0].trade_date, "2026-10-27");
        assert_eq!(flows["000008.SZ"][0].metrics.len(), 17);
        assert_eq!(flows["000008.SZ"][0].metrics[0], 0.5);
        assert_eq!(flows["000008.SZ"][0].timestamp, 1_793_059_200);
    }

    #[test]
    fn fund_flow_rejects_malformed_compact_rows() {
        let short_row = serde_json::json!({
            "code": 20000,
            "data": {
                "000008.SZ": {
                    "fund_flow": [["000008.SZ", "2026-10-27", 1.0]]
                }
            }
        });
        let short_error =
            error_or_panic(parse_fund_flow_batch(&short_row), "short row should fail");
        assert!(short_error.to_string().contains("行列数异常"));

        let mut row = fund_flow_row("000008.SZ", "2026-10-27", 1_793_059_200);
        row[6] = serde_json::json!("not-a-number");
        let invalid_metric = serde_json::json!({
            "code": 20000,
            "data": { "000008.SZ": { "fund_flow": [row] } }
        });
        assert!(parse_fund_flow_batch(&invalid_metric).is_err());
    }

    #[test]
    fn parses_stock_data_limit_up_fields() {
        let data = ok_or_panic(
            parse_stock_data(&serde_json::json!({
                "code": 20000,
                "data": {
                    "300539.SZ": {
                        "symbol": "300539.SZ",
                        "stock_chi_name": "横河精密",
                        "change_percent": 0.2,
                        "price": 19.68,
                        "turnover_ratio": 0.1144,
                        "non_restricted_capital": 4299386024.16,
                        "total_capital": 5287566548.16,
                        "per": 110.9983,
                        "limit_up_days": 1,
                        "last_limit_up": 1793066691_i64,
                        "limit_status": 1,
                        "nearly_new_acc_pcp": 0
                    }
                }
            })),
            "stock data should parse",
        );

        let metrics = &data["300539.SZ"];
        assert_eq!(metrics.stock_chi_name, "横河精密");
        assert_eq!(metrics.limit_up_days, 1);
        assert_eq!(metrics.last_limit_up, 1_793_066_691);
        assert_eq!(metrics.limit_status, 1);
    }

    #[test]
    fn parses_stock_labels_by_symbol() {
        let labels = ok_or_panic(
            parse_stock_labels(&serde_json::json!({
                "code": 20000,
                "data": {
                    "000333.SZ": [{
                        "code": "000333.SZ",
                        "symbol": "美的集团",
                        "label_name": "白马",
                        "label_type": "bai_ma",
                        "description": "业绩稳定且优良的行业龙头公司。",
                        "effect": 1
                    }],
                    "000048.SZ": null
                }
            })),
            "stock labels should parse",
        );

        assert_eq!(labels["000333.SZ"][0].label_name, "白马");
        assert_eq!(labels["000333.SZ"][0].symbol, "美的集团");
        assert_eq!(labels["000333.SZ"][0].effect, 1);
        assert!(labels["000048.SZ"].is_empty());
    }

    #[tokio::test]
    async fn fund_flow_rejects_empty_request_before_network() {
        let client = XuanguBaoClient::new();
        assert!(client.fetch_fund_flow_batch(&[], 5).await.is_err());
        assert!(client
            .fetch_fund_flow_batch(&["000008.SZ"], 0)
            .await
            .is_err());
    }

    #[test]
    fn parses_plate_detail_fund_flow_and_constituents() {
        let response = serde_json::json!({
            "code": 20000,
            "data": {
                "id": 26346142_i64,
                "name": "机器人",
                "desc": "产业链",
                "avg_pcp": 0.0123,
                "core_avg_pcp": 0.0178,
                "fund_flow": 1206444563.59,
                "rise_count": 360,
                "fall_count": 180,
                "stay_count": 30,
                "core_stocks_count": 541,
                "stocks": [{
                    "symbol": "603370.SS",
                    "core_flag": 100,
                    "desc": "核心零部件",
                    "need_pay": false
                }],
                "industrial_chains": [{
                    "id": 9488,
                    "name": "终端应用",
                    "order": 3,
                    "stocks": ["000034.SZ"],
                    "stock_premium_infos": [{
                        "symbol": "000034.SZ",
                        "core_flag": 100,
                        "desc": "机器人业务",
                        "need_pay": false
                    }]
                }],
                "allow_comment": true,
                "create_name": "编辑",
                "display_after_at": 0,
                "hang_ye_long_tou_stocks_count": 3,
                "has_north": 2,
                "hide": false,
                "is_fav": false,
                "need_pay": false,
                "premium_desc": "高级内容",
                "question_answers": [{
                    "question": "题材是什么？",
                    "html_answer": "<p>机器人产业链</p>"
                }],
                "seo_desc": "机器人题材",
                "seo_keywords": "机器人",
                "seo_title": "机器人",
                "sub_sets": [{"plate_id": 1, "plate_name": "子题材"}],
                "subject_id": 9527,
                "update_name": "编辑二",
                "whether_display_plate_stock_portfolio": true
            }
        });

        let detail = ok_or_panic(parse_plate_detail(&response), "plate detail should parse");
        assert_eq!(detail.id, 26_346_142);
        assert_eq!(detail.fund_flow, Some(1_206_444_563.59));
        assert_eq!(detail.hang_ye_long_tou_stocks_count, 3);
        assert_eq!(detail.has_north, 2);
        assert_eq!(detail.question_answers[0].question, "题材是什么？");
        assert_eq!(
            detail.sub_sets.as_deref().map(|subsets| subsets.len()),
            Some(1)
        );
        assert!(detail.whether_display_plate_stock_portfolio);
        assert_eq!(detail.stocks[0].symbol, "603370.SS");
        assert_eq!(detail.industrial_chains[0].stocks, vec!["000034.SZ"]);
        assert_eq!(
            detail.industrial_chains[0].stock_premium_infos[0].symbol,
            "000034.SZ"
        );
    }

    #[test]
    fn plate_rankings_allow_missing_stock_mtm_and_empty_items() {
        let response = serde_json::json!({
            "code": 20000,
            "data": {
                "items": [{
                    "plate_id": 22114510_i64,
                    "plate_name": "玻纤",
                    "pcp": 0.0174,
                    "stocks": [{"symbol": "600545.SS", "name": "卓郎智能"}]
                }]
            }
        });
        let items = ok_or_panic(parse_plate_incdec(&response), "ranking should parse");
        assert_eq!(items[0].stocks[0].mtm, None);

        let empty = ok_or_panic(
            parse_plate_incdec(&serde_json::json!({
                "code": 20000,
                "data": { "items": [] }
            })),
            "empty ranking should parse",
        );
        assert!(empty.is_empty());
    }

    #[test]
    fn parses_plate_rank_ids_and_type_codes() {
        let ids = ok_or_panic(
            parse_plate_rank_ids(&serde_json::json!({
                "code": 20000,
                "data": [49981185_i64, 16997081_i64]
            })),
            "plate rank should parse",
        );
        assert_eq!(ids, vec![49_981_185, 16_997_081]);
        assert_eq!(PlateRankType::All.as_str(), "0");
        assert_eq!(PlateRankType::Concept.as_str(), "1");
        assert_eq!(PlateRankType::Industry.as_str(), "2");
        assert_eq!(PlateRankType::Style.as_str(), "3");
    }

    #[test]
    fn parses_plate_index_history_and_realtime() {
        let history = ok_or_panic(
            parse_plate_index_history(&serde_json::json!({
                "code": 20000,
                "data": [{
                    "date_time": 1_793_059_200_i64,
                    "open": 1407.06,
                    "high": 1420.29,
                    "low": 1396.92,
                    "close": 1401.71
                }]
            })),
            "plate history should parse",
        );
        assert_eq!(history[0].timestamp, 1_793_059_200);
        assert_eq!(history[0].close, 1401.71);

        let realtime = ok_or_panic(
            parse_plate_index_realtime(&serde_json::json!({
                "code": 20000,
                "data": {
                    "items": [{"time": 1_793_064_300_i64, "index": 1665.97806}],
                    "prev_close_idx": 1662.3212
                }
            })),
            "plate realtime should parse",
        );
        assert_eq!(realtime.prev_close, 1662.3212);
        assert_eq!(realtime.items[0].timestamp, 1_793_064_300);
    }

    #[test]
    fn parses_market_indicators_distribution_and_security_metadata() {
        let indicators = ok_or_panic(
            parse_market_indicators(&serde_json::json!({
                "code": 20000,
                "data": [{
                    "timestamp": 1_793_064_300_i64,
                    "rise_count": 934,
                    "fall_count": 1356,
                    "limit_up_count": 45,
                    "limit_down_count": 4,
                    "limit_up_broken_count": 7,
                    "limit_up_broken_ratio": 0.13,
                    "market_temperature": 56.2,
                    "yesterday_limit_up_avg_pcp": 0.008
                }]
            })),
            "indicators should parse",
        );
        assert_eq!(indicators[0].rise_count, Some(934));
        assert_eq!(indicators[0].limit_up_broken_count, Some(7));

        let distribution = ok_or_panic(
            parse_market_pcp_distribution(&serde_json::json!({
                "code": 20000,
                "data": {
                    "0": 160,
                    "1": 934,
                    "-1": 1356,
                    "limit_up_count": 45,
                    "limit_down_count": 4,
                    "st_limit_up_count": 1,
                    "st_limit_down_count": 1,
                    "halt_count": 360,
                    "total_count": 5576,
                    "ts": 1_793_070_587_i64
                }
            })),
            "distribution should parse",
        );
        assert_eq!(distribution.buckets.get(&1), Some(&934));
        assert_eq!(distribution.buckets.get(&-1), Some(&1356));
        assert_eq!(distribution.total_count, 5576);

        let metadata = ok_or_panic(
            parse_security_type_metadata(
                "000001.SS",
                &serde_json::json!({
                    "code": 20000,
                    "data": {
                        "000001.SS": {
                            "market_type": "SS",
                            "securities_type": "index",
                            "sub_type": ""
                        }
                    }
                }),
            ),
            "security metadata should parse",
        );
        assert_eq!(metadata.securities_type, "index");
    }

    #[test]
    fn parses_null_plate_leaders_and_reports_business_errors() {
        let summaries = ok_or_panic(
            parse_plate_summaries(&serde_json::json!({
                "code": 20000,
                "data": {
                    "26346142": {
                        "plate_id": 26346142_i64,
                        "plate_name": "机器人",
                        "core_avg_pcp": -0.004164,
                    "fund_flow": 1206444563.59,
                    "rise_count": 360,
                    "fall_count": 180,
                    "stay_count": 30,
                    "limit_up_count": 8,
                    "core_avg_pcp_rank": 12,
                    "core_avg_pcp_rank_change": 3,
                    "top_n_stocks": {
                        "items": [{
                            "symbol": "603370.SS",
                            "stock_chi_name": "柯力传感",
                            "change_percent": 0.02,
                            "price_change": 1.2
                        }]
                    },
                    "bottom_n_stocks": {"items": []},
                        "jinrilongtou": null,
                        "renqilongtou": ["301196.SZ"],
                        "hangyelongtou": ["300124.SZ"]
                    }
                }
            })),
            "plate summaries should parse",
        );
        assert_eq!(summaries[0].today_leaders, None);
        assert_eq!(summaries[0].fund_flow, Some(1_206_444_563.59));
        assert_eq!(summaries[0].rise_count, 360);
        assert_eq!(summaries[0].core_avg_pcp_rank, Some(12));
        assert_eq!(
            summaries[0].top_n_stocks.items[0].stock_chi_name,
            "柯力传感"
        );
        assert_eq!(
            summaries[0].popular_leaders.as_deref(),
            Some(&["301196.SZ".to_string()][..])
        );

        let error = error_or_panic(
            parse_plate_summaries(&serde_json::json!({
                "code": 50000,
                "message": "参数错误",
                "data": {}
            })),
            "business error should fail",
        );
        assert!(error.to_string().contains("参数错误"));
    }
}
