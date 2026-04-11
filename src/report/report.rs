use std::fmt::Write as FmtWrite;
use std::fs;
use std::io::Write;

use chrono::Local;

use crate::api::jiuyangongshe::{FieldPlate, TimelineDay};
use crate::api::xuangubao::{MarketOverview, PlateAbnormalEvent, Stock};
use crate::roles::analyst::Role;

// ─── 单个角色的分析结果 ───────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RoleAnalysis {
    pub role: Role,
    pub content: String,
    pub elapsed_ms: u64, // LLM 响应耗时，方便 debug
}

// ─── 完整报告数据 ─────────────────────────────────────────────

#[derive(Debug)]
pub struct Report {
    pub date: String,
    pub overview: MarketOverview,
    pub limit_up: Vec<Stock>,
    /// 选股宝板块异动数据
    pub plate_abnormal: Vec<PlateAbnormalEvent>,
    /// 韭研公社异动数据（按板块分组）
    pub field_items: Vec<FieldPlate>,
    /// 韭研公社时间线数据（按日期分组）
    pub timeline: Vec<TimelineDay>,
    pub analyses: Vec<RoleAnalysis>,
}

impl Report {
    pub fn new(
        overview: MarketOverview,
        limit_up: Vec<Stock>,
        plate_abnormal: Vec<PlateAbnormalEvent>,
        field_items: Vec<FieldPlate>,
        timeline: Vec<TimelineDay>,
        analyses: Vec<RoleAnalysis>,
    ) -> Self {
        Self {
            date: Local::now().format("%Y-%m-%d %H:%M").to_string(),
            overview,
            limit_up,
            plate_abnormal,
            field_items,
            timeline,
            analyses,
        }
    }

    // ─── 渲染成纯文本（终端输出用）────────────────────────────

    pub fn render_text(&self) -> String {
        let mut out = String::new();

        // 头部
        writeln!(out, "╔══════════════════════════════════════════╗").unwrap();
        writeln!(out, "║     📊 A股市场 AI 多角色分析报告         ║").unwrap();
        writeln!(out, "║     {}                    ║", self.date).unwrap();
        writeln!(out, "╚══════════════════════════════════════════╝").unwrap();
        writeln!(out).unwrap();

        // 市场概览区块
        self.render_overview(&mut out);

        // 涨停结构分析
        self.render_limit_up_structure(&mut out);

        // 各角色分析
        for analysis in &self.analyses {
            self.render_role_section(&mut out, analysis);
        }

        // 尾部提示
        writeln!(out, "─────────────────────────────────────────").unwrap();
        writeln!(out, "⚠️  以上内容由 AI 生成，仅供参考，不构成投资建议").unwrap();
        writeln!(out, "─────────────────────────────────────────").unwrap();

        out
    }

    // ─── 渲染成 Markdown（存文件 / 推送用）────────────────────

    pub fn render_markdown(&self) -> String {
        let mut out = String::new();

        writeln!(out, "# 📊 A股市场 AI 分析报告").unwrap();
        writeln!(out, "> 生成时间：{}", self.date).unwrap();
        writeln!(out).unwrap();

        // 市场概览
        writeln!(out, "## 市场概览").unwrap();
        writeln!(out, "| 指标 | 数值 |").unwrap();
        writeln!(out, "|------|------|").unwrap();
        writeln!(out, "| 上涨家数 | {} |", self.overview.rise_count).unwrap();
        writeln!(out, "| 下跌家数 | {} |", self.overview.fall_count).unwrap();
        writeln!(out, "| 涨停家数 | {} |", self.overview.limit_up_count).unwrap();
        writeln!(out, "| 跌停家数 | {} |", self.overview.limit_down_count).unwrap();
        writeln!(out, "| 炸板率 | {} |", self.overview.bomb_rate).unwrap();
        writeln!(
            out,
            "| 涨跌比 | {:.2} |",
            self.overview.rise_count as f64 / self.overview.fall_count.max(1) as f64
        )
        .unwrap();
        writeln!(out).unwrap();

        // 连板结构
        writeln!(out, "## 连板高度分布").unwrap();
        let board_map = self.calc_board_distribution();
        for (boards, count) in &board_map {
            writeln!(out, "- {}板：{}家", boards, count).unwrap();
        }
        writeln!(out).unwrap();

        // 涨停股明细
        self.render_limit_up_table(&mut out);

        // 选股宝板块异动
        self.render_plate_abnormal(&mut out);

        // 韭研公社异动
        self.render_field_table(&mut out);

        // 韭研公社时间线
        self.render_timeline(&mut out);

        // 各角色
        for analysis in &self.analyses {
            writeln!(out, "## {} 视角", analysis.role.name()).unwrap();
            writeln!(out, "{}", analysis.content).unwrap();
            writeln!(out, "> 响应耗时：{}ms", analysis.elapsed_ms).unwrap();
            writeln!(out).unwrap();
        }

        writeln!(out, "---").unwrap();
        writeln!(out, "> ⚠️ AI 生成内容，仅供参考，不构成投资建议").unwrap();

        out
    }

