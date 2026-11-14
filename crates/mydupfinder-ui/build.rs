//! Exports git information for the in-app version string
//! (`MYDUPFINDER_GIT_DESCRIBE`, `MYDUPFINDER_GIT_SHA`) and embeds the application
//! icon and version information into the Windows executable (a no-op on every
//! other host OS).

use std::path::PathBuf;
use std::process::Command;

/// Value used when no git information is available.
const UNKNOWN: &str = "unknown";

/// Run `git <args>` and return its trimmed stdout, or `None` on any failure.
fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8(out.stdout).ok()?;
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// Absolute path printed by `git rev-parse <flag>` (git prints it relative to the
/// current directory, which is the crate directory here).
fn git_path(flag: &str) -> Option<PathBuf> {
    let p = PathBuf::from(git(&["rev-parse", flag])?);
    if p.is_absolute() {
        Some(p)
    } else {
        std::env::current_dir().ok().map(|cwd| cwd.join(p))
    }
}

/// Ask cargo to rerun when `HEAD`, the ref it points to, the index or a
/// work-tree source changes.
/// In a worktree `HEAD` and the index live in the worktree git dir, while
/// branch refs and `packed-refs` live in the common git dir.
fn watch_git() {
    let (Some(git_dir), Some(common_dir)) = (git_path("--git-dir"), git_path("--git-common-dir"))
    else {
        return;
    };
    let head = git_dir.join("HEAD");
    println!("cargo:rerun-if-changed={}", head.display());
    println!("cargo:rerun-if-changed={}", git_dir.join("index").display());
    println!(
        "cargo:rerun-if-changed={}",
        common_dir.join("packed-refs").display()
    );
    if let Ok(content) = std::fs::read_to_string(&head)
        && let Some(r) = content.trim().strip_prefix("ref:")
    {
        println!(
            "cargo:rerun-if-changed={}",
            common_dir.join(r.trim()).display()
        );
    }
    watch_work_tree();
}

/// Work-tree inputs of the binary, relative to the repository root.
/// Directories are scanned recursively by cargo; `target/` must stay out of
/// this list, or every build would rerun the script.
const WORK_TREE_INPUTS: &[&str] = &[
    "crates",
    "assets",
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
];

/// Ask cargo to rerun when a tracked source changes in the work tree. An
/// unstaged edit touches neither `HEAD` nor the index, so without this the
/// binary would be recompiled with a stale `--dirty` result (and reverting the
/// edit would leave a stale `.dirty`).
fn watch_work_tree() {
    let Some(top) = git(&["rev-parse", "--show-toplevel"]).map(PathBuf::from) else {
        return;
    };
    for input in WORK_TREE_INPUTS {
        println!("cargo:rerun-if-changed={}", top.join(input).display());
    }
}

/// Export the git describe string and short sha as compile-time env vars.
fn export_git_info() {
    println!("cargo:rerun-if-env-changed=MYDUPFINDER_BUILD_GIT");
    watch_git();

    let override_value = std::env::var("MYDUPFINDER_BUILD_GIT")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());
    let (describe, sha) = match override_value {
        // The override replaces both values.
        Some(v) => (v.clone(), v),
        None => (
            git(&["describe", "--tags", "--always", "--dirty"]).unwrap_or_else(|| UNKNOWN.into()),
            git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| UNKNOWN.into()),
        ),
    };
    println!("cargo:rustc-env=MYDUPFINDER_GIT_DESCRIBE={describe}");
    println!("cargo:rustc-env=MYDUPFINDER_GIT_SHA={sha}");
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../assets/icons/icon.ico");
    export_git_info();

    #[cfg(windows)]
    embed_windows_resources();
}

#[cfg(windows)]
fn embed_windows_resources() {
    // Only when the *target* is Windows (the build script runs on the host).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../assets/icons/icon.ico")
        .set("ProductName", "myDupFinder UI")
        .set(
            "FileDescription",
            "Native viewer for myDupFinder dupReport.csv files",
        )
        .set("CompanyName", "Roger Briggen")
        .set("LegalCopyright", "Copyright (c) Roger Briggen")
        .set("OriginalFilename", "mydupfinder-ui.exe");
    // FileVersion / ProductVersion come from the package version automatically.
    if let Err(err) = res.compile() {
        println!("cargo:warning=failed to embed Windows resources: {err}");
        std::process::exit(1);
    }
}
