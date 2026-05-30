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
