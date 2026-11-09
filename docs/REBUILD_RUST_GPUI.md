# Rebuilding myDupFinder UI in pure Rust + GPUI

This document is a complete, self-contained brief for re-implementing the
current **myDupFinder UI** (Tauri 2 + Angular 20) as a **native Rust desktop
application built on [GPUI](https://www.gpui.rs/)** (the UI framework behind
the Zed editor), in a **new repository**.

It was written by analysing every source file of this repository
(`src-tauri/src/*.rs`, `src/app/**/*.ts`, `AGENTS.md`, `README.md`, CI, test
fixtures). Anything marked **(behaviour to preserve)** is what the current app
does today and users rely on; anything marked **(improvement)** is a
deliberate change that the rewrite should make because the old architecture
forced a compromise.

Copy this file into the new repo as the starting `AGENTS.md` / `CLAUDE.md`
(or link to it from there) and work through §11 "Implementation plan" in
order.

---

## 1. What the application does

A **read-only viewer** for the CSV reports produced by
[myDupFinder](https://github.com/rogerbriggen/myDupFinder) (a .NET backend
that scans/hashes files and compares two scan databases). The UI:

1. Opens a `<JobName> dupReport.csv`.
2. Reconstructs a **folder tree** from the file paths, with per-folder
   aggregate counts.
3. Shows a **virtual-scrolled table** of the files in the selected folder
   (optionally including descendants), with filters.
4. Runs an **identical-folders detector**: finds folder pairs (one from the
   *Base* scan, one from the *Second* scan) whose entire subtrees are
   duplicates of each other, lists them, and colours the tree
   green / yellow / red accordingly.

### Non-goals (unchanged)

- No scanning, hashing, or database writes. Read-only over the CSV.
- No deleting / moving files (may come later behind a confirmation dialog).
- No filesystem `stat` calls to enrich data — show only what the CSV has.
- The sibling *check report* format is detected and rejected, not parsed.

---

## 2. Target tech stack

