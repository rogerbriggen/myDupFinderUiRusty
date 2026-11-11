#!/bin/sh
# Parity check: old app (unchanged Rust backend + Angular logic) vs dupreport-core.
# Usage: tools/parity/run.sh <report.csv> <out-dir>
# Env: OLD_REPO = checkout of rogerbriggen/myDupFinderUi (default ../myDupFinderUi
#      next to this repo). Needs cargo and Node >= 23 (runs .ts directly).
set -e
P="$(cd "$(dirname "$0")" && pwd)"
OLD_REPO="${OLD_REPO:-$P/../../../myDupFinderUi}"
export OLD_REPO="$(cd "$OLD_REPO" && pwd)"
mkdir -p "$2"

# Old frontend logic, unchanged apart from the import lines Node's type
# stripping needs (`type` on type-only imports, explicit .ts paths).
TS="$P/old-ts"
mkdir -p "$TS"
A="$OLD_REPO/src/app/core"
cp "$A/models/report.ts" "$TS/report.ts"
sed "1s#.*#import { ALL_CATEGORIES, type Category, type IdenticalFolderPair, type Row } from './report.ts';#" \
  "$A/tree/folder-tree.ts" > "$TS/folder-tree.ts"
sed -e "1s#.*#import { type Row, type RowPage, type RowQuery } from './report.ts';#" \
    -e "2s#.*#import { splitFolderAndName } from './folder-tree.ts';#" \
  "$A/backend/row-filter.ts" > "$TS/row-filter.ts"

cargo build -q --manifest-path "$P/old-rust/Cargo.toml" --target-dir "$P/target"
cargo build -q --manifest-path "$P/new-rust/Cargo.toml" --target-dir "$P/target"
OLD="$P/target/debug/parity-old"
NEW="$P/target/debug/parity-new"

"$OLD" "$1" "$2"                          # rows + identical pairs
node "$P/old_ts_side.ts" "$2"             # old tree, dots, UI states, old filter
"$OLD" "$1" "$2" "$2/old_queries.json"    # old Rust filter on the same queries
"$NEW" "$1" "$2" "$2/states.json"         # new core on the same UI states
node "$P/compare.ts" "$2"
