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

### PR #6 — Toolbar filters (plan step 10)

| Type | Area | Deviation |
|---|---|---|
| Clarification | §6 / §7.3 Filter persistence | Opening another report resets only the sources (as in §6). The text filter, hash filter, categories, *descendants* and *hash column* are kept, so the user can apply the same filter to several reports. The toolbar's inputs are re-synced from the store on every store change (`set_value` emits no change event, so there is no feedback loop); a value set by anything but typing is therefore always shown. |
| Clarification | §7.3 Filter input | The store keeps the text as typed (untrimmed) and trims it when building the query, so the input never rewrites what the user types. |
| Change | §8 Debounce | The text/hash inputs call the store on every change without the ~100 ms debounce mentioned in §8: sync filtering of < 50k rows is a few ms, and larger reports are already filtered in the background with a generation counter that drops stale results. A debounce can be added if 200k-row reports feel laggy. |
| Clarification | §7.3 Esc | `Esc` is handled by a wrapper around each filter input: the input does not consume it (`clean_on_escape` is off), it bubbles to the wrapper, which empties the input and the store filter (only the focused input's filter is cleared). It does not move focus away from the input. Esc outside the inputs (e.g. in the folder tree) is not bound, so the tree's key handling is unchanged. |
| Clarification | §7.3 Ctrl/Cmd+F | New action `FocusFilter`, bound in the `Explorer` key context like `OpenReport`; focuses the path filter and does nothing while no report is open (the input is disabled then). It also works while a filter input is focused: gpui-component binds the same key to its `Search` action in the more specific `Input` context, but GPUI dispatches every matching binding in order until one is handled, and single-line inputs leave `Search` unhandled (covered by `ctrl_f_from_focused_hash_input_focuses_path_filter`). |
| Change | §6 Setters | Every filter setter (`select_folder`, `set_include_descendants`, `set_text_filter`, `set_hash_filter`, `set_category`, `set_source`) is a no-op when the value does not change, instead of always recomputing `filtered`. Otherwise e.g. Esc in an already empty filter produced a new list and scrolled the table back to the top. |
| Addition | §7.3 Dark-mode chips | The brief only gives the light colours. Light: fill `#e8f0ff`, border `#6796e1` (off: `#ccc`). Dark: fill `#23324d`, border `#6796e1`, off border `#555`. All three are `Palette` fields (`chip_on_bg`, `chip_on_border`, `chip_off_border`). |
| Clarification | §7.3 Chips | A chip is a pill-shaped `div` around gpui-component's `Checkbox` + label. Only the checkbox and its label are clickable, not the whole pill's padding. Chip text uses the default colour (not the category colour). Disabled (no report) chips use the checkbox's disabled style; the pill border/fill still reflects the stored state. |
| Clarification | §7.3 Source chips | Shown only for a report with `has_second_source`; a report with only a Base source shows none, so its `sources` stay `{Base}`. |
| Clarification | §7.3 Inputs | Fixed 200 px wide, gpui-component `Input` at `Size::Small` with 12 px text; the hash input uses the theme's monospace family (`Theme::mono_font_family`). The inputs have no clear (×) button, Esc is the only clear affordance. |
| Clarification | §7.3 Layout | Controls wrap. The category and source chips form one group with `margin-left: auto` (the flexible spacer): the group moves to the next toolbar line as a whole instead of chip by chip, stays right-aligned there, and only wraps internally when a full line is too narrow. |
| Clarification | Code | The `expect(dead_code)` on the text/hash/category/source setters was removed. (`Palette::status` and `TreeViewModel::{set_ranks, reveal}` were wired up in PR #8.) |
| Deferred | §7.3 | *Identical folders* button → done in PR #8 (step 11). |
| Open | Tests | Mouse clicks on chips are not covered by automated tests (their handlers are the tested store setters). Key handling is: `keyboard_typing_into_path_filter` sends real keystrokes (Ctrl+F, typed text, Esc) through GPUI's test platform. Manually verified on Windows: Ctrl+F focuses the path filter, typing `check` filters to 25 rows, chip group layout in light mode. Dark mode not checked visually. |

### PR #7 — CI: no release build on pull requests

| Type | Area | Deviation |
|---|---|---|
| Change | §10.1 Binary artifacts | Revises PR #5: the release build and artifact upload are skipped on pull requests, so the Linux PR check takes about 1 minute again instead of about 6½. Binaries are still built and uploaded on pushes to `main` (Linux, Windows), on `v*` tags (all three OSes) and on manual runs (the ticked OSes); for a binary from a PR branch, start a manual run on that branch. |

### PR #8 — Identical panel, status dots, green on top (plan step 11)

| Type | Area | Deviation |
|---|---|---|
| Clarification | §6 / §7.2 Panel visibility | The brief renders the panel only when `identical.len() > 0 \|\| identical_loading`, which would show nothing after a scan that found zero pairs. `ReportStore` has a new `identical_scanned` flag (set when a scan finishes, cleared by opening a report); the panel is rendered when `identical_loading \|\| identical_scanned`. It shows "Scanning…" while loading and empty, and "No identical folder pairs found." after a finished empty scan (the brief's second text "…or scan not started — click Identical folders" is not needed: a not-started scan renders no panel). Resolves the "scan done flag" idea from PR #3. |
| Clarification | §5.9 / §7.4 Dots | Dot semantics are unchanged: without pairs `compute_statuses` returns all `None`, so a finished empty scan shows no dots and keeps the "green on top" checkbox disabled. `ReportStore::has_statuses()` (= pairs exist and statuses computed) drives both. A folder with `None` status while dots are shown (cannot happen with the current core) gets a hollow dot without tooltip. |
| Clarification | §6 Store fields | `identical` and `statuses` are `Arc<Vec<…>>` (cheap clones into list callbacks and background-job comparisons), not plain `Vec`s. Both are empty until a scan has run. `load_identical` is a no-op without a report or while a scan is running; a rescan keeps the old result visible until the new one arrives. Opening a report clears all identical state and drops an in-flight scan (generation counter, same pattern as `open_report`); scan errors cannot occur (the core functions are infallible), so nothing writes `error`. |
| Addition | §7.3 Toolbar | *Identical folders* button as the last toolbar item (after the chips group). Label "Scanning…" and disabled while scanning; disabled without a report. |
| Clarification | §7.4 Green on top | Checkbox is enabled once statuses exist, default checked, and its state is kept across reports (like *auto-expand chains*). Ranks Green 0 / Yellow 1 / Red 2 / unknown 3 are handed to `TreeViewModel::set_ranks`; turning it off or opening a new report restores the alphabetical order. Only the flattened order is rebuilt: expansion and selection survive, and the selected row is scrolled back into view afterwards. The two real fixtures contain no folder whose green-first order differs from alphabetical (paired folders are already first), so the re-ordering test uses a small synthetic report written to a temp dir. |
| Clarification | §7.4 Status dot | 8 px round dot before the name, palette colours (`#2ea043` / `#d4a017` / `#cf222e`; the dark theme uses lighter variants `#3fb950` / `#e3b341` / `#f85149`, which the brief does not specify). Tooltip is `IdenticalStatus::tooltip`. The root row gets a dot too. |
| Clarification | §7.2 Panel splitter | Uses gpui-component's `v_resizable`/`resizable_panel` with the same custom painted handle as the vertical splitter (5 px, `splitter` / `splitter_hover`, hit band stays gpui-component's 9 px). The identical panel is a `fixed()` bottom panel (default 220 px, min 80 px, max `window_height − 160`, never below 80 px; recomputed on every render), so it keeps its height when the window is resized. The area above keeps gpui-component's default minimum (100 px). Text-selection suppression while dragging is not done explicitly (as for the vertical splitter). |
| Change | §7.2 Collapse | A small ghost button `▼` (gpui-component `Button`, tooltip "Collapse") collapses the panel into a full-width `▲ Identical folders` bar — `▼`/`▲` instead of the brief's `▾`/`▴`, because those small triangles render as a barely visible dot at 12 px. It sits below the splitter group; clicking the bar restores it. The split group is always rendered (the panel is just `visible(false)`), so the splitter state, i.e. the height the user dragged to, survives collapsing and restoring. The collapsed state is kept across scans and reports and not persisted across app restarts. The splitter is not shown while collapsed. |
| Clarification | §7.6 Table | Columns Base folder / Second folder (both link-styled, ellipsised, full path as tooltip), Files and Total size (right-aligned, 80 / 100 px). Rows (22 px) come from a `uniform_list`, sorted as returned. Header and the two folder columns share the width (min 160 px each). |
| Addition | §7.6 Second folder | The Second folder is clickable too (the brief offers this as an improvement). Both links behave the same: select the folder, expand all its ancestors (`TreeViewModel::reveal`), refilter the table and scroll the tree row into view. |
| Addition | §7.6 Link colour | Light `#1565c0` as specified, dark `#64b5f6` (the dark "Duplicate" blue; not in the brief), as `Palette::link`. Links have no hover colour change, only the pointer cursor. |
| Clarification | §7.6 Link target | A path without a node in the combined tree does nothing (cannot happen for pairs of the same report). A link to a folder that was already selected still reveals and scrolls to it. |
| Clarification | Code | Removed the `expect(dead_code)` of `Palette::status` and `TreeViewModel::{set_ranks, reveal}`. All `ReportStore` fields and setters are now used. Several UI source files had CRLF line endings in the working tree and were normalised to LF (git's `autocrlf` makes no difference in the diff). |
| Open | Tests | Real mouse clicks (toolbar button, panel links/rows, collapse button and bar, checkbox) and dragging the horizontal splitter are not covered; their handlers (`load_identical`, `activate_folder`, `set_collapsed`, `set_green_on_top`) are tested directly, and the layout renders in both panel states. The dot colour/tooltip rendering and the panel's pixel layout (ellipsis, alignment) are not asserted. Manually verified on Windows (light mode): the button scans the whole-lot fixture, the panel shows both pairs, the tree shows dots, clicking the `CheckDetail` link reveals and selects it (table shows its 4 rows), the collapse button collapses to the bar. Not checked in dark mode or with thousands of pairs. |

---

## In this PR (PR #9 — packaging and release CI, plan step 12)

| Type | Area | Deviation |
|---|---|---|
| Clarification | §10.2 Tool choice | `cargo-packager` 0.11.8 (not `cargo-bundle`): one config (`[package.metadata.packager]` in `crates/mydupfinder-ui/Cargo.toml`) covers .deb, AppImage, NSIS and .app/.dmg. Version, description and authors come from the package, so the version still lives only in the workspace `Cargo.toml`. |
| Clarification | §10.2 Windows format | NSIS setup .exe only, no `.msi` (WiX): cargo-packager downloads NSIS itself, so the runner needs no extra toolset. The installer is per-user (cargo-packager default `currentUser`). An MSI can be added later with `--formats wix`. |
| Clarification | §10.2 Windows icon | `build.rs` with `winresource` 0.1.34 embeds `assets/icons/icon.ico` and version info (ProductName, FileDescription, CompanyName, LegalCopyright, OriginalFilename; file/product version from the package). `winresource` is a `[target.'cfg(windows)'.build-dependencies]` entry, so Linux/macOS builds do not compile it; the code is `#[cfg(windows)]` and also checks `CARGO_CFG_TARGET_OS`. `Cargo.lock` gained only the `winresource` package (its dependencies were already locked). |
| Addition | §10.2 Metadata | Identifier `ch.rogerbriggen.mydupfinderui`, category Utility, icons from `assets/icons`, `out-dir` `target/packager`, `binaries-dir` `target/release`, and a `before-packaging-command` (`cargo build --release --locked -p mydupfinder-ui`), so `cargo packager --release -p mydupfinder-ui` is all that is needed. The formats are not in the config but chosen per OS on the command line. |
| Clarification | §10.2 Debian dependencies | Runtime libraries GPUI loads: libxkbcommon0, libxkbcommon-x11-0, libwayland-client0, libvulkan1, libfontconfig1, libfreetype6, libx11-xcb1, libxcb1 and `libasound2t64 | libasound2` (renamed in Ubuntu 24.04 / Debian 13; cargo-packager writes the string verbatim into `Depends`). The list is hand-picked, not derived with `dpkg-shlibdeps`. |
| Clarification | §10.2 macOS format | Only `--formats dmg` is requested; cargo-packager builds the `.app` as part of it and the .dmg contains it. A bare `.app` directory is not uploaded (artifacts would lose permissions and symlinks). |
| Addition | §10.2 CI package jobs | `package-linux` / `package-windows` / `package-macos` need their test job and run on the same events except pull requests (the implicit success of a skipped test job also skips the package job). Artifacts: `mydupfinder-ui-<os>-packages`, 14 days. The test jobs keep their bare-binary artifacts from PR #5/#7 (useful for a quick test; the package jobs rebuild the release binary, a few minutes of duplicated work on a cold cache, which `rust-cache` mitigates). |
| Clarification | §10.2 cargo-packager install | `taiki-e/install-action` has no `cargo-packager` manifest and the project publishes no prebuilt binaries, so CI runs `cargo install cargo-packager --locked --version 0.11.8` and caches the binary with `actions/cache` keyed on the version. Updating cargo-packager is a manual edit (Dependabot does not see it). |
| Clarification | §10.2 AppImage | Needs `file` and FUSE on the runner (`libfuse2t64`, falling back to `libfuse2`); `APPIMAGE_EXTRACT_AND_RUN=1` is set so the linuxdeploy AppImage tooling runs without FUSE if it is missing. |
| Change | §10.3 Release assets | The installers (.deb, .AppImage, setup .exe, .dmg) are attached to the GitHub Release as they are instead of zipping each platform's bundle: a zip would hide the installers and the .deb/.AppImage/.dmg are already single files. No zip of the bare binaries is attached (Deferred). |
| Addition | §10.3 Release job | Runs on `v*` tag pushes after all three package jobs (`contents: write`), first checks that the tag equals `v` + the workspace version and fails otherwise, then publishes with `softprops/action-gh-release@v3` (`generate_release_notes: true`, not a draft, fails on unmatched files). Artifacts are fetched with `actions/download-artifact@v8` (the counterpart of `upload-artifact@v7`). |
| Clarification | §10 Dependabot | `.github/dependabot.yml` already covers `cargo` (gpui group) and `github-actions`; unchanged. |
| Deferred | §10.2 Signing | No code signing or notarization (Windows SmartScreen / macOS Gatekeeper will warn). |
| Open | Verification | Verified on Windows only: `build.rs` embeds the icon/version info, `cargo packager --release -p mydupfinder-ui --formats nsis` produces `target/packager/mydupfinder-ui_0.1.0_x64-setup.exe` (about 7.3 MiB); the installer itself was not run. Not verified (only possible on CI): the Linux .deb/AppImage build (incl. the FUSE workaround and the dependency names), the macOS .dmg, the `actions/cache` for the installed binary, and the whole release job, which only runs for a `v*` tag. The workflow YAML parses; it was not run. |
