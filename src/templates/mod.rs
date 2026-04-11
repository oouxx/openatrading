use std::collections::BTreeMap;

use askama::Template;

use crate::api::jiuyangongshe::{FieldPlate, TimelineDay};
use crate::api::xuangubao::{MarketOverview, PlateAbnormalEvent, Stock};
use crate::report::report::RoleAnalysis;

#[derive(Debug)]
pub struct LimitUpStock {
    pub symbol: String,
    pub name: String,
    pub days: u32,
    pub change_percent: f64,
    pub turnover: f64,
    pub reason: String,
    pub bomb: u32,
    pub capital: f64,
}

#[derive(Debug)]
pub struct PlateAbnormalEventData {
    pub plate_name: String,
    pub pcp: String,
    pub related_stocks: Vec<RelatedStockData>,
}

#[derive(Debug)]
pub struct RelatedStockData {
    pub name: String,
    pub pcp: String,
}

#[derive(Debug)]
pub struct FieldPlateData {
    pub name: String,
    pub count: usize,
    pub reason: String,
    pub stocks: Vec<FieldStockData>,
}

#[derive(Debug)]
pub struct FieldStockData {
    pub time: String,
    pub code: String,
    pub name: String,
    pub num: String,
    pub desc: String,
}

#[derive(Debug)]
pub struct TimelineItemData {
    pub grade_emoji: String,
    pub title: String,
    pub date: String,
}

#[derive(Debug)]
pub struct TimelineDetailItem {
    pub grade_emoji: String,
    pub title: String,
    pub tags: String,
    pub content: String,
}

#[derive(Debug)]
pub struct AnalysisData {
    pub role_name: String,
    pub content: String,
    pub elapsed_ms: u64,
}

// ============== Markdown Templates ==============

#[derive(Template)]
#[template(path = "markdown/overview.md")]
pub struct OverviewMdTemplate {
    pub rise_count: u32,
    pub fall_count: u32,
    pub limit_up_count: u32,
    pub limit_down_count: u32,
    pub bomb_rate: f64,
    pub rise_fall_ratio: f64,
}

impl OverviewMdTemplate {
    pub fn render(overview: &MarketOverview) -> Self {
        let rise_fall_ratio = overview.rise_count as f64 / overview.fall_count.max(1) as f64;
        let bomb_rate = (overview.bomb_rate * 10.0).round() / 10.0;
        Self {
            rise_count: overview.rise_count,
            fall_count: overview.fall_count,
            limit_up_count: overview.limit_up_count,
            limit_down_count: overview.limit_down_count,
            bomb_rate,
            rise_fall_ratio,
        }
    }
}

#[derive(Template)]
#[template(path = "markdown/board_distribution.md")]
pub struct BoardDistributionMdTemplate {
    pub distribution: Vec<(u32, u32)>,
}

impl BoardDistributionMdTemplate {
    pub fn render(limit_up: &[Stock]) -> Self {
        let mut map: BTreeMap<u32, u32> = BTreeMap::new();
        for s in limit_up {
            *map.entry(s.limit_up_days).or_default() += 1;
        }
        let mut distribution: Vec<(u32, u32)> = map.into_iter().collect();
        distribution.sort_by(|a, b| b.0.cmp(&a.0));
        Self { distribution }
    }
}

#[derive(Template)]
#[template(path = "markdown/limit_up_table.md")]
pub struct LimitUpTableMdTemplate {
    pub stocks: Vec<LimitUpStock>,
}

