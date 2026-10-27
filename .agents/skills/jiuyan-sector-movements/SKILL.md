---
name: jiuyan-sector-movements
description: 获取并分析韭研公社盘中板块异动、题材轮动和资金攻击方向。用户提到韭研板块异动、盘中题材轮动、板块资金抱团或异动时间线时使用；不用于选股宝板块涨跌排行。
---

# 韭研板块异动

## 数据原则

- 数据来自韭研公社 `POST /api/v1/action/field`，不要改走 Rust Gateway。
- 先区分游客和登录态：`action/field` 通常需要 `SESSION` Cookie。
- 不绕过付费、登录或访问控制；拿不到数据时明确说明缺失。

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

`action/field` 通常需要登录态。自动生成的临时浏览器上下文没有你的登录 Cookie 时，需要临时提供：

```bash
export JIUYAN_SESSION_TOKEN="<browser SESSION cookie>"
```

已有 `JIUYAN_TIMESTAMP`/`JIUYAN_TOKEN` 时脚本会优先使用，不会启动浏览器。出现 `errCode=110` 时脚本会自动重新获取 token 并重试一次；仍然失败就停止并报告错误。不要把 token 写入文件、日志或仓库。

## 执行

先定位本 `SKILL.md` 所在目录（下文记为 `<skill-dir>`），然后运行：

```bash
python3 <skill-dir>/scripts/fetch_sector_movements.py \
  --date YYYY-MM-DD --compact
```

脚本会输出：

- 板块名称、异动数量和原因
- 板块内股票代码、名称
- `action_info`：时间、涨幅、连板信息、解读、价格等

接口细节见 `references/api.md`。

## 分析要求

- 按异动板块聚合，不要只罗列原始 JSON。
- 重点识别：反复出现的板块、同一板块内的扩散股票、强势股集中度、利好/利空一致性。
- 明确区分“上游返回事实”和“分析推断”。
- 没有数据或字段缺失时直接标注，不补造涨跌幅、连板数或原因。
- 默认用中文给出简洁结论：主线板块、核心个股、异动节奏和风险点。
