use crate::trade::{TradeAction, TradeLogEntry};
use serde::Serialize;

pub const INITIAL_CAPITAL: f64 = 1_000_000.0;

#[derive(Debug, Clone, Serialize)]
pub struct PnlPoint {
    pub date: String,
    pub equity: f64, // total account value
    pub cash: f64,
    pub market_value: f64,
    pub return_pct: f64, // cumulative return %
}

/// Compute P&L history from trade logs. Starts with 1,000,000 initial capital.
///
/// Each Open/Close pair is tracked by `position_id` so same-symbol positions
/// with different cost bases are accounted independently.
///
/// # Mark-to-market limitation
///
/// Intermediate points use entry price as a proxy for market value because
/// historical price snapshots are not stored.  The final point (appended when
/// `current_positions_value > 0`) uses the live `current_price` from the
/// positions table, so it reflects real mark-to-market.
pub fn compute_pnl_history(logs: &[TradeLogEntry], current_positions_value: f64) -> Vec<PnlPoint> {
    let initial_capital: f64 = INITIAL_CAPITAL;
    if logs.is_empty() {
        return vec![];
    }

    let mut points: Vec<PnlPoint> = vec![];
    let mut cash = initial_capital;

    // Track open positions: position_id -> (symbol, shares, entry_price)
    // Each position is tracked independently so same-symbol positions with
    // different cost bases don't get blended.
    let mut holdings: std::collections::HashMap<i64, (String, i64, f64)> =
        std::collections::HashMap::new();

    for log in logs {
        match log.action {
            TradeAction::Open => {
                let cost = log.price * log.shares as f64;
                cash -= cost;
                holdings.insert(
                    log.position_id,
                    (log.symbol.to_string(), log.shares, log.price),
                );
            },
            TradeAction::Close => {
                let proceeds = log.price * log.shares as f64;
                cash += proceeds;
                // Remove the exact position by position_id.
                // Each Close refers to the same position_id as its Open,
                // so we remove the correct cost basis.
                holdings.remove(&log.position_id);
            },
            _ => {},
        }

        // Mark-to-market: use entry price as proxy for open positions.
        // No historical price data is stored, so unrealized P&L only
        // appears in the final point (driven by current_positions_value).
        let market_value: f64 = holdings
            .iter()
            .map(|(_, (_, shares, entry_price))| *entry_price * *shares as f64)
            .sum();

        let equity = cash + market_value;
        let return_pct = (equity - initial_capital) / initial_capital * 100.0;

        points.push(PnlPoint {
            date: log.created_at.chars().take(10).collect::<String>(),
            equity: (equity * 100.0).round() / 100.0,
            cash: (cash * 100.0).round() / 100.0,
            market_value: (market_value * 100.0).round() / 100.0,
            return_pct: (return_pct * 100.0).round() / 100.0,
        });
    }

    // Add final point with current positions mark-to-market.
    // This uses live current_price values, giving accurate unrealized P&L
    // for any positions still open.
    if current_positions_value > 0.0 {
        if let Some(last) = points.last() {
            let equity_with_current = last.cash + current_positions_value;
            let return_pct = (equity_with_current - initial_capital) / initial_capital * 100.0;
            points.push(PnlPoint {
                date: crate::utils::time::beijing_now()
                    .format("%Y-%m-%d")
                    .to_string(),
                equity: (equity_with_current * 100.0).round() / 100.0,
                cash: last.cash,
                market_value: (current_positions_value * 100.0).round() / 100.0,
                return_pct: (return_pct * 100.0).round() / 100.0,
            });
        }
    }

    // Deduplicate by date: keep last entry per day
    let mut daily: Vec<PnlPoint> = vec![];
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for p in points.into_iter().rev() {
        if seen.insert(p.date.clone()) {
            daily.push(p);
        }
    }
    daily.reverse();
    daily
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_log(
        id: i64,
        pos_id: i64,
        action: TradeAction,
        price: f64,
        shares: i64,
        symbol: &str,
        day: u32,
    ) -> TradeLogEntry {
        TradeLogEntry {
            id,
            position_id: pos_id,
            action,
            price,
            shares,
            reason: "test".into(),
            created_at: format!("2026-06-{:02}T10:00:00", day),
            symbol: crate::types::StockCode::new(symbol),
        }
    }

    #[test]
    fn test_empty_logs() {
        let points = compute_pnl_history(&[], 0.0);
        assert!(points.is_empty());
    }

    #[test]
    fn test_single_open_close() {
        // Open day 10, close day 12 — two distinct dates
        let logs = vec![
            make_log(1, 10, TradeAction::Open, 10.0, 100, "600519.SS", 10),
            make_log(2, 10, TradeAction::Close, 12.0, 100, "600519.SS", 12),
        ];
        let points = compute_pnl_history(&logs, 0.0);
        assert_eq!(points.len(), 2);
        // After open: cash = 1_000_000 - 1000 = 999_000, market = 1000, equity = 1_000_000
        assert!((points[0].equity - 1_000_000.0).abs() < 1.0);
        // After close: cash = 1_000_200, market = 0, equity = 1_000_200
        assert!((points[1].equity - 1_000_200.0).abs() < 1.0);
        assert!((points[1].cash - 1_000_200.0).abs() < 1.0);
        assert_eq!(points[1].market_value, 0.0);
    }

    #[test]
    fn test_same_symbol_multiple_lots() {
        // Open lot 1: 100@10 on day 10
        // Open lot 2: 200@14 on day 11
        // Close lot 1: 100@15 on day 12
        let logs = vec![
            make_log(1, 10, TradeAction::Open, 10.0, 100, "600519.SS", 10),
            make_log(2, 20, TradeAction::Open, 14.0, 200, "600519.SS", 11),
            make_log(3, 10, TradeAction::Close, 15.0, 100, "600519.SS", 12),
        ];
        let points = compute_pnl_history(&logs, 0.0);
        assert_eq!(points.len(), 3);
        // After close of lot 1: remaining = lot 2: 200*14 = 2800 market value
        // Cash: 1_000_000 - 1000 - 2800 + 1500 = 997_700
        // Equity: 997_700 + 2800 = 1_000_500
        assert!((points[2].market_value - 2800.0).abs() < 1.0);
        assert!((points[2].equity - 1_000_500.0).abs() < 1.0);
    }

    #[test]
    fn test_different_symbols_independent() {
        let logs = vec![
            make_log(1, 10, TradeAction::Open, 10.0, 100, "600519.SS", 10),
            make_log(2, 20, TradeAction::Open, 20.0, 200, "000001.SZ", 11),
            make_log(3, 10, TradeAction::Close, 12.0, 100, "600519.SS", 12),
        ];
        let points = compute_pnl_history(&logs, 0.0);
        assert_eq!(points.len(), 3);
        // After close: 000001.SZ still open: 200*20 = 4000
        assert!((points[2].market_value - 4000.0).abs() < 1.0);
        // Cash: 1_000_000 - 1000 - 4000 + 1200 = 996_200
        assert!((points[2].cash - 996_200.0).abs() < 1.0);
    }

    #[test]
    fn test_final_point_with_current_value() {
        let logs = vec![make_log(
            1,
            10,
            TradeAction::Open,
            10.0,
            100,
            "600519.SS",
            10,
        )];
        // current market price = 15, so market_value = 1500
        let current_value = 1500.0;
        let points = compute_pnl_history(&logs, current_value);
        assert_eq!(points.len(), 2);
        // First point: equity = 1_000_000 (entry_price proxy)
        assert!((points[0].equity - 1_000_000.0).abs() < 1.0);
        // Final point: equity = cash(999_000) + current_value(1500) = 1_000_500
        assert!((points[1].equity - 1_000_500.0).abs() < 1.0);
        assert!((points[1].market_value - 1500.0).abs() < 1.0);
    }
}
