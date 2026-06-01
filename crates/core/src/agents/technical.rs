use crate::api::Client;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechnicalResult {
    pub levels: Vec<LevelStocks>,
    pub hot_sectors: Vec<(String, u32)>,
    pub risk_signals: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelStocks {
    pub days: u32,
    pub stocks: Vec<(String, String, f64, f64)>,
}

pub async fn analyze_technical(client: &Client) -> Result<TechnicalResult> {
    let limit_up = client.xuangubao.fetch_limit_up().await?;
    let overview = client.xuangubao.fetch_market_overview().await?;

    let mut levels_map: std::collections::HashMap<u32, Vec<(String, String, f64, f64)>> =
        Default::default();
    for stock in &limit_up {
        let key = stock.limit_up_days;
        levels_map.entry(key).or_default().push((
            stock.symbol.clone(),
            stock.stock_chi_name.clone(),
            stock.change_percent,
            stock.turnover_ratio,
        ));
    }

    let mut levels: Vec<LevelStocks> = levels_map
        .into_iter()
        .map(|(days, stocks)| LevelStocks { days, stocks })
        .collect();
    levels.sort_by_key(|b| std::cmp::Reverse(b.days));

    let mut sector_count: std::collections::HashMap<&str, u32> = Default::default();
    for stock in &limit_up {
        if let Some(ref reason) = stock.surge_reason {
            for plate in &reason.related_plates {
                *sector_count.entry(plate.plate_name.as_str()).or_default() += 1;
            }
        }
    }
    let mut hot_sectors: Vec<_> = sector_count
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    hot_sectors.sort_by_key(|b| std::cmp::Reverse(b.1));
    let hot_sectors: Vec<_> = hot_sectors.into_iter().take(5).collect();

    let mut risk_signals = Vec::new();

    if overview.bomb_rate > 0.3 {
        risk_signals.push(format!("炸板率高({:.1}%)", overview.bomb_rate * 100.0));
    }

    if overview.limit_down_count > 10 {
        risk_signals.push(format!("跌停家数多({})", overview.limit_down_count));
    }

    let high_turnover: usize = limit_up.iter().filter(|s| s.turnover_ratio > 30.0).count();
    if high_turnover > 5 {
        risk_signals.push(format!("高换手股较多({}只)", high_turnover));
    }

    let sectors_str: Vec<_> = hot_sectors.iter().map(|(s, _)| s.clone()).collect();
    let sectors_str = sectors_str.join(",");
    let risk_str = if risk_signals.is_empty() {
        "无".to_string()
    } else {
        risk_signals.join(",")
    };
    let summary = format!(
        "涨停{}只, 热门板块: {}, 风险信号: {}",
        limit_up.len(),
        sectors_str,
        risk_str
    );

    Ok(TechnicalResult {
        levels,
        hot_sectors,
        risk_signals,
        summary,
    })
}

pub fn build_prompt(result: &TechnicalResult) -> String {
    let levels_str = result
        .levels
        .iter()
        .take(3)
        .map(|l| {
            let stocks = l
                .stocks
                .iter()
                .take(3)
                .map(|(_code, name, pct, turn)| format!("{} {:.1}% 换手{:.1}%", name, pct, turn))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}板: {}", l.days, stocks)
        })
        .collect::<Vec<_>>()
        .join("\n");

    let sectors = result
        .hot_sectors
        .iter()
        .take(5)
        .map(|(name, cnt)| format!("{} ({}只)", name, cnt))
        .collect::<Vec<_>>()
        .join(", ");

    let risks = if result.risk_signals.is_empty() {
        "无明显风险".to_string()
    } else {
        result.risk_signals.join(", ")
    };

    format!(
        r#"你是技术分析专家，负责分析市场技术面数据。

## 今日技术面
### 连板梯队
{}

### 热门板块
{}

### 风险信号
{}

---
请分析：
1. 技术面偏向（强势/震荡/弱势）
2. 板块轮动方向
3. 风险提示

50字以内。"#,
        levels_str, sectors, risks,
    )
}
