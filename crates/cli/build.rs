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
                    // Last resort: read version from Cargo.toml (for Docker builds
                    // where no git history is available).
                    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").ok();
                    let version_from_toml = manifest_dir
                        .and_then(|dir| {
                            std::fs::read_to_string(std::path::Path::new(&dir).join("Cargo.toml"))
                                .ok()
                        })
                        .and_then(|s| {
                            s.lines()
                                .find(|l| l.starts_with("version"))
                                .and_then(|l| l.split('=').nth(1))
                                .map(|v| format!("v{}", v.trim().trim_matches('"')))
                        });
                    // 最终兜底：git 不可用且 Cargo.toml 也读不到时（例如源码被
                    // 裁剪到非标准布局的极端容器构建），编译期必须有一个字符串值。
                    // v0.0.0-dev 明确标识"无法确定版本"，而非伪造一个正常版本号，
                    // 调用方（doctor 报告、--version）会原样显示它，用户可感知降级。
                    // build script 无法返回 Err（会导致整个构建失败），故此处显式降级。
                    version_from_toml.unwrap_or_else(|| "v0.0.0-dev".to_string())
                })
        });

    println!("cargo:rustc-env=GIT_VERSION={version}");
    println!("cargo:rerun-if-env-changed=GIT_VERSION");

    // Watch git HEAD and tags so version updates on checkout / new tag.
    // CARGO_MANIFEST_DIR is set by Cargo; .git lives at the workspace root.
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
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
