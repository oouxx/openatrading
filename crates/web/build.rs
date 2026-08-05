fn main() {
    // ── Git commit short hash ──────────────────────────────────────────
    // Get the short hash of the commit that the current tag points to.
    // Falls back to HEAD short hash if not on a tag.
    let commit_short = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        })
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=GIT_COMMIT_SHORT={commit_short}");
    println!("cargo:rerun-if-env-changed=GIT_COMMIT_SHORT");

    // ── pi-rs engine version ───────────────────────────────────────────
    // Parse from Cargo.lock at workspace root: look for pi-coding-agent's
    // git source tag (e.g. "tag=v1.80.3").
    let pi_rs_version = std::env::current_dir()
        .ok()
        .and_then(|cwd| {
            // Walk up to find Cargo.lock (workspace root)
            let mut dir = cwd;
            loop {
                let lock = dir.join("Cargo.lock");
                if lock.exists() {
                    return Some(lock);
                }
                if !dir.pop() {
                    return None;
                }
            }
        })
        .and_then(|lock_path| {
            let content = std::fs::read_to_string(&lock_path).ok()?;
            // Find pi-coding-agent entry and extract tag from source
            let lines: Vec<&str> = content.lines().collect();
            for (i, line) in lines.iter().enumerate() {
                if line.trim() == "name = \"pi-coding-agent\"" {
                    // Look for source line in subsequent lines
                    for l in lines[i..].iter().take(10) {
                        let l = l.trim();
                        if l.starts_with("source = \"git+") {
                            if let Some(tag_start) = l.find("tag=") {
                                let tag_val = &l[tag_start + 4..];
                                if let Some(end) = tag_val.find(['#', '"']) {
                                    let ver = &tag_val[..end];
                                    return Some(ver.to_string());
                                }
                            }
                        }
                    }
                }
            }
            None
        })
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=PI_RS_VERSION={pi_rs_version}");
    println!("cargo:rerun-if-env-changed=PI_RS_VERSION");
    println!("cargo:rerun-if-changed=Cargo.lock");
}
