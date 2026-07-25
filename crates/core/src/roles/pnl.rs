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
#[must_use]
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
    let mut holdings: ahash::AHashMap<i64, (String, i64, f64)> = ahash::AHashMap::new();

    for log in logs {
        match log.action {
            TradeAction::Open => {
                let cost = log.price * log.shares as f64;
                cash -= cost;
                holdings.insert(
                    log.position_id,
                    (log.symbol.to_string(), log.shares, log.price),
                );
            }
            TradeAction::Close => {
                let proceeds = log.price * log.shares as f64;
                cash += proceeds;
                // Remove the exact position by position_id.
                // Each Close refers to the same position_id as its Open,
                // so we remove the correct cost basis.
                holdings.remove(&log.position_id);
            }
            _ => {}
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
