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
pub use model::{Category, Source};
