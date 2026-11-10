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

### PR #3 — GPUI shell, row table and review fixes (plan steps 7–8)

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
| Clarification | §6 Progress | Throttling never drops the parser's final update: an update held back inside the throttle window is delivered once parsing ends. |
| Change | §5.8 Identical detector (core fix) | **Bug fix of PR #2:** building the per-source folder maps stopped after linking a folder to its immediate parent, so grandparents and above were never registered and pairs above them were never found (the old Tauri code walked to the top). Now every ancestor is linked. Whole-lot fixture results are unchanged (asserted in a test). |
| Change | §5.8 Performance (core) | The detector returns early when there is no Base/Second pair at all (e.g. single-source reports), and the "first paired file in subtree" search is memoised per folder, so the candidate search is linear instead of one subtree DFS per folder. |
| Clarification | §4.1 / §5.4 Header | Header validation compares the **parsed** header fields joined with `;` (exactly the algorithm in §5.4 step 3), not the raw bytes. A quoted header field or a UTF-8 BOM (which the `csv` crate strips) is therefore accepted. Kept on purpose: such files are otherwise valid reports. |
| Clarification | §5.9 Statuses | `compute_statuses` returns all `None` for an empty pair list, as specified ("no scan yet" → no dots). A completed scan that found nothing therefore looks like "not scanned" in the tree; the panel text in §7.6 covers both cases. If the UI should show all-red after an empty scan, step 11 tracks a "scan done" flag in `ReportStore` — no core change needed. |

### PR #4 — Folder tree, chain auto-expansion (plan step 9)

| Type | Area | Deviation |
|---|---|---|
| Clarification | §7.4 Flatten model location | The flattened list and expansion state live in the UI crate (`tree_view_model.rs`), not in `dupreport-core`: expansion and status-rank ordering are presentation state, the core stays report data only. The model is GPUI-free and unit-tested; the cached list is an `Arc<Vec<(FolderId, depth)>>`, rebuilt only on expand/collapse, new report or `set_ranks`. |
| Clarification | §7.4 "Initially only the root is expanded" | Read as: the root's children are visible (rows: `/`, then its direct children), all deeper nodes collapsed — unless chain auto-expansion (next row) is on, which it is by default. |
| Addition | §7.4 Auto-expand chains | Not in the brief (requested during review of this PR). Expanding a folder — and the initial root expansion of a new report — also expands its chain of pass-through folders: while the current folder has exactly one subfolder and that subfolder holds no files itself and is not a leaf, the subfolder is expanded too. The walk stops at the first folder that branches, holds files or is a leaf, so e.g. `O:\DatenP\Programmieren\repos\myDupFinder\testdata` opens in one step. Header checkbox "auto-expand chains" in the tree pane, default **on**, kept across reports but not persisted across app restarts; switching it only affects later expansions. Collapsing is unchanged (one folder). The tree header wraps its two checkboxes onto a second line when the pane is too narrow (down to the 160 px minimum). |
| Clarification | §7.4 Sort hook | `TreeViewModel::set_ranks(tree, Option<Vec<u8>>)` takes a per-folder rank (lower first, stable, missing = 3) so step 11 can plug in green 0 / yellow 1 / red 2 / unknown 3. Nothing calls it yet (`expect(dead_code)`); no ordering by status is implemented. |
| Deferred | §7.4 Status dots, green-on-top | Header checkbox "● green on top" is rendered, default checked, and **disabled** (no statuses exist yet); toggling has no effect. No status dots and no identical-folder logic → step 11. |
| Clarification | §7.4 Keyboard | ↑/↓ move the selection over visible rows (clamped at both ends). → on a collapsed folder expands it (selection stays), on an expanded one moves to its first *displayed* child, on a leaf does nothing. ← on an expanded folder collapses it, otherwise moves to the parent (root: nothing). Enter/Space re-select the current row. Every selection move refilters the table, like a click. |
| Clarification | §7.4 Hidden selection | If the selected folder gets hidden by collapsing an ancestor, the selection is kept; keyboard navigation then starts from its nearest visible ancestor. Collapsing does not move the selection. |
| Clarification | §7.4 Leaf toggle | Only folders with children have a toggle handler (▸/▾ toggles without selecting). A leaf's `·` has none, so clicking it selects the row like any other part of the row. |
| Addition | §7.4 Tree row tooltip | Non-root rows show the full folder path as tooltip (names are ellipsised). |
| Addition | §7.4 Focus | The tree pane is a focusable key context (`FolderTree`); clicking a row or toggle focuses it. Key bindings are registered in `main.rs` (`bind_tree_keys`). Scrolling keeps the selected row visible (`ScrollStrategy::Nearest`); a new report scrolls to the top. |
| Change | §7.2 Splitter implementation | Uses gpui-component's `h_resizable`/`resizable_panel` for hit testing, cursor and drag, but paints its own handle: 5 px line in `Palette::splitter`, `splitter_hover` while hovered or dragged (the design system's own hairline/pill is replaced). The handle's hit band stays gpui-component's 9 px (straddling the boundary). Text-selection suppression while dragging is not done explicitly. |
| Clarification | §7.2 Splitter limits | Default 320 px, min 160 px; max is `window_width − 200` (never below 160), recomputed on every render from the viewport width. The tree panel is `fixed()`, so it keeps its width when the window is resized. The table panel uses gpui-component's default minimum (100 px). |
| Change | §7.2 Pane layout without report | The split layout (tree pane + splitter + right pane) is always rendered; without a report the tree pane shows only its header and the placeholder text appears in the right pane (previously full width). |
| Addition | §7.3 Toolbar | *descendants* checkbox (bound to `include_descendants`, default on, disabled without a report), placed before *hash column* as in the brief. |
| Clarification | §6 Selection reset | A new report resets expansion to the root only, selection to the root and scrolls the tree to the top; this is done by the tree view noticing a different `Arc<Report>`, not by the store. |
| Clarification | Code | `select_folder` / `set_include_descendants` / `Palette::{splitter, splitter_hover, row_selected}` lost their `expect(dead_code)`. Still pending: `Palette::status` (step 11), `TreeViewModel::{set_ranks, reveal}` (step 11), the text/hash/category/source setters (step 10). |
| Open | Tests | Real mouse clicks (row select vs. toggle with `stop_propagation`) and key-binding dispatch are not covered by automated tests; the view's handlers (`select`, `toggle`, `press_*`) are tested directly. Manually verified on Windows: clicking focuses the tree, ↓/→ expand and select, the table refilters. |

