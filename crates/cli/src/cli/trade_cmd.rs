use anyhow::Result;
use std::path::PathBuf;

use crate::cli::config::AppConfig;
use trending_agent_core::trade::sqlite::SqliteTradeManager;
use trending_agent_core::trade::{Position, TradeManager};

pub fn cmd_trade_add(
    project_root: &PathBuf,
    role: &str,
    symbol: &str,
    price: f64,
    shares: i64,
) -> Result<()> {
    let app_config = AppConfig::load(project_root)?;
    let db_path = project_root
        .join(".atrading")
        .join(&app_config.service.db_path);
    let tm = SqliteTradeManager::open(&db_path)?;
    let now = chrono::Local::now().format("%Y-%m-%d").to_string();
    let pos = Position {
        id: 0,
        role_name: role.to_string(),
        symbol: symbol.to_string(),
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
    let id = tm.add_position(pos)?;
    println!("已添加持仓 #{id}: {symbol} @ {price} x {shares}股");
    Ok(())
}

pub fn cmd_trade_list(project_root: &PathBuf, role: Option<&str>) -> Result<()> {
    let app_config = AppConfig::load(project_root)?;
    let db_path = project_root
        .join(".atrading")
        .join(&app_config.service.db_path);
    let tm = SqliteTradeManager::open(&db_path)?;
    let role_name = role.unwrap_or("zarili");
    let text = tm.format_positions(role_name)?;
    println!("{text}");
    Ok(())
}

pub fn cmd_trade_close(project_root: &PathBuf, id: i64, price: f64) -> Result<()> {
    let app_config = AppConfig::load(project_root)?;
    let db_path = project_root
        .join(".atrading")
        .join(&app_config.service.db_path);
    let tm = SqliteTradeManager::open(&db_path)?;
    tm.close_position(id, price, "手动平仓")?;
    println!("已平仓 #{id} @ {price}");
    Ok(())
}
