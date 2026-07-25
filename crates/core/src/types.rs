use serde::{Deserialize, Deserializer, Serialize};

/// 已知的 Yahoo Finance 后缀，用于符号归一化
const YAHOO_SUFFIXES: &[&str] = &[".SS", ".SZ", ".HK", ".TW"];

/// 去掉已知 Yahoo 后缀（大小写不敏感），统一为裸代码格式
fn strip_yahoo_suffix(s: &str) -> String {
    let upper = s.to_uppercase();
    for &suf in YAHOO_SUFFIXES {
        if let Some(rest) = upper.strip_suffix(suf) {
            return rest.to_string();
        }
    }
    s.to_string()
}

/// 股票代码（如 "000001"、"600519"）
/// 构造时自动去掉 `.SS`/`.SZ`/`.HK`/`.TW` 后缀，统一为裸代码格式。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, schemars::JsonSchema)]
pub struct StockCode(pub String);

impl<'de> Deserialize<'de> for StockCode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Self::new(s))
    }
}

impl StockCode {
    pub fn new(code: impl Into<String>) -> Self {
        Self(strip_yahoo_suffix(&code.into()))
    }

    #[must_use]
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
        Self::new(s)
    }
}

impl From<String> for StockCode {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}
