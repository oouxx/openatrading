pub mod cls;
pub mod cninfo;
pub mod eastmoney;
pub mod jiuyangongshe;
pub mod order_book;
pub mod sina;
pub mod taoguba;
pub mod utils;
pub mod xuangubao;
use cls::ClsClient;

use cninfo::CninfoClient;
use jiuyangongshe::JiuYanSession;
use xuangubao::XuanguBaoClient;

use crate::eastmoney::EastmoneyClient;
#[derive(Clone)]
pub struct Client {
    pub xuangubao: XuanguBaoClient,
    pub jiuyan: JiuYanSession,
    pub cls: ClsClient,
    pub eastmoney: EastmoneyClient,
    pub cninfo: CninfoClient,
}

impl Client {
    pub fn new() -> Self {
        Self {
            xuangubao: XuanguBaoClient::new(),
            jiuyan: JiuYanSession::new(),
            cls: ClsClient::new(),
            eastmoney: EastmoneyClient::new(),
            cninfo: CninfoClient::new(),
        }
    }
}

impl Client {
    /// 盘口五档统一使用新浪行情。
    ///
    /// 新浪一次返回完整买卖五档；档位量从这里开始统一换算成手。
    /// 新浪不提供涨跌停价，对应字段固定为 0。
    ///
    /// # Errors
    /// 股票代码无法识别、请求失败或新浪没有返回对应行情时返回错误。
    pub async fn fetch_order_book(&self, symbol: &str) -> anyhow::Result<order_book::OrderBook> {
        use anyhow::Context;

        let yahoo = crate::utils::to_yahoo_symbol(symbol)?;
        let (code, suffix) = yahoo
            .split_once('.')
            .with_context(|| format!("无法识别股票代码: {symbol}"))?;
        if !matches!(suffix, "SS" | "SZ") {
            anyhow::bail!("新浪盘口只支持沪深 A 股: {symbol}");
        }

        let sina = crate::sina::SinaQuoteClient::new()?;
        let quotes = sina.fetch_quotes(&[code]).await?;
        let quote = quotes
            .into_iter()
            .next()
            .with_context(|| format!("新浪没有返回 {code} 的行情"))?;

        Ok(order_book_from_sina_quote(code, &quote))
    }
}

/// 把新浪原始盘口转换为统一盘口模型：档位量由股换算成手。
fn order_book_from_sina_quote(
    code: &str,
    quote: &crate::sina::AuctionQuote,
) -> order_book::OrderBook {
    let level = |l: &crate::sina::SinaLevel| order_book::OrderBookLevel {
        price: l.price,
        volume: l.volume / 100.0,
    };
    order_book::OrderBook {
        symbol: code.to_string(),
        name: quote.name.clone(),
        last: quote.current_price,
        prev_close: quote.prev_close,
        open: 0.0,
        high: 0.0,
        low: 0.0,
        change_pct: quote.change_pct,
        // 新浪不提供涨跌停价
        limit_up: 0.0,
        limit_down: 0.0,
        bids: quote.bids.iter().map(level).collect(),
        asks: quote.asks.iter().map(level).collect(),
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_sina_order_book_volume_to_lots() {
        let level = |price: f64, volume: f64| crate::sina::SinaLevel { price, volume };
        let quote = crate::sina::AuctionQuote {
            code: "000001".to_string(),
            name: "平安银行".to_string(),
            prev_close: 11.2,
            current_price: 11.3,
            change_pct: 0.89,
            bid1_price: 11.29,
            bid1_volume: 19_800,
            ask1_price: 11.3,
            ask1_volume: 900,
            bids: vec![level(11.29, 19_800.0)],
            asks: vec![level(11.3, 900.0)],
            volume: 132_311_308,
            amount: 1_485_999_504.06,
            time: "09:34:32".to_string(),
        };

        let book = order_book_from_sina_quote("000001", &quote);

        assert_eq!(book.bids[0].volume, 198.0);
        assert_eq!(book.asks[0].volume, 9.0);
        assert_eq!(book.last, 11.3);
    }
}
