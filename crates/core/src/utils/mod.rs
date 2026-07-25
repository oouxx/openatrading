pub mod time;

/// Parse YAML frontmatter from `---\n...\n---` delimited content.
/// Returns `Some((frontmatter_raw, body))` on success, `None` if no frontmatter found.
#[must_use]
pub fn parse_yaml_frontmatter(raw: &str) -> Option<(String, String)> {
    let matter = gray_matter::Matter::<gray_matter::engine::YAML>::new();
    match matter.parse::<serde_json::Value>(raw) {
        Ok(parsed) if !parsed.matter.is_empty() => Some((parsed.matter, parsed.content)),
        _ => None,
    }
}
