use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use openatrading_agent::types::DynTool;
use tokio::sync::watch;

use super::tools;

/// 收集所有要对外暴露的行情类 DynTool。
pub fn build_market_tools() -> Vec<Arc<DynTool>> {
    vec![
        tools::get_limit_up(),
        tools::get_limit_down(),
        tools::get_super_stock(),
        tools::get_market_overview(),
        tools::get_yesterday_limit_up(),
        tools::get_dapan_abnormal(),
        tools::get_realtime_quote(),
        tools::get_plates_by_stock(),
        tools::get_market_trend(),
        tools::get_plate_abnormal(),
        tools::get_kline(),
        tools::get_telegraph(),
        tools::get_telegraph_allday(),
        tools::get_field_items(),
        tools::get_lhb_detail(),
        tools::get_lhb_combined(),
        tools::get_lhb_active_yyb(),
    ]
}

/// 持有工具集合，用作 rmcp ServerHandler 状态。
#[derive(Clone)]
pub struct MarketToolsServer {
    tools: Arc<Vec<Arc<DynTool>>>,
}

impl MarketToolsServer {
    pub fn new(tools: Vec<Arc<DynTool>>) -> Self {
        Self {
            tools: Arc::new(tools),
        }
    }

    pub fn tools(&self) -> &[Arc<DynTool>] {
        &self.tools
    }
}

pub async fn serve_stdio(
    _tools: Vec<Arc<DynTool>>,
    _shutdown: watch::Receiver<bool>,
) -> Result<()> {
    anyhow::bail!("not implemented yet")
}

pub async fn serve_http(
    _tools: Vec<Arc<DynTool>>,
    _bind: SocketAddr,
    _shutdown: watch::Receiver<bool>,
) -> Result<()> {
    anyhow::bail!("not implemented yet")
}