impl LimitUpTableMdTemplate {
    pub fn render(limit_up: &[Stock]) -> Option<Self> {
        if limit_up.is_empty() {
            return None;
        }
        let mut stocks = limit_up
            .iter()
            .map(|s| {
                let reason = s
                    .surge_reason
                    .as_ref()
                    .map(|r| {
                        r.related_plates
                            .iter()
                            .map(|p| p.plate_name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                LimitUpStock {
                    symbol: s.symbol.clone(),
                    name: s.stock_chi_name.clone(),
                    days: s.limit_up_days,
                    change_percent: (s.change_percent * 100.0).round() / 100.0,
                    turnover: (s.turnover_ratio * 100.0).round() / 100.0,
                    reason,
                    bomb: s.break_limit_up_times,
                    capital: (s.total_capital / 1e8 * 100.0).round() / 100.0,
                }
            })
            .collect::<Vec<_>>();
        stocks.sort_by(|a, b| {
            b.days
                .cmp(&a.days)
                .then(b.capital.partial_cmp(&a.capital).unwrap())
        });
        Some(Self { stocks })
    }
}

#[derive(Template)]
#[template(path = "markdown/plate_abnormal.md")]
pub struct PlateAbnormalMdTemplate {
    pub rise_count: usize,
    pub fall_count: usize,
    pub events_by_time: BTreeMap<String, Vec<PlateAbnormalEventData>>,
}

impl PlateAbnormalMdTemplate {
    pub fn render(events: &[PlateAbnormalEvent]) -> Option<Self> {
        if events.is_empty() {
            return None;
        }
        let mut rise_count = 0;
        let mut fall_count = 0;
        let mut events_by_time: BTreeMap<String, Vec<PlateAbnormalEventData>> = BTreeMap::new();
        for event in events {
            if let Some(d) = &event.plate_abnormal_event_data {
                if let Some(pcp) = d.pcp {
                    if pcp > 0.0 {
                        rise_count += 1;
                    } else {
                        fall_count += 1;
                    }
                }
                let ts = event.event_timestamp;
                let datetime = chrono::DateTime::from_timestamp(ts, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_else(|| ts.to_string());
                let plate_name = d.plate_name.clone().unwrap_or_default();
                let pcp = d
                    .pcp
                    .map(|p| format!("{:+.1}%", p * 100.0))
                    .unwrap_or_default();
                let related_stocks = d
                    .related_stocks
                    .iter()
                    .map(|s| RelatedStockData {
                        name: s.name.clone(),
                        pcp: format!("{:+.1}%", s.pcp * 100.0),
                    })
                    .collect();
                let event_data = PlateAbnormalEventData {
                    plate_name,
                    pcp,
                    related_stocks,
                };
                events_by_time.entry(datetime).or_default().push(event_data);
            }
        }
        Some(Self {
            rise_count,
            fall_count,
            events_by_time,
        })
    }
}

#[derive(Template)]
#[template(path = "markdown/field_items.md")]
pub struct FieldItemsMdTemplate {
    pub plates: Vec<FieldPlateData>,
}

impl FieldItemsMdTemplate {
    pub fn render(items: &[FieldPlate]) -> Option<Self> {
        if items.is_empty() {
            return None;
        }
        let plates = items
            .iter()
            .map(|plate| {
                let stocks = plate
                    .list
                    .iter()
                    .map(|stock| {
                        let action = stock.article.as_ref().and_then(|a| a.action_info.as_ref());
                        let time = action
                            .and_then(|a| a.time.as_deref())
                            .unwrap_or("")
                            .to_string();
                        let num = action
                            .and_then(|a| a.num.as_deref())
                            .unwrap_or("-")
                            .to_string();
                        let desc = action
                            .and_then(|a| a.expound.as_deref())
                            .unwrap_or("-")
                            .replace('\n', "<br>");
                        FieldStockData {
                            time,
                            code: stock.code.clone(),
                            name: stock.name.clone(),
                            num,
                            desc,
                        }
                    })
                    .collect();
                FieldPlateData {
                    name: plate.name.clone(),
                    count: plate.count as usize,
                    reason: plate.reason.clone(),
                    stocks,
                }
            })
            .collect();
        Some(Self { plates })
    }
}

#[derive(Template)]
#[template(path = "markdown/timeline.md")]
pub struct TimelineMdTemplate {
    pub timeline_items: Vec<TimelineItemData>,
    pub timeline_by_date: BTreeMap<String, Vec<TimelineDetailItem>>,
}

impl TimelineMdTemplate {
    pub fn render(timeline: &[TimelineDay]) -> Option<Self> {
        if timeline.is_empty() {
            return None;
        }
        let mut timeline_items = Vec::new();
        let mut timeline_by_date: BTreeMap<String, Vec<TimelineDetailItem>> = BTreeMap::new();
        for day in timeline.iter().rev() {
            if day.list.is_empty() {
                continue;
            }
            for item in &day.list {
                let grade_emoji = match item.timeline.grade {
                    5 => "⭐",
                    6 => "🌟",
                    _ => "•",
                }
                .to_string();
                timeline_items.push(TimelineItemData {
                    grade_emoji: grade_emoji.clone(),
                    title: item.title.clone(),
                    date: day.date.clone(),
                });
                let tags: Vec<&str> = item
                    .timeline
                    .theme_list
                    .iter()
                    .map(|t| t.name.as_str())
                    .collect();
                let tags_str = if tags.is_empty() {
                    String::new()
                } else {
                    tags.join(", ")
                };
                let content = if !item.content.is_empty() && item.content != " " {
                    item.content.replace('\n', " ")
                } else {
                    String::new()
                };
                timeline_by_date
                    .entry(day.date.clone())
                    .or_default()
                    .push(TimelineDetailItem {
                        grade_emoji,
                        title: item.title.clone(),
                        tags: tags_str,
                        content,
                    });
            }
        }
        Some(Self {
            timeline_items,
            timeline_by_date,
        })
    }
}

#[derive(Template)]
#[template(path = "markdown/analyses.md")]
pub struct AnalysesMdTemplate {
    pub analyses: Vec<AnalysisData>,
}

impl AnalysesMdTemplate {
    pub fn render(analyses: &[RoleAnalysis]) -> Self {
        let analyses = analyses
            .iter()
            .map(|a| AnalysisData {
                role_name: a.role.name().to_string(),
                content: a.content.clone(),
                elapsed_ms: a.elapsed_ms,
            })
            .collect();
        Self { analyses }
    }
}

#[derive(Template)]
#[template(path = "markdown/report.md")]
pub struct ReportMdTemplate {
    pub date: String,
    pub overview: String,
    pub board_distribution: String,
    pub limit_up_table: String,
    pub plate_abnormal: String,
    pub field_items: String,
    pub timeline: String,
    pub analyses: String,
}

impl ReportMdTemplate {
    pub fn from_report(
        date: &str,
        overview: &MarketOverview,
        limit_up: &[Stock],
        plate_abnormal: &[PlateAbnormalEvent],
        field_items: &[FieldPlate],
        timeline: &[TimelineDay],
        analyses: &[RoleAnalysis],
    ) -> Self {
        let overview = OverviewMdTemplate::render(overview).render().unwrap();
        let board_distribution = BoardDistributionMdTemplate::render(limit_up)
            .render()
            .unwrap();
        let limit_up_table = LimitUpTableMdTemplate::render(limit_up)
            .and_then(|t| t.render().ok())
            .unwrap_or_default();
        let plate_abnormal = PlateAbnormalMdTemplate::render(plate_abnormal)
            .and_then(|t| t.render().ok())
            .unwrap_or_default();
        let field_items = FieldItemsMdTemplate::render(field_items)
            .and_then(|t| t.render().ok())
            .unwrap_or_default();
        let timeline = TimelineMdTemplate::render(timeline)
            .and_then(|t| t.render().ok())
            .unwrap_or_default();
        let analyses = AnalysesMdTemplate::render(analyses).render().unwrap();
        Self {
            date: date.to_string(),
            overview,
            board_distribution,
            limit_up_table,
            plate_abnormal,
            field_items,
            timeline,
            analyses,
        }
    }
}

// ============== HTML Templates ==============

#[derive(Template)]
#[template(path = "html/overview.html")]
pub struct OverviewHtmlTemplate {
    pub rise_count: u32,
    pub fall_count: u32,
    pub limit_up_count: u32,
    pub limit_down_count: u32,
    pub bomb_rate: f64,
    pub rise_fall_ratio: f64,
}

impl OverviewHtmlTemplate {
    pub fn render(overview: &MarketOverview) -> Self {
        let rise_fall_ratio = overview.rise_count as f64 / overview.fall_count.max(1) as f64;
        let bomb_rate = (overview.bomb_rate * 10.0).round() / 10.0;
        Self {
            rise_count: overview.rise_count,
            fall_count: overview.fall_count,
            limit_up_count: overview.limit_up_count,
            limit_down_count: overview.limit_down_count,
            bomb_rate,
            rise_fall_ratio,
        }
    }
}

#[derive(Template)]
#[template(path = "html/board_distribution.html")]
pub struct BoardDistributionHtmlTemplate {
    pub distribution: Vec<(u32, u32)>,
}

impl BoardDistributionHtmlTemplate {
    pub fn render(limit_up: &[Stock]) -> Self {
        let mut map: BTreeMap<u32, u32> = BTreeMap::new();
        for s in limit_up {
            *map.entry(s.limit_up_days).or_default() += 1;
        }
        let mut distribution: Vec<(u32, u32)> = map.into_iter().collect();
        distribution.sort_by(|a, b| b.0.cmp(&a.0));
        Self { distribution }
    }
}

#[derive(Template)]
#[template(path = "html/limit_up_table.html")]
pub struct LimitUpTableHtmlTemplate {
    pub stocks: Vec<LimitUpStock>,
}

impl LimitUpTableHtmlTemplate {
    pub fn render(limit_up: &[Stock]) -> Option<Self> {
        if limit_up.is_empty() {
            return None;
        }
        let mut stocks = limit_up
            .iter()
            .map(|s| {
                let reason = s
                    .surge_reason
                    .as_ref()
                    .map(|r| {
                        r.related_plates
                            .iter()
                            .map(|p| p.plate_name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                LimitUpStock {
                    symbol: s.symbol.clone(),
                    name: s.stock_chi_name.clone(),
                    days: s.limit_up_days,
                    change_percent: (s.change_percent * 100.0).round() / 100.0,
                    turnover: (s.turnover_ratio * 100.0).round() / 100.0,
                    reason,
                    bomb: s.break_limit_up_times,
                    capital: (s.total_capital / 1e8 * 100.0).round() / 100.0,
                }
            })
            .collect::<Vec<_>>();
        stocks.sort_by(|a, b| {
            b.days
                .cmp(&a.days)
                .then(b.capital.partial_cmp(&a.capital).unwrap())
        });
        Some(Self { stocks })
    }
}

#[derive(Template)]
#[template(path = "html/plate_abnormal.html")]
pub struct PlateAbnormalHtmlTemplate {
    pub rise_count: usize,
    pub fall_count: usize,
    pub events_by_time: BTreeMap<String, Vec<PlateAbnormalEventData>>,
}

impl PlateAbnormalHtmlTemplate {
    pub fn render(events: &[PlateAbnormalEvent]) -> Option<Self> {
        if events.is_empty() {
            return None;
        }
        let mut rise_count = 0;
        let mut fall_count = 0;
        let mut events_by_time: BTreeMap<String, Vec<PlateAbnormalEventData>> = BTreeMap::new();
        for event in events {
            if let Some(d) = &event.plate_abnormal_event_data {
                if let Some(pcp) = d.pcp {
                    if pcp > 0.0 {
                        rise_count += 1;
                    } else {
                        fall_count += 1;
                    }
                }
                let ts = event.event_timestamp;
                let datetime = chrono::DateTime::from_timestamp(ts, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_else(|| ts.to_string());
                let plate_name = d.plate_name.clone().unwrap_or_default();
                let pcp = d
                    .pcp
                    .map(|p| format!("{:+.1}%", p * 100.0))
                    .unwrap_or_default();
                let related_stocks = d
                    .related_stocks
                    .iter()
                    .map(|s| RelatedStockData {
                        name: s.name.clone(),
                        pcp: format!("{:+.1}%", s.pcp * 100.0),
                    })
                    .collect();
                let event_data = PlateAbnormalEventData {
                    plate_name,
                    pcp,
                    related_stocks,
                };
                events_by_time.entry(datetime).or_default().push(event_data);
            }
        }
        Some(Self {
            rise_count,
            fall_count,
            events_by_time,
        })
    }
}

#[derive(Template)]
#[template(path = "html/field_items.html")]
pub struct FieldItemsHtmlTemplate {
    pub plates: Vec<FieldPlateData>,
}

impl FieldItemsHtmlTemplate {
    pub fn render(items: &[FieldPlate]) -> Option<Self> {
        if items.is_empty() {
            return None;
        }
        let plates = items
            .iter()
            .map(|plate| {
                let stocks = plate
                    .list
                    .iter()
                    .map(|stock| {
                        let action = stock.article.as_ref().and_then(|a| a.action_info.as_ref());
                        let time = action
                            .and_then(|a| a.time.as_deref())
                            .unwrap_or("")
                            .to_string();
                        let num = action
                            .and_then(|a| a.num.as_deref())
                            .unwrap_or("-")
                            .to_string();
                        let desc = action
                            .and_then(|a| a.expound.as_deref())
                            .unwrap_or("-")
                            .replace('\n', "<br>");
                        FieldStockData {
                            time,
                            code: stock.code.clone(),
                            name: stock.name.clone(),
                            num,
                            desc,
                        }
                    })
                    .collect();
                FieldPlateData {
                    name: plate.name.clone(),
                    count: plate.count as usize,
                    reason: plate.reason.clone(),
                    stocks,
                }
            })
            .collect();
        Some(Self { plates })
    }
}

#[derive(Template)]
#[template(path = "html/timeline.html")]
pub struct TimelineHtmlTemplate {
    pub timeline_items: Vec<TimelineItemData>,
    pub timeline_by_date: BTreeMap<String, Vec<TimelineDetailItem>>,
}

impl TimelineHtmlTemplate {
    pub fn render(timeline: &[TimelineDay]) -> Option<Self> {
        if timeline.is_empty() {
            return None;
        }
        let mut timeline_items = Vec::new();
        let mut timeline_by_date: BTreeMap<String, Vec<TimelineDetailItem>> = BTreeMap::new();
        for day in timeline.iter().rev() {
            if day.list.is_empty() {
                continue;
            }
            for item in &day.list {
                let grade_emoji = match item.timeline.grade {
                    5 => "⭐",
                    6 => "🌟",
                    _ => "•",
                }
                .to_string();
                timeline_items.push(TimelineItemData {
                    grade_emoji: grade_emoji.clone(),
                    title: item.title.clone(),
                    date: day.date.clone(),
                });
                let tags: Vec<&str> = item
                    .timeline
                    .theme_list
                    .iter()
                    .map(|t| t.name.as_str())
                    .collect();
                let tags_str = if tags.is_empty() {
                    String::new()
                } else {
                    tags.join(", ")
                };
                let content = if !item.content.is_empty() && item.content != " " {
                    item.content.replace('\n', " ")
                } else {
                    String::new()
                };
                timeline_by_date
                    .entry(day.date.clone())
                    .or_default()
                    .push(TimelineDetailItem {
                        grade_emoji,
                        title: item.title.clone(),
                        tags: tags_str,
                        content,
                    });
            }
        }
        Some(Self {
            timeline_items,
            timeline_by_date,
        })
    }
}

#[derive(Template)]
#[template(path = "html/analyses.html")]
pub struct AnalysesHtmlTemplate {
    pub analyses: Vec<AnalysisData>,
}

impl AnalysesHtmlTemplate {
    pub fn render(analyses: &[RoleAnalysis]) -> Self {
        let analyses = analyses
            .iter()
            .map(|a| AnalysisData {
                role_name: a.role.name().to_string(),
                content: a.content.clone(),
                elapsed_ms: a.elapsed_ms,
            })
            .collect();
        Self { analyses }
    }
}

#[derive(Template)]
#[template(path = "html/report.html")]
pub struct ReportHtmlTemplate {
    pub date: String,
    pub overview: String,
    pub board_distribution: String,
    pub limit_up_table: String,
    pub plate_abnormal: String,
    pub field_items: String,
    pub timeline: String,
    pub analyses: String,
}

impl ReportHtmlTemplate {
    pub fn from_report(
        date: &str,
        overview: &MarketOverview,
        limit_up: &[Stock],
        plate_abnormal: &[PlateAbnormalEvent],
        field_items: &[FieldPlate],
        timeline: &[TimelineDay],
        analyses: &[RoleAnalysis],
    ) -> Self {
        let overview = OverviewHtmlTemplate::render(overview).render().unwrap();
        let board_distribution = BoardDistributionHtmlTemplate::render(limit_up)
            .render()
            .unwrap();
        let limit_up_table = LimitUpTableHtmlTemplate::render(limit_up)
            .and_then(|t| t.render().ok())
            .unwrap_or_default();
        let plate_abnormal = PlateAbnormalHtmlTemplate::render(plate_abnormal)
            .and_then(|t| t.render().ok())
            .unwrap_or_default();
        let field_items = FieldItemsHtmlTemplate::render(field_items)
            .and_then(|t| t.render().ok())
            .unwrap_or_default();
        let timeline = TimelineHtmlTemplate::render(timeline)
            .and_then(|t| t.render().ok())
            .unwrap_or_default();
        let analyses = AnalysesHtmlTemplate::render(analyses).render().unwrap();
        Self {
            date: date.to_string(),
            overview,
            board_distribution,
            limit_up_table,
            plate_abnormal,
            field_items,
            timeline,
            analyses,
        }
    }
}
