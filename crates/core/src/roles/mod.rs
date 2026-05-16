use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct RoleConfig {
    pub name: String,
    pub display_name: String,
    pub dir: String,
    pub merge_mode: MergeMode,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeMode {
    Supplement,
    Override,
    BaseOnly,
}

pub mod prompt;
pub mod role;

pub const BUILTIN_STRATEGIES: &[(&str, &str, &str)] = &[
    (
        "zarili",
        "zarili量价派",
        include_str!("../../strategies/zarili/base.md"),
    ),
    (
        "trader",
        "游资操盘手",
        include_str!("../../strategies/trader/base.md"),
    ),
    (
        "quant",
        "量化分析师",
        include_str!("../../strategies/quant/base.md"),
    ),
    (
        "risk_officer",
        "风控官",
        include_str!("../../strategies/risk_officer/base.md"),
    ),
];

pub fn get_builtin_strategy(name: &str) -> Option<&'static str> {
    BUILTIN_STRATEGIES
        .iter()
        .find(|(n, _, _)| *n == name)
        .map(|(_, _, content)| *content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_builtin_strategy_exists() {
        let s = get_builtin_strategy("trader");
        assert!(s.is_some());
        assert!(s.unwrap().contains("游资操盘手"));
    }

    #[test]
    fn test_get_builtin_strategy_not_found() {
        assert!(get_builtin_strategy("nonexistent").is_none());
    }

    #[test]
    fn test_merge_mode_deserialize() {
        let config: RoleConfig = toml::from_str(
            r#"name = "test"
display_name = "测试"
dir = "strategies/test"
merge_mode = "supplement"
"#,
        )
        .unwrap();
        assert!(matches!(config.merge_mode, MergeMode::Supplement));
    }
}
