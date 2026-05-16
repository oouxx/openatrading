fn main() {
    let version = std::process::Command::new("git")
        .args(["describe", "--tags", "--abbrev=0"])
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
        .unwrap_or_else(|| "v0.0.0-dev".to_string());

    println!("cargo:rustc-env=GIT_VERSION={version}");
    println!("cargo:rerun-if-changed=.git/HEAD");
}
