//! Session file lookup utilities.
//!
//! Session lifecycle now uses the actor architecture:
//!
//! - `create_factory()` in `factory.rs` creates [`CreatedSession`] instances
//!   (owned `Arc<AgentSession>` + keep-alive services)
//! - `spawn_session_actor()` in `actor.rs` owns each session in a dedicated
//!   task (mailbox + event fan-out)
//! - `SessionPool` in `pool.rs` supervises the actors with LRU eviction
//!
//! This file retains only the session-file lookup helper that is shared
//! across the web and Tauri layers.

use std::path::PathBuf;

/// Find a session JSONL file by session ID in the given directory.
///
/// Session files are named `{timestamp}_{id}.jsonl`. This scans the directory
/// for a file whose name ends with `_{id}.jsonl`. Falls back to `{id}.jsonl`
/// for legacy naming.
pub fn find_session_file_by_id(session_dir: &str, id: &str) -> Option<String> {
    let dir = PathBuf::from(session_dir);
    // Legacy: direct {id}.jsonl
    let direct = dir.join(format!("{id}.jsonl"));
    if direct.exists() {
        return Some(direct.to_string_lossy().to_string());
    }
    // Current: {timestamp}_{id}.jsonl
    let suffix = format!("_{id}.jsonl");
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.ends_with(&suffix) {
                    return Some(path.to_string_lossy().to_string());
                }
            }
        }
    }
    None
}
