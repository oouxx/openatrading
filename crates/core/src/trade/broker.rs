use crate::api::Client as ApiClient;
use crate::roles::pnl::INITIAL_CAPITAL;
use crate::roles::prompt::TradeInstruction;
use crate::trade::paper::PaperBroker;
use crate::trade::{Position, TradeManager};
use anyhow::Result;

pub async fn execute_buy(
    trade: &dyn TradeManager,
    api: &ApiClient,
    role_name: &str,
    instr: &TradeInstruction,
) -> Result<()> {
    let broker = PaperBroker::default();

    let quote = api
        .xuangubao
        .fetch_realtime(&instr.symbol)
        .await
        .map_err(|e| anyhow::anyhow!("fetch realtime for {} failed: {e}", instr.symbol))?;

    let last_price = quote.last_px;
    if last_price <= 0.0 {
        anyhow::bail!("invalid price {} for {}", last_price, instr.symbol);
    }

    let exec_price = broker.buy_price(last_price);
    let shares = PaperBroker::round_shares(instr.shares);

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

    broker
        .validate_buy(shares, exec_price, same_symbol_value, total_position_value)
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let (_, commission) = broker.buy_cost(exec_price, shares);
    let total_deduct = exec_price * shares as f64 + commission;

    let today = crate::utils::time::beijing_now()
        .format("%Y-%m-%d")
        .to_string();

    let pos = Position {
        id: 0,
        role_name: role_name.to_string(),
        symbol: instr.symbol.clone(),
        stock_name: instr.name.clone(),
        entry_date: today,
        cost_price: exec_price,
        shares,
        current_price: Some(exec_price),
        stop_loss: None,
        target_price: None,
        entry_reason: instr.reason.clone(),
        sector: String::new(),
        position_pct: total_deduct / INITIAL_CAPITAL,
        created_at: String::new(),
        updated_at: String::new(),
    };

    let position_id = trade.add_position(pos)?;
    tracing::info!(
        "[cron] BUY {} {} {}shares @ {:.2} (last={:.2}) — {} (position_id={})",
        instr.symbol,
        instr.name,
        shares,
        exec_price,
        last_price,
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
    let broker = PaperBroker::default();
    let positions = trade.list_positions(role_name)?;

    let matching: Vec<&Position> = positions
        .iter()
        .filter(|p| p.symbol == instr.symbol)
        .collect();

    PaperBroker::validate_sell(&instr.symbol, matching.len() as i64)
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    for pos in matching {
        let last_price = match api.xuangubao.fetch_realtime(&pos.symbol).await {
            Ok(quote) if quote.last_px > 0.0 => quote.last_px,
            _ => pos.current_price.unwrap_or(pos.cost_price),
        };
        let exec_price = broker.sell_price(last_price);
        trade.close_position(pos.id, exec_price, &instr.reason)?;
        tracing::info!(
            "[cron] SELL {} {} (id={}) @ {:.2} (last={:.2}) — {}",
            pos.symbol,
            pos.stock_name,
            pos.id,
            exec_price,
            last_price,
            instr.reason
        );
    }

    Ok(())
}
