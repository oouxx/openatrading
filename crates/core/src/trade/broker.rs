use crate::roles::pnl::INITIAL_CAPITAL;
use crate::roles::prompt::TradeInstruction;
const MAX_SINGLE_POSITION_RATIO: f64 = 0.3;
const MAX_TOTAL_POSITION_RATIO: f64 = 0.9;
const MIN_SHARES: i64 = 100;
use crate::api::Client as ApiClient;
use crate::trade::{Position, TradeManager};
use anyhow::Result;

pub async fn execute_buy(
    trade: &dyn TradeManager,
    api: &ApiClient,
    role_name: &str,
    instr: &TradeInstruction,
) -> Result<()> {
    if instr.shares < MIN_SHARES {
        anyhow::bail!(
            "shares {} is less than minimum {}",
            instr.shares,
            MIN_SHARES
        );
    }

    if instr.shares % MIN_SHARES != 0 {
        anyhow::bail!(
            "shares {} is not a multiple of {}",
            instr.shares,
            MIN_SHARES
        );
    }

    let quote = api
        .xuangubao
        .fetch_realtime(&instr.symbol)
        .await
        .map_err(|e| anyhow::anyhow!("fetch realtime for {} failed: {e}", instr.symbol))?;

    let price = quote.last_px;
    if price <= 0.0 {
        anyhow::bail!("invalid price {} for {}", price, instr.symbol);
    }

    let cost = price * instr.shares as f64;

    let current_positions = trade.list_positions(role_name)?;
    let total_position_value: f64 = current_positions
        .iter()
        .map(|p| p.current_price.unwrap_or(p.cost_price) * p.shares as f64)
        .sum();
    let same_symbol_value: f64 = current_positions
        .iter()
        .filter(|p| p.symbol == instr.symbol)
        .map(|p| p.current_price.unwrap_or(p.cost_price) * p.shares as f64)
        .sum();

    if cost + same_symbol_value > INITIAL_CAPITAL * MAX_SINGLE_POSITION_RATIO {
        anyhow::bail!(
            "single stock position {:.0} + new {:.0} exceeds max {:.0} ({}% of capital)",
            same_symbol_value,
            cost,
            INITIAL_CAPITAL * MAX_SINGLE_POSITION_RATIO,
            (MAX_SINGLE_POSITION_RATIO * 100.0) as i32
        );
    }

    if total_position_value + cost > INITIAL_CAPITAL * MAX_TOTAL_POSITION_RATIO {
        anyhow::bail!(
            "total position value {:.0} + new {:.0} exceeds max {:.0} ({}% of capital)",
            total_position_value,
            cost,
            INITIAL_CAPITAL * MAX_TOTAL_POSITION_RATIO,
            (MAX_TOTAL_POSITION_RATIO * 100.0) as i32
        );
    }

    let today = crate::utils::time::beijing_now()
        .format("%Y-%m-%d")
        .to_string();

    let pos = Position {
        id: 0,
        role_name: role_name.to_string(),
        symbol: instr.symbol.clone(),
        stock_name: instr.name.clone(),
        entry_date: today,
        cost_price: price,
        shares: instr.shares,
        current_price: Some(price),
        stop_loss: None,
        target_price: None,
        entry_reason: instr.reason.clone(),
        sector: String::new(),
        position_pct: cost / INITIAL_CAPITAL,
        created_at: String::new(),
        updated_at: String::new(),
    };

    let position_id = trade.add_position(pos)?;
    tracing::info!(
        "[cron] BUY {} {} {}shares @ {:.2} — {} (position_id={})",
        instr.symbol,
        instr.name,
        instr.shares,
        price,
        instr.reason,
        position_id
    );

    Ok(())
}

pub async fn execute_sell(
    trade: &dyn TradeManager,
    api: &ApiClient,
    role_name: &str,
    instr: &TradeInstruction,
) -> Result<()> {
    let positions = trade.list_positions(role_name)?;

    let matching: Vec<&Position> = positions
        .iter()
        .filter(|p| p.symbol == instr.symbol)
        .collect();

    if matching.is_empty() {
        anyhow::bail!(
            "no open position found for {} in role {}",
            instr.symbol,
            role_name
        );
    }

    for pos in matching {
        let sell_price = match api.xuangubao.fetch_realtime(&pos.symbol).await {
            Ok(quote) if quote.last_px > 0.0 => quote.last_px,
            _ => pos.current_price.unwrap_or(pos.cost_price),
        };
        trade.close_position(pos.id, sell_price, &instr.reason)?;
        tracing::info!(
            "[cron] SELL {} {} (id={}) @ {:.2} — {}",
            pos.symbol,
            pos.stock_name,
            pos.id,
            sell_price,
            instr.reason
        );
    }

    Ok(())
}
