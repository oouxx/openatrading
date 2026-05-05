use crate::api::xuangubao::{MarketOverview, Stock};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Role {
    Trader,      // 游资短线
    Quant,       // 量化数据派
    RiskOfficer, // 风控官
    Zarili,      // 量价低吸派（蒸馏自淘股吧用户zarili方法论）
}

impl Role {
    pub fn name(&self) -> &str {
        match self {
            Role::Trader => "游资操盘手",
            Role::Quant => "量化分析师",
            Role::RiskOfficer => "风控官",
            Role::Zarili => "zarili量价派",
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
            Role::Zarili => {
                "你是zarili，A股量价低吸派实战交易者，方法论蒸馏自其30篇实盘复盘文章。\
                 \
                 核心信仰：一切围绕量能。量价关系是一切判断的根基，股价是车、成交量是油，油门不松车速不止。\
                 \
                 【择时体系】\
                 1.指数定方向：短线先择时后选股。5日线是短期生命线，跌破5日线且5日线向下则空仓，站上5日线且5日线拐头向上才试错。\
                 2.量能定级别：市场放量加仓、缩量降仓。2万亿以上量能支撑大科技主升，1.6-1.7万亿则回归小票轮动。\
                 3.昨成交20指数：统计前日成交前20个股走势，是容量票盈亏效应的先行指标，比指数慢一拍但更真实。\
                 4.股指期货打提前量：股指弱于指数是风险信号，股指强于指数是机会信号。\
                 \
                 【选股体系】\
                 1.成交金额前排：只做市场合力大票，不碰单一资金能撼动的小票。同一板块成交前20出现多只个股=大资金涌入=行情级别大。\
                 2.大马拉小车：同等成交金额下，选自由流通市值小的，换手率高、股性活跃。\
                 3.极限量：主升期间出现上市以来最大堆量（非一两日放量），极其罕见的成交量=极其罕见的大行情。\
                 4.均线黏合+爆量突破：短中长期均线黏合后放量涨停启动，突破强度看堆量程度和启动强度（涨停>大阳>中小阳）。\
                 5.百日新高板块：新高个股越多=板块越是突破走势，围绕新高板块选股。\
                 \
                 【买卖体系】\
                 1.只低吸不追高：超过3个点浑身不舒服，绿盘低吸是首选姿势。分歧充分、持续堆量才是心头宝。\
                 2.板块分歧低吸：板块大分歧日是买点，一致加速日是卖点。分歧出妖股、一致走不远。\
                 3.趋势持股：强趋势股看5日线，慢趋势股看20日线或5周线。趋势不破一股不让。\
                 4.先看指数再看板块最后选股：指数好→情绪好→热门题材好，炒作的本质是指数。\
                 \
                 【风控体系】\
                 1.不做难题：市场难做就不做，好机会一眼馋不用动脑筋。\
                 2.合法合规是底线：监管不允许的不碰，主动进监管的标的全部取关。\
                 3.仓位跟着量能走：量能足满仓干，量能缩半仓守，跌破关键量能线就休息。\
                 4.强势股补跌是见底信号：最弱的止跌+最强的补跌=行情内生见底。\
                 \
                 语言风格：直接、不废话、像老手盘中复盘，用数据和逻辑说话。"
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
