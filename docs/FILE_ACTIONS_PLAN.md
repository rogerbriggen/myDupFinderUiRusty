# Plan: file actions (delete duplicate files)

Status: **proposal, nothing implemented yet.** This plan turns the read-only
viewer into a tool that can also act on what it shows, mainly by deleting
duplicate files. It also lists what else is still missing in the app
([section 9](#9-what-else-is-missing)).

File actions were a non-goal so far (`AGENTS.md` §1, `REBUILD_RUST_GPUI.md`
§1: "may come later behind a confirmation dialog"). The first implementation
PR changes those sections.

---

## 1. Where the app stands

All 13 steps of `REBUILD_RUST_GPUI.md` §11 are merged and `v0.1.0` is tagged.
What matters for file actions:

| Area | Today | Consequence |
|---|---|---|
| Row table | Rows cannot be selected or focused. The only click target is the hash cell (copy). | Selection and marking have to be built first. |
| Groups | `Report::rows_by_group` exists but nothing uses it. The other copies of a file are only visible by filtering by hash. | The user cannot see what stays when a file is deleted. |
| File system | The app never touches the files in the report. Report paths are handled as `&str` only. | A new, clearly separated place for `std::fs` access is needed. |
| Persistence | None. `serde`, `serde_json`, `directories` and `rfd` are declared in the workspace `Cargo.toml` but unused. | A deletion is forgotten when the report is reopened. |
| Dialogs, menus | No modal dialog, no context menu, no menu bar. gpui-component ships `dialog`, `menu::ContextMenuExt` and `notification`. | Can be used as they are. |
| Platform calls | `gpui-pre` 0.3.11 has `App::reveal_path` and `App::open_with_system`. | "Show in file manager" and "Open" need no new crate. |

---

## 2. Goals and non-goals

**Goals**

1. Mark files for deletion, by hand and with rules ("all Second copies below
   this folder", "the Second folder of this identical pair").
2. Delete the marked files after a review dialog, to the recycle bin by
   default.
3. Never delete the last copy of a file's content (section 3).
4. Remember what was deleted, so a reopened report shows it.
5. Small non-destructive actions on a row or folder: copy path, show in file
   manager, open.

**Non-goals for this plan**

- Moving or renaming files, replacing duplicates by hard links or symlinks.
- Deleting `Unique`, `Missing`, `New` or `Changed` files. Only `Duplicate` and
  `Moved` rows have a proven second copy.
- Updating the myDupFinder scan databases. After deleting, the user rescans
  with the backend.
- Undo inside the app. Undo is the recycle bin.

---

## 3. Safety rules

Deleting is the first irreversible thing this app does, and the report it
works from is a snapshot that may be days old. These rules are the core of the
feature. Each one gets its own tests.

### 3.1 Rules on the report (pure, in `dupreport-core`)

- **R1 Category.** Only `Duplicate` and `Moved` rows can be marked.
- **R2 A keeper must remain.** For every marked row there must be at least one
  row with the **same non-empty hash and the same size** that is not marked
  and not already deleted. The check uses the hash, not only `GroupId`, so it
  holds no matter how the backend groups three or more copies. A row with an
  empty hash can never be marked.
- **R3 No silent override.** If R2 fails for a hash, none of its rows is
  deleted and the review dialog names the conflict. There is no "delete
  anyway".

### 3.2 Checks on disk (in the new `dupreport-actions` crate)

Run for every file **directly before** it is deleted, not once up front:

- **D1 Target is what the report says.** It exists, is a regular file (not a
  symlink, not a directory) and has the reported size.
- **D2 Keeper is still there.** At least one keeper from R2 passes D1.
- **D3 Target and keeper are two files.** They must not be the same physical
  file reached through two paths (mapped drive and UNC path, junction,
  symlinked folder, `subst`). Compared by file identity with the `same-file`
  crate. Without this check the "duplicate" could be the only copy.
- **D4 Content check (verification level).**
  - *Quick*: D1 to D3 only.
  - *Full* (default): additionally re-hash target and keeper with SHA-512 and
    compare both to the report hash. Slow on large files, shown with progress,
    can be cancelled.
- **D5 Skip, never guess.** A file that fails any check is skipped with a
  reason (`NotFound`, `SizeChanged`, `HashChanged`, `KeeperMissing`,
  `SameFile`, `NotAFile`, `Io`). The run continues with the next file.

### 3.3 Paths

Report paths are strings and may come from another machine or OS. They are
converted to `PathBuf` in exactly one place, in `dupreport-actions`. A path
that does not exist on this machine fails D1 and is skipped. There is no path
remapping in this plan (possible follow-up).

### 3.4 Recycle bin

Default is the recycle bin through the `trash` crate (Windows Recycle Bin,
macOS Trash, freedesktop trash). Permanent deletion is a separate choice in
the review dialog and is never the default.

Network shares and some removable drives have no recycle bin. On Windows the
shell then deletes permanently. The run therefore tests each volume before the
first delete on it; if it has no recycle bin, the files on it are skipped
unless the user chose permanent deletion. How to detect this reliably is an
open point (section 8).

---

## 4. Architecture

```
crates/dupreport-core/      + actions.rs    marks, rules R1 to R3, mark rules, DeletionPlan  (no fs)
crates/dupreport-actions/   new crate       disk checks D1 to D5, delete, journal            (std::fs, no GPUI)
crates/mydupfinder-ui/      + views         selection, mark column, menus, dialogs, progress
```

`dupreport-core` stays free of file-system access for report content (it only
reads the CSV), so its tests stay pure. `dupreport-actions` depends on the
core and is tested against temp directories. The UI only calls into both.

### 4.1 `dupreport-core::actions`

```rust
pub struct Marks { /* bitset over RowIdx */ }

pub enum MarkRefusal { Category, EmptyHash, WouldDeleteAllCopies }

pub struct DeletionPlan {
    pub items: Vec<PlanItem>,       // marked row + its keeper rows
    pub conflicts: Vec<Conflict>,   // hashes where R2 fails
    pub total_size: i64,
}

pub fn can_mark(report: &Report, marks: &Marks, deleted: &Marks, row: RowIdx) -> Result<(), MarkRefusal>;
pub fn build_plan(report: &Report, marks: &Marks, deleted: &Marks) -> DeletionPlan;

// Mark rules; each returns the rows it would add and never breaks R2.
pub fn mark_source_under(report: &Report, folder: FolderId, source: Source, ..) -> Vec<RowIdx>;
pub fn mark_all_but_one_per_hash(report: &Report, rows: &[RowIdx], keep: KeepRule, ..) -> Vec<RowIdx>;
```

`Report` gets a hash index (`rows_by_hash`), built on first use so opening a
report does not get slower. `KeepRule` starts with "first in report order" and
"shortest path"; "oldest/newest" is impossible because the CSV has no
timestamps.

### 4.2 `dupreport-actions`

```rust
pub enum Verify { Quick, Full }
pub enum Mode { RecycleBin, Permanent }
pub enum Outcome { Deleted, Skipped(SkipReason), Failed(ActionError) }

pub fn execute(plan: &DeletionPlan, report: &Report, opts: Options,
               on_progress: impl FnMut(Progress), cancel: &AtomicBool) -> Vec<(RowIdx, Outcome)>;
```

- Errors are a `thiserror` enum, as everywhere else.
- New dependencies: `trash`, `same-file`, `sha2`, plus `serde`/`serde_json`
  for the journal. Pin versions when adding them.
- **Journal:** one JSON line per attempt (time, path, size, hash, mode,
  outcome), appended to `<report file>.actions.jsonl` next to the report. If
  that folder is read-only, fall back to the OS config dir (`directories`).
  The journal is written before the next file is touched.
- A `dry_run` option runs all checks and deletes nothing. The tests and the
  review dialog's "Check now" button use it.

### 4.3 `ReportStore`

New state: `marks`, `deleted` (from the journal plus this session),
`focused_row`, `action: ActionState` (`Idle | Checking | Deleting { progress }
| Done { summary }`). The report itself is never mutated. Deleted rows stay in
the table, greyed and struck through, with a toolbar chip "hide deleted". Tree
counts and identical-folder dots keep describing the report as written; a
banner says "N files of this report were deleted, rescan for fresh numbers".

Opening a report loads its journal, so earlier deletions show up again.

---

## 5. UI

1. **Row focus and selection** in the table: click, Ctrl/Shift+click,
   ↑/↓, Ctrl+A. Selected-row colour already exists (`Palette::row_selected`).
2. **Mark column** (checkbox, first column). Disabled with a tooltip giving
   the reason when R1 or R2 refuses. `Space` toggles the selected rows.
3. **Group strip** below the table: every row with the focused row's hash,
   with source, folder and mark state, so the user sees what stays. This also
   covers part of the open "group rows by `GroupId`" user story.
4. **Context menu** on table rows and tree folders:
   - Row: Copy path · Show in file manager · Open · Mark / Unmark · Mark all
     other copies of this file.
   - Folder: Copy path · Show in file manager · Mark duplicates below this
     folder (Base / Second) · Unmark below this folder.
5. **Identical panel:** per pair "Mark Base folder" / "Mark Second folder".
   Marks every file below that side; R2 holds because the other side stays.
6. **Status bar** at the bottom: `N files marked, 12.3 GB` · *Clear marks* ·
   **Review and delete…**
7. **Review dialog** (modal): number of files and bytes, conflicts (R3),
   scrollable list "delete → kept copy", choices *Recycle bin / Permanent* and
   *Full / Quick* verification, **Check now** (dry run) and **Delete**. The
   Delete button states the number ("Move 1,204 files to the recycle bin").
   Permanent deletion needs a second confirmation.
8. **Progress** with cancel, then a **result summary**: deleted, skipped by
   reason, failed, with "Copy list" and "Open journal".
9. **Empty folders:** optional checkbox in the review dialog, "also remove
   folders that are empty afterwards". Only removes a folder that is really
   empty on disk (`remove_dir`, never recursive).

---

## 6. Delivery in pull requests

One concern per PR, each with tests and its `docs/DEVIATIONS.md` section.

| # | PR | Content | Destructive? |
|---|---|---|---|
| 1 | `docs:` scope change | Update non-goals in `AGENTS.md` and the brief, link this plan, fix the outdated README status line. | no |
| 2 | `feat:` row selection | Focus, multi-selection and keyboard in the row table. | no |
| 3 | `feat:` context menu | Copy path, show in file manager, open, on rows and tree folders. | no |
| 4 | `feat:` group strip | Hash index in the core, strip with all copies of the focused row. | no |
| 5 | `feat(core):` marks and plan | `actions.rs`: `Marks`, R1 to R3, mark rules, `DeletionPlan`. | no |
| 6 | `feat:` marking UI | Mark column, context-menu and identical-panel mark commands, status bar. Still no delete button. | no |
| 7 | `feat:` `dupreport-actions` | Disk checks D1 to D5, dry run, recycle-bin and permanent delete, journal. Not wired into the UI. | library only |
| 8 | `feat:` review and delete | Review dialog, progress, cancel, summary, deleted overlay, journal reload on open. | **yes** |
| 9 | `feat:` empty folders | Optional removal of folders left empty. | **yes** |

PRs 2 to 4 are useful on their own and carry no risk. The app cannot delete
anything before PR 8.

---

## 7. Tests

- **Core (PR 5):** R1 per category; R2 with pairs, with three or more copies,
  across `GroupId`s that share a hash, with already deleted keepers, with
  empty hashes; every mark rule against both fixtures and never able to
  produce a conflict; `build_plan` is deterministic.
- **Actions (PR 7):** temp-dir tests for every `SkipReason`: missing target,
  changed size, changed content with equal size, missing keeper, target and
  keeper as the same file through a hard link and through a symlinked folder,
  directory instead of file, cancel between two files. Dry run deletes
  nothing. Journal has one line per attempt and survives a reload.
  Recycle-bin tests are `#[ignore]`d on CI (headless Linux has no trash
  location guaranteed) and run by hand per OS.
- **UI (PRs 2, 6, 8):** `#[gpui::test]` on the store: marks survive filter
  changes, refused marks stay off, opening another report clears marks,
  the review dialog's counts equal the plan, a finished run moves rows from
  `marks` to `deleted`.
- **By hand before releasing PR 8:** on Windows, macOS and Linux, a copy of
  `testdata/filesBase` + `filesNew` from the backend repo: delete to the
  recycle bin, restore, permanent delete, a network share, a read-only file,
  a file open in another program.

---

## 8. Open decisions

| # | Question | Recommendation |
|---|---|---|
| 1 | Default verification level | **Full** (re-hash). The report can be old and has no timestamps, so size alone cannot detect a rewritten file. Quick stays available. |
| 2 | Volumes without a recycle bin | Skip and report, unless permanent deletion was chosen. Needs a spike on what the `trash` crate does on Windows network drives. |
| 3 | Where the journal lives | Next to the report, config dir as fallback. |
| 4 | Are `Moved` rows deletable | Yes, they have the same hash on both sides. Drop it if you want the first version stricter. |
| 5 | Single-DB reports: which copy to keep by default | No default; the user picks a `KeepRule` in the mark command. |
| 6 | Backend: add `FileLastModificationUTC` to the CSV | Worth asking for. Size plus modification time would make the Quick level almost as safe as Full and much faster. |
| 7 | Backend: scan databases are stale after a deletion | Document "rescan after deleting". A later option is to export the journal in a format the backend can apply. |

---

## 9. What else is missing

Independent of file actions, found while reading the code, the brief's backlog
and `docs/DEVIATIONS.md`.

**Functional gaps**

- **Group expand/collapse by `GroupId`.** A primary user story in `AGENTS.md`
  §1 and §4.1, still not implemented. The group strip (PR 4) covers the
  "see the other side" part.
- **Reclaimable space.** There is no number for "how much would deleting the
  duplicates free", per report or per folder. For a duplicate finder this is
  the most useful summary and it is cheap to compute from the hash index.
- **Persistence.** Last opened report, recent files, filters, column toggles,
  splitter sizes, tree options. Everything resets on restart.
- **Column sorting** in the row table (for example by size).
- **Copy / export.** Only the hash can be copied. No copy of a path or row,
  no export of the filtered rows or the identical-pairs list.
- **Check report** (`myDupFinder check`) is detected and rejected, not shown.
- **Renamed folders** whose files are all `Moved` are not found by the
  identical detector (known limitation).
- **Drag and drop** of a CSV onto the window; `*.csv` filter in the open
  dialog (`rfd` is declared for this but unused).
- **Progress and cancel** for the identical-folders scan.

**Platform and polish**

- **No menu bar.** On macOS there is no app menu (About, Quit) at all.
- **No About or help view.** The version is only in the title and toolbar.
- **Dark mode** was never checked visually; **macOS** was never run by a
  person (see the "Open" rows in `docs/DEVIATIONS.md`).
- No tabular-number font feature; filter inputs have no debounce and no
  clear button.
- No log file. The journal from this plan covers deletions only.

**Release and housekeeping**

- Installers are **not code-signed** (SmartScreen and Gatekeeper warn).
- Debian pre-release versions sort wrongly (`0.2.0-alpha.1` above `0.2.0`).
- `README.md` still says "early development, the workspace skeleton is in
  place".
- Unused workspace dependencies: `serde`, `serde_json`, `directories`, `rfd`
  (the first three get used by this plan).
- Real mouse clicks and drags have no automated test anywhere in the UI.
