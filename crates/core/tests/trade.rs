//! 模拟盘手动交易集成测试。
//!
//! 旧交易模块一个测试都没有，这批覆盖下单 / 平仓 / 估值 / 盈亏的完整链路。
//! 行情用假实现注入，测试不联网。
//!
//! 按 `clippy.toml`，测试里同样不用 `unwrap`/`expect`，改用 [`must`] 辅助函数
//! 把失败信息带出来。

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use openatrading_core::trade::{
    OrderError, OrderRequest, Price, Quote, QuoteSource, Shares, Side, TradeService, TradeStore,
};
use openatrading_core::types::StockCode;

/// 初始资金（元）。
const CAPITAL: f64 = 100_000.0;
/// 浮点比较容差（分）。
const EPS: f64 = 1e-6;

fn must<T, E: std::fmt::Display>(result: Result<T, E>, what: &str) -> T {
    result.unwrap_or_else(|e| panic!("{what} 失败: {e}"))
}

fn assert_close(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() < EPS,
        "{what}: 期望 {expected}，实际 {actual}"
    );
}

/// 可控的假行情源。
struct FakeQuotes {
    prices: HashMap<String, f64>,
    /// `true` 时整体返回 `Err`，用于验证「行情失败 → 估值留空而不是报错」。
    fail: bool,
}

impl FakeQuotes {
    fn new(pairs: &[(&str, f64)]) -> Self {
        Self {
            prices: pairs.iter().map(|(c, p)| ((*c).to_string(), *p)).collect(),
            fail: false,
        }
    }

    fn failing() -> Self {
        Self {
            prices: HashMap::new(),
            fail: true,
        }
    }
}

#[async_trait]
impl QuoteSource for FakeQuotes {
    async fn quotes(&self, codes: &[StockCode]) -> anyhow::Result<HashMap<StockCode, Quote>> {
        if self.fail {
            anyhow::bail!("假行情源按配置失败");
        }
        Ok(codes
            .iter()
            .filter_map(|code| {
                self.prices.get(code.as_str()).map(|price| {
                    (
                        code.clone(),
                        Quote {
                            name: format!("{}公司", code.as_str()),
                            last_price: *price,
                        },
                    )
                })
            })
            .collect())
    }
}

fn service(dir: &tempfile::TempDir, quotes: FakeQuotes) -> TradeService {
    let store = must(TradeStore::open(&dir.path().join("trade.db")), "打开交易库");
    TradeService::new(Arc::new(store), Arc::new(quotes))
}

fn order(side: Side, symbol: &str, shares: i64, price: f64) -> OrderRequest {
    OrderRequest {
        side,
        symbol: symbol.to_string(),
        stock_name: String::new(),
        shares: must(Shares::new(shares), "构造股数"),
        price: must(Price::new(price), "构造价格"),
        reason: String::new(),
    }
}

fn temp() -> tempfile::TempDir {
    must(tempfile::tempdir(), "创建临时目录")
}

// ── Happy path ───────────────────────────────────────────────────────────

#[tokio::test]
async fn buy_creates_position_and_reduces_cash() {
    let dir = temp();
    let svc = service(&dir, FakeQuotes::new(&[("300272", 6.0)]));

    let outcome = must(
        svc.place_order("首板", CAPITAL, &order(Side::Buy, "300272", 400, 5.0))
            .await,
        "买入",
    );
    assert_eq!(outcome.trade.side, Side::Buy);
    assert!(outcome.trade.realized_pnl.is_none(), "买入不应有已实现盈亏");
    let position = must(outcome.position.ok_or("应留下持仓"), "取持仓");
    assert_eq!(position.shares, 400);
    assert_close(position.cost_price, 5.0, "成本价");

    let account = must(svc.account("首板", CAPITAL).await, "账户快照");
    assert_close(account.cash, CAPITAL - 2000.0, "现金");
    assert_close(account.market_value, 2400.0, "市值");
    assert_close(account.unrealized_pnl, 400.0, "浮盈");
    assert!(account.valuation_complete, "行情可用时估值应完整");
}

