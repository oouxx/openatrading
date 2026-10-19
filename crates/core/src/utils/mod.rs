pub mod time;

/// Parse YAML frontmatter from `---\n...\n---` delimited content.
/// Returns `Some((frontmatter_raw, body))` on success, `None` if no frontmatter found.
pub fn parse_yaml_frontmatter(raw: &str) -> Option<(String, String)> {
    let matter = gray_matter::Matter::<gray_matter::engine::YAML>::new();
    match matter.parse::<serde_json::Value>(raw) {
        Ok(parsed) if !parsed.matter.is_empty() => Some((parsed.matter, parsed.content)),
        _ => None,
    }
}

/// 百分号解码（`%XX`）。
///
/// 先收集原始字节再整体按 UTF-8 解码：中文在 URL 里是多个 `%XX` 字节
/// （`开` = `%E5%BC%80`），逐个 `byte as char` 只会得到三个 Latin-1 字符，
/// 搜索词、角色名之类的中文参数就变成乱码了。
///
/// 用于 path 片段；query 组件里的 `+` 表示空格，调用方自行处理（见 Tauri `extract_query`）。
/// 非法转义（如孤立的 `%`、非十六进制）原样保留，不猜也不丢字符。
pub fn percent_decode(input: &str) -> String {
    let mut bytes: Vec<u8> = Vec::with_capacity(input.len());
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            let mut buf = [0u8; 4];
            bytes.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            continue;
        }
        let hex: String = chars.by_ref().take(2).collect();
        match u8::from_str_radix(&hex, 16) {
            Ok(byte) => bytes.push(byte),
            Err(_) => {
                bytes.push(b'%');
                bytes.extend_from_slice(hex.as_bytes());
            }
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

#[cfg(test)]
mod tests {
    use super::percent_decode;

    #[test]
    fn decodes_ascii_and_multibyte() {
        assert_eq!(percent_decode("300272"), "300272");
        assert_eq!(percent_decode("SH600519"), "SH600519");
        // 「开能健康」
        assert_eq!(
            percent_decode("%E5%BC%80%E8%83%BD%E5%81%A5%E5%BA%B7"),
            "开能健康"
        );
        // `%2B` 是字面加号，不能被当成空格
        assert_eq!(percent_decode("a%2Bb"), "a+b");
    }

    #[test]
    fn keeps_malformed_escapes() {
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("50%zz"), "50%zz");
        assert_eq!(percent_decode("%E5%"), "\u{fffd}%");
    }

    #[test]
    fn mixed_escaped_and_plain() {
        assert_eq!(percent_decode("%E8%8C%85%E5%8F%B0-600519"), "茅台-600519");
    }
}

