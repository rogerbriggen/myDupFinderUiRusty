//! Path helpers working on `&str` (§5.3).
//!
//! Report paths may use `\` or `/` regardless of the host OS, so
//! `std::path::Path` is never used here.

fn is_sep(c: char) -> bool {
    c == '\\' || c == '/'
}

/// Split at the last `\` or `/`. Without a separator the folder is `""`.
#[must_use]
pub fn split_folder_and_name(p: &str) -> (&str, &str) {
    match p.rfind(is_sep) {
        Some(i) => (&p[..i], &p[i + 1..]),
        None => ("", p),
    }
}

/// Folder part of `p`, or `None` when it is empty.
#[must_use]
pub fn parent_of(p: &str) -> Option<&str> {
    let (folder, _) = split_folder_and_name(p);
    if folder.is_empty() {
        None
    } else {
        Some(folder)
    }
}

/// True when `path` equals `folder` or lies below it. An empty `folder`
/// contains everything; `C:\ab` is not under `C:\a`.
#[must_use]
pub fn is_under_or_equal(path: &str, folder: &str) -> bool {
    if folder.is_empty() || path == folder {
        return true;
    }
    match path.strip_prefix(folder) {
        Some(rest) => rest.starts_with(is_sep),
        None => false,
    }
}

/// Join a folder and a name, using `\` if the folder contains one, else `/`.
#[must_use]
pub fn join_path(folder: &str, name: &str) -> String {
    if folder.is_empty() {
        return name.to_string();
    }
    let sep = if folder.contains('\\') { '\\' } else { '/' };
    let mut s = String::with_capacity(folder.len() + 1 + name.len());
    s.push_str(folder);
    s.push(sep);
    s.push_str(name);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_windows() {
        assert_eq!(
            split_folder_and_name(r"C:\a\b\one.txt"),
            (r"C:\a\b", "one.txt")
        );
    }

    #[test]
    fn split_posix() {
        assert_eq!(split_folder_and_name("/x/y/z.txt"), ("/x/y", "z.txt"));
    }

    #[test]
    fn split_bare_filename() {
        assert_eq!(split_folder_and_name("bare.txt"), ("", "bare.txt"));
    }

    #[test]
    fn split_mixed_separators_uses_last() {
        assert_eq!(split_folder_and_name(r"C:\a/b\c"), (r"C:\a/b", "c"));
    }

    #[test]
    fn parents() {
        assert_eq!(parent_of(r"C:\a"), Some("C:"));
        assert_eq!(parent_of("C:"), None);
        assert_eq!(parent_of("/x/y"), Some("/x"));
        assert_eq!(parent_of("/x"), None);
        assert_eq!(parent_of(""), None);
    }

    #[test]
    fn under_or_equal() {
        assert!(is_under_or_equal(r"C:\a\b", ""));
        assert!(is_under_or_equal(r"C:\a", r"C:\a"));
        assert!(is_under_or_equal(r"C:\a\b\c.txt", r"C:\a"));
        assert!(is_under_or_equal("/a/b", "/a"));
        assert!(!is_under_or_equal(r"C:\ab", r"C:\a"));
        assert!(!is_under_or_equal(r"C:\ab\c", r"C:\a"));
        assert!(!is_under_or_equal(r"C:\a", r"C:\a\b"));
    }

    #[test]
    fn joins() {
        assert_eq!(join_path("", "x.txt"), "x.txt");
        assert_eq!(join_path(r"C:\a", "x.txt"), r"C:\a\x.txt");
        assert_eq!(join_path("/a/b", "x.txt"), "/a/b/x.txt");
        assert_eq!(join_path("C:", "x"), "C:/x");
    }
}
