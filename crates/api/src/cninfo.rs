//! 巨潮资讯公告检索（按 akshare `stock_zh_a_disclosure_report_cninfo` 移植）。
//!
//! 提供沪深京、港股、三板、基金、债券等市场的公告全文检索，返回公告标题、
//! 时间、详情链接与 PDF 链接。接口来自巨潮资讯公开页面，不需要登录。

use anyhow::{bail, ensure, Context, Result};
use chrono::{DateTime, Duration, FixedOffset, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::str::FromStr;
use url::form_urlencoded;

use crate::utils::{beijing_now, compact_date};

const QUERY_URL: &str = "https://www.cninfo.com.cn/new/hisAnnouncement/query";
const DETAIL_URL: &str = "https://www.cninfo.com.cn/new/disclosure/detail";
const STATIC_URL: &str = "https://static.cninfo.com.cn";

// ─── Domain types ────────────────────────────────────────────────────────────

/// 巨潮资讯市场栏目。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum CninfoMarket {
    /// 沪深京 A 股。
    #[default]
    Szse,
    /// 港股。
    Hke,
    /// 三板。
    Third,
    /// 基金。
    Fund,
    /// 债券。
    Bond,
    /// 监管公告。
    Regulator,
    /// 预披露。
    PreDisclosure,
}

impl CninfoMarket {
    const fn column(self) -> &'static str {
        match self {
            Self::Szse => "szse",
            Self::Hke => "hke",
            Self::Third => "third",
            Self::Fund => "fund",
            Self::Bond => "bond",
            Self::Regulator => "regulator",
            Self::PreDisclosure => "pre_disclosure",
        }
    }

    const fn stock_list_url(self) -> Option<&'static str> {
        match self {
            Self::Szse => Some("https://www.cninfo.com.cn/new/data/szse_stock.json"),
            Self::Hke => Some("https://www.cninfo.com.cn/new/data/hke_stock.json"),
            Self::Third => Some("https://www.cninfo.com.cn/new/data/gfzr_stock.json"),
            Self::Fund => Some("https://www.cninfo.com.cn/new/data/fund_stock.json"),
            Self::Bond => Some("https://www.cninfo.com.cn/new/data/bond_stock.json"),
            Self::Regulator | Self::PreDisclosure => None,
        }
    }
}

impl fmt::Display for CninfoMarket {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Szse => "沪深京",
            Self::Hke => "港股",
            Self::Third => "三板",
            Self::Fund => "基金",
            Self::Bond => "债券",
            Self::Regulator => "监管",
            Self::PreDisclosure => "预披露",
        };
        formatter.write_str(value)
    }
}

impl FromStr for CninfoMarket {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "szse" | "沪深京" | "a股" | "ashare" => Ok(Self::Szse),
            "hke" | "港股" | "hk" => Ok(Self::Hke),
            "third" | "三板" | "neeq" => Ok(Self::Third),
            "fund" | "基金" => Ok(Self::Fund),
            "bond" | "债券" => Ok(Self::Bond),
            "regulator" | "监管" => Ok(Self::Regulator),
            "pre_disclosure" | "预披露" => Ok(Self::PreDisclosure),
            other => bail!("不支持的巨潮市场: {other}"),
        }
    }
}

/// 巨潮公告分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AnnouncementCategory {
    AnnualReport,
    SemiAnnualReport,
    FirstQuarterReport,
    ThirdQuarterReport,
    EarningsForecast,
    EquityDistribution,
    BoardMeeting,
    SupervisoryBoardMeeting,
    ShareholdersMeeting,
    DailyOperations,
    CorporateGovernance,
    IntermediaryReport,
    Ipo,
    SeasonedOffering,
    EquityIncentive,
    RightsIssue,
    ShareUnlock,
    CorporateBond,
    ConvertibleBond,
    OtherFinancing,
    EquityChange,
    Correction,
    Clarification,
    RiskWarning,
    SpecialTreatment,
    Delisting,
}

