fn main() {
    // Allow override via env var (Docker build, CI), fallback to git describe,
    // then fallback to reading version from Cargo.toml.
    let version = std::env::var("GIT_VERSION")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| {
            std::process::Command::new("git")
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
                .unwrap_or_else(|| {
                    // Fallback for Docker builds with no git history: Cargo always
                    // sets CARGO_PKG_VERSION (resolved from `version.workspace = true`
                    // to the workspace package version), so prefer it over parsing
                    // Cargo.toml by hand.
                    std::env::var("CARGO_PKG_VERSION")
                        .ok()
                        .map(|v| format!("v{}", v))
                        // 最终兜底：极端容器构建里 CARGO_PKG_VERSION 也读不到时，
                        // v0.0.0-dev 明确标识"无法确定版本"，而非伪造正常版本号；
                        // build script 不能返回 Err（会让整个构建失败），故显式降级。
                        .unwrap_or_else(|| "v0.0.0-dev".to_string())
                })
        });

    println!("cargo:rustc-env=GIT_VERSION={version}");
    println!("cargo:rerun-if-env-changed=GIT_VERSION");

    // Watch git HEAD and tags so version updates on checkout / new tag.
    // CARGO_MANIFEST_DIR is set by Cargo; .git lives at the workspace root.
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|_| panic!("CARGO_MANIFEST_DIR must be set by Cargo"));
    let manifest_path = std::path::Path::new(&manifest_dir);
    let git_dir = std::fs::canonicalize(manifest_path.join("../../.git"))
        .unwrap_or_else(|_| manifest_path.join("../../.git"));

    // In a worktree, .git is a file containing "gitdir: <path>"
    // The path in gitdir: is always absolute, use it as-is.
    let git_dir = if git_dir.is_file() {
        let content = std::fs::read_to_string(&git_dir).unwrap_or_default();
        let path_str = content.strip_prefix("gitdir:").unwrap_or(&content).trim();
        std::path::PathBuf::from(path_str)
    } else {
        git_dir
    };

    let head = git_dir.join("HEAD");
    if head.exists() {
        println!("cargo:rerun-if-changed={}", head.display());
    }
    let tags = git_dir.join("refs/tags");
    if tags.exists() {
        println!("cargo:rerun-if-changed={}", tags.display());
    }
}
