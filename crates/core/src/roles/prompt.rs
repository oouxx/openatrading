pub struct Prompt {
    pub system: String,
    pub user: String,
}

/// 构建 LLM prompt
/// - `strategy_text`: 角色策略文本（base.md ± supplement.md 拼装后的结果）
/// - `market_data_json`: 市场数据，JSON 字符串
/// - `display_name`: 角色中文名
pub fn build_prompt(strategy_text: &str, market_data_json: &str, display_name: &str) -> Prompt {
    Prompt {
        system: strategy_text.to_string(),
        user: format!(
            r#"## 今日市场数据

```json
{market_data}
```

请以【{name}】身份，用简洁专业的语言分析：
1. 今日市场情绪定性（偏强/震荡/偏弱）
2. 值得关注的机会或风险
3. 明日操作建议

控制在150字以内，直接给结论。"#,
            market_data = market_data_json,
            name = display_name,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_prompt_structure() {
        let prompt = build_prompt("策略正文", r#"{"rise": 3000}"#, "测试角色");
        assert_eq!(prompt.system, "策略正文");
        assert!(prompt.user.contains("测试角色"));
        assert!(prompt.user.contains(r#""rise": 3000"#));
    }
}
