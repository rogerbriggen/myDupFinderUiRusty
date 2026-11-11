// Parity harness, OLD frontend side: unchanged folder-tree.ts / row-filter.ts
// of the old Angular app, fed with the rows + pairs from the old Rust backend.
import { readFileSync, writeFileSync } from 'node:fs';
import { buildFolderTree, computeIdenticalStatuses, type FolderNode } from './old-ts/folder-tree.ts';
import { filterAndPage } from './old-ts/row-filter.ts';
import { ALL_CATEGORIES } from './old-ts/report.ts';

const out = process.argv[2];
const { rows } = JSON.parse(readFileSync(`${out}/old_rows.json`, 'utf8'));
const pairs = JSON.parse(readFileSync(`${out}/old_pairs.json`, 'utf8')).map(
  ([folderA, folderB, fileCount, totalSize]: any[]) => ({ folderA, folderB, fileCount, totalSize }));

const root = buildFolderTree(rows);
const tree: Record<string, unknown> = {};
const folders: string[] = [];
(function walk(n: FolderNode) {
  folders.push(n.path);
  tree[n.path] = [n.directFileCount, n.totalFileCount, n.totalSize,
    ALL_CATEGORIES.map((c) => n.categoryCounts[c]), n.children.map((c) => c.name)];
  n.children.forEach(walk);
})(root);
const statuses = Object.fromEntries(computeIdenticalStatuses(root, pairs));

// UI states to compare (what a user can set in the toolbar / tree).
const ALL_SRC = ['Base', 'Second'];
const hashes = [...new Set(rows.map((r: any) => r.fileSha512Hash as string))] as string[];
const h = hashes[0];
type UiState = { folder: string; desc: boolean; text: string; hash: string; cats: string[]; srcs: string[] };
const base: UiState = { folder: '', desc: true, text: '', hash: '', cats: [...ALL_CATEGORIES], srcs: ALL_SRC };
const states: UiState[] = [];
for (const folder of folders) for (const desc of [true, false]) states.push({ ...base, folder, desc });
const BS = String.fromCharCode(92); // backslash
for (const text of ['check', '  Check  ', 'FILESNEW', `demo${BS}find`, BS, '.cs', 'zzz', ' ', `O:${BS}DatenP`, `${BS}Helper${BS}`, 'éÜ'])
  states.push({ ...base, text });
for (const hash of [h.slice(0, 8).toLowerCase(), h, ` ${h.slice(0, 12)} `, h.slice(0, 4), h.slice(0, 7), h.slice(0, 7) + 'x', 'ABCDEF12', hashes[hashes.length - 1].slice(0, 8)])
  states.push({ ...base, hash });
for (const c of ALL_CATEGORIES) states.push({ ...base, cats: [c] });
states.push({ ...base, cats: [] });
for (const c of ALL_CATEGORIES) states.push({ ...base, cats: ALL_CATEGORIES.filter((x) => x !== c) });
for (const srcs of [['Base'], ['Second'], []]) states.push({ ...base, srcs });
for (const folder of folders) for (const desc of [true, false])
  for (const [text, cats, srcs] of [['s', ['Duplicate'], ALL_SRC], ['.cs', ['Moved', 'Changed', 'Missing'], ['Base']], ['', ['Unique', 'New'], ['Second']]] as const)
    states.push({ folder, desc, text, hash: '', cats: [...cats], srcs: [...srcs] });

// How the OLD report-store turned UI state into a query (report-store.ts:160-166).
const oldQueries = states.map((s) => ({
  folder: s.folder.length ? s.folder : undefined, includeDescendants: s.desc,
  text: s.text.trim() || undefined, hash: s.hash.trim() || undefined,
  categories: [...s.cats], sources: [...s.srcs], offset: 0, limit: Number.MAX_SAFE_INTEGER }));
const index = new Map(rows.map((r: any, i: number) => [r, i]));
const tsResults = oldQueries.map((q) => filterAndPage(rows, q).rows.map((r) => index.get(r)));

writeFileSync(`${out}/old_ts.json`, JSON.stringify({ tree, statuses, results: tsResults }));
writeFileSync(`${out}/old_queries.json`, JSON.stringify(oldQueries));
writeFileSync(`${out}/states.json`, JSON.stringify(states));
console.log(`old TS: ${folders.length} folders, ${states.length} UI states, ${pairs.length} pairs`);
