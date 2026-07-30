use anyhow::Result;

use crate::cli::config::AppConfig;
use openatrading_core::trade::sqlite::TradeManager;
use openatrading_core::trade::Position;
use openatrading_core::types::StockCode;

pub fn cmd_trade_add(role: &str, symbol: &str, price: f64, shares: i64) -> Result<()> {
    let app_config = AppConfig::load()?;
    let db_path = openatrading_core::atrading_home().join(&app_config.service.db_path);
    let tm = TradeManager::open(&db_path)?;
    let now = openatrading_core::utils::time::beijing_now()
        .format("%Y-%m-%d")
        .to_string();
    let pos = Position {
        id: 0,
        role_name: role.to_string(),
        symbol: StockCode::new(symbol),
        stock_name: String::new(),
        entry_date: now,
        cost_price: price,
        shares,
        current_price: None,
        stop_loss: None,
        target_price: None,
        entry_reason: String::new(),
        sector: String::new(),
        position_pct: 0.0,
        created_at: String::new(),
        updated_at: String::new(),
    };
    let id = tm.add_position(&pos)?;
    println!("已添加持仓 #{id}: {symbol} @ {price} x {shares}股");
    Ok(())
}

pub fn cmd_trade_list(role: Option<&str>) -> Result<()> {
    let app_config = AppConfig::load()?;
    let db_path = openatrading_core::atrading_home().join(&app_config.service.db_path);
    let tm = TradeManager::open(&db_path)?;
    if let Some(role_name) = role {
        let text = tm.format_positions(role_name)?;
        println!("{text}");
    } else {
        let positions = tm.list_all_positions()?;
        if positions.is_empty() {
            println!("暂无持仓");
        } else {
            for p in &positions {
                let pnl = match p.current_price {
                    Some(cp) if cp > 0.0 => {
                        format!("{:+.1}%", (cp - p.cost_price) / p.cost_price * 100.0)
                    }
                    _ => "N/A".to_string(),
                };
                println!(
                    "- #{} [{}] {} {} | 成本{} | 现价{} | {} | {}股",
                    p.id,
                    p.role_name,
                    p.symbol,
                    p.stock_name,
                    p.cost_price,
                    p.current_price
                        .map_or_else(|| "N/A".to_string(), |v| v.to_string()),
                    pnl,
                    p.shares,
                );
            }
        }
    }
    Ok(())
}

pub fn cmd_trade_close(id: i64, price: f64) -> Result<()> {
    let app_config = AppConfig::load()?;
    let db_path = openatrading_core::atrading_home().join(&app_config.service.db_path);
    let tm = TradeManager::open(&db_path)?;
    tm.close_position(id, price, "手动平仓")?;
    println!("已平仓 #{id} @ {price}");
    Ok(())
}
