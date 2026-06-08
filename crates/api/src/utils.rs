use chrono::{Datelike, FixedOffset};

/// Get current datetime in Beijing timezone (UTC+8).
pub fn beijing_now() -> chrono::DateTime<chrono::FixedOffset> {
    chrono::Utc::now().with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap())
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
