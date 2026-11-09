//! Pure domain logic for myDupFinder `dupReport.csv` files.
//!
//! This crate has no GUI dependency; see `docs/REBUILD_RUST_GPUI.md` §5.

pub mod error;
pub mod format;
pub mod identical;
pub mod model;
pub mod parser;
pub mod path;
pub mod query;
pub mod report;
pub mod status;
pub mod tree;

pub use error::ReportError;
pub use identical::find_identical_folders;
pub use model::{Category, IdenticalFolderPair, Row, RowIdx, Source};
pub use parser::{ParseProgress, parse_file, parse_reader};
pub use query::{RowQuery, filter};
pub use report::Report;
pub use status::{IdenticalStatus, compute_statuses};
pub use tree::{FolderId, FolderNode, FolderTree, ROOT};
