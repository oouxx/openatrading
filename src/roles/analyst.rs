use crate::api::xuangubao::{MarketOverview, Stock};

#[derive(Debug, Clone, Copy)]
pub enum Role {
    Trader,      // 游资短线
    Quant,       // 量化数据派
    RiskOfficer, // 风控官
}

impl Role {
    pub fn name(&self) -> &str {
        match self {
            Role::Trader => "游资操盘手",
            Role::Quant => "量化分析师",
            Role::RiskOfficer => "风控官",
        }
    }

    pub fn persona(&self) -> &str {
        match self {
            Role::Trader => {
                "你是一名专注A股短线的游资操盘手，有15年打板经验。\
                 擅长从涨停情绪、连板高度、封板资金研判龙头股。\
                 语言直接，像老手盘中复盘，不废话。"
            }
            Role::Quant => {
                "你是量化分析师，只相信数据和统计规律。\
                 关注涨跌比、换手率分布、行业集中度、情绪指标。\
                 用数字说话，给出客观判断。"
            }
            Role::RiskOfficer => {
                "你是风险控制官，专注识别市场风险和泡沫信号。\
                 偏保守，提醒潜在风险。关注炸板率、跌停数量、尾盘异动。\
                 不鼓励追高，提醒止损。"
            }
        }
    }
}

pub fn build_prompt(role: Role, overview: &MarketOverview, limit_up: &[Stock]) -> String {
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();

    let high_boards: Vec<&Stock> = limit_up.iter().filter(|s| s.limit_up_days >= 2).collect();

    let mut top_boards = high_boards.clone();
    top_boards.sort_by(|a, b| b.limit_up_days.cmp(&a.limit_up_days));
    let top_boards = &top_boards[..top_boards.len().min(5)];

    let mut industry_count: std::collections::HashMap<&str, u32> = Default::default();
    for s in limit_up {
        if let Some(ref reason) = s.surge_reason {
            for plate in &reason.related_plates {
                *industry_count.entry(plate.plate_name.as_str()).or_default() += 1;
            }
        }
    }
    let mut industry_vec: Vec<_> = industry_count.iter().collect();
    industry_vec.sort_by(|a, b| b.1.cmp(a.1));
    let top_industries: Vec<String> = industry_vec
        .iter()
        .take(3)
        .map(|(hy, cnt)| format!("{}({}家)", hy, cnt))
        .collect();

    let total_bomb: u32 = limit_up.iter().map(|s| s.break_limit_up_times).sum();
    let bomb_rate = if !limit_up.is_empty() {
        total_bomb as f64 / (limit_up.len() as f64 + total_bomb as f64) * 100.0
    } else {
        0.0
    };

    let top_boards_str: Vec<String> = top_boards
        .iter()
        .map(|s| {
            format!(
                "  {}({}) {}连板 涨幅:{:.2}% 炸板:{}次 换手:{:.2}%",
                s.stock_chi_name,
                s.symbol,
                s.limit_up_days,
                s.change_percent,
                s.break_limit_up_times,
                s.turnover_ratio
            )
        })
        .collect();

    format!(
        r#"{persona}

## 今日市场数据 {date}

### 大盘情绪
- 上涨家数：{rise} | 下跌家数：{fall}
- 涨停：{up}家 | 跌停：{down}家
- 涨跌比：{ratio:.2}
- 炸板率：{bomb_rate:.1}%

### 连板高度（2板+）
{boards}

### 热门行业 Top3
{industries}

---

请以【{role_name}】身份，用简洁专业的语言分析：
1. 今日市场情绪定性（偏强/震荡/偏弱）
2. 值得关注的机会或风险
3. 明日操作建议

控制在150字以内，直接给结论。"#,
        persona = role.persona(),
        date = date,
        rise = overview.rise_count,
        fall = overview.fall_count,
        up = overview.limit_up_count,
        down = overview.limit_down_count,
        ratio = overview.rise_count as f64 / overview.fall_count.max(1) as f64,
        bomb_rate = bomb_rate,
        boards = if top_boards_str.is_empty() {
            "  暂无连板个股".to_string()
        } else {
            top_boards_str.join("\n")
        },
        industries = top_industries.join(" / "),
        role_name = role.name(),
    )
}
