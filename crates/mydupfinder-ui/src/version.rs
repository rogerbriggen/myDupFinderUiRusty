//! App version shown in the window title and the toolbar: the SemVer from the
//! workspace `Cargo.toml` plus git information captured by `build.rs`
//! (SemVer build metadata, GitVersion style).

/// SemVer from the workspace `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// `git describe --tags --always --dirty` at build time (or `unknown`).
pub const GIT_DESCRIBE: &str = env!("MYDUPFINDER_GIT_DESCRIBE");
/// Short commit sha at build time (or `unknown`).
pub const GIT_SHA: &str = env!("MYDUPFINDER_GIT_SHA");

/// Format `version[+sha[.dirty]]`. Without usable git info it is just `version`.
pub fn format_version(version: &str, describe: &str, sha: &str) -> String {
    if sha.is_empty() || sha == "unknown" {
        return version.to_string();
    }
    if describe.ends_with("-dirty") {
        format!("{version}+{sha}.dirty")
    } else {
        format!("{version}+{sha}")
    }
}

/// The version of this build, e.g. `0.1.0+1bb8187` or `0.1.0+1bb8187.dirty`.
pub fn display_version() -> String {
    format_version(VERSION, GIT_DESCRIBE, GIT_SHA)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_build() {
        assert_eq!(
            format_version("0.1.0", "1bb8187", "1bb8187"),
            "0.1.0+1bb8187"
        );
    }

    #[test]
    fn dirty_build() {
        assert_eq!(
            format_version("0.1.0", "1bb8187-dirty", "1bb8187"),
            "0.1.0+1bb8187.dirty"
        );
    }

    #[test]
    fn tagged_build() {
        assert_eq!(
            format_version("0.1.0", "v0.1.0-3-gabc1234-dirty", "abc1234"),
            "0.1.0+abc1234.dirty"
        );
    }

    #[test]
    fn unknown_git() {
        assert_eq!(format_version("0.1.0", "unknown", "unknown"), "0.1.0");
        assert_eq!(format_version("0.1.0", "", ""), "0.1.0");
    }

    #[test]
    fn compiled_in_values_are_set() {
        assert!(!GIT_DESCRIBE.is_empty() && !GIT_SHA.is_empty());
        assert!(display_version().starts_with(VERSION));
    }
}