    // ─── 内部渲染辅助方法 ─────────────────────────────────────

    fn render_overview(&self, out: &mut String) {
        let rise = self.overview.rise_count;
        let fall = self.overview.fall_count;
        let ratio = rise as f64 / fall.max(1) as f64;

        // 情绪强度可视化
        let sentiment_bar = self.sentiment_bar(ratio);
        let sentiment_label = match ratio {
            r if r >= 3.0 => "🔥 极度亢奋",
            r if r >= 2.0 => "💪 情绪偏强",
            r if r >= 1.2 => "😐 震荡偏多",
            r if r >= 0.8 => "😶 多空均衡",
            _ => "🥶 情绪偏弱",
        };

        writeln!(out, "┌─ 市场概览 ─────────────────────────────").unwrap();
        writeln!(out, "│  上涨 {:>4} 家  │  下跌 {:>4} 家", rise, fall).unwrap();
        writeln!(
            out,
            "│  涨停 {:>4} 家  │  跌停 {:>4} 家",
            self.overview.limit_up_count, self.overview.limit_down_count
        )
        .unwrap();
        writeln!(out, "│  涨跌比 {:.2}  {}", ratio, sentiment_label).unwrap();
        writeln!(out, "│  {}", sentiment_bar).unwrap();
        writeln!(out, "└────────────────────────────────────────").unwrap();
        writeln!(out).unwrap();
    }

    fn render_limit_up_structure(&self, out: &mut String) {
        if self.limit_up.is_empty() {
            return;
        }

        writeln!(out, "┌─ 涨停结构 ─────────────────────────────").unwrap();

        // 连板分布
        let board_map = self.calc_board_distribution();
        let dist_str: Vec<String> = board_map
            .iter()
            .map(|(b, c)| format!("{}板×{}", b, c))
            .collect();
        writeln!(out, "│  连板分布：{}", dist_str.join(" / ")).unwrap();

        // 炸板情况
        let total_bomb: u32 = self.limit_up.iter().map(|s| s.break_limit_up_times).sum();
        let bomb_rate =
            total_bomb as f64 / (self.limit_up.len() as f64 + total_bomb as f64) * 100.0;
        writeln!(
            out,
            "│  炸板率：{:.1}%  {}",
            bomb_rate,
            bomb_emoji(bomb_rate)
        )
        .unwrap();

        // 热门行业 top3
        let industries = self.calc_top_industries(3);
        writeln!(out, "│  热门行业：{}", industries.join(" > ")).unwrap();

        // 连板 2+ 龙头列表
        let mut high: Vec<&Stock> = self
            .limit_up
            .iter()
            .filter(|s| s.limit_up_days >= 2)
            .collect();
        high.sort_by(|a, b| b.limit_up_days.cmp(&a.limit_up_days));

        if !high.is_empty() {
            writeln!(out, "│").unwrap();
            writeln!(out, "│  连板龙头（2板+）：").unwrap();
            for s in high.iter().take(5) {
                writeln!(
                    out,
                    "│    {} {} │ {}板 │ 封:{} │ 炸:{}次",
                    s.symbol,
                    s.stock_chi_name,
                    s.limit_up_days,
                    s.last_limit_up,
                    s.break_limit_up_times
                )
                .unwrap();
            }
        }

        writeln!(out, "└────────────────────────────────────────").unwrap();
        writeln!(out).unwrap();
    }