#[tokio::test]
async fn second_buy_merges_into_one_row_with_weighted_cost() {
    let dir = temp();
    let svc = service(&dir, FakeQuotes::new(&[("300272", 6.0)]));

    must(
        svc.place_order("首板", CAPITAL, &order(Side::Buy, "300272", 400, 5.0))
            .await,
        "第一次买入",
    );
    must(
        svc.place_order("首板", CAPITAL, &order(Side::Buy, "300272", 400, 7.0))
            .await,
        "第二次买入",
    );

    let (positions, _) = must(svc.positions("首板").await, "持仓列表");
    assert_eq!(positions.len(), 1, "同标的应合并为一行，而不是两行");
    assert_eq!(positions[0].shares, 800);
    // (5×400 + 7×400) / 800 = 6.0
    assert_close(positions[0].cost_price, 6.0, "加权成本");
}

#[tokio::test]
async fn partial_sell_keeps_cost_and_realizes_pnl() {
    let dir = temp();
    let svc = service(&dir, FakeQuotes::new(&[("600519", 12.0)]));

    must(
        svc.place_order("首板", CAPITAL, &order(Side::Buy, "600519", 1000, 10.0))
            .await,
        "建仓",
    );
    let outcome = must(
        svc.place_order("首板", CAPITAL, &order(Side::Sell, "600519", 400, 12.0))
            .await,
        "部分卖出",
    );

    // (12 - 10) × 400 = 800
    assert_close(
        must(
            outcome.trade.realized_pnl.ok_or("卖出应有已实现盈亏"),
            "盈亏",
        ),
        800.0,
        "已实现盈亏",
    );
    let position = must(outcome.position.ok_or("应仍持有剩余仓位"), "剩余持仓");
    assert_eq!(position.shares, 600, "卖出应只扣减指定股数");
    assert_close(position.cost_price, 10.0, "部分卖出不应改变成本价");

    let account = must(svc.account("首板", CAPITAL).await, "账户快照");
    // 现金 = 100000 - 10000 + 4800
    assert_close(account.cash, CAPITAL - 10000.0 + 4800.0, "现金");
    assert_close(account.realized_pnl, 800.0, "累计已实现盈亏");
}

#[tokio::test]
async fn full_sell_removes_position_and_lands_in_history() {
    let dir = temp();
    let svc = service(&dir, FakeQuotes::new(&[("600519", 11.0)]));

    must(
        svc.place_order("首板", CAPITAL, &order(Side::Buy, "600519", 500, 10.0))
            .await,
        "建仓",
    );
    let outcome = must(
        svc.place_order("首板", CAPITAL, &order(Side::Sell, "600519", 500, 11.0))
            .await,
        "清仓",
    );
    assert!(outcome.position.is_none(), "清仓后不应再有持仓");

    let (positions, _) = must(svc.positions("首板").await, "持仓列表");
    assert!(positions.is_empty(), "清仓后持仓表应为空");

    let closed = must(svc.closed_trades("首板", 100), "已平仓历史");
    assert_eq!(closed.len(), 1);
    assert_close(closed[0].realized_pnl, 500.0, "已实现盈亏");
    assert_close(closed[0].exit_price, 11.0, "卖出价");
    assert_close(closed[0].cost_price, 10.0, "成本价");
}

// ── Edge case ────────────────────────────────────────────────────────────

#[tokio::test]
async fn buy_spending_exactly_all_cash_leaves_zero() {
    let dir = temp();
    let svc = service(&dir, FakeQuotes::new(&[("300272", 10.0)]));

    // 全部资金买在 10.0：100000 / 10 = 10000 股（整手）
    must(
        svc.place_order("首板", CAPITAL, &order(Side::Buy, "300272", 10000, 10.0))
            .await,
        "用满现金买入",
    );
    let account = must(svc.account("首板", CAPITAL).await, "账户快照");
    assert_close(account.cash, 0.0, "现金应为 0");
}