impl AnnouncementCategory {
    const fn as_code(self) -> &'static str {
        match self {
            Self::AnnualReport => "category_ndbg_szsh",
            Self::SemiAnnualReport => "category_bndbg_szsh",
            Self::FirstQuarterReport => "category_yjdbg_szsh",
            Self::ThirdQuarterReport => "category_sjdbg_szsh",
            Self::EarningsForecast => "category_yjygjxz_szsh",
            Self::EquityDistribution => "category_qyfpxzcs_szsh",
            Self::BoardMeeting => "category_dshgg_szsh",
            Self::SupervisoryBoardMeeting => "category_jshgg_szsh",
            Self::ShareholdersMeeting => "category_gddh_szsh",
            Self::DailyOperations => "category_rcjy_szsh",
            Self::CorporateGovernance => "category_gszl_szsh",
            Self::IntermediaryReport => "category_zj_szsh",
            Self::Ipo => "category_sf_szsh",
            Self::SeasonedOffering => "category_zf_szsh",
            Self::EquityIncentive => "category_gqjl_szsh",
            Self::RightsIssue => "category_pg_szsh",
            Self::ShareUnlock => "category_jj_szsh",
            Self::CorporateBond => "category_gszq_szsh",
            Self::ConvertibleBond => "category_kzzq_szsh",
            Self::OtherFinancing => "category_qtrz_szsh",
            Self::EquityChange => "category_gqbd_szsh",
            Self::Correction => "category_bcgz_szsh",
            Self::Clarification => "category_cqdq_szsh",
            Self::RiskWarning => "category_fxts_szsh",
            Self::SpecialTreatment => "category_tbclts_szsh",
            Self::Delisting => "category_tszlq_szsh",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::AnnualReport => "年报",
            Self::SemiAnnualReport => "半年报",
            Self::FirstQuarterReport => "一季报",
            Self::ThirdQuarterReport => "三季报",
            Self::EarningsForecast => "业绩预告",
            Self::EquityDistribution => "权益分派",
            Self::BoardMeeting => "董事会",
            Self::SupervisoryBoardMeeting => "监事会",
            Self::ShareholdersMeeting => "股东大会",
            Self::DailyOperations => "日常经营",
            Self::CorporateGovernance => "公司治理",
            Self::IntermediaryReport => "中介报告",
            Self::Ipo => "首发",
            Self::SeasonedOffering => "增发",
            Self::EquityIncentive => "股权激励",
            Self::RightsIssue => "配股",
            Self::ShareUnlock => "解禁",
            Self::CorporateBond => "公司债",
            Self::ConvertibleBond => "可转债",
            Self::OtherFinancing => "其他融资",
            Self::EquityChange => "股权变动",
            Self::Correction => "补充更正",
            Self::Clarification => "澄清致歉",
            Self::RiskWarning => "风险提示",
            Self::SpecialTreatment => "特别处理和退市",
            Self::Delisting => "退市整理期",
        }
    }
}

impl fmt::Display for AnnouncementCategory {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

impl FromStr for AnnouncementCategory {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let value = value.trim();
        let category = match value {
            "年报" | "annual" => Self::AnnualReport,
            "半年报" | "semi_annual" => Self::SemiAnnualReport,
            "一季报" | "q1" => Self::FirstQuarterReport,
            "三季报" | "q3" => Self::ThirdQuarterReport,
            "业绩预告" | "forecast" => Self::EarningsForecast,
            "权益分派" | "dividend" => Self::EquityDistribution,
            "董事会" | "board" => Self::BoardMeeting,
            "监事会" | "supervisory" => Self::SupervisoryBoardMeeting,
            "股东大会" | "shareholders" => Self::ShareholdersMeeting,
            "日常经营" | "operations" => Self::DailyOperations,
            "公司治理" | "governance" => Self::CorporateGovernance,
            "中介报告" | "intermediary" => Self::IntermediaryReport,
            "首发" | "ipo" => Self::Ipo,
            "增发" | "offering" => Self::SeasonedOffering,
            "股权激励" | "incentive" => Self::EquityIncentive,
            "配股" | "rights" => Self::RightsIssue,
            "解禁" | "unlock" => Self::ShareUnlock,
            "公司债" | "corporate_bond" => Self::CorporateBond,
            "可转债" | "convertible_bond" => Self::ConvertibleBond,
            "其他融资" | "other_financing" => Self::OtherFinancing,
            "股权变动" | "equity_change" => Self::EquityChange,
            "补充更正" | "correction" => Self::Correction,
            "澄清致歉" | "clarification" => Self::Clarification,
            "风险提示" | "risk" => Self::RiskWarning,
            "特别处理和退市" | "special_treatment" => Self::SpecialTreatment,
            "退市整理期" | "delisting" => Self::Delisting,
            other => bail!("不支持的公告分类: {other}"),
        };
        Ok(category)
    }
}

