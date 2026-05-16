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
}
