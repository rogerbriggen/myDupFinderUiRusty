// Compares OLD (Rust backend + Angular logic) with NEW (dupreport-core).
import { readFileSync } from 'node:fs';
import { isDeepStrictEqual } from 'node:util';

const out = process.argv[2];
const rd = (f: string) => JSON.parse(readFileSync(`${out}/${f}`, 'utf8'));
const oldRows = rd('old_rows.json'), oldPairs = rd('old_pairs.json'), oldTs = rd('old_ts.json');
const oldRust = rd('old_rust_results.json'), states = rd('states.json'), nw = rd('new.json');

let failures = 0;
const check = (name: string, ok: boolean, detail = '') => {
  if (!ok) failures++;
  console.log(`${ok ? 'PASS' : 'FAIL'}  ${name}${detail ? '  ' + detail : ''}`);
};

check('job name', oldRows.job === nw.job, `${oldRows.job} | ${nw.job}`);
check('has second source', oldRows.hasSecond === nw.hasSecond, String(nw.hasSecond));
check(`parsed rows (${oldRows.rows.length})`, isDeepStrictEqual(oldRows.rows, nw.rows));

const oldFolders = Object.keys(oldTs.tree).sort(), newFolders = Object.keys(nw.tree).sort();
check(`folder set (${oldFolders.length})`, isDeepStrictEqual(oldFolders, newFolders));
const treeDiff = oldFolders.filter((f) => !isDeepStrictEqual(oldTs.tree[f], nw.tree[f]));
check('per-folder direct/total count, size, category counts, child order', treeDiff.length === 0,
  treeDiff.slice(0, 3).map((f) => `${JSON.stringify(f)}: old ${JSON.stringify(oldTs.tree[f])} new ${JSON.stringify(nw.tree[f])}`).join('; '));

check(`identical pairs (${oldPairs.length}, same order)`, isDeepStrictEqual(oldPairs, nw.pairs),
  `old ${JSON.stringify(oldPairs)} new ${JSON.stringify(nw.pairs)}`);
const sKeys = new Set([...Object.keys(oldTs.statuses), ...Object.keys(nw.statuses)]);
const sDiff = [...sKeys].filter((k) => oldTs.statuses[k] !== nw.statuses[k]);
check(`status dots (${Object.keys(oldTs.statuses).length} folders)`, sDiff.length === 0,
  sDiff.slice(0, 5).map((k) => `${k}: ${oldTs.statuses[k]} vs ${nw.statuses[k]}`).join('; '));

const tsVsRust = states.filter((_: unknown, i: number) => !isDeepStrictEqual(oldTs.results[i], oldRust[i])).length;
check(`old TS filter == old Rust filter (${states.length} UI states)`, tsVsRust === 0, `${tsVsRust} differ`);
const diffs = states.map((s: unknown, i: number) => [s, oldRust[i], nw.results[i]] as const)
  .filter(([, o, n]) => !isDeepStrictEqual(o, n));
check(`filtered rows old == new (${states.length} UI states)`, diffs.length === 0,
  diffs.slice(0, 3).map(([s, o, n]) => `${JSON.stringify(s)}: old ${o.length} new ${n.length}`).join('; '));
const counts = oldRust.map((r: number[]) => r.length);
console.log(`      filtered-row counts span ${Math.min(...counts)}..${Math.max(...counts)}; ${counts.filter((c: number) => c === 0).length} states match nothing`);

console.log(failures ? `${failures} FAILURE(S)` : 'ALL PASS');
process.exitCode = failures ? 1 : 0;