#[tokio::test]
async fn odd_lot_sell_is_allowed() {
    let dir = temp();
    let svc = service(&dir, FakeQuotes::new(&[("300272", 10.0)]));

    must(
        svc.place_order("首板", CAPITAL, &order(Side::Buy, "300272", 200, 10.0))
            .await,
        "建仓",
    );
    // 卖出 50 股（零股）——配股、送股产生零股后清仓是真实场景，必须允许。
    let outcome = must(
        svc.place_order("首板", CAPITAL, &order(Side::Sell, "300272", 50, 10.0))
            .await,
        "卖出零股",
    );
    assert_eq!(
        must(outcome.position.ok_or("应剩 150 股"), "剩余").shares,
        150
    );
}

#[tokio::test]
async fn quote_failure_leaves_valuation_empty_without_failing() {
    let dir = temp();
    // 先用可用行情建仓，再换成失败的行情源读账户。
    let store = Arc::new(must(
        TradeStore::open(&dir.path().join("trade.db")),
        "打开交易库",
    ));
    let ok_svc = TradeService::new(store.clone(), Arc::new(FakeQuotes::new(&[("300272", 6.0)])));
    must(
        ok_svc
            .place_order("首板", CAPITAL, &order(Side::Buy, "300272", 400, 5.0))
            .await,
        "建仓",
    );

    let broken = TradeService::new(store, Arc::new(FakeQuotes::failing()));
    let (positions, complete) = must(broken.positions("首板").await, "持仓列表");
    assert_eq!(positions.len(), 1, "行情挂了也要返回持仓");
    assert!(positions[0].last_price.is_none(), "拿不到行情应为 null");
    assert!(positions[0].market_value.is_none(), "拿不到行情应为 null");
    assert!(!complete, "应标记估值不完整");

    let account = must(broken.account("首板", CAPITAL).await, "账户快照");
    assert!(
        !account.valuation_complete,
        "账户也应标记估值不完整，供 UI 提示"
    );
    assert_close(account.cash, CAPITAL - 2000.0, "现金不受行情影响");
}

#[test]
fn negative_zero_is_normalized() {
    use openatrading_core::trade::round_money;
    // Rust 的 `Sum<f64>` 以 `-0.0` 为单位元，空求和的账户字段会带上它。
    assert!(
        round_money(-0.0).is_sign_positive(),
        "round_money 应把 -0.0 归一为 0.0，避免 JSON 出现 -0.0"
    );
    assert!(
        round_money(-1.23) < 0.0,
        "真负数不应被翻转成正数，实际 {}",
        round_money(-1.23)
    );
    assert!(
        round_money(-0.001).is_sign_positive(),
        "舍入到分之后为零的值应归一为 +0.0"
    );
    assert_close(round_money(1.234), 1.23, "四舍五入到分");
    // 已知限制：恰好落在半分上的值受 f64 表示影响（1.005 实际略小于 1.005，
    // 于是舍到 1.00）。要根治得换十进制类型，见计划里暂缓引入 rust_decimal 的说明。
    assert_close(round_money(1.005), 1.0, "半分值按 f64 实际表示舍入");
}

#[tokio::test]
async fn empty_account_has_no_negative_zero() {
    let dir = temp();
    let svc = service(&dir, FakeQuotes::new(&[]));
    let account = must(svc.account("空仓", CAPITAL).await, "账户快照");
    assert!(
        account.market_value.is_sign_positive(),
        "无持仓时市值应为 0.0 而非 -0.0，实际 {}",
        account.market_value
    );
    assert!(account.unrealized_pnl.is_sign_positive());
}

#[tokio::test]
async fn empty_account_equals_initial_capital() {
    let dir = temp();
    let svc = service(&dir, FakeQuotes::new(&[]));

    let account = must(svc.account("空仓", CAPITAL).await, "账户快照");
    assert_close(account.cash, CAPITAL, "现金");
    assert_close(account.equity, CAPITAL, "权益");
    assert_close(account.total_pnl, 0.0, "总盈亏");
    assert!(account.valuation_complete, "无持仓时估值视为完整");
    assert!(must(svc.positions("空仓").await, "持仓").0.is_empty());
}

