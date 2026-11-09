//! Error type for loading and interpreting reports (§5.2).

/// Everything that can go wrong while reading a report.
#[derive(Debug, thiserror::Error)]
pub enum ReportError {
    /// Underlying I/O failure.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// The CSV layer rejected the input.
    #[error("CSV parse error: {0}")]
    Csv(#[from] csv::Error),
    /// A recognised but unsupported file (e.g. a check report).
    #[error("Unsupported file: {0}")]
    Unsupported(String),
    /// The file is a dup report but its content is invalid.
    #[error("Invalid report: {0}")]
    InvalidReport(String),
}

impl ReportError {
    /// Variant name as shown in the UI banner (`"{Type}: {message}"`).
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Io(_) => "Io",
            Self::Csv(_) => "Csv",
            Self::Unsupported(_) => "Unsupported",
            Self::InvalidReport(_) => "InvalidReport",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_includes_prefix_and_message() {
        let e = ReportError::InvalidReport("Row 7: unknown Category 'Foo'".into());
        assert_eq!(
            e.to_string(),
            "Invalid report: Row 7: unknown Category 'Foo'"
        );
        assert_eq!(e.kind(), "InvalidReport");
    }

    #[test]
    fn io_error_converts() {
        let e: ReportError = std::io::Error::other("boom").into();
        assert_eq!(e.kind(), "Io");
        assert_eq!(e.to_string(), "I/O error: boom");
    }

    #[test]
    fn unsupported_display() {
        let e = ReportError::Unsupported("myDupFinder check reports are not supported yet.".into());
        assert!(e.to_string().starts_with("Unsupported file: "));
    }
}
