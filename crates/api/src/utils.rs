use chrono::{Datelike, FixedOffset, Utc};
use md5;
/// Get current datetime in Beijing timezone (UTC+8).
pub fn beijing_now() -> chrono::DateTime<chrono::FixedOffset> {
    chrono::Utc::now().with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap())
}

pub fn now_ts() -> i64 {
    // 1. 北京时区（UTC+8）
    let beijing = FixedOffset::east_opt(8 * 3600).unwrap();

    // 2. 当前时间（UTC -> 转北京时间）
    let now_utc = Utc::now();
    let now_bj = now_utc.with_timezone(&beijing);

    // 3. 转成毫秒时间戳（注意：timestamp_millis 是 UTC epoch）

    now_bj.timestamp_millis()
}

pub fn gen_jiuyan_token(timestamp_ms: i64) -> String {
    let secret = "REDACTED-JIUYAN-SECRET";
    let input = format!("{}:{}", secret, timestamp_ms);
    // 3. MD5 计算
    let token = format!("{:x}", md5::compute(input));
    token
}
pub fn html_to_markdown(html: &str) -> String {
    // Some Chinese sites use data-original for lazy-loaded images
    let html = regex::Regex::new(r#"data-original="([^"]*)""#)
        .unwrap()
        .replace_all(html, |caps: &regex::Captures| {
            format!("src=\"{}\"", &caps[1])
        });
    html2text::from_read(html.as_bytes(), usize::MAX)
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn last_n_trading_days(n: usize) -> Vec<String> {
    let mut dates = Vec::with_capacity(n);
    let mut current = chrono::Utc::now().with_timezone(&beijing_tz()).date_naive();
    // If today is a weekend, start from Friday
    while dates.len() < n {
        match current.weekday() {
            chrono::Weekday::Sat => {
                current = current.pred_opt().unwrap();
                continue;
            },
            chrono::Weekday::Sun => {
                current = current.pred_opt().unwrap();
                continue;
            },
            _ => {
                dates.push(current.format("%Y%m%d").to_string());
                current = current.pred_opt().unwrap();
            },
        }
    }
    dates
}

fn beijing_tz() -> FixedOffset {
    FixedOffset::east_opt(8 * 3600).unwrap()
}

use anyhow::{bail, Result};

/// 将 A 股股票代码规范化为 Yahoo Finance symbol
pub fn to_yahoo_symbol(input: &str) -> Result<String> {
    let input = input.trim();

    // 1. 已经是 Yahoo 格式（含 .SS / .SZ / .HK 等）
    if let Some((code, suffix)) = input.split_once('.') {
        let suffix_upper = suffix.to_uppercase();
        match suffix_upper.as_str() {
            "SS" | "SZ" | "HK" | "TW" => {
                let normalized_code = normalize_code(code)?;
                return Ok(format!("{}.{}", normalized_code, suffix_upper));
            },
            _ => {},
        }
    }

    // 2. 带交易所前缀，如 SH600000 / SZ000001 / sh.600000 / sz.000001
    let (maybe_exchange, raw_code) = parse_prefixed(input);

    // 3. 规范化为6位纯数字
    let code = normalize_code(raw_code)?;

    // 4. 确定交易所后缀
    let suffix = if let Some(ex) = maybe_exchange {
        match ex.to_uppercase().as_str() {
            "SH" | "SS" => "SS",
            "SZ" => "SZ",
            _ => bail!("未知交易所前缀: {ex}"),
        }
    } else {
        infer_exchange(&code)?
    };

    Ok(format!("{code}.{suffix}"))
}

fn parse_prefixed(input: &str) -> (Option<&str>, &str) {
    let upper = input.to_uppercase();
    for prefix in ["SH", "SZ", "SS"] {
        if upper.starts_with(prefix) {
            let rest = &input[prefix.len()..];
            let rest = rest.trim_start_matches(['.', '-', '_']);
            return (Some(&input[..prefix.len()]), rest);
        }
    }
    (None, input)
}

fn normalize_code(raw: &str) -> Result<String> {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        bail!("输入不含任何数字: {:?}", raw);
    }
    if digits.len() > 6 {
        bail!("代码位数超过6位: {}", digits);
    }
    Ok(format!("{:0>6}", digits))
}

/// 根据 A 股/场内基金代码推断所属交易所
///
/// 返回值：
/// - "SS" 上海证券交易所（主板、科创板、ETF/LOF、债券等）
/// - "SZ" 深圳证券交易所（主板、创业板、ETF/LOF、可转债等）
/// - "BJ" 北京证券交易所
fn infer_exchange(code: &str) -> Result<&'static str> {
    if code.is_empty() {
        bail!("空代码");
    }
    if code.len() < 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
        bail!("代码格式不正确，需为 6 位数字代码: '{code}'");
    }

    let p2 = &code[0..2];
    let p3 = &code[0..3];

    // 北交所：83/87/88 开头的存量代码，920 开头为 2023 年后新股代码段
    if matches!(p2, "83" | "87" | "88") || p3 == "920" {
        return Ok("BJ");
    }

    // 上海证券交易所
    let is_sh = matches!(
        p3,
        "600" | "601" | "603" | "605"          // 主板
            | "688"                             // 科创板
            | "900"                             // B股
            | "510" | "511" | "512" | "513"
            | "515" | "516" | "517" | "518"
            | "519" | "520" | "588"              // ETF / LOF
            | "500" | "501" | "502" // 其他场内基金
    ) || matches!(p2, "10" | "11"); // 国债/公司债/可转债

    if is_sh {
        return Ok("SS");
    }

    // 深圳证券交易所
    let is_sz = matches!(
        p3,
        "000" | "001" | "002" | "003"          // 主板
            | "300" | "301"                     // 创业板
            | "200"                              // B股
            | "150" | "151" | "152" | "159"
            | "160" | "161" | "162" | "163"
            | "164" | "165" | "166" | "167"
            | "168" | "184"                       // ETF / LOF
            | "123" | "127" | "128" | "129"
            | "148" | "149" // 可转债
    );

    if is_sz {
        return Ok("SZ");
    }

    bail!("无法推断交易所，代码 '{code}' 不在已知前缀范围内")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_codes() {
        assert_eq!(to_yahoo_symbol("600000").unwrap(), "600000.SS"); // 浦发银行
        assert_eq!(to_yahoo_symbol("000001").unwrap(), "000001.SZ"); // 平安银行
        assert_eq!(to_yahoo_symbol("300750").unwrap(), "300750.SZ"); // 宁德时代
        assert_eq!(to_yahoo_symbol("688981").unwrap(), "688981.SS"); // 中芯国际
        assert_eq!(to_yahoo_symbol("200001").unwrap(), "200001.SZ"); // 深市B股
        assert_eq!(to_yahoo_symbol("900901").unwrap(), "900901.SS"); // 沪市B股
    }

    #[test]
    fn test_pad_zeros() {
        assert_eq!(to_yahoo_symbol("1").unwrap(), "000001.SZ");
        assert_eq!(to_yahoo_symbol("001").unwrap(), "000001.SZ");
        assert_eq!(to_yahoo_symbol("6000").unwrap(), "006000.SZ"); // 首位0，深市
    }

    #[test]
    fn test_with_exchange_prefix() {
        assert_eq!(to_yahoo_symbol("SH600000").unwrap(), "600000.SS");
        assert_eq!(to_yahoo_symbol("SZ000001").unwrap(), "000001.SZ");
        assert_eq!(to_yahoo_symbol("sh600000").unwrap(), "600000.SS");
        assert_eq!(to_yahoo_symbol("sz000001").unwrap(), "000001.SZ");
        assert_eq!(to_yahoo_symbol("sh.600000").unwrap(), "600000.SS");
        assert_eq!(to_yahoo_symbol("sz.000001").unwrap(), "000001.SZ");
        assert_eq!(to_yahoo_symbol("SH-600000").unwrap(), "600000.SS");
        assert_eq!(to_yahoo_symbol("SZ_000001").unwrap(), "000001.SZ");
    }

    #[test]
    fn test_already_yahoo_format() {
        assert_eq!(to_yahoo_symbol("600000.SS").unwrap(), "600000.SS");
        assert_eq!(to_yahoo_symbol("000001.SZ").unwrap(), "000001.SZ");
        assert_eq!(to_yahoo_symbol("600000.ss").unwrap(), "600000.SS"); // 小写后缀
        assert_eq!(to_yahoo_symbol("00700.HK").unwrap(), "000700.HK"); // 港股补零
    }

    #[test]
    fn test_whitespace() {
        assert_eq!(to_yahoo_symbol("  600000  ").unwrap(), "600000.SS");
        assert_eq!(to_yahoo_symbol("\t000001\n").unwrap(), "000001.SZ");
    }

    #[test]
    fn test_errors() {
        assert!(to_yahoo_symbol("abc").is_err()); // 无数字
        assert!(to_yahoo_symbol("12345678").is_err()); // 超过6位
        assert!(to_yahoo_symbol("700000").is_err()); // 首位7无法推断
        assert!(to_yahoo_symbol("500000").is_err()); // 首位5无法推断
        assert!(to_yahoo_symbol("").is_err()); // 空字符串
    }

    #[test]
    fn test_error_messages() {
        let err = to_yahoo_symbol("abc").unwrap_err();
        assert!(err.to_string().contains("不含任何数字"));

        let err = to_yahoo_symbol("700000").unwrap_err();
        assert!(err.to_string().contains("无法推断交易所"));

        let err = to_yahoo_symbol("1234567").unwrap_err();
        assert!(err.to_string().contains("超过6位"));
    }
}
