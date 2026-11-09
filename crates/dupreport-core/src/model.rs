//! Core report types (§5.1).

/// File classification written by the backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Category {
    Duplicate,
    Moved,
    Unique,
    Missing,
    New,
    Changed,
}

impl Category {
    /// All categories in toolbar-chip order.
    pub const ALL: [Category; 6] = [
        Category::Duplicate,
        Category::Moved,
        Category::Unique,
        Category::Missing,
        Category::New,
        Category::Changed,
    ];

    /// Exact, case-sensitive parse of the CSV value.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.as_str() == s)
    }

    /// The CSV spelling.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Duplicate => "Duplicate",
            Self::Moved => "Moved",
            Self::Unique => "Unique",
            Self::Missing => "Missing",
            Self::New => "New",
            Self::Changed => "Changed",
        }
    }
}

/// Which scan database a row came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Source {
    Base,
    Second,
}

impl Source {
    /// All sources in chip order.
    pub const ALL: [Source; 2] = [Source::Base, Source::Second];

    /// Exact, case-sensitive parse of the CSV value.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.as_str() == s)
    }

    /// The CSV spelling.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Base => "Base",
            Self::Second => "Second",
        }
    }
}

/// Index into `Report::rows`.
pub type RowIdx = u32;

/// One CSV record of a dup report.
///
/// Construct with [`Row::new`] so the precomputed `folder_len` stays
/// consistent with `path`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// Full path (`FilenameAndPath`), using `\` or `/`.
    pub path: String,
    /// Size in bytes.
    pub size: i64,
    /// SHA-512 hex (any case); may be empty.
    pub hash: String,
    /// Classification.
    pub category: Category,
    /// Originating database.
    pub source: Source,
    /// Group shared by rows describing the same logical file.
    pub group_id: i32,
    /// Byte index of the last path separator, or `u32::MAX` if there is none.
    pub folder_len: u32,
}

impl Row {
    /// Build a row and precompute `folder_len` from `path`.
    #[must_use]
    pub fn new(
        path: String,
        size: i64,
        hash: String,
        category: Category,
        source: Source,
        group_id: i32,
    ) -> Self {
        let folder_len = match path.rfind(['\\', '/']) {
            Some(i) => u32::try_from(i).expect("path shorter than 4 GiB"),
            None => u32::MAX,
        };
        Self {
            path,
            size,
            hash,
            category,
            source,
            group_id,
            folder_len,
        }
    }

    /// Folder part of the path; `""` when there is no separator.
    #[must_use]
    pub fn folder(&self) -> &str {
        if self.folder_len == u32::MAX {
            ""
        } else {
            &self.path[..self.folder_len as usize]
        }
    }

    /// File name part of the path.
    #[must_use]
    pub fn filename(&self) -> &str {
        if self.folder_len == u32::MAX {
            &self.path
        } else {
            // Separators are ASCII, so +1 is a char boundary.
            &self.path[self.folder_len as usize + 1..]
        }
    }
}

/// A folder in the Base scan whose whole subtree is duplicated in a folder
/// of the Second scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdenticalFolderPair {
    /// Base-side folder.
    pub folder_a: String,
    /// Second-side folder.
    pub folder_b: String,
    /// Files in the Base subtree.
    pub file_count: usize,
    /// Bytes in the Base subtree.
    pub total_size: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(path: &str) -> Row {
        Row::new(
            path.to_string(),
            1,
            String::new(),
            Category::Unique,
            Source::Base,
            1,
        )
    }

    #[test]
    fn row_folder_and_filename() {
        let r = row(r"C:\a\b\one.txt");
        assert_eq!((r.folder(), r.filename()), (r"C:\a\b", "one.txt"));
        let r = row("/x/y/z.txt");
        assert_eq!((r.folder(), r.filename()), ("/x/y", "z.txt"));
        let r = row("bare.txt");
        assert_eq!((r.folder(), r.filename()), ("", "bare.txt"));
        assert_eq!(r.folder_len, u32::MAX);
        let r = row("/top.txt");
        assert_eq!((r.folder(), r.filename()), ("", "top.txt"));
    }

    #[test]
    fn row_handles_non_ascii_paths() {
        let r = row("C:\\Fotos\\Zürich\\bild ä.jpg");
        assert_eq!(r.folder(), "C:\\Fotos\\Zürich");
        assert_eq!(r.filename(), "bild ä.jpg");
    }

    #[test]
    fn category_round_trips_in_canonical_order() {
        let names: Vec<_> = Category::ALL.iter().map(|c| c.as_str()).collect();
        assert_eq!(
            names,
            ["Duplicate", "Moved", "Unique", "Missing", "New", "Changed"]
        );
        for c in Category::ALL {
            assert_eq!(Category::parse(c.as_str()), Some(c));
        }
    }

    #[test]
    fn category_parse_is_exact() {
        assert_eq!(Category::parse("duplicate"), None);
        assert_eq!(Category::parse(" Duplicate"), None);
        assert_eq!(Category::parse(""), None);
    }

    #[test]
    fn category_as_usize_matches_all_index() {
        for (i, c) in Category::ALL.into_iter().enumerate() {
            assert_eq!(c as usize, i);
        }
    }

    #[test]
    fn source_round_trips() {
        for s in Source::ALL {
            assert_eq!(Source::parse(s.as_str()), Some(s));
        }
        assert_eq!(Source::parse("base"), None);
    }
}