/// 公告条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Announcement {
    pub code: String,
    pub name: String,
    pub title: String,
    /// `YYYY-MM-DD HH:MM:SS`（北京时间）。
    pub announcement_time: String,
    /// 公告详情页。
    pub url: String,
    /// 公告 PDF 地址（存在时）。
    pub pdf_url: Option<String>,
}

/// 公告检索请求（可选参数较多，使用 builder）。
#[derive(Debug, Clone)]
pub struct CninfoQuery {
    /// 股票/基金/债券代码，留空表示全市场检索。
    pub symbol: String,
    pub market: CninfoMarket,
    pub keyword: Option<String>,
    pub category: Option<AnnouncementCategory>,
    /// `YYYYMMDD` 或 `YYYY-MM-DD`。
    pub start_date: String,
    /// `YYYYMMDD` 或 `YYYY-MM-DD`。
    pub end_date: String,
    /// 每页条数，1~100。
    pub page_size: usize,
    /// 最多拉取页数，防止异常大的结果集。
    pub max_pages: usize,
}

impl CninfoQuery {
    /// 创建一个默认查询最近 30 天沪深京公告的请求。
    pub fn new(symbol: impl Into<String>) -> Self {
        let today = beijing_now().date_naive();
        let start = today - Duration::days(30);
        Self {
            symbol: symbol.into(),
            market: CninfoMarket::Szse,
            keyword: None,
            category: None,
            start_date: start.format("%Y%m%d").to_string(),
            end_date: today.format("%Y%m%d").to_string(),
            page_size: 30,
            max_pages: 50,
        }
    }

    #[must_use]
    pub fn market(mut self, market: CninfoMarket) -> Self {
        self.market = market;
        self
    }

    #[must_use]
    pub fn keyword(mut self, keyword: impl Into<String>) -> Self {
        self.keyword = Some(keyword.into());
        self
    }

    #[must_use]
    pub fn category(mut self, category: AnnouncementCategory) -> Self {
        self.category = Some(category);
        self
    }

    #[must_use]
    pub fn date_range(
        mut self,
        start_date: impl Into<String>,
        end_date: impl Into<String>,
    ) -> Self {
        self.start_date = start_date.into();
        self.end_date = end_date.into();
        self
    }

    #[must_use]
    pub fn page_size(mut self, page_size: usize) -> Self {
        self.page_size = page_size;
        self
    }

    #[must_use]
    pub fn max_pages(mut self, max_pages: usize) -> Self {
        self.max_pages = max_pages;
        self
    }
}

// ─── Client ──────────────────────────────────────────────────────────────────

/// 巨潮资讯公告客户端。
#[derive(Clone)]
pub struct CninfoClient {
    client: Client,
}

impl Default for CninfoClient {
    fn default() -> Self {
        Self::new()
    }
}

