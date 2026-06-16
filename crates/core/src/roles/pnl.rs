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
pub fn compute_pnl_history(logs: &[TradeLogEntry], current_positions_value: f64) -> Vec<PnlPoint> {
    let initial_capital: f64 = INITIAL_CAPITAL;
    if logs.is_empty() {
        return vec![];
    }

    let mut points: Vec<PnlPoint> = vec![];
    let mut cash = initial_capital;
    // Track open positions: symbol -> (shares, avg_cost)
    let mut holdings: std::collections::HashMap<String, (i64, f64)> =
        std::collections::HashMap::new();

    for log in logs {
        match log.action {
            TradeAction::Open => {
                let cost = log.price * log.shares as f64;
                cash -= cost;
                holdings
                    .entry(log.symbol.to_string())
                    .and_modify(|(existing_shares, existing_avg)| {
                        let total_shares = *existing_shares + log.shares;
                        let total_cost =
                            *existing_avg * *existing_shares as f64 + log.price * log.shares as f64;
                        *existing_shares = total_shares;
                        *existing_avg = total_cost / total_shares as f64;
                    })
                    .or_insert((log.shares, log.price));
            },
            TradeAction::Close => {
                let proceeds = log.price * log.shares as f64;
                cash += proceeds;
                if log.shares == 0 {
                    holdings.remove(log.symbol.as_str());
                } else {
                    holdings
                        .entry(log.symbol.to_string())
                        .and_modify(|(existing_shares, _)| {
                            *existing_shares -= log.shares;
                            if *existing_shares <= 0 {
                                *existing_shares = 0;
                            }
                        });
                    if let Some((s, _)) = holdings.get(log.symbol.as_str()) {
                        if *s == 0 {
                            holdings.remove(log.symbol.as_str());
                        }
                    }
                }
            },
            _ => {},
        }

        // Mark-to-market: use last known price for open positions
        let market_value: f64 = holdings
            .iter()
            .map(|(_, (shares, avg_cost))| {
                *avg_cost * *shares as f64 // use cost as proxy when no current price
            })
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

    // Add final point with current positions mark-to-market
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