#[tokio::test]
async fn cash_stays_consistent_across_many_trades() {
    let dir = temp();
    let svc = service(&dir, FakeQuotes::new(&[("300272", 11.0)]));

    for _ in 0..7 {
        must(
            svc.place_order("首板", CAPITAL, &order(Side::Buy, "300272", 100, 10.0))
                .await,
            "买入",
        );
    }
    must(
        svc.place_order("首板", CAPITAL, &order(Side::Sell, "300272", 300, 11.0))
            .await,
        "卖出",
    );

    // 现金 = 100000 - 7×1000 + 3300
    let account = must(svc.account("首板", CAPITAL).await, "账户快照");
    assert_close(account.cash, CAPITAL - 7000.0 + 3300.0, "现金");
    let (positions, _) = must(svc.positions("首板").await, "持仓");
    assert_eq!(positions[0].shares, 400);
}

// ── Failure case ─────────────────────────────────────────────────────────

#[tokio::test]
async fn insufficient_cash_is_rejected_without_side_effects() {
    let dir = temp();
    let svc = service(&dir, FakeQuotes::new(&[("300272", 10.0)]));

    let err = svc
        .place_order("首板", CAPITAL, &order(Side::Buy, "300272", 20000, 10.0))
        .await
        .err()
        .unwrap_or_else(|| panic!("超出现金的买入应被拒绝"));
    assert!(
        matches!(err, OrderError::InsufficientCash { .. }),
        "应返回 InsufficientCash，实际 {err:?}"
    );
    assert!(err.is_client_error(), "应映射为 HTTP 400");

    // 不落库、无半截状态
    let (positions, _) = must(svc.positions("首板").await, "持仓");
    assert!(positions.is_empty(), "被拒的下单不应留下持仓");
    let account = must(svc.account("首板", CAPITAL).await, "账户");
    assert_close(account.cash, CAPITAL, "被拒的下单不应动现金");
    assert!(
        must(svc.trades("首板", 100), "流水").is_empty(),
        "不应留下流水"
    );
}

#[tokio::test]
async fn selling_more_than_held_is_rejected() {
    let dir = temp();
    let svc = service(&dir, FakeQuotes::new(&[("300272", 10.0)]));
    must(
        svc.place_order("首板", CAPITAL, &order(Side::Buy, "300272", 100, 10.0))
            .await,
        "建仓",
    );

    let err = svc
        .place_order("首板", CAPITAL, &order(Side::Sell, "300272", 200, 10.0))
        .await
        .err()
        .unwrap_or_else(|| panic!("超额卖出应被拒绝"));
    assert!(
        matches!(
            err,
            OrderError::InsufficientShares {
                held: 100,
                requested: 200
            }
        ),
        "应报持有 100 试图卖出 200，实际 {err:?}"
    );

    let (positions, _) = must(svc.positions("首板").await, "持仓");
    assert_eq!(positions[0].shares, 100, "被拒的卖出不应改动持仓");
}

#[tokio::test]
async fn selling_without_position_is_rejected() {
    let dir = temp();
    let svc = service(&dir, FakeQuotes::new(&[("300272", 10.0)]));

    let err = svc
        .place_order("首板", CAPITAL, &order(Side::Sell, "300272", 100, 10.0))
        .await
        .err()
        .unwrap_or_else(|| panic!("无持仓卖出应被拒绝"));
    assert!(
        matches!(err, OrderError::InsufficientShares { held: 0, .. }),
        "应报持有 0，实际 {err:?}"
    );
}

#[tokio::test]
async fn buy_must_be_board_lot() {
    let dir = temp();
    let svc = service(&dir, FakeQuotes::new(&[("300272", 10.0)]));

    let err = svc
        .place_order("首板", CAPITAL, &order(Side::Buy, "300272", 150, 10.0))
        .await
        .err()
        .unwrap_or_else(|| panic!("非整手买入应被拒绝"));
    assert!(matches!(err, OrderError::InvalidShares(_)), "实际 {err:?}");
    assert!(err.is_client_error());
}

