//! Bake the current git branch + short hash into the binary so the app can show
//! "branch · vX.Y.Z" in its footer. Falls back to empty strings off a git tree.
use std::process::Command;

fn git(args: &[&str]) -> String {
    Command::new("git")
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

fn main() {
    let branch = git(&["rev-parse", "--abbrev-ref", "HEAD"]);
    let hash = git(&["rev-parse", "--short", "HEAD"]);
    println!("cargo:rustc-env=GIT_BRANCH={branch}");
    println!("cargo:rustc-env=GIT_HASH={hash}");
    // Re-run if the checked-out commit/branch changes.
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs");
}
