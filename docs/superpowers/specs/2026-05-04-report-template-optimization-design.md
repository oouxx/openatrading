# 报告模板优化设计

## 概述

优化现有报告模板，提升信息密度、决策效率、视觉层次和交互性。同时整合淘股吧和选股宝数据源，通过子页面架构避免主页面臃肿。

## 设计目标

1. **信息密度提升** — 同样空间展示更多关键数据
2. **决策效率** — 突出风险提示、机会信号，方便快速决策
3. **视觉层次** — 让报告更易读，重点更突出
4. **交互性** — 添加折叠、筛选、排序等轻量交互
5. **数据源整合** — 展示淘股吧、选股宝数据到子页面

## 数据源架构

### 子页面结构
```
dist/
├── index.html              # 主报告页（精简：概览+信号）
├── reports/
│   └── YYYY-MM-DD-HHMM.html  # 完整报告
├── community/            # 子页面：社群文章
│   ├── index.html         # 文章列表页
│   └── article-XXX.html # 单篇文章页
├── taoguba/             # 子页面：淘股吧数据
│   ├── index.html         # 用户列表/帖子列表
│   └── user-XXX.html      # 单个用户帖子页
└── xuangubao/          # 子页面：选股宝数据
    ├── index.html         # 数据概览
    ├── limit-up.html     # 涨停详情
    ├── limit-down.html   # 跌停详情
    └── plates.html       # 板块异动
```

### 数据流向
```
淘股吧 (TGB) ─── TgbClient ─── save_user_posts() ─── methodology/tgb/{user_id}/
                                                ↓
                                        生成 HTML ─── dist/taoguba/

选股宝 (XGB) ─── XuanguBaoClient ─── fetch_limit_up/down/plate ─── Report struct
                                                ↓
                                        生成 HTML ─── dist/xuangubao/
```

## 第1部分：数据层

### 新增字段到 `Report` 结构体

```rust
// src/report/report.rs
pub struct Report {
    // ...现有字段
    
    /// 风险等级：低/中/高（基于涨跌比、炸板率）
    pub risk_level: String,
    /// 机会信号：连板龙头、热门板块等
    pub opportunity_signals: Vec<String>,
    /// 决策摘要：给决策者的一句话总结
    pub decision_summary: String,
    /// 淘股吧数据（可选）
    pub taoguba_users: Vec<TgbUserSummary>,
    /// 选股宝额外数据（跌停、板块等）
    pub xuangubao_extra: XuanGubaoExtra,
}
```

### 淘股吧数据结构

```rust
// src/api/taoguba.rs（新增导出结构）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TgbUserSummary {
    pub user_id: String,
    pub nickname: String,
    pub post_count: usize,
    pub top_post_title: String,  // 置顶帖标题
    pub last_update: String,      // 最后更新时间
}
```

### 选股宝额外数据

```rust
// src/api/xuangubao.rs（新增结构）
#[derive(Debug, Clone)]
pub struct XuanGubaoExtra {
    pub limit_down: Vec<Stock>,      // 跌停数据
    pub plate_abnormal: Vec<PlateAbnormalEvent>, // 板块异动
}
```

### 风险等级计算逻辑

```rust
// src/report/report.rs → Report::new()
let risk_level = if ratio < 0.8 && total_bomb_rate > 30.0 {
    "高".to_string()  // 多空失衡 + 炸板频繁
} else if ratio < 1.2 || total_bomb_rate > 20.0 {
    "中".to_string()
} else {
    "低".to_string()
};

// 机会信号
let mut signals = vec![];
if !high_boards.is_empty() {
    signals.push(format!("连板龙头：{} ({}板)", 
        high_boards[0].stock_chi_name, high_boards[0].limit_up_days));
}
if !hot_industries.is_empty() {
    signals.push(format!("热门板块：{}", hot_industries[0]));
}

// 决策摘要
let decision_summary = format!(
    "当前市场情绪{}，上涨{}家/下跌{}家，建议{}",
    sentiment_label, rise, fall,
    if risk_level == "高" { "观望" } else { "适度参与" }
);
```