#[test]
fn non_positive_shares_and_price_are_rejected_at_construction() {
    assert!(Shares::new(0).is_err(), "0 股应被拒绝");
    assert!(Shares::new(-100).is_err(), "负数股数应被拒绝");
    assert!(Price::new(0.0).is_err(), "0 价应被拒绝");
    assert!(Price::new(-1.0).is_err(), "负价应被拒绝");
    assert!(Price::new(f64::NAN).is_err(), "NaN 应被拒绝");
    assert!(Price::new(f64::INFINITY).is_err(), "无穷大应被拒绝");
}

#[tokio::test]
async fn malformed_symbol_is_rejected() {
    let dir = temp();
    let svc = service(&dir, FakeQuotes::new(&[]));

    // 只断言归一化**确实**拒绝的输入：完全不含数字，或数字超过 6 位。
    // 其余（短数字补零、非数字字符被剥离、未识别后缀被忽略）都是刻意的宽容行为，
    // 见下一个测试。
    for bad in ["", "   ", "abc", "1234567"] {
        let err = svc
            .place_order("首板", CAPITAL, &order(Side::Buy, bad, 100, 10.0))
            .await
            .err()
            .unwrap_or_else(|| panic!("非法代码 {bad:?} 应被拒绝"));
        assert!(
            matches!(err, OrderError::InvalidSymbol(_)),
            "代码 {bad:?} 应报 InvalidSymbol，实际 {err:?}"
        );
    }
}

#[tokio::test]
async fn symbol_normalization_is_lenient_by_design() {
    // `to_yahoo_symbol` / `normalize_code`（crates/api/src/utils.rs）是全项目行情
    // 工具共用的归一化，行为刻意宽松。这里把两处宽松点固定下来，避免以后被当成
    // bug 反复讨论——若哪天要收紧，改的应该是那个共用函数，并同步所有调用方。
    let dir = temp();
    let svc = service(&dir, FakeQuotes::new(&[]));

    // 1) 不足 6 位的数字左补零
    let padded = must(
        svc.place_order("首板", CAPITAL, &order(Side::Buy, "12", 100, 10.0))
            .await,
        "短代码买入",
    );
    assert_eq!(padded.trade.symbol.as_str(), "000012", "应补零为 6 位");

    // 2) 未识别的后缀被忽略，退回按裸代码解析
    let ignored = must(
        svc.place_order("首板", CAPITAL, &order(Side::Buy, "600519.XX", 100, 10.0))
            .await,
        "未知后缀买入",
    );
    assert_eq!(
        ignored.trade.symbol.as_str(),
        "600519",
        "未识别后缀应被忽略而不是报错"
    );
}

#[tokio::test]
async fn first_order_auto_creates_account() {
    let dir = temp();
    let svc = service(&dir, FakeQuotes::new(&[("300272", 10.0)]));

    let outcome = must(
        svc.place_order("全新角色", CAPITAL, &order(Side::Buy, "300272", 100, 10.0))
            .await,
        "首次下单应自动建账而不是报错",
    );
    assert_eq!(outcome.trade.role_name, "全新角色");
}

#[tokio::test]
async fn symbol_and_name_are_normalized_and_backfilled() {
    let dir = temp();
    let svc = service(&dir, FakeQuotes::new(&[("600519", 10.0)]));

    // 带后缀的 Yahoo 写法应归一化成裸代码；未填名称时由行情补全。
    let outcome = must(
        svc.place_order("首板", CAPITAL, &order(Side::Buy, "600519.SS", 100, 10.0))
            .await,
        "买入",
    );
    assert_eq!(outcome.trade.symbol.as_str(), "600519", "应存为裸代码");
    assert_eq!(outcome.trade.stock_name, "600519公司", "应由行情补全名称");
}

