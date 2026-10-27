---
name: jiuyan-market-intelligence
description: 获取并分析韭研公社社区文章、热门股票和讨论热度。用户提到市场情报、社区舆情、韭研热帖、题材讨论度或热点扩散时使用；不用于实时行情快照。
---

# 韭研市场情报

## 数据原则

- 数据来自韭研公社社区文章接口，不要改走 Rust Gateway。
- 公开文章可直接读取；遇到付费、权限或登录限制时明确说明，不绕过限制。
- 这是社区讨论样本，不等同于全市场事实或确定交易信号。

## 获取请求凭证

Skill 自带 `scripts/get_token.mjs`，会在需要时启动系统 Chrome 的 headless CDP，截获：
- `JIUYAN_TIMESTAMP`
- `JIUYAN_TOKEN`
- 如果当前浏览器上下文有 `SESSION` Cookie，也返回 `session_token`

不需要 Playwright，也不依赖仓库外的脚本。要求 Node 22+ 和本机 Chrome/Chromium。

如果自动发现 Chrome 失败，设置：

```bash
export JIUYAN_CHROME_PATH="/path/to/chrome-or-chromium"
```

需要登录内容但自动返回的 `session_token` 为空时，再临时提供：

```bash
export JIUYAN_SESSION_TOKEN="<browser SESSION cookie>"
```

已有 `JIUYAN_TIMESTAMP`/`JIUYAN_TOKEN` 时脚本会优先使用，不会启动浏览器。出现 `errCode=110` 时脚本会自动重新获取 token 并重试一次；仍然失败就停止并报告错误。不要把 token 写入文件、日志或仓库。

## 执行

先定位本 `SKILL.md` 所在目录（下文记为 `<skill-dir>`），然后运行：

```bash
python3 <skill-dir>/scripts/fetch_market_intelligence.py \
  --pages 2 --page-size 20 --compact
```

脚本会汇总：

- 社区文章标题、作者、互动计数、发布时间
- 关联股票列表
- 全样本股票提及次数

接口细节见 `references/api.md`。

## 分析要求

- 按股票提及次数、互动量和跨文章重复度识别社区热点。
- 区分高质量观点、资讯转载、标题党和付费摘要。
- 将“市场事实”“作者观点”“你的归纳”分开表达。
- 不根据单一帖子直接下结论；说明样本页数和时间范围。
- 默认输出：热点股票、主要题材、情绪倾向、分歧点、需要继续验证的事项。