impl CninfoClient {
    /// 创建客户端。
    ///
    /// # Panics
    /// Panics if the reqwest client cannot be built (should never happen).
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent(
                    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
                     AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36",
                )
                .build()
                .unwrap_or_else(|error| panic!("failed to build reqwest client: {error}")),
        }
    }

    /// 检索公告。
    ///
    /// 返回结果按巨潮接口默认顺序排列。`symbol` 留空时可做全市场关键词检索。
    ///
    /// # Errors
    /// Returns an error if dates are invalid, the symbol cannot be resolved in
    /// the selected market, the HTTP request fails, or the response cannot be
    /// parsed.
    pub async fn announcements(&self, query: &CninfoQuery) -> Result<Vec<Announcement>> {
        let start = compact_date(&query.start_date)?;
        let end = compact_date(&query.end_date)?;
        ensure!(
            query.page_size > 0 && query.page_size <= 100,
            "page_size 必须在 1~100"
        );
        ensure!(
            query.max_pages > 0 && query.max_pages <= 200,
            "max_pages 必须在 1~200"
        );

        let stock = if query.symbol.trim().is_empty() {
            String::new()
        } else {
            let code = normalize_cninfo_code(&query.symbol);
            let org_id = self
                .org_id(query.market, &code)
                .await?
                .with_context(|| format!("{}市场未找到代码 {code}", query.market))?;
            format!("{code},{org_id}")
        };

        let se_date = format!(
            "{}-{}-{}~{}-{}-{}",
            &start[0..4],
            &start[4..6],
            &start[6..8],
            &end[0..4],
            &end[4..6],
            &end[6..8]
        );

        let mut page = 1;
        let mut pages_to_fetch = query.max_pages;
        let mut announcements = Vec::new();
        loop {
            let page_query = CninfoPageQuery {
                stock: &stock,
                se_date: &se_date,
                page,
            };
            let response = self.query_page(query, &page_query).await?;

            if page == 1 {
                let total = response.total_announcement.unwrap_or(0).max(0) as usize;
                if total == 0 {
                    break;
                }
                let total_pages = total.div_ceil(query.page_size);
                pages_to_fetch = pages_to_fetch.min(total_pages);
            }

            let rows = response.announcements.unwrap_or_default();
            if rows.is_empty() {
                break;
            }
            announcements.extend(rows.into_iter().map(parse_announcement));

            if page >= pages_to_fetch {
                break;
            }
            page += 1;
        }
        Ok(announcements)
    }

    async fn query_page(
        &self,
        query: &CninfoQuery,
        page_query: &CninfoPageQuery<'_>,
    ) -> Result<CninfoResponse> {
        let payload = vec![
            ("pageNum", page_query.page.to_string()),
            ("pageSize", query.page_size.to_string()),
            ("column", query.market.column().to_string()),
            ("tabName", "fulltext".to_string()),
            ("plate", String::new()),
            ("stock", page_query.stock.to_string()),
            (
                "searchkey",
                query.keyword.as_deref().unwrap_or_default().to_string(),
            ),
            ("secid", String::new()),
            (
                "category",
                query
                    .category
                    .map(|category| category.as_code().to_string())
                    .unwrap_or_default(),
            ),
            ("trade", String::new()),
            ("seDate", page_query.se_date.to_string()),
            ("sortName", String::new()),
            ("sortType", String::new()),
            ("isHLtitle", "true".to_string()),
        ];

        self.client
            .post(QUERY_URL)
            .form(&payload)
            .send()
            .await
            .context("巨潮公告请求失败")?
            .json()
            .await
            .context("巨潮公告响应不是合法 JSON")
    }

    async fn org_id(&self, market: CninfoMarket, code: &str) -> Result<Option<String>> {
        let Some(url) = market.stock_list_url() else {
            bail!("{market}市场不支持按代码检索，请将 symbol 留空");
        };
        let response: Value = self
            .client
            .get(url)
            .send()
            .await
            .with_context(|| format!("获取{market}证券代码表失败"))?
            .json()
            .await
            .with_context(|| format!("{market}证券代码表不是合法 JSON"))?;
        let rows = response["stockList"]
            .as_array()
            .with_context(|| format!("{market}证券代码表缺少 stockList"))?;
        Ok(rows.iter().find_map(|row| {
            if row["code"].as_str() == Some(code) {
                row["orgId"].as_str().map(std::string::ToString::to_string)
            } else {
                None
            }
        }))
    }
}

// ─── Raw response ────────────────────────────────────────────────────────────

struct CninfoPageQuery<'a> {
    stock: &'a str,
    se_date: &'a str,
    page: usize,
}

#[derive(Debug, Deserialize)]
struct CninfoResponse {
    #[serde(rename = "totalAnnouncement")]
    total_announcement: Option<i64>,
    announcements: Option<Vec<CninfoRow>>,
}

#[derive(Debug, Deserialize)]
struct CninfoRow {
    #[serde(rename = "secCode")]
    sec_code: String,
    #[serde(rename = "secName")]
    sec_name: String,
    #[serde(rename = "announcementTitle")]
    announcement_title: String,
    #[serde(rename = "announcementTime")]
    announcement_time: i64,
    #[serde(rename = "announcementId")]
    announcement_id: String,
    #[serde(rename = "orgId")]
    org_id: String,
    #[serde(rename = "adjunctUrl")]
    adjunct_url: Option<String>,
}

// ─── Parsing helpers ─────────────────────────────────────────────────────────