// ── 迁移 ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn open_migrates_legacy_schema_holding_data() {
    // 复现真实旧库的形状：`trade_logs` 有指向 `positions(id)` 的外键，且两表都有数据。
    // 迁移必须先删子表再删父表——先删父表会撞 FOREIGN KEY constraint failed，
    // 这个坑只有拿带数据的旧库才能试出来（全新库试不出来）。
    let dir = temp();
    let db = dir.path().join("legacy.db");
    {
        let conn = must(rusqlite::Connection::open(&db), "建旧库");
        must(
            conn.execute_batch(
                "CREATE TABLE positions (
                     id INTEGER PRIMARY KEY AUTOINCREMENT,
                     role_name TEXT NOT NULL,
                     symbol TEXT NOT NULL,
                     stock_name TEXT NOT NULL,
                     shares INTEGER NOT NULL,
                     cost_price REAL NOT NULL);
                 CREATE TABLE trade_logs (
                     id INTEGER PRIMARY KEY AUTOINCREMENT,
                     position_id INTEGER NOT NULL REFERENCES positions(id),
                     action TEXT NOT NULL,
                     price REAL NOT NULL,
                     shares INTEGER NOT NULL);
                 INSERT INTO positions (role_name, symbol, stock_name, shares, cost_price)
                     VALUES ('首板', '300272', '光纤技术公司', 400, 5.32);
                 INSERT INTO trade_logs (position_id, action, price, shares)
                     VALUES (1, 'Open', 5.32, 400);",
            ),
            "建旧表并灌数据",
        );
    }

    let store = must(TradeStore::open(&db), "打开并迁移旧库");
    assert!(
        must(store.list_positions("首板"), "读持仓").is_empty(),
        "旧交易数据按决策清空，新表应为空"
    );
    assert!(
        must(store.cash_and_realized("首板", CAPITAL), "读账户").0 == CAPITAL,
        "迁移后账户应等于初始资金"
    );

    // 迁移后可以正常下单
    let svc = TradeService::new(
        Arc::new(store),
        Arc::new(FakeQuotes::new(&[("300272", 6.0)])),
    );
    let outcome = must(
        svc.place_order("首板", CAPITAL, &order(Side::Buy, "300272", 100, 5.0))
            .await,
        "迁移后下单",
    );
    assert_eq!(outcome.trade.shares, 100);
}

#[tokio::test]
async fn empty_name_does_not_erase_existing_one() {
    let dir = temp();
    let svc = service(&dir, FakeQuotes::new(&[("300272", 6.0)]));
    let mut buy = order(Side::Buy, "300272", 400, 5.0);
    buy.stock_name = "光纤技术公司".to_string();
    must(svc.place_order("首板", CAPITAL, &buy).await, "买入");

    // 不带名称的卖出：服务层会尝试用行情补全，但持仓名称以**非空入参**为准，
    // 空值不会把已有名称清掉。
    let mut sell = order(Side::Sell, "300272", 100, 6.0);
    sell.stock_name = String::new();
    must(svc.place_order("首板", CAPITAL, &sell).await, "卖出");
    let (positions, _) = must(svc.positions("首板").await, "持仓");
    assert!(!positions[0].stock_name.is_empty(), "名称不应被清空");

    // 明确传名称时以传入值为准
    let mut rename = order(Side::Buy, "300272", 100, 6.0);
    rename.stock_name = "新名称".to_string();
    must(svc.place_order("首板", CAPITAL, &rename).await, "改名买入");
    let (positions, _) = must(svc.positions("首板").await, "持仓");
    assert_eq!(positions[0].stock_name, "新名称");
}

