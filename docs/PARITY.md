# Parity check: old app vs. new app (plan step 13)

Plan step 13 asks to open both fixtures in the old and the new app side by side
and check that per-folder counts, filtered row counts and identical pairs match.
This was done at the logic level, with the old app's own code, and on more data
than the two fixtures.

## Method

The harness lives in [`tools/parity`](../tools/parity/README.md) and can be
re-run at any time.

- **Old side.** It uses the old code as is, not a re-implementation:
  - The old Tauri backend (`error`, `model`, `csv_parser`, `query`, `identical`)
    is compiled unchanged from a checkout of `rogerbriggen/myDupFinderUi`.
  - The old Angular logic (`folder-tree.ts` builds the tree and dots,
    `row-filter.ts` filters, `report.ts` defines the categories) runs under
    Node. Only its import lines are edited.
  - UI states are turned into queries the way the old `report-store.ts` does
    it: text and hash are trimmed (empty means no filter), and all
    categories and sources are always sent.
- **New side.** `dupreport-core` of this repo. UI states are turned into
  queries exactly as `ReportStore::query()` does it.
- **Compared** (see the harness README for details):
  - parsed rows;
  - the folder set, and for each folder the counts, size, category counts and
    child order;
  - identical pairs and their order;
  - the status dot of every folder;
  - the filtered rows for every UI state, each a combination of tree
    selection, *include descendants*, path filter, hash filter, category chips
    and source chips;
  - as a side check, the old TS filter against the old Rust filter.
- **Synthetic reports** (`gen_synthetic.ts`, deterministic per seed) add what
  the fixtures lack:
  - many identical subtrees at different depths, and copies spoiled by
    Missing, New, Changed or Moved rows;
  - Moved-only folders, unrelated duplicates and Unique rows;
  - umlauts, accents, spaces, quotes and mixed case in names;
  - empty hashes on New rows;
  - `/` instead of `\` as the separator.

The GUI itself (rendering, clicks) was not compared side by side. The new GUI
was checked by hand during steps 9–11 (see `DEVIATIONS.md`). Everything it
shows comes from the values compared here.

## Results

| Report | Rows | Folders | Identical pairs | UI states | Result |
|---|---:|---:|---:|---:|---|
| `Demo DiffFindDupsTheWholeLot dupReport.csv` | 64 | 23 | 2 | 219 | all equal |
| `Demo DiffFindOnlyDups dupReport.csv` | 25 | 15 | 0 | 155 | all equal |
| synthetic, seed 42 (`\` and `/`) | 1052 | 651 | 46 | 5243 | all equal |
| synthetic, seed 7 (`\` and `/`) | 1132 | 708 | 65 | 5699 | all equal |
| synthetic, seed 1234 (`\` and `/`) | 1099 | 678 | 53 | 5459 | all equal |
| synthetic, seed 99 (`\` and `/`) | 1106 | 686 | 56 | 5523 | all equal |
| synthetic, seed 2026 (`\` and `/`) | 1602 | 1035 | 64 | 8315 | all equal |

In every run, the old TS filter and the old Rust filter also agreed on every state.

## Differences found

1. **Folder order with umlauts and accents — fixed.** The brief (§5.6) says to
   sort children with `to_lowercase()`. That puts `Ärger`, `Österreich` and
   `Übersicht` after `z`. The old UI's
   `localeCompare(…, {sensitivity: 'accent'})` uses ICU collation and sorts
   them next to `a`, `o` and `u`, with `café` right after `cafe`.
   `dupreport-core` now sorts with an ICU4X collator at secondary strength
   (crate `icu_collator`, root locale; Node gives the same order for `de-CH`
   and the root locale), with a lowercase fallback. This was the only
   difference on real-world-shaped data.
2. **Mixed `\` and `/` in one report — not changed.** The old tree finds a
   child by its name but keeps the path of the first spelling it saw. So
   `C:\a\b` and `C:/a/b` end up in one node whose path matches only one
   spelling. The tree, the dots and the folder filter then disagree with each
   other. The new app keeps them as separate nodes, which stays consistent.
   myDupFinder writes one separator per report, so this does not happen with
   real reports.
3. **A file named like a sibling folder — not changed.** With *include
   descendants*, the old filter's `is_under_or_equal` (a string-prefix test)
   counts the file `…\a\x` as part of the folder `…\a\x`. The new filter takes
   the selected node's subtree, so that file is not included. A file and a
   folder of the same name cannot sit side by side on a real file system.

Differences 2 and 3 only show up with `SEP_MODE=mixed` or with a hand-made
report. They are listed in `DEVIATIONS.md`.
