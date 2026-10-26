use anyhow::{Context, Result};
use encoding_rs::GBK;
use reqwest::Client;
use std::time::Duration;

// ── 竞价行情数据结构 ────────────────────────────────

/// 盘口一档。新浪这一列给的是**股**（东财给手，注意换算）。
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct SinaLevel {
    pub price: f64,
    /// 挂单量（股）
    pub volume: f64,
}

/// 个股集合竞价实时快照（9:15-9:25 持续刷新）。
///
/// 新浪一次就回完整五档，`bids[0]`/`asks[0]` 是买一/卖一，按「离现价由近到远」
/// 排列；`bid1_*`/`ask1_*` 是老字段（等于 `bids[0]`/`asks[0]`），保留是为了
/// 不破坏已有调用方。
#[derive(Debug, Clone, serde::Serialize)]
pub struct AuctionQuote {
    pub code: String,
    pub name: String,
    pub prev_close: f64,
    pub current_price: f64, // 竞价撮合价，9:15-9:25 持续刷新
    pub change_pct: f64,    // 涨跌幅%
    pub bid1_price: f64,    // 买一价
    pub bid1_volume: u64,   // 买一量(股)
    pub ask1_price: f64,    // 卖一价
    pub ask1_volume: u64,   // 卖一量(股)
    /// 买一 … 买五（量单位：股）
    pub bids: Vec<SinaLevel>,
    /// 卖一 … 卖五（量单位：股）
    pub asks: Vec<SinaLevel>,
    pub volume: u64,  // 累计撮合量(股)
    pub amount: f64,  // 累计成交额
    pub time: String, // 数据时间
}

// ── 新浪行情客户端 ────────────────────────────────────

pub struct SinaQuoteClient {
    client: Client,
}

impl SinaQuoteClient {
    /// Create a new `SinaQuoteClient`.
    ///
    /// # Errors
    /// Returns an error if the reqwest client cannot be built.
    pub fn new() -> Result<Self> {
        let client = Client::builder().timeout(Duration::from_secs(5)).build()?;
        Ok(Self { client })
    }

    /// 批量获取竞价行情。
    ///
    /// `codes`: 不带前缀的纯代码，如 `["000001", "600519"]`。
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn fetch_quotes(&self, codes: &[&str]) -> Result<Vec<AuctionQuote>> {
        if codes.is_empty() {
            return Ok(vec![]);
        }
        let list = codes
            .iter()
            .map(|c| format!("{}{}", prefix_for(c), c))
            .collect::<Vec<_>>()
            .join(",");

        let url = format!("https://hq.sinajs.cn/list={list}");

        let bytes = self
            .client
            .get(&url)
            .header("Referer", "https://finance.sina.com.cn")
            .send()
            .await
            .context("请求新浪行情接口失败")?
            .bytes()
            .await?;

        let (text, _, had_errors) = GBK.decode(&bytes);
        if had_errors {
            tracing::warn!("[sina] GBK 解码存在异常字符");
        }

        Ok(parse_sina_response(&text, codes))
    }
}

fn prefix_for(code: &str) -> &'static str {
    match code.chars().next() {
        Some('0' | '3') => "sz",
        Some('4' | '8') => "bj",
        _ => "sh",
    }
}

// ── 响应解析 ──────────────────────────────────────────

fn parse_sina_response(text: &str, codes: &[&str]) -> Vec<AuctionQuote> {
    let mut quotes = Vec::with_capacity(codes.len());

    for (line, code) in text.lines().zip(codes.iter()) {
        // var hq_str_sz000001="平安银行,9.300,9.250,9.300,...";
        let (Some(start), Some(end)) = (line.find('"'), line.rfind('"')) else {
            continue;
        };
        if end <= start + 1 {
            continue; // 停牌/无数据
        }
        let fields: Vec<&str> = line[start + 1..end].split(',').collect();
        if fields.len() < 32 {
            continue;
        }

        let f = |i: usize| fields[i].parse::<f64>().unwrap_or(0.0);
        // 买卖五档：10..19 = (买一量,买一价) … (买五量,买五价)，
        // 20..29 = (卖一量,卖一价) … (卖五量,卖五价)。量单位是股。
        let level = |vol: usize, price: usize| SinaLevel {
            price: f(price),
            volume: f(vol),
        };
        let bids: Vec<SinaLevel> = (0..5).map(|i| level(10 + i * 2, 11 + i * 2)).collect();
        let asks: Vec<SinaLevel> = (0..5).map(|i| level(20 + i * 2, 21 + i * 2)).collect();
        let prev_close = f(2);
        let current_price = f(3);
        let change_pct = if prev_close > 0.0 {
            (current_price - prev_close) / prev_close * 100.0
        } else {
            0.0
        };

        quotes.push(AuctionQuote {
            code: (*code).to_string(),
            name: fields[0].to_string(),
            prev_close,
            current_price,
            change_pct,
            bid1_price: f(6),
            bid1_volume: fields[10].parse().unwrap_or(0),
            ask1_price: f(7),
            ask1_volume: fields[20].parse().unwrap_or(0),
            bids,
            asks,
            volume: fields[8].parse().unwrap_or(0),
            amount: f(9),
            time: fields[31].to_string(),
        });
    }

    quotes
}

