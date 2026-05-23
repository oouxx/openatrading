use crate::api::xuangubao::{PlateAbnormalEvent, RelatedStock};
use serde::Serialize;

/// Events + pre-computed time snapshots for the replay frontend.
#[derive(Debug, Serialize)]
pub struct ReplayData {
    pub date: String,
    pub events: Vec<PlateAbnormalEvent>,
    pub snapshots: Vec<TimeSnapshot>,
    pub market_phase: String,
}

/// Aggregated view of a 5‑minute window.
#[derive(Debug, Serialize)]
pub struct TimeSnapshot {
    pub timestamp: i64,
    pub time_label: String,
    pub emotion_score: f64,
    pub bull_count: u32,
    pub bear_count: u32,
    pub top_plates: Vec<TopPlate>,
}

#[derive(Debug, Serialize, Clone)]
pub struct TopPlate {
    pub plate_name: String,
    pub pcp: f64,
    pub good_or_bad: i32,
    pub related_stocks: Vec<RelatedStock>,
}

struct PlateAccumulator {
    pcp: f64,
    good_or_bad: i32,
    related_stocks: Vec<RelatedStock>,
}

/// Build replay data: sort events, generate 5‑min snapshots.
pub fn build_replay_data(date: &str, events: &[PlateAbnormalEvent]) -> ReplayData {
    let snapshots = build_snapshots(events);
    let market_phase = crate::web::cache::current_market_phase();
    ReplayData {
        date: date.to_string(),
        events: events.to_vec(),
        snapshots,
        market_phase: format!("{:?}", market_phase).to_lowercase(),
    }
}

/// Slice events into 5‑minute buckets across 09:30–15:00, computing
/// cumulative emotion scores and top plates per bucket.
fn build_snapshots(events: &[PlateAbnormalEvent]) -> Vec<TimeSnapshot> {
    if events.is_empty() {
        return vec![];
    }

    let start_ts = events.first().unwrap().event_timestamp;
    let end_ts = events.last().unwrap().event_timestamp;
    let bucket_secs: i64 = 5 * 60;

    let first_bucket = (start_ts / bucket_secs) * bucket_secs;
    let last_bucket = (end_ts / bucket_secs) * bucket_secs;

    let mut snapshots: Vec<TimeSnapshot> = Vec::new();
    let mut bull_acc = 0u32;
    let mut bear_acc = 0u32;
    let mut plate_map: std::collections::HashMap<String, PlateAccumulator> =
        std::collections::HashMap::new();
    let mut ev_idx = 0usize;

    let mut bucket_start = first_bucket;
    while bucket_start <= last_bucket {
        let bucket_end = bucket_start + bucket_secs;

        let mut bucket_bull = 0u32;
        let mut bucket_bear = 0u32;
        while ev_idx < events.len() && events[ev_idx].event_timestamp < bucket_end {
            let ev = &events[ev_idx];
            match ev.good_or_bad {
                1 => bucket_bull += 1,
                2 => bucket_bear += 1,
                _ => {}
            }
            if ev.event_type == 11001 {
                if let Some(ref pd) = ev.plate_abnormal_event_data {
                    if let (Some(ref name), Some(pcp)) = (&pd.plate_name, pd.pcp) {
                        let acc =
                            plate_map
                                .entry(name.clone())
                                .or_insert_with(|| PlateAccumulator {
                                    pcp,
                                    good_or_bad: ev.good_or_bad,
                                    related_stocks: pd.related_stocks.clone(),
                                });
                        if pcp.abs() > acc.pcp.abs() {
                            acc.pcp = pcp;
                            acc.good_or_bad = ev.good_or_bad;
                        }
                        if acc.related_stocks.len() < pd.related_stocks.len() {
                            acc.related_stocks = pd.related_stocks.clone();
                        }
                    }
                }
            }
            ev_idx += 1;
        }

        bull_acc += bucket_bull;
        bear_acc += bucket_bear;

        let emotion = if bull_acc + bear_acc > 0 {
            bull_acc as f64 / (bull_acc + bear_acc) as f64 * 100.0
        } else {
            50.0
        };

        let mut top: Vec<TopPlate> = plate_map
            .drain()
            .map(|(plate_name, acc)| TopPlate {
                plate_name,
                pcp: acc.pcp,
                good_or_bad: acc.good_or_bad,
                related_stocks: acc.related_stocks,
            })
            .collect();
        top.sort_by(|a, b| b.pcp.abs().partial_cmp(&a.pcp.abs()).unwrap());
        top.truncate(5);

        let time_label = unix_to_hhmm(bucket_start);

        snapshots.push(TimeSnapshot {
            timestamp: bucket_start,
            time_label,
            emotion_score: (emotion * 10.0).round() / 10.0,
            bull_count: bull_acc,
            bear_count: bear_acc,
            top_plates: top,
        });

        bucket_start = bucket_end;
    }

    snapshots
}

fn unix_to_hhmm(ts: i64) -> String {
    let secs_since_midnight = ts.rem_euclid(86400);
    let h = secs_since_midnight / 3600;
    let m = (secs_since_midnight % 3600) / 60;
    format!("{h:02}:{m:02}")
}

/// Assemble the text prompt for AI analysis from snapshot data.
pub fn build_analysis_prompt(date: &str, snapshots: &[TimeSnapshot]) -> String {
    let mut table = String::from("时间       情绪温度  多/空  主线板块\n");

    for s in snapshots.iter().step_by(2) {
        let top_names: Vec<String> = s.top_plates.iter().map(|p| p.plate_name.clone()).collect();
        table.push_str(&format!(
            "{}      {:.0}       {}/{:<2}  {}\n",
            s.time_label,
            s.emotion_score,
            s.bull_count,
            s.bear_count,
            top_names.join("、"),
        ));
    }

    format!(
        r#"你是一位经验丰富的A股短线交易复盘专家。请根据以下{date}行情回放数据，对全天行情进行结构化分析。

{content}

请从以下维度输出分析报告（Markdown格式）：
1. **全天情绪节奏**：情绪高点/低点出现时间，整体多空趋势
2. **主线板块**：全天最强板块，持续时长，是否有切换
3. **关键时间节点**：情绪突变点分析，可能的原因推断
4. **板块轮动特征**：资金从哪个板块流向哪个板块
5. **复盘结论**：当日适合什么策略，哪类股票机会更大，有什么风险需要注意

要求：分析具体、有数据支撑，避免泛泛而谈。"#,
        date = date,
        content = table,
    )
}
