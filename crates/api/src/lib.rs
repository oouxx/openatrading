pub mod cls;
pub mod cninfo;
pub mod eastmoney;
pub mod jiuyangongshe;
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
    /// 盘口五档：优先东方财富（含涨跌停价、字段最全），东财不可用时退回
    /// 新浪五档（只有价与量）。
    ///
    /// 返回里的 `source` 标明实际来源——降级是显式行为，不静默：调用方/UI
    /// 能看出这份盘口少了涨跌停，而不是把 0 当成真实跌停价。
    ///
    /// # Errors
    /// 两个来源都失败时返回东财的错误（首要来源的问题更值得暴露）。
    pub async fn fetch_order_book(&self, symbol: &str) -> anyhow::Result<eastmoney::OrderBook> {
        match self.eastmoney.fetch_order_book(symbol).await {
            Ok(book) => Ok(book),
            Err(primary) => match self.sina_order_book(symbol).await {
                Ok(book) => Ok(book),
                Err(_) => Err(primary),
            },
        }
    }

    /// 新浪五档 → [`eastmoney::OrderBook`]（量从股换算成手，无涨跌停）。
    async fn sina_order_book(&self, symbol: &str) -> anyhow::Result<eastmoney::OrderBook> {
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

        // 新浪档位量是股，`OrderBookLevel` 的口径是手（1 手 = 100 股）。
        let level = |l: &crate::sina::SinaLevel| eastmoney::OrderBookLevel {
            price: l.price,
            volume: l.volume / 100.0,
        };
        Ok(eastmoney::OrderBook {
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
            source: eastmoney::OrderBookSource::Sina,
        })
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}
