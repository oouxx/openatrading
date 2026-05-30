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

// ── Types ──────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DragonTigerEntry {
    pub stock_code: String,
    pub stock_name: String,
    pub trade_date: String,
    pub reason: String,
    pub close_price: Option<f64>,
    pub change_pct: Option<f64>,
    pub inst_buy_amount: Option<f64>,
    pub inst_sell_amount: Option<f64>,
    pub dept_buy_amount: Option<f64>,
    pub dept_sell_amount: Option<f64>,
    pub net_buy: f64,
}

/// Parse Sina's LHB data page and extract entries
pub fn parse_lhb_detail(html: &str) -> Result<Vec<DragonTigerEntry>> {
    let re = Regex::new(r"var\s+LHBData\s*=\s*(\{.*?\});").unwrap();
    let cap = re.captures(html).context("LHBData JSON block not found")?;
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
            stock_code: raw.stock_code,
            stock_name: raw.stock_name,
            trade_date: raw.trade_date,
            reason: raw.reason,
            close_price: raw.close_price.parse().ok(),
            change_pct: raw.change_pct.trim_end_matches('%').parse().ok(),
            inst_buy_amount: if inst_buy > 0.0 { Some(inst_buy) } else { None },
            inst_sell_amount: if inst_sell > 0.0 { Some(inst_sell) } else { None },
            dept_buy_amount: if dept_buy > 0.0 { Some(dept_buy) } else { None },
            dept_sell_amount: if dept_sell > 0.0 { Some(dept_sell) } else { None },
            net_buy: net,
        }
    }).collect();

    Ok(entries)
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
    #[serde(alias = "ticketcode")]
    stock_code: String,
    #[serde(alias = "ticketname")]
    stock_name: String,
    #[serde(alias = "tradedate")]
    trade_date: String,
    #[serde(alias = "reason")]
    reason: String,
    #[serde(alias = "closeprice")]
    close_price: String,
    #[serde(alias = "changepercent")]
    change_pct: String,
    #[serde(default, alias = "JGBuy")]
    jgbuy: String,
    #[serde(default, alias = "JGSell")]
    jgsell: String,
    #[serde(default, alias = "DeptBuy")]
    deptbuy: String,
    #[serde(default, alias = "DeptSell")]
    deptsell: String,
}

impl SinaClient {
    pub async fn fetch_dragon_tiger(&self) -> Result<Vec<DragonTigerEntry>> {
        let url = "http://vip.stock.finance.sina.com.cn/q/go.php/vLHBData/kind/jgzz/index.phtml";
        let resp = self.client.get(url).send().await?;
        let body = resp.text_with_charset("gb2312").await?;
        parse_lhb_detail(&body)
    }
}

// ── Tests ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_lhb_detail_valid() {
        let html = r#"<html><script>var LHBData = {"data":[{"ticketcode":"600519","ticketname":"贵州茅台","tradedate":"2025-01-20","reason":"日涨幅偏离值达7%","closeprice":"1685.00","changepercent":"10.00%","JGBuy":"15200","JGSell":"8200","DeptBuy":"28500","DeptSell":"3500"}]};</script></html>"#;
        let entries = parse_lhb_detail(html).unwrap();
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        assert_eq!(e.stock_code, "600519");
        assert_eq!(e.stock_name, "贵州茅台");
        assert_eq!(e.trade_date, "2025-01-20");
        assert_eq!(e.reason, "日涨幅偏离值达7%");
        assert_eq!(e.close_price, Some(1685.0));
        assert_eq!(e.change_pct, Some(10.0));
        assert_eq!(e.inst_buy_amount, Some(15200.0));
        assert_eq!(e.inst_sell_amount, Some(8200.0));
        assert_eq!(e.dept_buy_amount, Some(28500.0));
        assert_eq!(e.dept_sell_amount, Some(3500.0));
        // net = (15200+28500) - (8200+3500) = 32000
        assert_eq!(e.net_buy, 32000.0);
    }

    #[test]
    fn test_parse_lhb_detail_empty_fields() {
        let html = r#"<script>var LHBData = {"data":[{"ticketcode":"000001","ticketname":"平安银行","tradedate":"2025-01-20","reason":"日换手率达20%","closeprice":"12.50","changepercent":"-3.2%","JGBuy":"0","JGSell":"0","DeptBuy":"6200","DeptSell":"0"}]};</script>"#;
        let entries = parse_lhb_detail(html).unwrap();
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        assert_eq!(e.stock_name, "平安银行");
        assert_eq!(e.inst_buy_amount, None);
        assert_eq!(e.inst_sell_amount, None);
        assert_eq!(e.dept_buy_amount, Some(6200.0));
        assert_eq!(e.dept_sell_amount, None);
        assert_eq!(e.net_buy, 6200.0);
        assert_eq!(e.change_pct, Some(-3.2));
    }

    #[test]
    fn test_parse_lhb_detail_negative_net() {
        let html = r#"<script>var LHBData = {"data":[{"ticketcode":"000056","ticketname":"皇庭国际","tradedate":"2025-01-20","reason":"日跌幅偏离值达7%","closeprice":"4.50","changepercent":"-9.80%","JGBuy":"0","JGSell":"0","DeptBuy":"1800","DeptSell":"5000"}]};</script>"#;
        let entries = parse_lhb_detail(html).unwrap();
        let e = &entries[0];
        assert_eq!(e.net_buy, -3200.0);
    }

    #[test]
    fn test_parse_lhb_detail_multiple_entries() {
        let html = r#"<script>var LHBData = {"data":[
            {"ticketcode":"600519","ticketname":"贵州茅台","tradedate":"2025-01-20","reason":"A","closeprice":"1","changepercent":"1%","JGBuy":"100","JGSell":"0","DeptBuy":"200","DeptSell":"0"},
            {"ticketcode":"000001","ticketname":"平安银行","tradedate":"2025-01-20","reason":"B","closeprice":"2","changepercent":"2%","JGBuy":"300","JGSell":"0","DeptBuy":"400","DeptSell":"0"}
        ]};</script>"#;
        let entries = parse_lhb_detail(html).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].stock_code, "600519");
        assert_eq!(entries[1].stock_code, "000001");
    }

    #[test]
    fn test_parse_lhb_detail_no_data() {
        let html = r#"<script>var LHBData = {"data":[]};</script>"#;
        let entries = parse_lhb_detail(html).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_lhb_detail_missing_block() {
        let html = "<html>no data here</html>";
        let result = parse_lhb_detail(html);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_lhb_detail_gb2312_variable_name() {
        // Test with different variable name (should still match LHBData)
        let html = r#"<script type="text/javascript">var LHBData = {"data":[{"ticketcode":"300750","ticketname":"宁德时代","tradedate":"2025-01-20","reason":"日涨幅达15%","closeprice":"228.50","changepercent":"15.00%","JGBuy":"10200","JGSell":"5100","DeptBuy":"24800","DeptSell":"1800"}]};</script>"#;
        let entries = parse_lhb_detail(html).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].stock_name, "宁德时代");
        assert_eq!(entries[0].net_buy, 28100.0);
    }

    pub fn test_parse_lhb_detail_net_buy() {
        let html = r#"<script>var LHBData = {"data":[{"ticketcode":"002031","ticketname":"巨轮智能","tradedate":"2025-01-20","reason":"test","closeprice":"5.80","changepercent":"10.02%","JGBuy":"","JGSell":"","DeptBuy":"3800","DeptSell":"2000"}]};</script>"#;
        let entries = parse_lhb_detail(html).unwrap();
        assert_eq!(entries[0].net_buy, 1800.0);
    }
}
