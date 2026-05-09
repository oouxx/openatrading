# Design System

设计方向：

Bloomberg Terminal
+
A股游资复盘
+
AI Dashboard

---

# 色彩规则

上涨：红
下跌：绿
中性：灰
AI：紫

必须严格遵守 A 股规则。

---

# 颜色 Token

```css
--color-bg: #0d0d1a;
--color-surface: #111124;
--color-up: #FF5C5C;
--color-down: #1D9E75;
--color-neutral: #888780;
--color-ai: #7F77DD;

# Typography

金融数字必须：

- tabular-nums
- 等宽数字

层级：

- KPI
- 情绪
- 龙头
- 风险

必须明显区分。

---

# UI 原则

1. 高信息密度
2. 低视觉噪音
3. AI 感
4. 专业终端感
5. 强层级感
6. 模块联动感

---

# 卡片系统

卡片不能平均化。

市场状态卡必须最高优先级。

风险卡必须明显。

AI 卡必须具有独立视觉语言。

---

# 禁止事项

禁止：

- 白色背景 / 白色金融网页风格
- 娱乐化动画 / 卡通化 UI
- 普通后台风格 / SaaS Dashboard 风格
- ToB SaaS 风格
- 低密度留白设计 / 过度动画

禁止做的事：

- 传统 F10 页面
- 完整券商交易系统
- 资讯门户 / 纯资讯聚合
- 大而全行情系统
- 传统量化平台
- TradingView 作为核心终端