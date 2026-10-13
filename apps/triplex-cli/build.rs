//! Bakes the version `triplex --version` prints into the binary. The
//! version is the latest `workspace-v*` git tag, not a field in
//! `Cargo.toml`; a build a few commits past a tag reads like
//! `0.33.0-4-g608e295`. A build without git history (the Docker image) passes
//! the version in through the `TRIPLEX_VERSION` environment variable.

use std::process::Command;

fn main() {
    // ponytail: no rerun-if-changed, so Cargo reruns this only when a file in
    // this crate changes; an incremental build after a new tag alone keeps the
    // old string. A source build without git or tags and without
    // TRIPLEX_VERSION (a tarball) reports 0.0.0-untagged.
    let from_git = || {
        Command::new("git")
            .args(["describe", "--tags", "--match", "workspace-v*"])
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| {
                String::from_utf8_lossy(&out.stdout)
                    .trim()
                    .trim_start_matches("workspace-v")
                    .to_string()
            })
    };
    let version = std::env::var("TRIPLEX_VERSION")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(from_git)
        .unwrap_or_else(|| "0.0.0-untagged".to_string());
    println!("cargo:rustc-env=TRIPLEX_VERSION={version}");
}
