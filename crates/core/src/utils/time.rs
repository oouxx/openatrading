/// Returns the current time in Beijing timezone (UTC+8).
///
/// # Panics
/// Panics if the system clock is unavailable or the timezone offset is invalid.
#[must_use]
pub fn beijing_now() -> chrono::DateTime<chrono::FixedOffset> {
    chrono::Utc::now().with_timezone(
        &chrono::FixedOffset::east_opt(8 * 3600)
            .unwrap_or_else(|| panic!("UTC+8 offset should always be valid")),
    )
}