    fn render_role_section(&self, out: &mut String, analysis: &RoleAnalysis) {
        let emoji = match analysis.role {
            Role::Trader => "⚡",
            Role::Quant => "📐",
            Role::RiskOfficer => "🛡️",
        };

        writeln!(
            out,
            "┌─ {} {} 视角 ({} ms) ──────────────────────",
            emoji,
            analysis.role.name(),
            analysis.elapsed_ms
        )
        .unwrap();

        // 把内容每行加 │ 前缀，保持视觉一致
        for line in analysis.content.lines() {
            writeln!(out, "│  {}", line).unwrap();
        }

        writeln!(out, "└────────────────────────────────────────").unwrap();
        writeln!(out).unwrap();
    }

    // ─── 渲染涨停股 Markdown 表格 ─────────────────────────────

    fn render_limit_up_table(&self, out: &mut String) {
        if self.limit_up.is_empty() {
            return;
        }

        writeln!(out, "## 涨停股明细").unwrap();
        writeln!(
            out,
            "| 代码 | 名称 | 连板数 | 涨跌幅% | 换手率% | 涨停原因 | 炸板次数 | 流通市值(亿) |"
        )
        .unwrap();
        writeln!(
            out,
            "|------|------|--------|---------|---------|----------|----------|--------------|"
        )
        .unwrap();

        // 按连板数降序、流通市值降序排序
        let mut sorted = self.limit_up.clone();
        sorted.sort_by(|a, b| {
            b.limit_up_days.cmp(&a.limit_up_days).then(
                b.total_capital
                    .partial_cmp(&a.total_capital)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
        });

        for s in &sorted {
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

            writeln!(
                out,
                "| {} | {} | {}板 | {:.2} | {:.2} | {} | {} | {:.2} |",
                s.symbol,
                s.stock_chi_name,
                s.limit_up_days,
                s.change_percent,
                s.turnover_ratio,
                reason,
                s.break_limit_up_times,
                s.total_capital / 1e8,
            )
            .unwrap();
        }

        writeln!(out).unwrap();
    }

    // ─── 渲染选股宝板块异动 Markdown ─────────────────────────────

    fn render_plate_abnormal(&self, out: &mut String) {
        if self.plate_abnormal.is_empty() {
            return;
        }

        writeln!(out, "## 板块异动").unwrap();
        writeln!(out).unwrap();

        // Mermaid 饼图
        writeln!(out, "### 涨跌分布").unwrap();
        writeln!(out, "```mermaid").unwrap();
        writeln!(out, "pie title 板块涨跌").unwrap();

        let mut rise_count = 0;
        let mut fall_count = 0;
        for event in &self.plate_abnormal {
            if let Some(d) = &event.plate_abnormal_event_data {
                if let Some(pcp) = d.pcp {
                    if pcp > 0.0 {
                        rise_count += 1;
                    } else {
                        fall_count += 1;
                    }
                }
            }
        }
        writeln!(out, "    上涨 : {}", rise_count).unwrap();
        writeln!(out, "    下跌 : {}", fall_count).unwrap();
        writeln!(out, "```").unwrap();
        writeln!(out).unwrap();

        // 按事件时间分组渲染
        writeln!(out, "### 按时间排序").unwrap();

        // 按时间戳倒序分组
        let mut events_by_time: std::collections::BTreeMap<String, Vec<&PlateAbnormalEvent>> =
            Default::default();
        for event in &self.plate_abnormal {
            let ts = event.event_timestamp;
            let datetime = chrono::DateTime::from_timestamp(ts, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| ts.to_string());
            events_by_time.entry(datetime).or_default().push(event);
        }

        for (datetime, events) in events_by_time.iter().rev() {
            writeln!(out, "#### {}", datetime).unwrap();
            writeln!(out).unwrap();

            for event in events {
                let plate_data = match &event.plate_abnormal_event_data {
                    Some(d) => d,
                    None => continue,
                };

                let plate_name = plate_data.plate_name.as_deref().unwrap_or("-");
                let pcp = plate_data
                    .pcp
                    .map(|p| format!("{:+.1}%", p * 100.0))
                    .unwrap_or_else(|| "-".to_string());

                writeln!(out, "##### {}", plate_name).unwrap();
                writeln!(out, "- 涨跌幅: {}", pcp).unwrap();

                if !plate_data.related_stocks.is_empty() {
                    writeln!(out, "- 关联股票:").unwrap();
                    for stock in &plate_data.related_stocks {
                        let stock_pcp = format!("{:+.1}%", stock.pcp * 100.0);
                        writeln!(out, "  - {} ({})", stock.name, stock_pcp).unwrap();
                    }
                }
            }

            writeln!(out).unwrap();
        }

        writeln!(out).unwrap();
    }

    // ─── 渲染韭研公社异动 Markdown 表格（按板块分组）──────────

    fn render_field_table(&self, out: &mut String) {
        if self.field_items.is_empty() {
            return;
        }

        writeln!(out, "## 韭研公社异动").unwrap();

        for plate in &self.field_items {
            // 板块标题 + 异动原因
            writeln!(out, "### {}（{}只）", plate.name, plate.count).unwrap();
            if !plate.reason.is_empty() {
                writeln!(out, "> {}", plate.reason).unwrap();
            }
            writeln!(out).unwrap();

            // 该板块异动股票列表
            if plate.list.is_empty() {
                continue;
            }

            writeln!(out, "| 时间 | 代码 | 名称 | 连板 | 异动理由 |").unwrap();
            writeln!(out, "|------|------|------|------|----------|").unwrap();

            for stock in &plate.list {
                let action = stock.article.as_ref().and_then(|a| a.action_info.as_ref());
                let time = action.and_then(|a| a.time.as_deref()).unwrap_or("");
                let num = action.and_then(|a| a.num.as_deref()).unwrap_or("-");
                let desc = action
                    .and_then(|a| a.expound.as_deref())
                    .unwrap_or("-")
                    .replace('\n', "<br>");

                writeln!(
                    out,
                    "| {} | {} | {} | {} | {} |",
                    time, stock.code, stock.name, num, desc
                )
                .unwrap();
            }

            writeln!(out).unwrap();
        }
    }

    // ─── 渲染韭研公社时间线 Markdown ─────────────────────────────

    fn render_timeline(&self, out: &mut String) {
        if self.timeline.is_empty() {
            return;
        }

        writeln!(out, "## 时间线").unwrap();
        writeln!(out).unwrap();

        // Mermaid 甘特图
        writeln!(out, "### 事件分布").unwrap();
        writeln!(out, "```mermaid").unwrap();
        writeln!(out, "gantt").unwrap();
        writeln!(out, "    title 时间线事件").unwrap();
        writeln!(out, "    dateFormat YYYY-MM-DD").unwrap();
        writeln!(out).unwrap();

        for day in self.timeline.iter().rev() {
            if day.list.is_empty() {
                continue;
            }
            for item in &day.list {
                let title = item.title.clone();
                let grade = match item.timeline.grade {
                    5 => "⭐",
                    6 => "🌟",
                    _ => "•",
                };
                writeln!(out, "    {} {} :crit, {}, 1d", grade, title, day.date).unwrap();
            }
        }

        writeln!(out, "```").unwrap();
        writeln!(out).unwrap();

        // 详细列表
        writeln!(out, "### 详细事件").unwrap();
        for day in self.timeline.iter().rev() {
            if day.list.is_empty() {
                continue;
            }

            writeln!(out, "#### {}", day.date).unwrap();
            writeln!(out).unwrap();

            for item in &day.list {
                let grade_emoji = match item.timeline.grade {
                    5 => "⭐",
                    6 => "🌟",
                    _ => "•",
                };

                let tags: Vec<&str> = item
                    .timeline
                    .theme_list
                    .iter()
                    .map(|t| t.name.as_str())
                    .collect();
                let tag_str = if tags.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", tags.join(", "))
                };

                let content_preview = if !item.content.is_empty() && item.content != " " {
                    Some(item.content.replace('\n', " "))
                } else {
                    None
                };

                writeln!(out, "- {} **{}**{}", grade_emoji, item.title, tag_str).unwrap();
                if let Some(content) = content_preview {
                    writeln!(out, "  - {}", content).unwrap();
                }
            }

            writeln!(out).unwrap();
        }
    }

