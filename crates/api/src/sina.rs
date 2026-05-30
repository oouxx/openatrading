use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct SinaClient {
    client: reqwest::Client,
}

impl SinaClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
                .build()
                .unwrap(),
        }
    }
}

impl Default for SinaClient {
    fn default() -> Self { Self::new() }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DragonTigerEntry {
    pub stock_code: String,
    pub stock_name: String,
    pub trade_date: String,
    /// 上榜原因
    pub reason: String,
    /// 收盘价
    pub close_price: Option<f64>,
    /// 涨跌幅%
    pub change_pct: Option<f64>,
    /// 机构席位买入额(万元)
    pub inst_buy_amount: Option<f64>,
    /// 机构席位卖出额(万元)
    pub inst_sell_amount: Option<f64>,
    /// 营业部买入总额(万元)
    pub dept_buy_amount: Option<f64>,
    /// 营业部卖出总额(万元)
    pub dept_sell_amount: Option<f64>,
    /// 净买入额(万元)
    pub net_buy: f64,
    /// 买方营业部明细
    pub buy_depts: Vec<DeptDetail>,
    /// 卖方营业部明细
    pub sell_depts: Vec<DeptDetail>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeptDetail {
    pub name: String,
    pub amount: f64,
    /// "机构" 或 "营业部"
    pub dept_type: String,
}

impl SinaClient {
    /// Fetch dragon-tiger board data from Sina
    pub async fn fetch_dragon_tiger(&self) -> Result<Vec<DragonTigerEntry>> {
        let url = "http://vip.stock.finance.sina.com.cn/q/go.php/vLHBData/kind/jgzz/index.phtml";
        let resp = self.client.get(url).send().await?;
        let body = resp.text_with_charset("gb2312").await?;

        // Extract JSON from JavaScript: var LHBData = {...};
        let re = Regex::new(r"var\s+LHBData\s*=\s*(\{.*?\});").unwrap();
        let cap = re.captures(&body).context("LHBData not found in response")?;
        let json_str = cap.get(1).unwrap().as_str();

        let parsed: SinaLhbResponse = serde_json::from_str(json_str)
            .context("failed to parse LHBData JSON")?;

        let entries = parsed.data.into_iter().map(|raw| {
            let inst_buy = parse_amount(&raw.jgbuy);
            let inst_sell = parse_amount(&raw.jgsell);
            let dept_buy = parse_amount(&raw.deptbuy);
            let dept_sell = parse_amount(&raw.deptsell);
            let net = (inst_buy + dept_buy) - (inst_sell + dept_sell);

            DragonTigerEntry {
                stock_code: raw.stock_code.clone(),
                stock_name: raw.stock_name.clone(),
                trade_date: raw.trade_date.clone(),
                reason: raw.reason.clone(),
                close_price: raw.close_price.parse().ok(),
                change_pct: raw.change_pct.trim_end_matches('%').parse().ok(),
                inst_buy_amount: if inst_buy > 0.0 { Some(inst_buy) } else { None },
                inst_sell_amount: if inst_sell > 0.0 { Some(inst_sell) } else { None },
                dept_buy_amount: if dept_buy > 0.0 { Some(dept_buy) } else { None },
                dept_sell_amount: if dept_sell > 0.0 { Some(dept_sell) } else { None },
                net_buy: net,
                buy_depts: vec![],
                sell_depts: vec![],
            }
        }).collect();

        Ok(entries)
    }
}

fn parse_amount(s: &str) -> f64 {
    s.trim().parse::<f64>().unwrap_or(0.0)
}

#[derive(Debug, Deserialize)]
struct SinaLhbResponse {
    data: Vec<SinaLhbRaw>,
}

#[derive(Debug, Deserialize)]
struct SinaLhbRaw {
    /// 股票代码
    #[serde(alias = "ticketcode")]
    stock_code: String,
    /// 股票名称
    #[serde(alias = "ticketname")]
    stock_name: String,
    /// 交易日期
    #[serde(alias = "tradedate")]
    trade_date: String,
    /// 上榜原因
    #[serde(alias = "reason")]
    reason: String,
    /// 收盘价
    #[serde(alias = "closeprice")]
    close_price: String,
    /// 涨跌幅
    #[serde(alias = "changepercent")]
    change_pct: String,
    /// 机构买入
    #[serde(default, alias = "JGBuy")]
    jgbuy: String,
    /// 机构卖出
    #[serde(default, alias = "JGSell")]
    jgsell: String,
    /// 营业部买入
    #[serde(default, alias = "DeptBuy")]
    deptbuy: String,
    /// 营业部卖出
    #[serde(default, alias = "DeptSell")]
    deptsell: String,
    #[serde(default)]
    #[allow(dead_code)]
    buy_dept: String,
    #[serde(default)]
    #[allow(dead_code)]
    sell_dept: String,
}