fn parse_announcement(row: CninfoRow) -> Announcement {
    let time = format_cninfo_time(row.announcement_time);
    let encoded_time: String = form_urlencoded::byte_serialize(time.as_bytes()).collect();
    let url = format!(
        "{DETAIL_URL}?stockCode={}&announcementId={}&orgId={}&announcementTime={encoded_time}",
        row.sec_code, row.announcement_id, row.org_id
    );
    let pdf_url = row
        .adjunct_url
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            if value.starts_with("http://") || value.starts_with("https://") {
                value
            } else {
                format!("{STATIC_URL}/{}", value.trim_start_matches('/'))
            }
        });
    Announcement {
        code: row.sec_code,
        name: row.sec_name,
        title: row.announcement_title,
        announcement_time: time,
        url,
        pdf_url,
    }
}

fn format_cninfo_time(timestamp_ms: i64) -> String {
    let Some(offset) = FixedOffset::east_opt(8 * 3600) else {
        return timestamp_ms.to_string();
    };
    let Some(time) = DateTime::<Utc>::from_timestamp_millis(timestamp_ms) else {
        return timestamp_ms.to_string();
    };
    time.with_timezone(&offset)
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

fn normalize_cninfo_code(symbol: &str) -> String {
    let symbol = symbol.trim();
    let without_suffix = symbol.split('.').next().unwrap_or(symbol);
    let lower = without_suffix.to_ascii_lowercase();
    for prefix in ["sh", "sz", "bj"] {
        if let Some(code) = lower.strip_prefix(prefix) {
            if !code.is_empty() && code.bytes().all(|byte| byte.is_ascii_digit()) {
                return code.to_string();
            }
        }
    }
    without_suffix.to_string()
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_announcement() {
        let row = CninfoRow {
            sec_code: "600519".to_string(),
            sec_name: "贵州茅台".to_string(),
            announcement_title: "贵州茅台2026年半年度报告".to_string(),
            announcement_time: 1_786_723_200_000,
            announcement_id: "1225475868".to_string(),
            org_id: "gssh0600519".to_string(),
            adjunct_url: Some("finalpage/2026-08-15/1225475868.PDF".to_string()),
        };
        let announcement = parse_announcement(row);
        assert_eq!(announcement.code, "600519");
        assert_eq!(announcement.announcement_time, "2026-08-15 00:00:00");
        assert!(announcement.url.contains("announcementId=1225475868"));
        assert_eq!(
            announcement.pdf_url.as_deref(),
            Some("https://static.cninfo.com.cn/finalpage/2026-08-15/1225475868.PDF")
        );
    }

    #[test]
    fn normalizes_cninfo_codes() {
        assert_eq!(normalize_cninfo_code("600519.SS"), "600519");
        assert_eq!(normalize_cninfo_code("sh600519"), "600519");
        assert_eq!(normalize_cninfo_code("00700.HK"), "00700");
        assert_eq!(normalize_cninfo_code("600519"), "600519");
    }

    #[test]
    fn category_codes_match_akshare() {
        assert_eq!(
            AnnouncementCategory::AnnualReport.as_code(),
            "category_ndbg_szsh"
        );
        assert_eq!(
            AnnouncementCategory::Delisting.as_code(),
            "category_tszlq_szsh"
        );
        assert_eq!(AnnouncementCategory::RiskWarning.label(), "风险提示");
    }

    #[test]
    fn parses_market_names() {
        assert_eq!(
            "沪深京"
                .parse::<CninfoMarket>()
                .unwrap_or(CninfoMarket::Szse),
            CninfoMarket::Szse
        );
        assert_eq!(
            "港股".parse::<CninfoMarket>().unwrap_or(CninfoMarket::Szse),
            CninfoMarket::Hke
        );
        assert!("不存在".parse::<CninfoMarket>().is_err());
    }

    #[tokio::test]
    #[ignore = "integration test, requires network"]
    async fn integration_announcements() {
        let query = CninfoQuery::new("600519")
            .date_range("20260101", "20261021")
            .page_size(10)
            .max_pages(2);
        let rows = CninfoClient::new()
            .announcements(&query)
            .await
            .unwrap_or_else(|error| panic!("request failed: {error}"));
        assert!(!rows.is_empty());
        assert!(rows.iter().all(|row| row.code == "600519"));
    }
}