| Concern            | Choice                                                                                   | Notes |
| ------------------ | ---------------------------------------------------------------------------------------- | ----- |
| Language           | Rust stable (edition 2021 or 2024), MSRV ≥ the one GPUI requires                         | |
| UI                 | `gpui` (Zed's GPU-accelerated UI framework)                                              | Pin an exact version / git rev — the API still moves between releases. |
| Widgets (optional) | `gpui-component` (longbridge) for `Input`, `Checkbox`, `Button`, `Resizable` panels, `Table`, `Tree` | Saves a lot of work (text input in raw GPUI is non-trivial). If you skip it, build a minimal text input yourself. Pin the version that matches your `gpui` version. |
| CSV                | `csv = "1"`                                                                              | `;` delimiter, `"` quote, streaming `read_record`. |
| Errors             | `thiserror = "2"`                                                                        | |
| File dialog        | GPUI's `cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: false, .. })` | Native dialog. `rfd` is a fallback if you need extension filters (`*.csv`). |
| Persistence (later)| `serde`, `serde_json`, `directories` (config dir)                                        | For last-opened report + UI prefs (backlog). |
| Tests              | `cargo test` (core crate fully headless), `gpui::TestAppContext` for view tests          | |
| Lint / format      | `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`                         | Mandatory in CI. |

There is **no JavaScript, no webview, no IPC** any more. That removes the
whole `ReportBackend` / `invoke()` / serde-camelCase / handle-registry layer
and the double copy of all rows (Rust `Vec<Row>` + JS `Row[]`).

> **GPUI API caveat.** Method names below (`cx.new`, `cx.notify()`,
> `cx.spawn`, `uniform_list`, `cx.listener`, `cx.processor`,
> `prompt_for_paths`, `Render::render(&mut self, window, cx)`) match recent
> GPUI (2025–2026, post "`Window` + `Context<T>`" refactor). Verify against
> the pinned version's examples (`crates/gpui/examples/` in the Zed repo)
> before writing code and adjust names; the architecture does not depend on
> them.

---

## 3. Repository layout (new repo)

Use a Cargo **workspace** so the domain logic is testable without any GUI
dependency and could later be reused by a CLI or another front-end (this
replaces the old "swap-able `ReportBackend`" requirement).

```
mydupfinder-ui/
├─ Cargo.toml                 # [workspace] members = ["crates/*"]
├─ AGENTS.md                  # = this file, adapted
├─ README.md
├─ testdata/                  # copy verbatim from the old repo (see §4.6)
│   ├─ Demo DiffFindOnlyDups dupReport.csv
│   └─ Demo DiffFindDupsTheWholeLot dupReport.csv
├─ crates/
│   ├─ dupreport-core/        # NO gpui dependency. Pure logic + unit tests.
│   │   └─ src/
│   │       ├─ lib.rs
│   │       ├─ model.rs       # Category, Source, Row, IdenticalFolderPair
│   │       ├─ path.rs        # split_folder_and_name, parent_of, is_under…
│   │       ├─ parser.rs      # streaming CSV parser + progress callback
│   │       ├─ error.rs       # ReportError (thiserror)
│   │       ├─ report.rs      # Report: rows + indexes (by folder, by group)
│   │       ├─ query.rs       # RowQuery + filter → Vec<RowIdx>
│   │       ├─ tree.rs        # FolderTree (arena) + aggregates
│   │       ├─ identical.rs   # identical-folder detector
│   │       ├─ status.rs      # green/yellow/red status per folder
│   │       └─ format.rs      # format_size, format_hash_short
│   └─ mydupfinder-ui/        # the GPUI binary
│       └─ src/
│           ├─ main.rs        # App::new().run(…), window, actions, menus
│           ├─ app_state.rs   # Entity<ReportStore> — the view model
│           ├─ views/
│           │   ├─ explorer.rs      # root layout + splitters
│           │   ├─ toolbar.rs
│           │   ├─ folder_tree.rs   # uniform_list over flattened tree
│           │   ├─ row_table.rs     # uniform_list over filtered row indexes
│           │   └─ identical_panel.rs
│           └─ theme.rs       # colours (light + dark)
└─ .github/workflows/ci.yml
```

Rules carried over from the current `AGENTS.md`:

- Conventional Commits; one concern per PR; never commit to `main` directly.
- `cargo fmt` on every touched file before finishing; CI runs `--check`.
- `cargo clippy -- -D warnings` in CI.
- Errors are typed enums, never bare `String`s.

---

## 4. Input format — the dup report CSV (byte-exact contract)

### 4.1 Header line (exact)

```
FilenameAndPath;FileSize;FileSha512Hash;Category;Source;GroupId
```

- Delimiter **`;`** (not RFC 4180 comma). No comment lines.
- UTF-8, no BOM. Line endings `\r\n` (written by .NET on Windows); accept `\n` too.
- Only `FilenameAndPath` is quoted, always, with `"`; embedded `"` → `""`.
- Other fields are raw and cannot contain `;`.

### 4.2 Columns

| # | Column            | Rust type | Notes |
|---|-------------------|-----------|-------|
| 1 | `FilenameAndPath` | `String`  | Full path, e.g. `C:\photos\2024\img.jpg`. May use `\` or `/`. |
| 2 | `FileSize`        | `i64`     | Bytes. |
| 3 | `FileSha512Hash`  | `String`  | SHA-512 hex. **The real fixtures are UPPERCASE** (AGENTS.md says lowercase — treat as case-insensitive). May be **empty** for `New` rows. |
| 4 | `Category`        | enum      | `Duplicate`, `Moved`, `Unique`, `Missing`, `New`, `Changed`. Exact, case-sensitive. |
| 5 | `Source`          | enum      | `Base` or `Second`. Single-DB jobs: all `Base`. |
| 6 | `GroupId`         | `i32`     | Rows describing the same logical file share a GroupId. `Missing`/`New`/`Unique` are singletons. |

### 4.3 Sort order in file

`ORDER BY Category, GroupId, Source, FilenameAndPath (case-insensitive)`.
Present rows **in file order** unless the user sorts (behaviour to preserve:
the table today never re-sorts).

### 4.4 Category semantics

- **Duplicate** — same hash. Same-DB mode: every row in the group is a copy on
  disk (all `Base`). Two-DB mode: one `Base` + one `Second` per group.
- **Moved** — same hash + same filename, different relative path. Pair.
- **Changed** — same relative path, different hash. Pair.
- **Missing** — in Base, absent in Second. Singleton.
- **New** — in Second, absent in Base. Singleton (hash may be empty).
- **Unique** — no duplicate anywhere. Singleton.

### 4.5 Check-report sniffing

If the first line (after trimming `\r\n`) **starts with**
`# myDupFinder check report`, fail with `ReportError::Unsupported("myDupFinder
check reports are not supported yet.")`. Do this by peeking the
`BufReader` buffer (`fill_buf`) without consuming, then hand the same
`BufReader` to `csv::ReaderBuilder`.

### 4.6 Fixtures (copy into new repo `testdata/`)

| File | Rows | Content |
|------|------|---------|
| `Demo DiffFindOnlyDups dupReport.csv` | 25 | All `Duplicate;Base` (same-DB mode). |
| `Demo DiffFindDupsTheWholeLot dupReport.csv` | 64 | 48 Duplicate, 12 Changed, 2 Missing, 2 Moved; Base + Second. Paths like `O:\…\myDupFinder\testdata\filesBase\demo\…` vs `…\filesNew\demo\…`. |

Also copy `testdata/filesBase/`, `testdata/filesNew/` and the `*.xml` project
files / `*.db` if you want to regenerate reports with the .NET backend; they
are not needed by the UI tests.

---

## 5. Domain model (`dupreport-core`)

### 5.1 Types

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Category { Duplicate, Moved, Unique, Missing, New, Changed }
// ALL in this exact order — it is the chip order in the toolbar.
impl Category {
    pub const ALL: [Category; 6] = [Duplicate, Moved, Unique, Missing, New, Changed];
    pub fn parse(s: &str) -> Option<Self>;   // exact match
    pub fn as_str(self) -> &'static str;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Source { Base, Second }

pub struct Row {
    pub path: String,          // FilenameAndPath
    pub size: i64,
    pub hash: String,          // may be empty
    pub category: Category,
    pub source: Source,
    pub group_id: i32,
    // (improvement) precomputed at parse time, so filters/table don't re-split:
    pub folder_len: u32,       // byte index of last separator, or u32::MAX if none
}
impl Row {
    pub fn folder(&self) -> &str;    // "" when no separator
    pub fn filename(&self) -> &str;
}

pub struct IdenticalFolderPair {
    pub folder_a: String,   // Base side
    pub folder_b: String,   // Second side
    pub file_count: usize,  // files in the Base subtree
    pub total_size: i64,    // bytes in the Base subtree
}

pub type RowIdx = u32;      // index into Report::rows
```

Keep `Row` extensible (AGENTS §3.5): if the backend adds CSV columns later,
add `Option<…>` fields; never rely on positional tuples.

### 5.2 Errors

```rust
#[derive(Debug, thiserror::Error)]
pub enum ReportError {
    #[error("I/O error: {0}")]          Io(#[from] std::io::Error),
    #[error("CSV parse error: {0}")]    Csv(#[from] csv::Error),
    #[error("Unsupported file: {0}")]   Unsupported(String),
    #[error("Invalid report: {0}")]     InvalidReport(String),
}
```

(The old `UnknownHandle` variant disappears — there are no handles.)

The UI displays errors as `"{Type}: {message}"` in a red banner under the
toolbar, e.g. `InvalidReport: Row 7: unknown Category 'Foo'` (behaviour to
preserve).

### 5.3 Path helpers (`path.rs`) — exact semantics

Paths may contain either `\` or `/` regardless of host OS. **Never** use
`std::path::Path` for report paths (a Windows path on Linux has no
components). Work on `&str`:

- `split_folder_and_name(p) -> (&str, &str)`: split at the **last** `\` or
  `/`. No separator → `("", p)`.
  - `C:\a\b\one.txt` → (`C:\a\b`, `one.txt`); `/x/y/z.txt` → (`/x/y`, `z.txt`); `bare.txt` → (``, `bare.txt`).
  - Note `/x/y` → parent `` … and `C:\a` → parent `C:`. Top-level tree nodes
    are therefore `C:`, `D:`, `O:` (or `` for POSIX root — see 5.6).
- `parent_of(p) -> Option<&str>`: folder part, `None` if empty.
- `is_under_or_equal(path, folder)`: `folder == ""` → true; `path == folder`
  → true; else `path.starts_with(folder)` **and** the next char is `\` or `/`.
  (Prevents `C:\ab` matching folder `C:\a`.)
- `join_path(folder, name)`: `folder == ""` → `name`; else separator is `\`
  if `folder` contains `\`, otherwise `/`.

### 5.4 Parser (`parser.rs`)

```rust
pub const EXPECTED_HEADER: &str =
    "FilenameAndPath;FileSize;FileSha512Hash;Category;Source;GroupId";
pub const CHECK_REPORT_MARKER: &str = "# myDupFinder check report";
pub const PROGRESS_INTERVAL: usize = 5_000;

pub struct ParseProgress { pub rows_read: usize, pub bytes_read: u64, pub total_bytes: Option<u64> }

pub fn parse_file(path: &Path, on_progress: impl FnMut(ParseProgress)) -> Result<Vec<Row>, ReportError>;
pub fn parse_reader<R: Read>(r: R, total_bytes: Option<u64>, on_progress: impl FnMut(ParseProgress))
    -> Result<Vec<Row>, ReportError>;
pub fn job_name_from_path(path: &Path) -> String;
```

Algorithm (behaviour to preserve):

1. `BufReader::with_capacity(64 * 1024, reader)`; sniff first line (§4.5).
2. `csv::ReaderBuilder::new().delimiter(b';').quote(b'"').has_headers(true).flexible(false)`.
3. Join the parsed header fields with `;`; if `!= EXPECTED_HEADER` →
   `InvalidReport("Unexpected CSV header: '<actual>'")`.
4. Loop `read_record(&mut StringRecord)`; line number for messages is
   `rows.len() + 2`. Errors:
   - field count ≠ 6 → `"Row {n} has {k} fields, expected 6"` (csv's own
     `flexible(false)` error may fire first — either is fine);
   - `FileSize` not `i64` → `"Row {n}: invalid FileSize: {e}"`;
   - unknown category → `"Row {n}: unknown Category '{v}'"`;
   - unknown source → `"Row {n}: unknown Source '{v}'"`;
   - `GroupId` not `i32` → `"Row {n}: invalid GroupId: {e}"`.
5. Every `PROGRESS_INTERVAL` rows, and **once more after the last row**,
   call `on_progress` with `bytes_read = rdr.position().byte()` (monotonic).
6. Never read the file into one `String`.

`job_name_from_path`: file stem with a trailing `" dupReport"` stripped
(`/tmp/Foo Bar dupReport.csv` → `Foo Bar`); if no suffix, the stem.

### 5.5 Report + indexes (`report.rs`) — (improvement)

The old code re-scanned all rows for every filter change. Build indexes once
after parsing (on the background thread):

```rust
pub struct Report {
    pub job_name: String,
    pub source_path: PathBuf,
    pub rows: Vec<Row>,
    pub has_second_source: bool,          // any row.source == Second
    pub tree: FolderTree,                 // §5.6
    pub rows_by_folder: Vec<Vec<RowIdx>>, // indexed by FolderId; direct files only, file order
    pub rows_by_group: HashMap<i32, SmallVec<[RowIdx; 2]>>, // for future group expand/collapse
}
pub fn Report::load(path, on_progress) -> Result<Report, ReportError>;
```

### 5.6 Folder tree (`tree.rs`)

Arena-based, built once per report from **all** rows (both sources in one
tree — behaviour to preserve: the tree shows `C:` and `D:` side by side).

```rust
pub type FolderId = u32;
pub struct FolderNode {
    pub path: String,           // full path; "" for synthetic root
    pub name: String,           // last segment; "" for root
    pub parent: Option<FolderId>,
    pub children: Vec<FolderId>,// sorted, see below
    pub direct_file_count: u32,
    pub total_file_count: u32,  // recursive
    pub total_size: i64,        // recursive
    pub category_counts: [u32; 6], // recursive, indexed by Category as usize
}
pub struct FolderTree { pub nodes: Vec<FolderNode>, pub by_path: HashMap<String, FolderId> }
pub const ROOT: FolderId = 0;
```

Build algorithm:

1. Root node (id 0) with `path = ""`.
2. For each row: `folder = row.folder()`; `ensure_path(folder)` recursively
   creates missing ancestors (split folder → parent + name). Increment
   `direct_file_count` on that node; then walk up to the root adding
   `total_file_count += 1`, `total_size += size`, `category_counts[cat] += 1`.
3. Sort every node's `children` by name, **case-insensitive** (JS used
   `localeCompare(…, {sensitivity: 'accent'})`; in Rust compare
   `to_lowercase()` then fall back to raw for determinism).
4. Rows with no separator hang directly off the root.

Required tests (port of `folder-tree.spec.ts`): aggregates counts and sizes
recursively; sorts children alphabetically case-insensitive; bare-filename
rows under the root; Windows + POSIX split; bare filename split.

### 5.7 Query / filter (`query.rs`)

```rust
pub struct RowQuery {
    pub folder: Option<String>,           // None or "" = no folder filter
    pub include_descendants: bool,
    pub text: Option<String>,             // trimmed; None if empty
    pub hash: Option<String>,             // trimmed; None if empty
    pub categories: Option<Vec<Category>>,// None = no filter; Some(empty) = match NOTHING
    pub sources: Option<Vec<Source>>,     // same semantics
}
pub fn filter(report: &Report, q: &RowQuery) -> Vec<RowIdx>;   // file order preserved
```

All filters are AND-combined (behaviour to preserve):

- **Folder** — `include_descendants == false`: `row.folder() == folder`
  exactly. `true`: `is_under_or_equal(row.path, folder)`.
  (improvement) Use the tree: collect the selected node's subtree
  `FolderId`s and concatenate `rows_by_folder` — then re-sort indexes to
  restore file order, or iterate rows once with a `HashSet<FolderId>`/bitset.
- **Text** — case-insensitive **substring of the full path** (folder *and*
  filename). Old Rust used ASCII lowercase, old JS used Unicode
  `toLowerCase()`; the UI behaved like JS → use Unicode `to_lowercase()`
  (improvement: precompute a lowercase path per row if profiling demands it).
- **Hash** — compare case-insensitively (uppercase both). If the query is
  **≥ 8 chars → prefix match**, otherwise **exact match** (so a 4-char query
  only matches a 4-char hash — behaviour to preserve, it's in the spec).
- **Category** — row category must be in the set.
- **Source** — row source must be in the set.

No pagination is needed any more (the old `offset/limit` existed only for
IPC). The table virtualises over `Vec<RowIdx>`.

Required tests (port of `query.rs` + `row-filter.spec.ts`): exact folder
(immediate children only); folder with descendants; category; text
case-insensitive; hash prefix ≥ 8; source; empty category list → nothing;
empty source list → nothing; `None` category → everything; `C:\ab` not under
`C:\a`.

### 5.8 Identical-folders detector (`identical.rs`)

Port **exactly** (it is tested and correct for its scope):

Definitions: `tree_base` / `tree_second` = per-source folder maps
`folder path → { files: BTreeMap<filename, RowIdx>, subfolders: BTreeMap<name, child path> }`
(every ancestor up to the top is registered as a subfolder entry of its
parent).

1. **pair map**: for rows with category `Duplicate` or `Moved`, group by
   `group_id`; take the **first** Base row and **first** Second row of each
   group; if both exist insert `base.path → second.path`.
2. **forbidden folders** per source: every folder (and all its ancestors)
   that directly contains a `Unique`, `Missing`, `New` or `Changed` row of
   that source.
3. For each Base folder `F` (every key of `tree_base`):
   - **candidate**: DFS the subtree of `F` for the first file that has a
     pair partner; let `d` = depth of that file's folder below `F`; walk up
     from the partner's folder `d` more levels → candidate `G`. None found →
     skip.
   - `is_identical(F, G)` (memoised on `(F, G)`):
     - false if either is `""`; false if `F ∈ forbidden_base` or
       `G ∈ forbidden_second`; false if either node missing;
     - same number of direct files; for every filename in `F`: `G` has the
       same filename **and** `pair_map[join(F,name)] == join(G,name)`;
     - same number of subfolders; each same-named child pair recursively
       identical.
   - if identical: push `{F, G, file_count = recursive file count of F in
     tree_base, total_size = sum of sizes of Base rows under F}`.
4. Keep only **maximal** pairs: drop `(A, B)` if any `(parent^k(A),
   parent^k(B))` (same k for both, walking up in lockstep) is also a pair.
5. Sort by `total_size` descending.

Known limitation (keep, document): renamed subfolders whose contents are all
`Moved` are not detected because child names must match.

(improvement) The old `subtree_size` scanned all rows per pair (O(n·pairs)).
Compute Base-only subtree sizes bottom-up once. Also run the whole detector
on a background thread and report progress (the old one blocked).

Required tests: simple identical pair (2 files, sizes 100+200 → 1 pair
`C:\a\b` ↔ `D:\x\y`, count 2, size 300); rejected when a `Missing` row is in
the subtree; maximal pair preferred over its subfolder pair; whole-lot
fixture runs without panic.

### 5.9 Identical status per folder (`status.rs`)

Computed over the **combined** tree (§5.6) from the pair list:

```rust
pub enum IdenticalStatus { Green, Yellow, Red }
pub fn compute_statuses(tree: &FolderTree, pairs: &[IdenticalFolderPair]) -> Vec<Option<IdenticalStatus>>;
// returns all None when pairs is empty  ("no scan yet" → no dots rendered)
```

Rules (behaviour to preserve, post-order walk from root):

- Node path is `folder_a` or `folder_b` of any pair → **Green**, and paint its
  whole subtree Green (even if it holds direct files).
- Leaf (no children) and not paired → **Red**.
- Otherwise, from children's statuses: `has_yellow || (has_green && has_red)`
  → **Yellow**; all green → **Green**; else **Red**. A node with
  `direct_file_count > 0` counts as having a red child (loose files downgrade
  an all-green parent to Yellow).

Tooltips: Green "Entire subtree has an identical match elsewhere"; Yellow
"Some descendants have an identical match elsewhere"; Red "No identical match
in this subtree".

Required tests (port of `computeIdenticalStatuses` specs): empty pairs →
empty; paired folder + subtree green; parent yellow when only some children
green; parent green when all children green; loose files + green children →
yellow; paired folder with direct files stays green; red when nothing matches.

### 5.10 Formatting (`format.rs`)

- `format_size(bytes: i64) -> String`: negative → `"–"`; `< 1024` →
  `"{n} B"`; else divide by 1024 through `KB, MB, GB, TB, PB`; value `< 10`
  → one decimal (`"1.5 KB"`), else rounded integer (`"540 KB"`).
- `format_hash_short(h) -> &str`: first 12 chars (or whole if ≤ 12; `""` if
  empty).

---

## 6. Application state (`mydupfinder-ui/src/app_state.rs`)

One GPUI entity owns everything (replaces Angular `ReportStore` signals):

```rust
pub struct ReportStore {
    pub report: Option<Arc<Report>>,
    pub load: LoadState,                 // Idle | Loading { progress: Option<ParseProgress> }
    pub error: Option<String>,

    // view state
    pub selected_folder: FolderId,       // ROOT initially
    pub include_descendants: bool,       // default TRUE
    pub text_filter: String,             // ""
    pub hash_filter: String,             // ""
    pub categories: BTreeSet<Category>,  // default = all 6
    pub sources: BTreeSet<Source>,       // default from report (see below)
    pub show_hash: bool,                 // default FALSE

    // derived
    pub filtered: Arc<Vec<RowIdx>>,      // recomputed on every filter change

    // identical
    pub identical: Vec<IdenticalFolderPair>,
    pub identical_loading: bool,
    pub statuses: Vec<Option<IdenticalStatus>>, // empty until scan ran
}
```

Behaviour (preserve):

- **open_report(path)**: clear error; set loading; drop current report and
  clear identical results; on a background thread `Report::load(path, |p|
  …)` forwarding progress to the UI (throttle to ~10/s); on success set
  `report`, reset `selected_folder = ROOT`, reset
  `sources = if has_second_source { {Base, Second} } else { {Base} }`
  (prevents a stale `{Second}` filter hiding every row of a single-DB report
  whose source chips are hidden), recompute `filtered`. On error set
  `error`. Always clear loading.
- **pick_and_open**: native file dialog (CSV files); cancel → nothing.
- Every filter setter updates the field and recomputes `filtered`
  (synchronously if `rows < 50_000`, otherwise on the background executor
  with a generation counter so stale results are dropped — the old spec's
  "filter on backend above 50k" requirement maps to this).
- Toggling category/source keeps canonical order (`Category::ALL`,
  `[Base, Second]`).
- **load_identical**: set `identical_loading`; compute on background thread;
  store pairs; compute `statuses`; clear loading; errors → `error`.
- `show_hash` changes rendering only, no refilter.
- Text/hash filter values are `trim()`-ed and empty → `None` when building
  the `RowQuery`.

Call `cx.notify()` after every mutation.

---

## 7. UI specification

### 7.1 Window

- Title: `myDupFinder UI` (old default was `mydupfinderui`, 800×600). Use a
  sensible default like 1280×800, min ~800×500.
- Light and dark theme following the OS (`window.appearance()`). The old app
  only flipped background/foreground in dark mode; define a palette for both.

### 7.2 Layout

```
┌──────────────────────────────────────────────────────────────────────┐
│ Toolbar (wraps): [Open report…] Job — N rows [path filter] [hash…]   │
│   ☑ descendants ☐ hash column ……… (Duplicate)(Moved)… (Base)(Second) │
│   [Identical folders]                                                │
│ Error banner (only when error)                                       │
├───────────────┬┬─────────────────────────────────────────────────────┤
│ ☑ ● green on  ││ Showing N rows — folder: C:\…                       │
│   top         ││ Folder | Filename | Size | Category | (Hash) | Src | Group
│ ▾ /       64  ││ … virtual rows (22 px each) …                       │
│   ▸ O:    64  ││                                                     │
│ (tree pane)   ││ (or placeholder when no report is open)             │
├───────────────┴┴─────────────────────────────────────────────────────┤
│ ═ horizontal splitter ═                                              │
│ IDENTICAL FOLDERS                                              [▾]   │
│ Base folder | Second folder | Files | Total size                     │
└──────────────────────────────────────────────────────────────────────┘
```

- **Vertical splitter** between tree and table: 5 px, drag to resize. Sidebar
  default **320 px**, min **160 px**, max `window_width − 200`.
- **Identical panel** at the bottom, only rendered when
  `identical.len() > 0 || identical_loading`. Default height **220 px**, min
  **80 px**, max `window_height − 160`; draggable 5 px horizontal splitter
  above it. Header "IDENTICAL FOLDERS" with a `▾` button to collapse; when
  collapsed a full-width `▴ Identical folders` bar restores it.
- Splitters: grey `#ccc`, hover/active `#6796e1`; while dragging, suppress
  text selection and show the resize cursor. (`gpui-component`'s
  `h_resizable`/`v_resizable` provide this.)
- Placeholder when no report: "No report open. Click **Open report…** in the
  toolbar above to load a `dupReport.csv`."

### 7.3 Toolbar

Left to right, wrapping, 12 px font, 6 px gap:

1. **Open report…** button → `pick_and_open`. Shows "Loading…" and is
   disabled while loading. (improvement) While loading show a determinate
   progress bar from `bytes_read / total_bytes`.
2. Status text when a report is open: **`{job_name}`** — `{row_count} rows`.
3. Text input, placeholder `path / filename filter`; disabled without report.
4. Text input (monospace), placeholder `hash filter (≥8 chars)`; disabled
   without report.
5. Checkbox **descendants** (`include_descendants`).
6. Checkbox **hash column** (`show_hash`).
7. Flexible spacer.
8. Six **category chips** (checkbox + label, pill-shaped, `on` state = light
   blue fill `#e8f0ff` + border `#6796e1`).
9. Two **source chips** `Base`, `Second` — **only when
   `has_second_source`**.
10. **Identical folders** button → `load_identical`; shows "Scanning…",
    disabled without report or while scanning.

Error banner below: background `#fdecea`, text `#b71c1c`.

(improvement) Keyboard: `Ctrl/Cmd+O` = open; `Ctrl/Cmd+F` focuses path
filter; `Esc` in a filter clears it. Register as GPUI `actions!`.

### 7.4 Folder tree pane

- Header row: checkbox **● green on top** (default **checked**), disabled
  (greyed, `not-allowed`) until statuses exist.
- Rendered as a **flattened list** with `uniform_list` (virtualised — trees
  can have tens of thousands of folders). Flatten = pre-order walk of
  expanded nodes. Initially only the root (`""`) is expanded.
- When green-on-top is on and statuses exist, order each node's children by
  rank Green 0, Yellow 1, Red 2, unknown 3, stable (alphabetical tiebreak).
- Each row: indent `8 + depth × 14` px; toggle glyph `▾` expanded / `▸`
  collapsed / `·` leaf (disabled); status dot 8 px (green `#2ea043`, yellow
  `#d4a017`, red `#cf222e`) with tooltip — only when statuses exist; name
  (root shows `/`), ellipsised; right-aligned `total_file_count` (11 px,
  60 % opacity, tabular numbers).
- Click row → select folder (refilter). Click toggle → expand/collapse only
  (does not select). Hover bg `rgba(64,128,255,.08)`, selected
  `rgba(64,128,255,.18)`.
- (improvement) Keyboard: ↑/↓ move selection, →/← expand/collapse,
  Enter/Space select — the old one only had Enter/Space.
- Performance budget: expanding a node ≤ 50 ms. Cache the flattened list and
  rebuild only on expand/collapse/sort toggle/new report.

### 7.5 Row table

- Meta line: `Showing {n} row(s)` and, when a folder is selected,
  ` — folder: {path}` (root shows `(root)`).
- Sticky header + `uniform_list` body over `filtered: Arc<Vec<RowIdx>>`,
  **fixed row height 22 px**. No row cap.
- Columns (old CSS grid widths):

  | Column   | Width                      | Content |
  |----------|----------------------------|---------|
  | Folder   | flex 2, min 160            | `row.folder()`, ellipsis, tooltip full |
  | Filename | flex 1, min 140            | `row.filename()` |
  | Size     | 90                         | `format_size`, tooltip `"{raw} B"` |
  | Category | 100                        | coloured text (below) |
  | Hash     | 130 — **only if show_hash**| `format_hash_short`, tooltip full hash; (improvement) click copies full hash to clipboard (`cx.write_to_clipboard`) as AGENTS §4.2 requires |
  | Source   | 80                         | `Base` / `Second` |
  | Group    | 70                         | `group_id` |

- Category colours: Duplicate `#1565c0`, Moved `#6a1b9a`, Unique `#2e7d32`,
  Missing `#c62828`, New `#ef6c00`, Changed `#ad1457`.
- When `filtered` shrinks, clamp/reset the scroll position (the old code had
  a crash fix for exactly this: start index past the end).
- 12 px font, tabular numbers, 1 px `#eee` row separators.

### 7.6 Identical-folders panel

- Empty + loading → "Scanning…"; empty + not loading → "No identical folder
  pairs (or scan not started — click “Identical folders”)."
- Table: **Base folder** (link-styled button `#1565c0`, underlined — click
  selects that folder in the tree **and should expand its ancestors and
  scroll it into view** (improvement; the old one only selected)), **Second
  folder** (plain text, tooltip; (improvement) also make clickable),
  **Files** (right), **Total size** (`format_size`, right).
- Sorted as returned (total size desc).

---

## 8. Threading and performance

| Budget (from AGENTS §4.5) | How to meet it in GPUI |
|---|---|
| First paint of a 200k-row report ≤ 2 s | Parse + build tree + indexes in `cx.background_executor().spawn(…)`; UI stays responsive and shows progress; one `Arc<Report>` handed to the UI. |
| Filter change → first visible row ≤ 150 ms | Filtering 200k rows in native Rust is a few ms; still debounce text input (~100 ms) and use a generation counter for background filtering above 50k rows. |
| Tree expansion ≤ 50 ms | Arena tree + cached flattened `Vec<(FolderId, depth)>`; `uniform_list`. |
| Memory: no duplicated row set | Single `Arc<Report>`; table and filters use `RowIdx` (`u32`). Never clone `Row`s for display — format only visible rows inside the `uniform_list` callback. |

Pattern for background work:

```rust
fn open_report(&mut self, path: PathBuf, cx: &mut Context<Self>) {
    self.load = LoadState::Loading { progress: None };
    self.error = None;
    cx.notify();
    let (tx, rx) = smol::channel::unbounded::<ParseProgress>(); // or futures channel
    let task = cx.background_executor().spawn(async move {
        Report::load(&path, |p| { let _ = tx.try_send(p); })
    });
    cx.spawn(async move |this, cx| {
        // optionally drain rx to update progress
        let result = task.await;
        this.update(cx, |this, cx| { this.finish_open(result, cx); }).ok();
    }).detach();
}
```

Keep the `Task` handle in the struct (instead of `detach`) if you want
opening a second report to cancel the first.

---

## 9. Testing

- `dupreport-core`: all unit tests listed in §5.4–§5.10 (the old repo had 22
  Rust + 25 Vitest cases; port all of them). Include:
  - parser: only-dups sample, whole-lot sample with all six categories,
    rejects check report (`Unsupported`), rejects wrong header
    (`InvalidReport`), both fixtures parse, job-name extraction, progress
    fires at `PROGRESS_INTERVAL` and at the end with monotonic `bytes_read`,
    `total_bytes` passed through;
  - default source filter: `{Base, Second}` for two-source, `{Base}` for
    single-DB.
- Add a **synthetic 200k-row benchmark** (`cargo bench` or an `#[ignore]`d
  test) asserting load < 2 s in release.
- UI crate: a few `#[gpui::test]` tests with `TestAppContext` driving
  `ReportStore` (open fixture → row count; toggle category → filtered
  count; select folder; run identical scan on the whole-lot fixture).

---

## 10. CI, packaging, release

Mirror the old pipeline (`.github/workflows/build.yml`) without the Node
parts:

1. **Test** job per OS: `cargo fmt --all -- --check`,
   `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace`.
   Gating (preserve): Linux on PRs + `main` + tags; Windows on `main` +
   tags; macOS on tags only.
2. **Package** job per OS (depends on its test job):
   - Linux deps for GPUI: `libxkbcommon-dev libxkbcommon-x11-dev
     libwayland-dev libvulkan-dev libx11-xcb-dev libfontconfig-dev
     libssl-dev` (check GPUI's current list); build `--release`; bundle with
     `cargo-packager` or `cargo-bundle` (`.deb`, `.AppImage`).
   - Windows: release `.exe` (+ `windows_subsystem = "windows"` in release,
     as today's `main.rs`), optional `.msi` via `cargo-wix` or
     `cargo-packager`; embed the icon with `winresource`.
   - macOS: `.app` / `.dmg` via `cargo-bundle` / `cargo-packager`; GPUI uses
     Metal, needs Xcode CLT.
3. **Release** on `v*` tags: zip each platform's bundle and attach with
   `softprops/action-gh-release`.
- Dependabot: `cargo` + `github-actions` ecosystems (drop npm). Group GPUI
  updates and test them by hand — they can break the build.
- App identifier: `ch.rogerbriggen.mydupfinderui`; reuse the icons from
  `src-tauri/icons/` (32×32, 128×128, 128×128@2x, `.icns`, `.ico`).
- Version lives only in the workspace `Cargo.toml` now (no more syncing
  `package.json` ↔ `tauri.conf.json`).

---

## 11. Implementation plan (ordered)

1. **Workspace skeleton** — two crates, CI test job, fmt/clippy clean,
   fixtures copied.
2. **Core model + path helpers + format** with tests.
3. **Streaming parser** with progress + all parser tests.
4. **Report + FolderTree + indexes** with tree tests.
5. **Query/filter** with filter tests.
6. **Identical detector + statuses** with tests (port verbatim first, then
   apply the O(n) subtree-size improvement, tests must stay green).
7. **GPUI shell**: window, theme, `ReportStore` entity, Open-report via file
   dialog + background load + error banner.
8. **Row table** with `uniform_list`, hash toggle, tooltips, category colours.
9. **Folder tree** with `uniform_list`, expand/collapse, selection,
   descendants toggle.
10. **Toolbar filters** (text, hash, category chips, source chips).
11. **Identical panel + status dots + green-on-top + resizable splitters.**
12. **Packaging + release CI.**
13. **Parity check**: open both fixtures in old and new app side-by-side;
    counts per folder, filtered row counts and identical pairs must match.

### Deferred backlog (carry over from the old README)

- Group rows sharing a `GroupId` with expand/collapse (use
  `rows_by_group`).
- Persist last-opened report + UI prefs (column toggles, filters, splitter
  sizes) as JSON in the OS config dir.
- Progress events for the identical-folder scan.
- Detect renamed (all-`Moved`) subfolders as identical.
- Parse the `myDupFinder check` report format (comma CSV with `# Key=Value`
  header comments; columns `Category,PathMoved,ScanItemId,FilenameAndPath_DB,
  FilenameAndPath_Disk,…`).
- Column sorting, drag-and-drop a CSV onto the window, recent-files menu,
  open report from a command-line argument.

---

## 12. Mapping: old file → new location

| Old (Tauri/Angular)                                  | New (Rust/GPUI) |
|------------------------------------------------------|-----------------|
| `src-tauri/src/model.rs`                             | `dupreport-core/src/model.rs`, `path.rs` |
| `src-tauri/src/csv_parser.rs`                        | `dupreport-core/src/parser.rs` |
| `src-tauri/src/error.rs`                             | `dupreport-core/src/error.rs` (no DTO / serde needed) |
| `src-tauri/src/query.rs`, `src/app/core/backend/row-filter.ts`, `report-store.ts#applyFilter` | `dupreport-core/src/query.rs` (one implementation instead of three) |
| `src-tauri/src/identical.rs`                         | `dupreport-core/src/identical.rs` |
| `src/app/core/tree/folder-tree.ts`                   | `dupreport-core/src/tree.rs`, `status.rs` |
| `src/app/core/format.ts`                             | `dupreport-core/src/format.rs` |
| `src-tauri/src/state.rs`, `commands.rs`, `lib.rs`    | removed — replaced by `ReportStore` entity + background tasks |
| `src/app/core/backend/*-report-backend.ts`, `environment.ts` | removed — no IPC; core crate is the abstraction, tests replace the mock |
| `src/app/features/explorer/report-store.ts`          | `mydupfinder-ui/src/app_state.rs` |
| `explorer.component.ts`                              | `views/explorer.rs` |
| `toolbar.component.ts`                               | `views/toolbar.rs` |
| `folder-tree.component.ts`                           | `views/folder_tree.rs` |
| `row-table.component.ts`                             | `views/row_table.rs` |
| `identical-panel.component.ts`                       | `views/identical_panel.rs` |
| `src/styles.css` + component styles                  | `theme.rs` |
| `.github/workflows/build.yml`                        | `.github/workflows/ci.yml` (cargo only) |