// ── 测试 ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// 五档解析：10..19 = 买一..买五、20..29 = 卖一..卖五，量按股原样保留。
    /// 报文取自真实接口（2026-10-26 收盘后，卖档全 0 是正常形态）。
    #[test]
    fn parses_five_level_depth() {
        let line = r#"var hq_str_sz000001="平安银行,11.150,11.200,11.300,11.340,11.130,11.290,11.300,132311308,1485999504.060,11300,11.300,200,11.280,300,11.270,400,11.260,500,11.250,0,0.000,0,0.000,0,0.000,0,0.000,0,0.000,2026-10-26,15:13:45,00";"#;
        let quotes = parse_sina_response(line, &["000001"]);
        let q = quotes.first().unwrap_or_else(|| panic!("应解析出 1 条"));
        assert_eq!(q.bids.len(), 5);
        assert_eq!(q.asks.len(), 5);
        // 买一 = 11.300 / 11300 股，买五 = 11.250 / 500 股
        assert!((q.bids[0].price - 11.300).abs() < 1e-9);
        assert!((q.bids[0].volume - 11300.0).abs() < 1e-9);
        assert!((q.bids[4].price - 11.250).abs() < 1e-9);
        assert!((q.bids[4].volume - 500.0).abs() < 1e-9);
        // 收盘后卖档全 0：档位存在但价量为 0，由调用方判定「无挂单」
        assert!((q.asks[0].price - 0.0).abs() < 1e-9);
        assert_eq!(q.bid1_volume, 11300);
    }

    use std::time::Instant;

    #[tokio::test]
    #[ignore = "integration test, requires network and live auction data"]
    async fn test_fetch_single_stock() {
        let client = SinaQuoteClient::new().unwrap_or_else(|e| panic!("test setup failed: {e}"));
        let start = Instant::now();
        let quotes = client
            .fetch_quotes(&["000001"])
            .await
            .unwrap_or_else(|e| panic!("test failed: {e}"));
        let elapsed = start.elapsed();
        assert!(!quotes.is_empty(), "should return at least 1 quote");
        let q = &quotes[0];
        assert_eq!(q.code, "000001");
        assert!(!q.name.is_empty(), "name should not be empty");
        assert!(q.prev_close > 0.0, "prev_close should be > 0");
        assert!(
            q.current_price > 0.0 || q.volume > 0,
            "price or volume should be meaningful"
        );
        eprintln!(
            "[sina test] {} {} | prev={:.2} price={:.2} chg={:.2}% | bid {:.2}x{} ask {:.2}x{} | vol={} amount={:.0} | {}ms",
            q.code,
            q.name,
            q.prev_close,
            q.current_price,
            q.change_pct,
            q.bid1_price,
            q.bid1_volume,
            q.ask1_price,
            q.ask1_volume,
            q.volume,
            q.amount,
            elapsed.as_millis()
        );
    }

    #[tokio::test]
    async fn test_fetch_batch() {
        let client = SinaQuoteClient::new().unwrap_or_else(|e| panic!("test setup failed: {e}"));
        let codes = ["000001", "600519", "300750", "000858"];
        let quotes = client
            .fetch_quotes(&codes)
            .await
            .unwrap_or_else(|e| panic!("test failed: {e}"));
        assert_eq!(quotes.len(), codes.len(), "all codes should return data");
        for q in &quotes {
            assert!(!q.name.is_empty(), "{} name empty", q.code);
            eprintln!(
                "{} {} chg={:.2}% vol={}",
                q.code, q.name, q.change_pct, q.volume
            );
        }
    }
}
