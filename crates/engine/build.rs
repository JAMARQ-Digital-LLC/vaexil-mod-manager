//! Decides whether this is the release build (see `src/scratch.rs`): a
//! release-profile build made by GitHub Actions (release.yml, main-build.yml)
//! or from the shared checkout `...\JAMARQ\Apps\vaexil-mod-manager`. Every
//! other build, a worktree's included, is a dev build.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=GITHUB_ACTIONS");
    let release_profile = std::env::var("PROFILE").is_ok_and(|p| p == "release");
    let ci = std::env::var("GITHUB_ACTIONS").is_ok_and(|v| v == "true");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let root = std::fs::canonicalize(&root).unwrap_or(root);
    let root = root.to_string_lossy().replace('/', "\\").to_lowercase();
    let shared = root.trim_end_matches('\\').ends_with(r"\jamarq\apps\vaexil-mod-manager");
    if release_profile && (ci || shared) {
        println!("cargo:rustc-env=VMM_RELEASE_IDENTITY=1");
    }
}
