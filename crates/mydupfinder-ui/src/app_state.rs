//! The `ReportStore` entity: the single view model behind all views (§6).

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use dupreport_core::{
    Category, FolderId, ParseProgress, ROOT, Report, ReportError, RowIdx, RowQuery, Source, filter,
};
use gpui::{Context, PathPromptOptions, Task};

/// Reports with at least this many rows are filtered on the background executor.
pub const ASYNC_FILTER_THRESHOLD: usize = 50_000;

/// Minimum time between two progress updates sent to the UI (~10/s).
const PROGRESS_THROTTLE: Duration = Duration::from_millis(100);

/// Whether a report is currently being loaded.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum LoadState {
    /// Nothing in flight.
    #[default]
    Idle,
    /// A report is being parsed.
    Loading {
        /// Latest throttled parser progress, if any arrived yet.
        progress: Option<ParseProgress>,
    },
}

impl LoadState {
    /// True while a report is being loaded.
    #[must_use]
    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading { .. })
    }
}

/// Everything the views display, plus the operations that change it.
pub struct ReportStore {
    /// The loaded report, shared with background jobs.
    pub report: Option<Arc<Report>>,
    /// Load state.
    pub load: LoadState,
    /// Last error, formatted as `"{Type}: {message}"`.
    pub error: Option<String>,

    /// Selected folder in the tree.
    pub selected_folder: FolderId,
    /// Whether the table includes rows of descendant folders.
    pub include_descendants: bool,
    /// Path / filename substring filter (untrimmed as typed).
    pub text_filter: String,
    /// Hash filter (untrimmed as typed).
    pub hash_filter: String,
    /// Enabled categories.
    pub categories: BTreeSet<Category>,
    /// Enabled sources.
    pub sources: BTreeSet<Source>,
    /// Whether the table shows the hash column.
    pub show_hash: bool,

    /// Row indexes passing all filters, in file order.
    pub filtered: Arc<Vec<RowIdx>>,

    /// Row count from which filtering moves to the background executor.
    async_filter_threshold: usize,
    /// Bumped on every refilter; stale background results are dropped.
    filter_generation: u64,
    /// Bumped on every open; stale loads are dropped.
    open_generation: u64,
    load_task: Option<Task<()>>,
    filter_task: Option<Task<()>>,
}

impl Default for ReportStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ReportStore {
    /// An empty store with default view state (§6).
    #[must_use]
    pub fn new() -> Self {
        Self {
            report: None,
            load: LoadState::Idle,
            error: None,
            selected_folder: ROOT,
            include_descendants: true,
            text_filter: String::new(),
            hash_filter: String::new(),
            categories: Category::ALL.into_iter().collect(),
            sources: Source::ALL.into_iter().collect(),
            show_hash: false,
            filtered: Arc::new(Vec::new()),
            async_filter_threshold: ASYNC_FILTER_THRESHOLD,
            filter_generation: 0,
            open_generation: 0,
            load_task: None,
            filter_task: None,
        }
    }

