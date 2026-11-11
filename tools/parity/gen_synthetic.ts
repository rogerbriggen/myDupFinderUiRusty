// Deterministic synthetic two-DB dup report that exercises what the two small
// fixtures do not: many identical subtrees at different depths (maximal-pair
// selection), copies spoiled by a Missing / New / Changed / Moved file
// (forbidden folders), Moved-only folders, unrelated duplicates, Unique rows,
// non-ASCII / spaces / mixed case / quotes in names, `/` separators, empty
// hashes on New rows. Usage: node gen_synthetic.ts <out.csv> [seed]
import { writeFileSync } from 'node:fs';

const BS = String.fromCharCode(92);
let seed = Number(process.argv[3] ?? 42) >>> 0;
const rnd = () => ((seed = (Math.imul(seed ^ (seed >>> 15), 2246822519) + 0x9e3779b9) >>> 0) / 2 ** 32);
const pick = <T>(a: readonly T[]) => a[Math.floor(rnd() * a.length)];
const chance = (p: number) => rnd() < p;
let hashCounter = 0;
const newHash = () => {
  hashCounter++;
  let h = '';
  for (let i = 0; i < 128; i++) h += '0123456789ABCDEF'[Math.floor(rnd() * 16)];
  return h.slice(0, 120) + hashCounter.toString(16).toUpperCase().padStart(8, '0');
};

const NAMES = ['src', 'Docs', 'photos', 'Fotos 2024', 'Übersicht', 'café', 'build', 'lib', 'a', 'A b',
  'deep', 'x', 'Projekt "Alpha"', 'misc', 'data', 'Old', 'new', 'Ärger', 'set1', 'set2'];
const FILES = ['readme.txt', 'Main.cs', 'image.JPG', 'Bild ä.png', 'notes.md', 'data.bin', 'a.txt', 'B.TXT', 'x.y', 'report "v2".pdf'];

type F = { rel: string; name: string; hash: string; size: number };
type Tree = { files: F[]; dirs: Map<string, Tree> };

function mkTree(depth: number, rel: string): Tree {
  const t: Tree = { files: [], dirs: new Map() };
  const nf = depth === 0 ? Math.floor(rnd() * 3) : Math.floor(rnd() * 4);
  for (let i = 0; i < nf; i++) {
    const name = pick(FILES);
    if (!t.files.some((f) => f.name === name)) t.files.push({ rel, name, hash: newHash(), size: 1 + Math.floor(rnd() * 100000) });
  }
  const nd = depth >= 5 ? 0 : Math.floor(rnd() * (depth < 2 ? 4 : 3));
  for (let i = 0; i < nd; i++) {
    const name = pick(NAMES);
    if (!t.dirs.has(name)) t.dirs.set(name, mkTree(depth + 1, rel ? `${rel}${BS}${name}` : name));
  }
  return t;
}
const allFiles = (t: Tree): F[] => [...t.files, ...[...t.dirs.values()].flatMap(allFiles)];

type Out = { path: string; size: number; hash: string; cat: string; src: string; group: number };
const out: Out[] = [];
let group = 0;
const join = (root: string, rel: string, name: string, sep = BS) =>
  [root, rel, name].filter(Boolean).join(BS).split(BS).join(sep);

for (let p = 0; p < 40; p++) {
  const proj = `proj${p}`;
  const tree = mkTree(0, '');
  const files = allFiles(tree);
  // SEP_MODE: "mixed" (10 % of projects use `/`), "slash" (all `/`), default all `\`.
  const mode = process.env.SEP_MODE ?? 'backslash';
  const sep = mode === 'slash' ? '/' : mode === 'mixed' && chance(0.1) ? '/' : BS;
  const baseRoot = `C:${BS}data${BS}${proj}`;
  // Second copy lives under another root, sometimes at a different depth.
  const secondRoot = chance(0.5) ? `D:${BS}backup${BS}${proj}` : `D:${BS}backup${BS}nested${BS}${p % 3}${BS}${proj}`;
  const spoil = chance(0.5); // spoil some subtrees with Missing / New / Changed / Moved
  for (const f of files) {
    const r = rnd();
    if (spoil && r < 0.06) {
      out.push({ path: join(baseRoot, f.rel, f.name, sep), size: f.size, hash: f.hash, cat: 'Missing', src: 'Base', group: ++group });
    } else if (spoil && r < 0.12) {
      out.push({ path: join(baseRoot, f.rel, f.name, sep), size: f.size, hash: f.hash, cat: 'Changed', src: 'Base', group: ++group });
      out.push({ path: join(secondRoot, f.rel, f.name, sep), size: f.size + 1, hash: newHash(), cat: 'Changed', src: 'Second', group });
    } else if (spoil && r < 0.16) {
      const g = ++group;
      out.push({ path: join(baseRoot, f.rel, f.name, sep), size: f.size, hash: f.hash, cat: 'Moved', src: 'Base', group: g });
      out.push({ path: join(secondRoot, `${f.rel}${BS}moved`, f.name, sep), size: f.size, hash: f.hash, cat: 'Moved', src: 'Second', group: g });
    } else {
      const g = ++group;
      out.push({ path: join(baseRoot, f.rel, f.name, sep), size: f.size, hash: f.hash, cat: 'Duplicate', src: 'Base', group: g });
      out.push({ path: join(secondRoot, f.rel, f.name, sep), size: f.size, hash: f.hash, cat: 'Duplicate', src: 'Second', group: g });
    }
  }
  if (spoil && chance(0.5)) out.push({ path: join(secondRoot, pick(files)?.rel ?? '', 'extra new.txt', sep), size: 5, hash: chance(0.5) ? '' : newHash(), cat: 'New', src: 'Second', group: ++group });
  if (chance(0.3)) out.push({ path: join(baseRoot, '', `unique ${p}.dat`, sep), size: 7, hash: newHash(), cat: 'Unique', src: 'Base', group: ++group });
}
// Unrelated duplicates (same hash, different names / places).
for (let i = 0; i < 30; i++) {
  const h = newHash(), g = ++group;
  out.push({ path: `C:${BS}inbox${BS}dl${i}.zip`, size: 99, hash: h, cat: 'Duplicate', src: 'Base', group: g });
  out.push({ path: `D:${BS}archive${BS}y${i % 4}${BS}copy of dl${i}.zip`, size: 99, hash: h, cat: 'Duplicate', src: 'Second', group: g });
}

const CAT_ORDER = ['Changed', 'Duplicate', 'Missing', 'Moved', 'New', 'Unique']; // ORDER BY Category (string)
out.sort((a, b) => CAT_ORDER.indexOf(a.cat) - CAT_ORDER.indexOf(b.cat) || a.group - b.group
  || a.src.localeCompare(b.src) || a.path.toLowerCase().localeCompare(b.path.toLowerCase()));
const quote = (s: string) => `"${s.replaceAll('"', '""')}"`;
const lines = ['FilenameAndPath;FileSize;FileSha512Hash;Category;Source;GroupId',
  ...out.map((r) => [quote(r.path), r.size, r.hash, r.cat, r.src, r.group].join(';'))];
writeFileSync(process.argv[2], lines.join('\r\n') + '\r\n');
console.log(`synthetic report: ${out.length} rows, ${group} groups`);
