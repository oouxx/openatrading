pub fn html_to_markdown(html: &str) -> String {
    // 第一步：将 <img> 标签替换为 Markdown 格式，保持原始位置
    let img_re = regex::Regex::new(r#"<img[^>]*>"#).unwrap();
    let html_with_md_images = img_re
        .replace_all(html, |caps: &regex::Captures| {
            let img_tag = &caps[0];
            // 优先使用 data-original（高清大图），其次 src
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

    // 第二步：将 <br> 和 <br/> 替换为换行符
    let br_re = regex::Regex::new(r#"<br\s*/?>"#).unwrap();
    let text_with_newlines = br_re.replace_all(&html_with_md_images, "\n");

    // 第三步：移除所有 HTML 标签，只保留文本
    let tag_re = regex::Regex::new(r#"<[^>]+>"#).unwrap();
    let text_only = tag_re.replace_all(&text_with_newlines, "");

    // 第四步：解码 HTML 实体
    let text_decoded = text_only
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"");

    // 第五步：清理多余空白行
    text_decoded
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}
