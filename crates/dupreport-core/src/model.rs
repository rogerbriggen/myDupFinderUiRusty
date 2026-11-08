//! Core report types (§5.1). `Row` and `IdenticalFolderPair` follow in step 2.

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

#[cfg(test)]
mod tests {
    use super::*;

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
