use serde::Serialize;

pub struct Prompt {
    pub system: String,
    pub user: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

pub fn default_tools() -> Vec<ToolDef> {
    vec![ToolDef {
        name: "query_market_memory".into(),
        description: "查询近N日市场历史数据，用于对比分析".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "days": {
                    "type": "integer",
                    "description": "查询天数，默认5天",
                    "default": 5
                },
                "source": {
                    "type": "string",
                    "enum": ["xuangubao", "jiuyan", "cls", "sentiment", "all"],
                    "description": "数据源，all表示所有",
                    "default": "all"
                }
            }
        }),
    }]
}

/// system = 策略 + 持仓 + 工具 + 核心原则
/// user = 市场数据 + 分析指令
pub fn build_prompt(
    strategy_text: &str,
    market_data_json: &str,
    position_context: &str,
    available_tools: &[ToolDef],
    display_name: &str,
) -> Prompt {
    let tools_json = serde_json::to_string_pretty(available_tools).unwrap_or_default();

    Prompt {
        system: format!(
            r#"{strategy}

## 当前持仓
{positions}

## 可用工具
```json
{tools}
```

## 核心原则
- 红涨绿跌（A股规则）
- 严格遵守上述策略体系进行交易决策
- 结合持仓情况，给出明确的加仓、减仓、持有、清仓建议及理由
- 用数据和逻辑说话，不预测具体涨跌点位"#,
            strategy = strategy_text,
            positions = position_context,
            tools = tools_json,
        ),
        user: format!(
            r#"## 今日市场数据

```json
{market_data}
```

请以【{name}】身份，根据策略体系分析当前市场状态和持仓情况，给出明确的持仓调整建议。
需要历史数据对比时可调用 query_market_memory。"#,
            market_data = market_data_json,
            name = display_name,
        ),
    }
}

