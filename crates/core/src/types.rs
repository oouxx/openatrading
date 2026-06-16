use serde::{Deserialize, Serialize};

/// 股票代码（如 "000001"、"600519"）
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
pub struct StockCode(pub String);

impl StockCode {
    pub fn new(code: impl Into<String>) -> Self {
        Self(code.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for StockCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for StockCode {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for StockCode {
    fn from(s: String) -> Self {
        Self(s)
    }
}
