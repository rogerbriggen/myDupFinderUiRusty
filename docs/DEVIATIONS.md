# Deviations from `REBUILD_RUST_GPUI.md`

Running log of every place where the implementation knowingly differs from
[`REBUILD_RUST_GPUI.md`](REBUILD_RUST_GPUI.md) — spec ambiguities that had to
be resolved, intentional changes, and parts that are deferred to a later step.

**Rule:** every PR that deviates from the brief adds its entries here under its
own heading. When the PR is merged, its section moves into "Already in `main`".

Legend: **Change** = behaves differently from the brief on purpose ·
**Clarification** = brief was ambiguous or contradictory, this is the chosen
reading · **Addition** = not in the brief (or pulled forward from the backlog) ·
**Deferred** = not done yet, planned for a later step · **Open** = known gap
without a planned step yet.

---

## Already in `main`

### PR #1 — Workspace skeleton and CI (plan step 1)

| Type | Area | Deviation |
|---|---|---|
| Clarification | §2 Dependencies | The brief names the `gpui` crate. `gpui-component` 0.9.0 (latest) is built on Zed's snapshot crate **`gpui-pre` =0.3.11**, so the UI uses `gpui = { package = "gpui-pre" }`; the older crates.io `gpui` 0.2.2 is incompatible. Starting the app needs the separate crate **`gpui-pre-platform`** (`gpui_platform::application()`). All three are pinned with `=` and must be bumped together. |
| Clarification | §2 MSRV | `rust-version = "1.99"` and `rust-toolchain.toml` pin the toolchain the project was verified with, not a minimum derived from GPUI. CI installs the toolchain from `rust-toolchain.toml` instead of `@stable`. |
| Change | §10.1 CI gating | Windows and macOS jobs can additionally be started manually (`workflow_dispatch`). Rustfmt is its own job and runs on every trigger. |
| Deferred | §10.2–10.3 | Packaging (`.deb`/`.AppImage`/`.msi`/`.dmg`), icon embedding (`winresource`) and the release job → plan step 12. Note: `gpui-component`'s `gpui-fast` feature brings its own Windows manifest; watch for a conflict when embedding resources. |
| Clarification | §4.6 Fixtures | Only the two `dupReport.csv` files were copied to `testdata/` (the optional `filesBase/`, `filesNew/`, `*.xml`, `*.db` were not). `.gitattributes` keeps them byte-exact (CRLF). Icons copied to `assets/icons/` (subset: 32, 128, 128@2x, `.ico`, `.icns`, `.png`). |
| Clarification | §3 Repo files | The brief suggests copying it as `AGENTS.md`. Instead the existing `AGENTS.md` was updated (Tauri/Angular → GPUI) and links to the brief; `CLAUDE.md` imports `AGENTS.md`. |

### PR #2 — Core domain logic (plan steps 2–6)

| Type | Area | Deviation |
|---|---|---|
| Change | §5.4 Parser | `csv` reader uses `flexible(true)` plus an own field-count check, so a wrong field count always yields the spec message `InvalidReport("Row N has K fields, expected 6")` instead of a raw `csv` error. |
| Clarification | §5.4 Progress | The final progress callback always fires, even when the row count is an exact multiple of `PROGRESS_INTERVAL` — the same row count can then be reported twice. |
| Change | §5.8 Identical detector | File pairs are matched on the **real row paths**, not on `join_path(F, name)`. Per §5.3, `join_path("C:", "x")` gives `C:/x`, which would never equal the row path `C:\x`. |
| Clarification | §5.8 Candidate search | The DFS for a candidate partner checks a folder's files before its subfolders, both in name order (the brief doesn't specify an order). |
| Change | §5.8 Performance | Folders that can never be identical (forbidden) are skipped before the candidate DFS. Same result, faster. Base subtree sizes are computed bottom-up once (the improvement the brief asks for). |
| Clarification | §5.8 Sorting | Pairs with equal `total_size` are tie-broken by Base folder path, so the output is deterministic. |
| Clarification | §5.7 Query | A `RowQuery.folder` that has no node in the tree matches **nothing**. `filter` trims `text`/`hash` itself as well (the store also does). |
| Change | §5.5 / §9 API | `Report::default_sources()` returns `Vec<Source>` (canonical order) rather than a set. `FolderTree::build` returns the tree **and** `rows_by_folder` in one pass. Additional constructor `Report::from_rows(job_name, source_path, rows)` for tests. |
| Addition | §5.2 Errors | `ReportError::kind()` returns the variant name used for the UI banner. |
| Open | §5.8 | Known limitation kept as specified: renamed subfolders whose files are all `Moved` are not detected (documented in `identical.rs`, covered by a test). |

---

## In this PR (PR #3 — GPUI shell and row table, plan steps 7–8)

| Type | Area | Deviation |
|---|---|---|
| Addition | §5.2 Errors (core) | `ReportError::message()` (message without the `Display` prefix) and `banner()` (`"{Kind}: {message}"`), used for the error banner. |
| Change | §6 Errors | File-dialog failures are shown as `Dialog: …` — an error kind that is not a `ReportError` variant. |
| Change | §6 `pick_and_open` | The native dialog (`prompt_for_paths`) offers **no `*.csv` extension filter**; GPUI has no API for it. `rfd` (mentioned as fallback in §2) is not used yet. |
| Addition | Backlog item | Opening a report given as the first command-line argument (`cargo run -p mydupfinder-ui -- "<file>.csv"`) was pulled forward from the deferred backlog — useful for manual testing. |
| Clarification | §6 Progress | Progress is forwarded through a `smol` channel and throttled to one update per 100 ms (≈10/s as specified). |
| Clarification | §6 Query building | When all categories / all sources are selected, the store sends `None` (no filter) instead of the full list — same result, cheaper. |
| Clarification | §7.1 Theme | `gpui_component::init` forces its own theme to light; the explorer re-syncs it with the OS appearance (`Theme::sync_system_appearance`) on start and on every appearance change, in addition to the app's own `Palette`. |
| Clarification | §7.5 Scroll | Whenever the `filtered` set changes (new `Arc`), the table scrolls back to the top (the brief allows "clamp/reset"). |
| Open | §7.5 Font | No tabular-number font feature — no GPUI API for it was found. |
| Deferred | §6 / §7.6 | Identical-folder fields (`identical`, `identical_loading`, `statuses`) are not in `ReportStore` yet → step 11. |
| Deferred | §7.3 Toolbar | Only *Open report…* (+ progress bar), status text and *hash column* are present. Text/hash inputs, *descendants* checkbox, category/source chips and *Identical folders* button → step 10 (11 for the button). `Ctrl/Cmd+F` / `Esc` → step 10. |
| Deferred | §7.2 / §7.4 | No tree pane and no splitters yet; the row table spans the full width → steps 9 and 11. |
| Open | Tests | The `Ctrl/Cmd+O` action dispatch has no automated test (the store's open path is tested). |
| Clarification | Code | Setters and palette fields not yet used by UI controls carry targeted `#[expect(dead_code, reason = …)]`; they are removed as steps 9–11 wire them up. |
