'use strict';
// Writes the reference data the Rust port is tested against: what the rules block of index.html
// decides, rule by rule. Node only, no packages.
//
// Rerun (from the repo root; about 10 s for the first file):
//   node tools/js-reference.js
// It writes two gzip files into crates/ant-core/tests/data/:
//   js-300k.jsonl.gz   every rule of 2 to 12 letters that starts with R (4,094 rules), advance(ant, 300000)
//   js-demo.jsonl.gz   the 28 startRules() ants, advance(ant, 2000000)
// One JSON line per rule. grid and vis are FNV-1a 32 hashes of the whole byte arrays (side x side).
// Never edit the files by hand: if the Rust differs, fix the Rust.
const fs = require('fs'), path = require('path'), zlib = require('zlib');

const PAGE = path.join(__dirname, '..', 'index.html');
const OUT = path.join(__dirname, '..', 'crates', 'ant-core', 'tests', 'data');

// the rules block: from the THE RULES banner to the THE PICTURE banner (as check.js cutBlock does)
function cutBlock(file) {
  const lines = fs.readFileSync(file, 'utf8').split('\n');
  const isBar = s => /^\s*\/\/ ?={20,}\s*$/.test(s || '');
  let a = lines.findIndex(l => l.includes('THE RULES'));
  if (a < 0) throw new Error('no banner with THE RULES in ' + file);
  if (isBar(lines[a - 1])) a--;
  let b = lines.findIndex((l, i) => i > a + 1 && l.includes('THE PICTURE'));
  if (b < 0) throw new Error('no banner with THE PICTURE in ' + file);
  if (isBar(lines[b - 1])) b--;
  return lines.slice(a, b).join('\n');
}
const vm = require('vm');
const api = new Function(cutBlock(PAGE) +
  '\nreturn { makeAnt, advance, startRules, evenRuns };')();
// the same block once more in an empty context: it must not touch the page
vm.runInNewContext('(function () {\n' + cutBlock(PAGE) + '\nadvance(makeAnt("RL"), 50000);\n})()', {});

function fnv(bytes, h = 0x811c9dc5) {
  for (let i = 0; i < bytes.length; i++) { h ^= bytes[i]; h = Math.imul(h, 0x01000193); }
  return h >>> 0;
}
const bytesOf = a => new Uint8Array(a.buffer, a.byteOffset, a.byteLength);   // little-endian host
const lastOf = r => r === null ? null : { period: r.period, shift: r.shift, from: r.from, endsAt: r.endsAt, rounds: r.rounds === undefined ? null : r.rounds };

function record(rule, steps) {
  const a = api.makeAnt(rule);
  api.advance(a, steps);
  let proof = null;
  if (a.proof) {
    const P = a.proof;
    const flat = Int32Array.from(P.cells.flat());
    let h = fnv(bytesOf(flat));
    h = fnv(bytesOf(P.pathX), h); h = fnv(bytesOf(P.pathY), h); h = fnv(P.pathH, h);
    proof = {
      at: P.at, base: P.base, period: P.period, shift: P.shift, onset: P.onset,
      letters: P.letters === Infinity ? 'inf' : P.letters, cells: P.cells.length, hash: h,
    };
  }
  return {
    rule, even: api.evenRuns(rule), state: a.state, steps: a.steps, simulated: a.simulated,
    x: a.x, y: a.y, h: a.h, minX: a.minX, maxX: a.maxX, minY: a.minY, maxY: a.maxY,
    side: a.side, maxVisits: a.maxVisits, homeVisits: a.homeVisits, lastHome: a.lastHome,
    lostAt: a.lostAt, nextCheck: a.nextCheck, nextFull: a.nextFull,
    repeat: lastOf(a.repeat), fake: lastOf(a.fake), proof,
    gridHash: fnv(a.grid), visHash: fnv(a.vis),
  };
}

function write(name, rules, steps) {
  const t0 = Date.now();
  const text = rules.map(r => JSON.stringify(record(r, steps))).join('\n') + '\n';
  const gz = zlib.gzipSync(Buffer.from(text), { level: 9 });
  fs.mkdirSync(OUT, { recursive: true });
  fs.writeFileSync(path.join(OUT, name), gz);
  console.log(`${name}: ${rules.length} rules, ${steps} steps, ${text.length} bytes, ${gz.length} gzipped, ${((Date.now() - t0) / 1000).toFixed(1)} s`);
}

const census = [];
for (let n = 2; n <= 12; n++) for (let b = 0; b < 1 << (n - 1); b++) {
  let rule = 'R';
  for (let k = n - 2; k >= 0; k--) rule += (b >> k) & 1 ? 'R' : 'L';
  census.push(rule);
}
write('js-300k.jsonl.gz', census, 300000);
write('js-demo.jsonl.gz', api.startRules(), 2000000);
