//! 用**真实行情**验证模拟盘交易。
//!
//! 与 `trade.rs` 的区别：那边用注入的假行情覆盖逻辑分支，这边打真实网络
//! （选股宝实时行情接口），验的是「真实数据进来之后算法是否还对」。
//!
//! 因此默认 `#[ignore]`——仓库里 `crates/api/src/eastmoney.rs` 的网络测试同样是
//! 这个约定，离线或接口不可用时不会让 `cargo test` 变红。
//!
//! 运行：
//! ```bash
//! cargo test -p openatrading-core --test trade_live -- --ignored --nocapture
//! ```

use std::sync::Arc;

use openatrading_core::api::Client as ApiClient;
use openatrading_core::trade::{
    ApiQuoteSource, OrderRequest, Price, Shares, Side, TradeService, TradeStore,
};

/// 大额本金，保证真实股价下也买得起。
const CAPITAL: f64 = 10_000_000.0;

fn must<T, E: std::fmt::Display>(result: Result<T, E>, what: &str) -> T {
    result.unwrap_or_else(|e| panic!("{what} 失败: {e}"))
}

/// 走真实行情源的服务。
fn live_service(dir: &tempfile::TempDir) -> TradeService {
    let store = must(TradeStore::open(&dir.path().join("live.db")), "打开交易库");
    TradeService::new(
        Arc::new(store),
        Arc::new(ApiQuoteSource::new(ApiClient::new())),
    )
}

fn order(side: Side, symbol: &str, shares: i64, price: f64) -> OrderRequest {
    OrderRequest {
        side,
        symbol: symbol.to_string(),
        stock_name: String::new(),
        shares: must(Shares::new(shares), "构造股数"),
        price: must(Price::new(price), "构造价格"),
        reason: "真实行情测试".to_string(),
    }
}

fn assert_close(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() < 0.01,
        "{what}: 期望 {expected}，实际 {actual}"
    );
}

/// 买入两只真实标的，断言估值完全由**真实最新价**推出。
///
/// 覆盖的是最容易出错的一环：`last_price` 从行情接口到市值/浮盈的换算，
/// 以及行情键（Yahoo 格式）与持仓键（裸代码）的映射。
#[tokio::test]
#[ignore = "需要网络：打真实行情接口"]
async fn live_quotes_drive_valuation() {
    let dir = must(tempfile::tempdir(), "临时目录");
    let svc = live_service(&dir);

    // 600519 贵州茅台、000001 平安银行；成交价随便给，反正估值看的是最新价。
    for (symbol, shares) in [("600519.SS", 100), ("000001.SZ", 1000)] {
        must(
            svc.place_order("实盘验证", CAPITAL, &order(Side::Buy, symbol, shares, 1.0))
                .await,
            "买入",
        );
    }

    let (positions, complete) = must(svc.positions("实盘验证").await, "持仓");
    assert_eq!(positions.len(), 2, "两只标的应各占一行");
    assert!(complete, "真实行情应可得，估值应完整");

    for p in &positions {
        let last = must(
            p.last_price
                .ok_or_else(|| format!("{} 没拿到真实最新价", p.symbol)),
            "最新价",
        );
        assert!(last > 0.0, "{} 最新价应为正，实际 {last}", p.symbol);

        println!(
            "{} {} 股数={} 成本={:.3} 最新价={:.3} 市值={:?} 浮盈={:?} ({:?}%)",
            p.symbol,
            p.stock_name,
            p.shares,
            p.cost_price,
            last,
            p.market_value,
            p.unrealized_pnl,
            p.unrealized_pct
        );

        // 市值 = 最新价 × 股数（行情键与持仓键对不上时这里会先炸）
        assert_close(
            must(p.market_value.ok_or("缺市值"), "市值"),
            last * p.shares as f64,
            &format!("{} 市值", p.symbol),
        );
        // 浮盈 = (最新价 - 成本) × 股数
        assert_close(
            must(p.unrealized_pnl.ok_or("缺浮盈"), "浮盈"),
            (last - p.cost_price) * p.shares as f64,
            &format!("{} 浮盈", p.symbol),
        );
        assert!(
            !p.stock_name.is_empty(),
            "{} 未填名称时应由真实行情补全",
            p.symbol
        );
    }

    // 账户三项必须自洽：权益 = 现金 + 市值；总盈亏 = 权益 - 本金
    let account = must(svc.account("实盘验证", CAPITAL).await, "账户");
    assert_close(account.equity, account.cash + account.market_value, "权益");
    assert_close(account.total_pnl, account.equity - CAPITAL, "总盈亏");
    assert_close(
        account.unrealized_pnl,
        account.market_value
            - positions
                .iter()
                .map(|p| p.cost_price * p.shares as f64)
                .sum::<f64>(),
        "浮盈合计",
    );

    println!(
        "账户：现金={:.2} 市值={:.2} 权益={:.2} 浮盈={:.2}",
        account.cash, account.market_value, account.equity, account.unrealized_pnl
    );
}