/// Build prompt for automated decision flow (cron).
/// Requires LLM to output structured JSON trade instructions.
pub fn build_decision_prompt(
    strategy_text: &str,
    market_context: &str,
    position_context: &str,
    historical_context: &str,
    display_name: &str,
) -> Prompt {
    Prompt {
        system: format!(
            r#"{strategy}

## 核心原则
- 红涨绿跌（A股规则）
- 严格遵守上述策略体系进行交易决策
- 结合持仓情况，给出明确的操作建议
- 用数据和逻辑说话，不预测具体涨跌点位

## 输出格式要求
你必须严格按照以下 JSON 格式输出，不要输出其他内容。外层用 ```json ``` 包裹：

```json
{{
  "analysis": "市场分析和复盘（不超过200字）",
  "trades": [
    {{
      "action": "buy|sell|hold",
      "symbol": "股票代码，如 000001.SZ",
      "name": "股票名称",
      "shares": 100,
      "reason": "操作理由（简短）"
    }}
  ]
}}
```

规则：
- action 只能是 buy（开仓）、sell（清仓）、hold（持有不动）
- buy: 必须填写 symbol、name、shares（100的整数倍）、reason
- sell: 必须填写 symbol、name、reason；shares 填 0（系统自动清全部持仓）
- hold: 只需填写 symbol、name、reason；表示维持当前持仓不变
- 如果没有任何操作建议，trades 输出空数组 []
- shares 数量要合理，初始资金 1,000,000 元，注意仓位控制
- 不要建议买入 ST 股票"#,
            strategy = strategy_text,
        ),
        user: format!(
            r#"{market}

## 当前持仓
{positions}

## 近5日历史数据
{historical}

请以【{name}】身份，根据策略体系分析当前市场状态和持仓情况，给出今日复盘和明日操作建议。严格按照 JSON 格式输出。"#,
            market = market_context,
            positions = position_context,
            historical = historical_context,
            name = display_name,
        ),
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TradeInstruction {
    pub action: String,
    pub symbol: String,
    pub name: String,
    #[serde(default)]
    pub shares: i64,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct DecisionOutput {
    pub analysis: String,
    #[serde(default)]
    pub trades: Vec<TradeInstruction>,
}

/// Parse LLM response into structured DecisionOutput.
/// Extracts JSON from markdown code blocks if present.
pub fn parse_decision_output(raw: &str) -> Option<DecisionOutput> {
    let json_str = extract_json_block(raw).unwrap_or_else(|| raw.to_string());

    match serde_json::from_str::<DecisionOutput>(&json_str) {
        Ok(d) => Some(d),
        Err(e) => {
            tracing::warn!(
                "[decision] JSON parse error: {e}, raw: {}",
                &raw[..raw.len().min(500)]
            );
            None
        },
    }
}

fn extract_json_block(text: &str) -> Option<String> {
    let start_marker = "```json";
    let end_marker = "```";

    let mut search_from = 0;
    while let Some(start_idx) = text[search_from..].find(start_marker) {
        let abs_start = search_from + start_idx;
        let content_start = abs_start + start_marker.len();
        if let Some(end_idx) = text[content_start..].find(end_marker) {
            let candidate = text[content_start..content_start + end_idx]
                .trim()
                .to_string();
            if candidate.starts_with('{') {
                return Some(candidate);
            }
            search_from = content_start + end_idx + end_marker.len();
        } else {
            break;
        }
    }

    let mut search_from = 0;
    while let Some(start_idx) = text[search_from..].find("```") {
        let abs_start = search_from + start_idx;
        let content_start = abs_start + 3;
        if let Some(end_idx) = text[content_start..].find("```") {
            let candidate = text[content_start..content_start + end_idx]
                .trim()
                .to_string();
            if candidate.starts_with('{') {
                return Some(candidate);
            }
            search_from = content_start + end_idx + 3;
        } else {
            break;
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_prompt_with_positions() {
        let tools = default_tools();
        let prompt = build_prompt(
            "策略正文",
            r#"{"rise": 3000}"#,
            "- #1 000001 平安银行 | 成本10.5",
            &tools,
            "测试角色",
        );
        assert!(prompt.system.contains("策略正文"));
        assert!(prompt.system.contains("当前持仓"));
        assert!(prompt.system.contains("000001"));
        assert!(prompt.system.contains("query_market_memory"));
        assert!(prompt.system.contains("红涨绿跌"));
        assert!(prompt.user.contains("测试角色"));
        assert!(prompt.user.contains("query_market_memory"));
    }

    #[test]
    fn test_build_prompt_empty_positions() {
        let tools = default_tools();
        let prompt = build_prompt("策略", "{}", "暂无持仓", &tools, "测试");
        assert!(prompt.system.contains("暂无持仓"));
    }

    #[test]
    fn test_build_decision_prompt() {
        let prompt = build_decision_prompt(
            "策略正文",
            "## 市场数据\n上涨3000家",
            "- #1 000001 平安银行 | 成本10.5",
            "近5日数据...",
            "zarili",
        );
        assert!(prompt.system.contains("策略正文"));
        assert!(prompt.system.contains("JSON"));
        assert!(prompt.system.contains("buy|sell|hold"));
        assert!(prompt.user.contains("zarili"));
        assert!(prompt.user.contains("近5日数据"));
    }

    #[test]
    fn test_parse_decision_output_json_block() {
        let raw = r#"分析完毕。

```json
{
  "analysis": "市场情绪偏暖，可适当加仓",
  "trades": [
    {
      "action": "buy",
      "symbol": "000001.SZ",
      "name": "平安银行",
      "shares": 1000,
      "reason": "量能放大突破5日线"
    }
  ]
}
```

以上是操作建议。"#;
        let output = parse_decision_output(raw).unwrap();
        assert_eq!(output.analysis, "市场情绪偏暖，可适当加仓");
        assert_eq!(output.trades.len(), 1);
        assert_eq!(output.trades[0].action, "buy");
        assert_eq!(output.trades[0].symbol, "000001.SZ");
        assert_eq!(output.trades[0].shares, 1000);
    }

    #[test]
    fn test_parse_decision_output_plain_json() {
        let raw = r#"{"analysis": "空仓观望", "trades": []}"#;
        let output = parse_decision_output(raw).unwrap();
        assert_eq!(output.analysis, "空仓观望");
        assert!(output.trades.is_empty());
    }

    #[test]
    fn test_parse_decision_output_invalid() {
        let raw = "这是一段普通文本，没有JSON";
        assert!(parse_decision_output(raw).is_none());
    }

    #[test]
    fn test_parse_decision_output_sell() {
        let raw = r#"```json
{
  "analysis": "风险偏高，减仓",
  "trades": [
    {"action": "sell", "symbol": "600519.SH", "name": "贵州茅台", "shares": 0, "reason": "高位放量滞涨"}
  ]
}
```"#;
        let output = parse_decision_output(raw).unwrap();
        assert_eq!(output.trades[0].action, "sell");
        assert_eq!(output.trades[0].shares, 0);
    }
}
