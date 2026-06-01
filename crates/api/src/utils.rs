use chrono::{Datelike, FixedOffset};

/// Get current datetime in Beijing timezone (UTC+8).
pub fn beijing_now() -> chrono::DateTime<chrono::FixedOffset> {
    chrono::Utc::now().with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap())
}

pub fn html_to_markdown(html: &str) -> String {
    let img_re = regex::Regex::new(r#"<img[^>]*>"#).unwrap();
    let html_with_md_images = img_re
        .replace_all(html, |caps: &regex::Captures| {
            let img_tag = &caps[0];
            let data_original_re = regex::Regex::new(r#"data-original="([^"]*)""#).unwrap();
            let src_re = regex::Regex::new(r#"src="([^"]*)""#).unwrap();
            let src = data_original_re
                .captures(img_tag)
                .and_then(|c| c.get(1).map(|m| m.as_str()))
                .or_else(|| {
                    src_re
                        .captures(img_tag)
                        .and_then(|c| c.get(1).map(|m| m.as_str()))
                })
                .unwrap_or("");
            if src.is_empty() || src.starts_with("data:") || src.contains("placeHolder") {
                String::new()
            } else {
                format!("\n![image]({})\n", src)
            }
        })
        .to_string();

    let br_re = regex::Regex::new(r#"<br\s*/?>"#).unwrap();
    let text_with_newlines = br_re.replace_all(&html_with_md_images, "\n");

    let tag_re = regex::Regex::new(r#"<[^>]+>"#).unwrap();
    let text_only = tag_re.replace_all(&text_with_newlines, "");

    text_only
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
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