/// 真实行情下走完「建仓 → 部分卖出 → 清仓」，核对现金与已实现盈亏。
#[tokio::test]
#[ignore = "需要网络：打真实行情接口"]
async fn live_round_trip_keeps_cash_and_pnl_consistent() {
    let dir = must(tempfile::tempdir(), "临时目录");
    let svc = live_service(&dir);

    // 200 股 @1.0 只是个占位成交价，成本按它记，后面按真实价卖出算盈亏。
    must(
        svc.place_order(
            "实盘验证",
            CAPITAL,
            &order(Side::Buy, "600519.SS", 200, 100.0),
        )
        .await,
        "建仓",
    );
    assert_close(
        must(svc.account("实盘验证", CAPITAL).await, "账户").cash,
        CAPITAL - 200.0 * 100.0,
        "建仓后现金",
    );

    // 部分卖出 80 股 @150.0
    let partial = must(
        svc.place_order(
            "实盘验证",
            CAPITAL,
            &order(Side::Sell, "600519.SS", 80, 150.0),
        )
        .await,
        "部分卖出",
    );
    assert_close(
        must(partial.trade.realized_pnl.ok_or("应有已实现盈亏"), "盈亏"),
        (150.0 - 100.0) * 80.0,
        "部分卖出已实现盈亏",
    );
    assert_eq!(
        must(partial.position.ok_or("应剩 120 股"), "剩余").shares,
        120
    );

    // 清仓剩余 120 股 @90.0（亏）
    let rest = must(
        svc.place_order(
            "实盘验证",
            CAPITAL,
            &order(Side::Sell, "600519.SS", 120, 90.0),
        )
        .await,
        "清仓",
    );
    assert!(rest.position.is_none(), "清仓后不应再有持仓");

    let account = must(svc.account("实盘验证", CAPITAL).await, "账户");
    // 现金 = 本金 - 200×100 + 80×150 + 120×90 = 本金 + 2800
    assert_close(
        account.cash,
        CAPITAL - 20_000.0 + 12_000.0 + 10_800.0,
        "现金",
    );
    assert_close(account.market_value, 0.0, "清仓后市值应为 0");
    assert_close(account.equity, account.cash, "清仓后权益等于现金");
    // 已实现 = (150-100)×80 + (90-100)×120 = 4000 - 1200 = 2800
    assert_close(account.realized_pnl, 2_800.0, "累计已实现盈亏");
    assert_close(account.total_pnl, 2_800.0, "总盈亏");

    let closed = must(svc.closed_trades("实盘验证", 10), "已平仓历史");
    assert_eq!(closed.len(), 2, "两次卖出应各留一条记录");
    println!(
        "已平仓：{:?}",
        closed
            .iter()
            .map(|c| (c.shares, c.cost_price, c.exit_price, c.realized_pnl))
            .collect::<Vec<_>>()
    );
}

/// 真实行情下，K 线/行情键与持仓键的映射是否正确（含带后缀与裸代码两种写法）。
#[tokio::test]
#[ignore = "需要网络：打真实行情接口"]
async fn live_symbol_formats_resolve_to_same_quote() {
    let dir = must(tempfile::tempdir(), "临时目录");
    let svc = live_service(&dir);

    // 同一只股票，两种写法买两次，应合并为一行（归一化后再合并）
    must(
        svc.place_order(
            "实盘验证",
            CAPITAL,
            &order(Side::Buy, "600519.SS", 100, 1.0),
        )
        .await,
        "带后缀买入",
    );
    must(
        svc.place_order("实盘验证", CAPITAL, &order(Side::Buy, "600519", 100, 1.0))
            .await,
        "裸代码买入",
    );

    let (positions, _) = must(svc.positions("实盘验证").await, "持仓");
    assert_eq!(positions.len(), 1, "两种写法应归一化后合并为一行");
    assert_eq!(positions[0].shares, 200);
    assert_eq!(positions[0].symbol.as_str(), "600519");
    assert!(
        positions[0].last_price.is_some_and(|p| p > 0.0),
        "裸代码也必须能拿到真实行情"
    );
    println!(
        "合并后：{} {} 股，最新价 {:?}",
        positions[0].symbol, positions[0].shares, positions[0].last_price
    );
}