## 第2部分：视觉层

### CSS 优化方案

```css
/* 卡片式布局替代纯文本流 */
.card {
    background: #fff;
    border-radius: 8px;
    padding: 20px;
    margin-bottom: 20px;
    box-shadow: 0 2px 8px rgba(0,0,0,0.1);
    border-left: 4px solid #007bff;  /* 风险等级颜色 */
}
.card.risk-high { border-left-color: #dc3545; }  /* 红：高风险 */
.card.risk-mid { border-left-color: #ffc107; }  /* 黄：中风险 */
.card.risk-low  { border-left-color: #28a745; }  /* 绿：低风险 */

/* 信息密度提升：网格布局 */
.grid-2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 15px;
}
.grid-3 {
    display: grid;
    grid-template-columns: 1fr 1fr 1fr;
    gap: 10px;
}

/* 信号标签 */
.signal {
    display: inline-block;
    padding: 4px 8px;
    border-radius: 4px;
    font-size: 13px;
    margin-right: 8px;
    font-weight: 500;
}
.signal-bull { background: #d4edda; color: #155724; }  /* 做多信号 */
.signal-bear { background: #f8d7da; color: #721c24; }  /* 做空信号 */

/* 表格优化 */
table th { position: sticky; top: 0; }  /* 表头固定 */
tr:hover { background: #e9ecef; }       /* 行悬停 */
```

### 子页面链接样式

```css
.subpage-links {
    margin: 20px 0;
    padding: 15px;
    background: #f8f9fa;
    border-radius: 6px;
}
.subpage-links a {
    display: inline-block;
    margin: 5px 10px 5px 0;
    padding: 8px 16px;
    background: #007bff;
    color: #fff;
    text-decoration: none;
    border-radius: 4px;
    font-size: 14px;
}
.subpage-links a:hover { background: #0056b3; }
```

## 第3部分：交互层

### 轻量级 JS 功能

```javascript
// 1. 折叠/展开（复用现有的 <details>）
document.querySelectorAll('.card').forEach(card => {
    card.addEventListener('click', () => card.classList.toggle('collapsed'));
});

// 2. 简单筛选（按风险等级、板块）
function filterReports(level) {
    document.querySelectorAll('.card').forEach(card => {
        card.style.display = (level === 'all' || card.dataset.risk === level) ? '' : 'none';
    });
}

// 3. 子页面导航（高亮当前页）
const currentPage = window.location.pathname.split('/').pop();
document.querySelectorAll('.subpage-links a').forEach(link => {
    if (link.getAttribute('href').includes(currentPage)) {
        link.style.background = '#28a745';
    }
});
```

### 交互元素添加

```html
<!-- 筛选栏 -->
<div class="filter-bar">
    <button onclick="filterReports('all')">全部</button>
    <button onclick="filterReports('high')">高风险</button>
    <button onclick="filterReports('mid')">中风险</button>
    <button onclick="filterReports('low')">低风险</button>
</div>

<!-- 子页面导航 -->
<div class="subpage-links">
    <a href="community/index.html">📰 社群文章</a>
    <a href="taoguba/index.html">📊 淘股吧</a>
    <a href="xuangubao/limit-up.html">📈 选股宝涨停</a>
    <a href="xuangubao/plates.html">📊 板块异动</a>
</div>

<!-- 卡片式报告块 -->
<div class="card risk-high" data-risk="high">
    <h2>📊 涨停结构</h2>
    <div class="grid-2">
        <div>连板分布：3板×5 2板×12</div>
        <div>炸板率：23.5% 🚨</div>
    </div>
    <div class="signals">
        <span class="signal signal-bear">炸板频繁</span>
        <span class="signal signal-bull">连板龙头：XX股票</span>
    </div>
</div>
```

## 第4部分：代码改造

### 1. `src/report/report.rs` 改造