    /// Show the native file dialog and open the chosen report. Cancel does nothing.
    pub fn pick_and_open(&mut self, cx: &mut Context<Self>) {
        if self.load.is_loading() {
            return;
        }
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Open dupReport.csv".into()),
        });
        cx.spawn(async move |this, cx| match picked.await {
            Ok(Ok(Some(paths))) => {
                if let Some(path) = paths.into_iter().next() {
                    this.update(cx, |this, cx| this.open_report(path, cx)).ok();
                }
            }
            Ok(Ok(None)) | Err(_) => {}
            Ok(Err(e)) => {
                this.update(cx, |this, cx| {
                    this.error = Some(format!("Dialog: {e}"));
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    /// Load `path` on the background executor, forwarding throttled progress.
    /// A previous load still in flight is cancelled.
    pub fn open_report(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.open_generation += 1;
        let generation = self.open_generation;
        self.error = None;
        self.load = LoadState::Loading { progress: None };
        self.report = None;
        self.filter_task = None;
        self.filter_generation += 1;
        self.filtered = Arc::new(Vec::new());
        cx.notify();

        let (tx, rx) = smol::channel::unbounded::<ParseProgress>();
        let job = cx.background_executor().spawn(async move {
            let mut last_sent: Option<Instant> = None;
            Report::load(&path, |p| {
                if last_sent.is_none_or(|t| t.elapsed() >= PROGRESS_THROTTLE) {
                    last_sent = Some(Instant::now());
                    let _ = tx.try_send(p);
                }
            })
            // `tx` is dropped here, which ends the progress loop below.
        });

        self.load_task = Some(cx.spawn(async move |this, cx| {
            while let Ok(p) = rx.recv().await {
                let alive = this.update(cx, |this, cx| {
                    if this.open_generation == generation && this.load.is_loading() {
                        this.load = LoadState::Loading { progress: Some(p) };
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    return;
                }
            }
            let result = job.await;
            this.update(cx, |this, cx| {
                if this.open_generation == generation {
                    this.finish_open(result, cx);
                }
            })
            .ok();
        }));
    }

    /// Apply the result of a load: reset view state or record the error.
    fn finish_open(&mut self, result: Result<Report, ReportError>, cx: &mut Context<Self>) {
        self.load = LoadState::Idle;
        self.load_task = None;
        match result {
            Ok(report) => {
                self.selected_folder = ROOT;
                self.sources = report.default_sources().into_iter().collect();
                self.report = Some(Arc::new(report));
                self.refilter(cx);
            }
            Err(e) => {
                self.error = Some(e.banner());
                cx.notify();
            }
        }
    }

    /// The query for the current view state (§6: trimmed, empty becomes `None`).
    fn query(&self) -> RowQuery {
        let non_empty = |s: &str| {
            let t = s.trim();
            (!t.is_empty()).then(|| t.to_string())
        };
        RowQuery {
            folder: self
                .report
                .as_ref()
                .map(|r| r.tree.node(self.selected_folder).path.clone()),
            include_descendants: self.include_descendants,
            text: non_empty(&self.text_filter),
            hash: non_empty(&self.hash_filter),
            categories: (self.categories.len() < Category::ALL.len())
                .then(|| self.categories.iter().copied().collect()),
            sources: (self.sources.len() < Source::ALL.len())
                .then(|| self.sources.iter().copied().collect()),
        }
    }

    /// Recompute `filtered`: synchronously for small reports, otherwise on the
    /// background executor with a generation counter so stale results are dropped.
    fn refilter(&mut self, cx: &mut Context<Self>) {
        self.filter_generation += 1;
        let generation = self.filter_generation;
        let Some(report) = self.report.clone() else {
            self.filter_task = None;
            self.filtered = Arc::new(Vec::new());
            cx.notify();
            return;
        };
        let query = self.query();
        if report.rows.len() < self.async_filter_threshold {
            self.filter_task = None;
            self.filtered = Arc::new(filter(&report, &query));
            cx.notify();
            return;
        }
        let job = cx
            .background_executor()
            .spawn(async move { filter(&report, &query) });
        self.filter_task = Some(cx.spawn(async move |this, cx| {
            let result = job.await;
            this.update(cx, |this, cx| {
                if this.filter_generation == generation {
                    this.filtered = Arc::new(result);
                    this.filter_task = None;
                    cx.notify();
                }
            })
            .ok();
        }));
    }

    /// Show or hide the hash column (rendering only, no refilter).
    pub fn set_show_hash(&mut self, show: bool, cx: &mut Context<Self>) {
        self.show_hash = show;
        cx.notify();
    }
}

// The setters below are wired to the folder tree (step 9) and the toolbar
// filters (step 10); until then only the tests call them.
impl ReportStore {
    /// Select a folder in the tree.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "used by the folder tree (step 9)")
    )]
    pub fn select_folder(&mut self, folder: FolderId, cx: &mut Context<Self>) {
        self.selected_folder = folder;
        self.refilter(cx);
    }

    /// Include or exclude rows of descendant folders.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "used by the folder tree (step 9)")
    )]
    pub fn set_include_descendants(&mut self, on: bool, cx: &mut Context<Self>) {
        self.include_descendants = on;
        self.refilter(cx);
    }

    /// Set the path / filename substring filter.
    #[cfg_attr(not(test), expect(dead_code, reason = "used by the toolbar (step 10)"))]
    pub fn set_text_filter(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.text_filter = text.into();
        self.refilter(cx);
    }

    /// Set the hash filter.
    #[cfg_attr(not(test), expect(dead_code, reason = "used by the toolbar (step 10)"))]
    pub fn set_hash_filter(&mut self, hash: impl Into<String>, cx: &mut Context<Self>) {
        self.hash_filter = hash.into();
        self.refilter(cx);
    }

    /// Enable or disable one category (canonical order is kept by the set).
    #[cfg_attr(not(test), expect(dead_code, reason = "used by the toolbar (step 10)"))]
    pub fn set_category(&mut self, category: Category, on: bool, cx: &mut Context<Self>) {
        if on {
            self.categories.insert(category);
        } else {
            self.categories.remove(&category);
        }
        self.refilter(cx);
    }

    /// Enable or disable one source.
    #[cfg_attr(not(test), expect(dead_code, reason = "used by the toolbar (step 10)"))]
    pub fn set_source(&mut self, source: Source, on: bool, cx: &mut Context<Self>) {
        if on {
            self.sources.insert(source);
        } else {
            self.sources.remove(&source);
        }
        self.refilter(cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{AppContext, Entity, TestAppContext};

    const WHOLE_LOT: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../testdata/Demo DiffFindDupsTheWholeLot dupReport.csv"
    );
    const ONLY_DUPS: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../testdata/Demo DiffFindOnlyDups dupReport.csv"
    );

    fn open(cx: &mut TestAppContext, path: &str) -> Entity<ReportStore> {
        let store = cx.new(|_| ReportStore::new());
        store.update(cx, |s, cx| s.open_report(PathBuf::from(path), cx));
        assert!(store.read_with(cx, |s, _| s.load.is_loading()));
        cx.run_until_parked();
        store
    }

    #[gpui::test]
    fn opens_only_dups_fixture(cx: &mut TestAppContext) {
        let store = open(cx, ONLY_DUPS);
        store.read_with(cx, |s, _| {
            assert_eq!(s.load, LoadState::Idle);
            assert!(s.error.is_none());
            assert_eq!(s.report.as_ref().unwrap().rows.len(), 25);
            assert_eq!(s.filtered.len(), 25);
            assert_eq!(s.selected_folder, ROOT);
            assert_eq!(
                s.sources.iter().copied().collect::<Vec<_>>(),
                [Source::Base]
            );
        });
    }

    #[gpui::test]
    fn opens_whole_lot_fixture(cx: &mut TestAppContext) {
        let store = open(cx, WHOLE_LOT);
        store.read_with(cx, |s, _| {
            assert_eq!(s.report.as_ref().unwrap().rows.len(), 64);
            assert_eq!(s.filtered.len(), 64);
            assert_eq!(s.sources.len(), 2);
        });
    }

    #[gpui::test]
    fn toggling_category_changes_filtered(cx: &mut TestAppContext) {
        let store = open(cx, WHOLE_LOT);
        store.update(cx, |s, cx| s.set_category(Category::Duplicate, false, cx));
        // 64 rows, 48 of them Duplicate.
        assert_eq!(store.read_with(cx, |s, _| s.filtered.len()), 16);
        store.update(cx, |s, cx| s.set_category(Category::Duplicate, true, cx));
        assert_eq!(store.read_with(cx, |s, _| s.filtered.len()), 64);
    }

    #[gpui::test]
    fn show_hash_does_not_refilter(cx: &mut TestAppContext) {
        let store = open(cx, WHOLE_LOT);
        let before = store.read_with(cx, |s, _| s.filtered.clone());
        store.update(cx, |s, cx| s.set_show_hash(true, cx));
        store.read_with(cx, |s, _| {
            assert!(s.show_hash);
            assert!(Arc::ptr_eq(&before, &s.filtered));
        });
    }

    #[gpui::test]
    fn background_filter_applies_latest_result(cx: &mut TestAppContext) {
        let store = open(cx, WHOLE_LOT);
        store.update(cx, |s, _| s.async_filter_threshold = 0);
        store.update(cx, |s, cx| {
            s.set_category(Category::Duplicate, false, cx);
            s.set_category(Category::Changed, false, cx);
        });
        // Nothing applied yet: results arrive from the background executor.
        assert_eq!(store.read_with(cx, |s, _| s.filtered.len()), 64);
        cx.run_until_parked();
        // 64 - 48 Duplicate - 12 Changed = 4 (Missing + Moved).
        assert_eq!(store.read_with(cx, |s, _| s.filtered.len()), 4);
    }

    #[gpui::test]
    fn missing_file_sets_io_error(cx: &mut TestAppContext) {
        let store = open(cx, "definitely/not/here dupReport.csv");
        store.read_with(cx, |s, _| {
            assert!(
                s.error.as_deref().unwrap().starts_with("Io: "),
                "{:?}",
                s.error
            );
            assert!(s.report.is_none());
            assert_eq!(s.load, LoadState::Idle);
        });
    }

    #[gpui::test]
    fn check_report_sets_unsupported_error(cx: &mut TestAppContext) {
        let dir = std::env::temp_dir().join(format!("mydupfinder-ui-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("check report.csv");
        std::fs::write(
            &file,
            "# myDupFinder check report\r\n# Key=Value\r\nCategory,PathMoved\r\n",
        )
        .unwrap();
        let store = open(cx, file.to_str().unwrap());
        std::fs::remove_dir_all(&dir).ok();
        store.read_with(cx, |s, _| {
            assert!(
                s.error.as_deref().unwrap().starts_with("Unsupported: "),
                "{:?}",
                s.error
            );
        });
    }

    #[gpui::test]
    fn reopening_resets_sources_and_error(cx: &mut TestAppContext) {
        let store = open(cx, "definitely/not/here.csv");
        store.update(cx, |s, cx| s.open_report(PathBuf::from(WHOLE_LOT), cx));
        assert!(store.read_with(cx, |s, _| s.error.is_none()));
        cx.run_until_parked();
        store.update(cx, |s, cx| s.set_source(Source::Second, false, cx));
        assert_eq!(store.read_with(cx, |s, _| s.filtered.len()), 33);
        store.update(cx, |s, cx| s.open_report(PathBuf::from(ONLY_DUPS), cx));
        cx.run_until_parked();
        store.read_with(cx, |s, _| {
            assert_eq!(
                s.sources.iter().copied().collect::<Vec<_>>(),
                [Source::Base]
            );
            assert_eq!(s.filtered.len(), 25);
        });
    }
}

#[cfg(test)]
mod filter_tests {
    use super::*;
    use gpui::{AppContext, TestAppContext};

    const WHOLE_LOT: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../testdata/Demo DiffFindDupsTheWholeLot dupReport.csv"
    );

    #[gpui::test]
    fn text_hash_and_folder_filters(cx: &mut TestAppContext) {
        let store = cx.new(|_| ReportStore::new());
        store.update(cx, |s, cx| s.open_report(PathBuf::from(WHOLE_LOT), cx));
        cx.run_until_parked();

        // Case-insensitive substring over the full path; 31 Second rows live under filesNew.
        store.update(cx, |s, cx| s.set_text_filter("  FILESNEW ", cx));
        assert_eq!(store.read_with(cx, |s, _| s.filtered.len()), 31);
        store.update(cx, |s, cx| s.set_text_filter("", cx));

        // Hash prefix (>= 8 chars) matches both sides of the first pair.
        store.update(cx, |s, cx| s.set_hash_filter("02f16dc4", cx));
        assert_eq!(store.read_with(cx, |s, _| s.filtered.len()), 2);
        store.update(cx, |s, cx| s.set_hash_filter("", cx));

        // Folder selection: direct rows only vs. descendants.
        let (id, direct) = store.read_with(cx, |s, _| {
            let report = s.report.as_ref().unwrap();
            let id = report.tree.find(report.rows[0].folder()).unwrap();
            (id, report.rows_by_folder[id as usize].len())
        });
        store.update(cx, |s, cx| {
            s.set_include_descendants(false, cx);
            s.select_folder(id, cx);
        });
        assert_eq!(store.read_with(cx, |s, _| s.filtered.len()), direct);
        store.update(cx, |s, cx| s.set_include_descendants(true, cx));
        assert!(store.read_with(cx, |s, _| s.filtered.len()) >= direct);
        store.update(cx, |s, cx| s.select_folder(ROOT, cx));
        assert_eq!(store.read_with(cx, |s, _| s.filtered.len()), 64);
    }
}