#[tokio::test]
async fn concurrent_orders_never_overdraw_cash() {
    let dir = temp();
    // 每单 1000 股 × 10 元 = 10000 元，10 单并发共需 100000 元，恰好等于本金；
    // 再多一单必然被拒。这里跑 12 单，断言现金不为负、且被拒的单数 > 0。
    let svc = Arc::new(service(&dir, FakeQuotes::new(&[("300272", 10.0)])));

    let mut tasks = Vec::new();
    for _ in 0..12 {
        let svc = svc.clone();
        tasks.push(tokio::spawn(async move {
            svc.place_order("首板", CAPITAL, &order(Side::Buy, "300272", 1000, 10.0))
                .await
                .is_ok()
        }));
    }
    let mut ok = 0;
    for task in tasks {
        if must(task.await, "join") {
            ok += 1;
        }
    }

    let account = must(svc.account("首板", CAPITAL).await, "账户快照");
    assert!(
        account.cash >= -EPS,
        "现金不允许为负，实际 {}",
        account.cash
    );
    assert!(ok < 12, "并发超出本金时应有订单被拒，实际全部成功");

    let trades = must(svc.trades("首板", 100), "流水");
    assert_eq!(
        trades.len() as i64,
        ok as i64,
        "成功的单数应与落库流水数一致（失败的不得留痕）"
    );
}

// ── 区间收益分析（pnl_calendar） ──────────────────────────────────────────

/// 可控的假日线源。
struct FakeDailyCloses {
    map: openatrading_core::trade::history::DailyCloseMap,
}

#[async_trait]
impl openatrading_core::trade::DailyCloseSource for FakeDailyCloses {
    async fn daily_closes(
        &self,
        _codes: &[String],
        _trading_days: u32,
    ) -> anyhow::Result<openatrading_core::trade::history::DailyCloseMap> {
        Ok(self.map.clone())
    }
}

/// 建一个「有成交 + 有日线」的服务：10 元买入 1000 股，随后两天收盘 12 / 9。
fn calendar_service(dir: &tempfile::TempDir) -> TradeService {
    use std::collections::BTreeMap;

    let store = must(TradeStore::open(&dir.path().join("trade.db")), "打开交易库");
    let mut series = BTreeMap::new();
    let day = openatrading_core::utils::time::beijing_now()
        .format("%Y-%m-%d")
        .to_string();
    series.insert(day.clone(), 12.0);
    let mut map = openatrading_core::trade::history::DailyCloseMap::new();
    map.insert("600000.SS".to_string(), series);
    map.insert("000001.SS".to_string(), BTreeMap::from([(day, 3000.0)]));

    TradeService::with_daily_closes(
        Arc::new(store),
        Arc::new(FakeQuotes::new(&[("600000", 12.0)])),
        Arc::new(FakeDailyCloses { map }),
    )
}

/// 区间分析用当日收盘价给持仓估值：买入价 10、当日收盘 12，
/// 权益应为「现金 90000 + 1000×12 = 102000」，收益率 +2%。
#[tokio::test]
async fn pnl_calendar_uses_daily_close() {
    use openatrading_core::trade::{Granularity, PnlCalendarRequest};

    let dir = temp();
    let svc = calendar_service(&dir);
    must(
        svc.place_order("r", CAPITAL, &order(Side::Buy, "600000", 1000, 10.0))
            .await,
        "下单",
    );

    let today = openatrading_core::utils::time::beijing_now()
        .format("%Y-%m-%d")
        .to_string();
    let cal = must(
        svc.pnl_calendar(&PnlCalendarRequest {
            role: "r".into(),
            initial_capital: CAPITAL,
            from: today.clone(),
            to: today,
            granularity: Granularity::Day,
        })
        .await,
        "区间分析",
    );

    assert_eq!(cal.points.len(), 1);
    assert_close(cal.points[0].equity, 102_000.0, "当日权益");
    assert_close(cal.summary.total_pnl, 2_000.0, "区间盈亏");
    assert_close(cal.summary.total_pct, 2.0, "区间收益率");
    // 基准确认：上证同日收盘 3000，区间内无量价变化，涨跌幅为 0
    let sh = must(
        cal.benchmarks
            .iter()
            .find(|b| b.name == "上证")
            .ok_or("缺上证基准"),
        "找上证基准",
    );
    assert_close(sh.pct.unwrap_or(f64::NAN), 0.0, "上证涨跌幅");
}