    // ─── 数据计算辅助 ─────────────────────────────────────────

    fn calc_board_distribution(&self) -> Vec<(u32, u32)> {
        let mut map: std::collections::BTreeMap<u32, u32> = Default::default();
        for s in &self.limit_up {
            *map.entry(s.limit_up_days).or_default() += 1;
        }
        // 降序排列（高板在前）
        let mut vec: Vec<(u32, u32)> = map.into_iter().collect();
        vec.sort_by(|a, b| b.0.cmp(&a.0));
        vec
    }

    fn calc_top_industries(&self, n: usize) -> Vec<String> {
        let mut map: std::collections::HashMap<&str, u32> = Default::default();
        for s in &self.limit_up {
            if let Some(ref reason) = s.surge_reason {
                for plate in &reason.related_plates {
                    *map.entry(plate.plate_name.as_str()).or_default() += 1;
                }
            }
        }
        let mut vec: Vec<_> = map.into_iter().collect();
        vec.sort_by(|a, b| b.1.cmp(&a.1));
        vec.iter()
            .take(n)
            .map(|(hy, cnt)| format!("{}({}家)", hy, cnt))
            .collect()
    }

    fn sentiment_bar(&self, ratio: f64) -> String {
        let filled = ((ratio / 4.0) * 20.0).min(20.0) as usize;
        let bar = "█".repeat(filled) + &"░".repeat(20 - filled);
        format!("[{}] {:.2}", bar, ratio)
    }
}