```rust
// 新增字段
#[derive(Debug, Clone)]
pub struct Report {
    pub date: String,
    pub overview: MarketOverview,
    pub limit_up: Vec<Stock>,
    pub plate_abnormal: Vec<PlateAbnormalEvent>,
    pub field_items: Vec<FieldPlate>,
    pub timeline: Vec<TimelineDay>,
    pub analyses: Vec<RoleAnalysis>,
    pub sentiment: Option<SentimentResult>,
    // 新增
    pub risk_level: String,
    pub opportunity_signals: Vec<String>,
    pub decision_summary: String,
    pub taoguba_users: Vec<TgbUserSummary>,
    pub xuangubao_extra: XuanGubaoExtra,
}
```

### 2. `src/cli/mod.rs` 新增子页面生成命令

```rust
#[derive(Subcommand)]
pub enum Commands {
    // ...existing commands...
    /// 生成淘股吧子页面
    TaoGuBa(TaoGuBaOpts),
    /// 生成选股宝子页面
    XuanGuBao(XuanGuBaoOpts),
}

#[derive(clap::Args)]
pub struct TaoGuBaOpts {
    /// 用户ID列表（逗号分隔）
    #[arg(short, long)]
    pub users: String,
    /// 输出目录
    #[arg(short, long, default_value = "./dist/taoguba")]
    pub output: String,
}

#[derive(clap::Args)]
pub struct XuanGuBaoOpts {
    /// 输出目录
    #[arg(short, long, default_value = "./dist/xuangubao")]
    pub output: String,
}
```

### 3. 模板文件新增

```
templates/
├── html/
│   ├── taoguba/
│   │   ├── index.html      # 淘股吧首页模板
│   │   └── user.html        # 用户帖子页模板
│   ├── xuangubao/
│   │   ├── index.html      # 选股宝首页
│   │   ├── limit-up.html  # 涨停详情
│   │   └── plates.html     # 板块异动
│   └── subpage-links.html  # 子页面导航组件
└── markdown/
    ├── taoguba/
    └── xuangubao/
```

## 第5部分：工作流

### CLI 命令

```bash
# 生成完整报告 + 子页面
cargo run -- run --format html --output ./dist/reports/

# 生成淘股吧子页面
cargo run -- taoguba --users "2577911,1234567" --output ./dist/taoguba/

# 生成选股宝子页面
cargo run -- xuangubao --output ./dist/xuangubao/

# 生成索引页（包含所有子页面链接）
cargo run -- index --dir ./dist/
```

### 自动化脚本

```bash
#!/bin/bash
# scripts/generate-all.sh
set -e

echo "📡 生成主报告..."
cargo run -- run --format html --output ./dist/reports/

echo "📡 生成淘股吧子页面..."
cargo run -- taoguba --users "2577911" --output ./dist/taoguba/

echo "📡 生成选股宝子页面..."
cargo run -- xuangubao --output ./dist/xuangubao/

echo "📡 生成索引页..."
cargo run -- index --dir ./dist/

echo "✅ 所有页面生成完成！"
```

## 错误处理

- 淘股吧数据获取失败：跳过该用户，继续其他用户
- 选股宝数据解析失败：使用空数据，不中断生成
- 子页面生成失败：输出错误日志，主报告不受影响
- 模板渲染失败：`anyhow::bail!` 向上传递错误

## 测试计划

1. 本地运行 `cargo run -- run --format html --output ./dist/reports/`
2. 验证 `dist/reports/*.html` 生成
3. 运行 `cargo run -- taoguba --users "2577911" --output ./dist/taoguba/`
4. 验证 `dist/taoguba/index.html` 包含用户链接
5. 运行 `cargo run -- xuangubao --output ./dist/xuangubao/`
6. 验证 `dist/xuangubao/limit-up.html` 包含涨停数据
7. 运行 `cargo run -- index --dir ./dist/`
8. 验证 `dist/index.html` 包含所有子页面链接
9. 本地启动静态服务器测试：`cd dist && python3 -m http.server 8080`
10. 验证子页面链接可点击，数据展示正确

## 后续扩展

- **定时自动生成**：GitHub Actions 定时运行脚本并推送
- **报告过期清理**：索引页只显示最近 N 篇报告
- **搜索功能**：前端添加报告搜索
- **数据对比**：跨时间段数据对比（本周 vs 上周）
