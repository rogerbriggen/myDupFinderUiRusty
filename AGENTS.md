# AGENTS.md

Instructions for Claude Code, GitHub Copilot, Codex (and other AI assistants) working on the
**myDupFinder UI Rusty** repository.

---

## 1. Purpose of this repository

A desktop UI that visualizes the duplicate / whole-lot reports produced by
[myDupFinder](https://github.com/rogerbriggen/myDupFinder) (the .NET 10
backend). The UI loads the CSV report (`<JobName> dupReport.csv`) and
presents it as an **explorer-like tree** so the user can navigate, filter,
and inspect duplicate / missing / new / changed / moved files at scale.

This repo does NOT scan files itself. It only consumes the artifacts the
backend produces (CSV report today; maybe SQLite DB or JSON later).

### Primary user stories

- Open a `dupReport.csv` and browse it as a folder tree (per scan-root).
- See which **complete subfolders are identical** across two roots
  (every file in folder A has a duplicate in folder B and vice-versa).
- Filter by folder/filename substring, by hash, and by category
  (`Duplicate`, `Moved`, `Unique`, `Missing`, `New`, `Changed`).
- For each row show: folder, filename, hash, category.
  The hash column must be toggleable (off by default for readability).
- Group rows that share a `GroupId` and let the user expand/collapse them.

### Non-goals (for now)

- No scanning, hashing, or DB writes — read-only over the report.
- No automatic file deletion / move. (May come later behind a confirmation
  dialog; out of scope for v1.)

---

## 2. Tech stack

| Layer    | Choice                                                   | Notes                                                        |
| -------- | -------------------------------------------------------- | ------------------------------------------------------------ |
| UI       | **Rust + GPUI** (`gpui-pre` + `gpui-component`)          | Native; no webview / JS / IPC. Versions pinned exactly.      |
| Logic    | **Rust**, `dupreport-core` crate (no GPUI dependency)    | `csv`, `thiserror`; testable headless.                       |
| Tests    | `cargo test --workspace`; `gpui::TestAppContext` for views |                                                            |

The implementation brief (architecture, domain model, UI spec, performance
budgets, CI/release, ordered plan) is **`docs/REBUILD_RUST_GPUI.md`**. Follow
its section 11 in order.

### Repository layout & commands

