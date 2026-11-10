# Parity harness (plan step 13)

Compares the old app (Tauri + Angular, repo `rogerbriggen/myDupFinderUi`) with
`dupreport-core` on the same dup report. Results and method:
[`docs/PARITY.md`](../../docs/PARITY.md).

The **old** side is the old app's own code, not a re-implementation. The
backend modules `error`, `model`, `csv_parser`, `query` and `identical` are
compiled unchanged via `#[path]` (see `old-rust/build.rs`). The frontend logic
`folder-tree.ts`, `row-filter.ts` and `report.ts` runs under Node; `run.sh`
copies it and edits only the import lines.

## Run

Needs cargo, Node ≥ 23 (runs `.ts` directly) and a POSIX shell (Git Bash on
Windows).

```bash
# OLD_REPO defaults to ../myDupFinderUi next to this repo
OLD_REPO=/path/to/myDupFinderUi tools/parity/run.sh "testdata/Demo DiffFindDupsTheWholeLot dupReport.csv" tools/parity/out/wholelot
```

A synthetic report with many identical and spoiled subtrees, non-ASCII names
and empty hashes (`SEP_MODE` = `backslash` (default), `slash` or `mixed`):

```bash
node tools/parity/gen_synthetic.ts tools/parity/out/syn.csv 42
tools/parity/run.sh tools/parity/out/syn.csv tools/parity/out/syn
```

The script prints one PASS/FAIL line per check and exits non-zero on any
failure.

## What is compared

1. Parsed rows (path, size, hash, category, source), job name, second-source flag.
2. Folder tree: the folder set and, for each folder, direct and total file
   count, total size, per-category counts and child order.
3. Identical folder pairs (content and order) and the status dot of every folder.
4. Filtered rows for a few thousand UI states (folder × include descendants,
   text, hash, category and source chips, combinations). Each state is turned
   into a query the way each app's store does it: old `report-store.ts`, new
   `ReportStore::query()`. The old TS filter is also checked against the old
   Rust filter.