fn bomb_emoji(rate: f64) -> &'static str {
    match rate as u32 {
        0..=10 => "✅ 封板稳健",
        11..=20 => "⚠️ 略有松动",
        21..=35 => "🚨 炸板频繁",
        _ => "💥 情绪极度不稳",
    }
}

// ─── 输出目标 ─────────────────────────────────────────────────

pub struct Reporter {
    pub report: Report,
}

impl Reporter {
    pub fn new(report: Report) -> Self {
        Self { report }
    }

    /// 打印到终端
    pub fn print(&self) {
        print!("{}", self.report.render_text());
    }

    /// 写入 Markdown 文件
    pub fn save_markdown(&self, dir: &str) -> anyhow::Result<String> {
        let filename = format!("{}/report_{}.md", dir, Local::now().format("%Y%m%d_%H%M"));
        fs::create_dir_all(dir)?;
        fs::write(&filename, self.report.render_markdown())?;
        Ok(filename)
    }

    /// 写入纯文本文件
    pub fn save_text(&self, dir: &str) -> anyhow::Result<String> {
        let filename = format!("{}/report_{}.txt", dir, Local::now().format("%Y%m%d_%H%M"));
        fs::create_dir_all(dir)?;
        fs::write(&filename, self.report.render_text())?;
        Ok(filename)
    }

    /// flush 到任意 writer（方便将来接 socket/pipe）
    pub fn write_to<W: Write>(&self, mut w: W) -> anyhow::Result<()> {
        w.write_all(self.report.render_text().as_bytes())?;
        Ok(())
    }
}