```
Cargo.toml                 # virtual workspace; version + shared deps live here only
crates/dupreport-core/     # pure logic: model, parser, tree, query, identical, status, format
crates/mydupfinder-ui/     # GPUI binary (views/, app_state.rs, theme.rs)
testdata/                  # real report fixtures (CRLF, byte-exact: never normalise)
assets/icons/              # app icons
```

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p mydupfinder-ui
```


## 3. Input format — the CSV report

The backend writes the report to `<ReportPath>/<JobName> dupReport.csv`.
It is produced by both modes of `MyDupFinderFindDupsJobDTO.EFindDupsMode`:

- `FindOnlyDups` — only `Duplicate` rows. Single-DB or two-DB.
- `FindDupsTheWholeLot` — every file, classified into one of:
  `Duplicate`, `Moved`, `Unique`, `Missing`, `New`, `Changed`.

There are 2 real reports for writing tests against:

- `testdata/Demo DiffFindOnlyDups dupReport.csv` for `FindOnlyDups`
- `testdata/Demo DiffFindDupsTheWholeLot dupReport.csv` for `FindDupsTheWholeLot`

### 3.1 Header line (exact, byte-for-byte)

```
FilenameAndPath;FileSize;FileSha512Hash;Category;Source;GroupId
```

- Delimiter: **semicolon (`;`)**. Not RFC 4180. No header comments.
- Encoding: UTF-8, no BOM. Line endings as written by .NET (`\r\n` on Windows).
- Only `FilenameAndPath` is quoted (always, with `"`). Embedded `"` becomes `""`.
- Other fields are written raw — they cannot contain `;` by construction
  (sizes are integers, hashes are hex, enums are identifiers, GroupId is int).

### 3.2 Columns

| #   | Column            | Type   | Notes                                                                                                                                                                                                                                           |
| --- | ----------------- | ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | `FilenameAndPath` | string | Full path, e.g. `C:\photos\2024\img.jpg`. Always quoted.                                                                                                                                                                                        |
| 2   | `FileSize`        | int64  | Bytes.                                                                                                                                                                                                                                          |
| 3   | `FileSha512Hash`  | hex    | Lowercase SHA-512 hex. May be empty for `New` rows that were not hashed.                                                                                                                                                                        |
| 4   | `Category`        | enum   | One of: `Duplicate`, `Moved`, `Unique`, `Missing`, `New`, `Changed`.                                                                                                                                                                            |
| 5   | `Source`          | enum   | `Base` or `Second`. Which DB the row came from. Single-DB jobs use `Base` for all rows.                                                                                                                                                         |
| 6   | `GroupId`         | int32  | Monotonically increasing. Rows describing the same logical file (both sides of a Duplicate / Changed / Moved pair, or every copy in a same-DB duplicate group) share one GroupId. `Missing`, `New`, `Unique` each get their own unique GroupId. |

### 3.3 Sort order in the file

Rows are pre-sorted by the backend and SHOULD be presented in this order
unless the UI is actively filtering/sorting:

```
ORDER BY Category, GroupId, Source, FilenameAndPath (case-insensitive)
```

### 3.4 Category semantics (memorize these)

- **Duplicate** — same hash across rows. In same-DB mode every row in the
  group is a separate copy on disk. In two-DB mode each pair has one
  `Base` and one `Second`.
- **Moved** — same hash + same filename, but different relative path between
  the two DBs. Paired row (Base + Second), shared GroupId.
- **Changed** — same relative path, different hash. Paired (Base + Second).
- **Missing** — present in Base DB, absent in Second DB. Singleton row.
- **New** — present in Second DB, absent in Base DB. Singleton row.
- **Unique** — file has no duplicate anywhere in the report. Singleton row.

### 3.5 What is NOT in the CSV (and what to do about it)

The CSV is intentionally lean. Several fields from the underlying
`ScanItemDto` are NOT present:

- `PathBase` (the relative-path anchor)
- All timestamps (`FileCreationUTC`, `FileLastModificationUTC`,
  `FirstScanDateUTC`, `LastScanDateUTC`, `LastSha512ScanDateUTC`)
- `ScanName`, `OriginComputer`, `ScanExecutionComputer`
- Job metadata (which DBs were compared, when, by whom)

For v1 the UI shows only what the CSV provides. If extra columns become
necessary later the backend will add them to the CSV — do not invent
derived data (filesystem `stat` calls, etc.) on the UI side.

Design `Row` so additional columns are a non-breaking addition
(prefer an extensible `Row` shape with optional fields over a fixed
positional tuple).

### 3.6 Sibling format: the _check_ report

`myDupFinder check` produces a different CSV (RFC 4180, comma-separated,
with `# Key=Value` header comments and columns
`Category,PathMoved,ScanItemId,FilenameAndPath_DB,FilenameAndPath_Disk,…`).
**That is a separate file with a separate schema** and is out of scope for
v1. Detect it by sniffing the first line: if it starts with
`# myDupFinder check report`, show a "not supported yet" message rather
than parsing it as a dup report.

---

## 4. UI requirements

### 4.1 Layout

```
┌───────────────────────────────────────────────────────────┐
│ Toolbar: Open report · Filters · Column toggles · Search  │
├──────────────┬────────────────────────────────────────────┤
│              │                                            │
│  Tree pane   │   Detail / row pane                        │
│  (folders)   │   (virtual-scroll table of files)          │
│              │                                            │
└──────────────┴────────────────────────────────────────────┘
```

- **Tree pane (left):** folder hierarchy reconstructed from
  `FilenameAndPath`. Each folder node shows its recursive file count and an
  identical-status dot (see 4.3).
- **Detail pane (right):** rows for the selected folder (and optionally
  descendants). Use a virtualised `uniform_list` — reports can have
  hundreds of thousands of rows.
- Rows sharing a `GroupId` are visually grouped; expand/collapse toggles
  show the other side(s) of the pair/group.

### 4.2 Columns and toggles

Default visible: **Folder, Filename, Size, Category**.
Toggleable (off by default): **Hash**.

- Hash: show first 12 chars + tooltip with full hash; copy-on-click.
- Size: human-readable (`1.2 MB`), raw bytes in tooltip.

### 4.3 Identical-subfolders detector

