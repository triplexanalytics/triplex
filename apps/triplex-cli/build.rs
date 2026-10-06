//! Bakes the version `triplex --version` prints into the binary. The
//! version is the latest `workspace-v*` git tag, not a field in
//! `Cargo.toml`; a build a few commits past a tag reads like
//! `0.33.0-4-g608e295`.

use std::process::Command;

fn main() {
    // ponytail: no rerun-if-changed, so Cargo reruns this only when a file in
    // this crate changes; an incremental build after a new tag alone keeps the
    // old string. A source build without git or tags (tarball, Docker without
    // .git) reports 0.0.0-untagged; pass the version in when packaging exists.
    let version = Command::new("git")
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
        .unwrap_or_else(|| "0.0.0-untagged".to_string());
    println!("cargo:rustc-env=TRIPLEX_VERSION={version}");
}
