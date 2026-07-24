/// Get current datetime in Beijing timezone (UTC+8).
#[must_use] 
pub fn beijing_now() -> chrono::DateTime<chrono::FixedOffset> {
    chrono::Utc::now().with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap())
}