Compute in `dupreport-core` on a background thread. A folder pair `(A, B)` is "completely identical"
when:

1. For every file under `A` there is a `Duplicate` or `Moved` row whose
   counterpart is under `B`, AND
2. For every file under `B` there is a `Duplicate` or `Moved` row whose
   counterpart is under `A`, AND
3. Neither subtree contains any `Unique`, `Missing`, `New`, or `Changed`
   rows.

Return a list of `(folderA, folderB, fileCount, totalSize)` pairs sorted
by `totalSize desc`. Surface them in the tree (folder nodes get an
"identical to: …" chip) and in a dedicated "Identical folders" panel
accessible from the toolbar.

Implementation hint: walk folders bottom-up; a folder is identical to
another iff each of its immediate children is either (a) a file with a
`Duplicate`/`Moved` partner in the corresponding child of the other
folder, or (b) a subfolder already proven identical to the corresponding
subfolder of the other folder.

### 4.4 Filters

Combined with AND semantics. Each filter resettable independently.

- **Path/filename substring** (case-insensitive, applies to folder path
  AND filename).
- **Hash** — exact match (paste full SHA-512) or prefix match (≥8 chars).
- **Category** — multi-select chips. Default: all selected.
- **Source** — `Base` / `Second` / both. Hidden when the report contains
  only one source.

Filtering runs over indexes of the single in-memory `Report`. Below 50k rows
it is synchronous; above that it runs on the background executor with a
generation counter so stale results are dropped.

### 4.5 Performance budget

- First paint of an opened 200k-row report: ≤ 2s on a typical laptop.
- Filter change → first visible row: ≤ 150ms.
- Tree expansion: ≤ 50ms per node.
- Memory: one `Arc<Report>`; tables and filters use `RowIdx` (`u32`) and never
  clone `Row`s.

---

## 5. Project conventions

### Rust

- CSV parsing via the `csv` crate, configured with `;` delimiter and
  `quote(b'"')`. Stream rows; do NOT load the whole file into one `String`.
- Long-running work (parse, identical-folder scan) runs on GPUI's background
  executor and reports progress back to the `ReportStore` entity.
- Errors are typed enums (`thiserror`), never bare `String`s.
- Never use `std::path::Path` for report paths (they may use `\` or `/`
  regardless of host OS); work on `&str`.
- `cargo fmt` on touched files; `cargo clippy --workspace --all-targets -- -D warnings`
  and `cargo test --workspace` are part of CI.
- The version lives only in the workspace `Cargo.toml`.

### Commits / PRs

- Conventional Commits (`feat:`, `fix:`, `chore:`, `docs:`, `refactor:`).
- One concern per PR. Never commit to `main` directly.
- Update this file when the report schema changes or a major UI contract
  shifts.
- Every deviation from `docs/REBUILD_RUST_GPUI.md` (resolved ambiguity,
  intentional change, deferred part) goes into `docs/DEVIATIONS.md`. Add it
  in the PR itself as a new `### PR #<n> — <title>` section at the end of
  "Already in `main`" (the end of the file), so the file is correct as soon as
  the PR is merged and nothing has to be moved afterwards. Open the PR first
  if you need its number.

---

## 7. Reference: the backend types

Verbatim shape of the DTOs the backend uses (C#). The UI does not need
to re-implement them, but knowing them helps when reading the source repo.

```csharp
// MyDupFinderFindDupsJobDTO.cs
public enum EFindDupsMode { FindOnlyDups, FindDupsTheWholeLot }

public class MyDupFinderFindDupsJobDTO {
    public string JobName;
    public string DatabaseFileBase;   // Base DB (always required)
    public string DatabaseFile;       // Second DB (optional → single-DB mode)
    public EFindDupsMode FindDupsMode;
    public string ReportPath;         // directory; report is "<ReportPath><JobName> dupReport.csv"
}

// DupReportWriter.cs — one CSV row
internal sealed record DupReportRow(
    ScanItemDto Item,
    DupReportCategory Category,   // Duplicate, Moved, Unique, Missing, New, Changed
    DupReportSource   Source,     // Base, Second
    int               GroupId);
```

When in doubt, the source of truth is
`src/rogerbriggen.myDupFinderLib/FindDups/DupReportWriter.cs` in the
backend repo.