### PR #5 — CI: manual OS selection and binary artifacts

| Type | Area | Deviation |
|---|---|---|
| Addition | §10.1 Manual runs | `workflow_dispatch` has three boolean inputs (`linux`, `windows`, `macos`, all default on), so a manual run on any branch can build just the chosen OSes. The automatic gating (Linux on PRs/main/tags, Windows on main/tags, macOS on tags) is unchanged. |
| Addition | §10.1 Binary artifacts | Every OS job also runs `cargo build --release --locked -p mydupfinder-ui` and uploads the executable as a workflow artifact (`mydupfinder-ui-linux-x64`, `-windows-x64`, `-macos-arm64`; 14 days). These are bare, unsigned executables for testing; installers and GitHub Releases stay in step 12. The release build adds a few minutes per job, including the Linux PR job. |

---

## In this PR (PR #6 — toolbar filters, plan step 10)

| Type | Area | Deviation |
|---|---|---|
| Clarification | §6 / §7.3 Filter persistence | Opening another report resets only the sources (as in §6). The text filter, hash filter, categories, *descendants* and *hash column* are kept, so the user can apply the same filter to several reports. The toolbar's inputs are re-synced from the store on every store change (`set_value` emits no change event, so there is no feedback loop); a value set by anything but typing is therefore always shown. |
| Clarification | §7.3 Filter input | The store keeps the text as typed (untrimmed) and trims it when building the query, so the input never rewrites what the user types. |
| Change | §8 Debounce | The text/hash inputs call the store on every change without the ~100 ms debounce mentioned in §8: sync filtering of < 50k rows is a few ms, and larger reports are already filtered in the background with a generation counter that drops stale results. A debounce can be added if 200k-row reports feel laggy. |
| Clarification | §7.3 Esc | `Esc` is handled by a wrapper around each filter input: the input does not consume it (`clean_on_escape` is off), it bubbles to the wrapper, which empties the input and the store filter (only the focused input's filter is cleared). It does not move focus away from the input. Esc outside the inputs (e.g. in the folder tree) is not bound, so the tree's key handling is unchanged. |
| Clarification | §7.3 Ctrl/Cmd+F | New action `FocusFilter`, bound in the `Explorer` key context like `OpenReport`; focuses the path filter and does nothing while no report is open (the input is disabled then). |
| Addition | §7.3 Dark-mode chips | The brief only gives the light colours. Light: fill `#e8f0ff`, border `#6796e1` (off: `#ccc`). Dark: fill `#23324d`, border `#6796e1`, off border `#555`. All three are `Palette` fields (`chip_on_bg`, `chip_on_border`, `chip_off_border`). |
| Clarification | §7.3 Chips | A chip is a pill-shaped `div` around gpui-component's `Checkbox` + label. Only the checkbox and its label are clickable, not the whole pill's padding. Chip text uses the default colour (not the category colour). Disabled (no report) chips use the checkbox's disabled style; the pill border/fill still reflects the stored state. |
| Clarification | §7.3 Source chips | Shown only for a report with `has_second_source`; a report with only a Base source shows none, so its `sources` stay `{Base}`. |
| Clarification | §7.3 Inputs | Fixed 200 px wide, gpui-component `Input` at `Size::Small` with 12 px text; the hash input uses the theme's monospace family (`Theme::mono_font_family`). The inputs have no clear (×) button, Esc is the only clear affordance. |
| Clarification | §7.3 Layout | Controls wrap. The category and source chips form one group with `margin-left: auto` (the flexible spacer): the group moves to the next toolbar line as a whole instead of chip by chip, stays right-aligned there, and only wraps internally when a full line is too narrow. |
| Clarification | Code | The `expect(dead_code)` on the text/hash/category/source setters was removed. Still pending: `Palette::status`, `TreeViewModel::{set_ranks, reveal}` (step 11). |
| Deferred | §7.3 | *Identical folders* button → step 11. |
| Open | Tests | Mouse clicks on chips are not covered by automated tests (their handlers are the tested store setters). Key handling is: `keyboard_typing_into_path_filter` sends real keystrokes (Ctrl+F, typed text, Esc) through GPUI's test platform. Manually verified on Windows: Ctrl+F focuses the path filter, typing `check` filters to 25 rows, chip group layout in light mode. Dark mode not checked visually. |
