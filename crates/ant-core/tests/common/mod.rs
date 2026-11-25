// Helpers shared by the test files: a small JSON reader for the fixtures, FNV-1a, a seeded random
// number generator, and the plain ant (a HashMap grid, nothing recorded but what a check needs)
// that shares no code with the crate.
#![allow(dead_code)]
use ant_core::*;
use std::collections::HashMap;
use std::io::Read;

// ---------------------------------------------------------------------------------------------
// JSON: just enough for the fixture lines (null, numbers, strings, arrays, objects, booleans)
#[derive(Debug, Clone, PartialEq)]
pub enum J {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

impl J {
    pub fn get(&self, key: &str) -> &J {
        match self {
            J::Obj(v) => v.iter().find(|(k, _)| k == key).map(|(_, v)| v).unwrap_or_else(|| panic!("no field {key}")),
            _ => panic!("not an object"),
        }
    }
    pub fn is_null(&self) -> bool {
        *self == J::Null
    }
    pub fn i(&self) -> i64 {
        match self {
            J::Num(n) => *n as i64,
            other => panic!("not a number: {other:?}"),
        }
    }
    pub fn s(&self) -> &str {
        match self {
            J::Str(s) => s,
            other => panic!("not a string: {other:?}"),
        }
    }
    pub fn b(&self) -> bool {
        match self {
            J::Bool(b) => *b,
            other => panic!("not a bool: {other:?}"),
        }
    }
    pub fn arr(&self) -> &[J] {
        match self {
            J::Arr(a) => a,
            other => panic!("not an array: {other:?}"),
        }
    }
}

pub fn parse_json(text: &str) -> J {
    let b = text.as_bytes();
    let mut at = 0;
    let v = value(b, &mut at);
    ws(b, &mut at);
    assert_eq!(at, b.len(), "trailing text in JSON");
    v
}

fn ws(b: &[u8], at: &mut usize) {
    while *at < b.len() && b[*at].is_ascii_whitespace() {
        *at += 1;
    }
}

fn value(b: &[u8], at: &mut usize) -> J {
    ws(b, at);
    match b[*at] {
        b'n' => {
            *at += 4;
            J::Null
        }
        b't' => {
            *at += 4;
            J::Bool(true)
        }
        b'f' => {
            *at += 5;
            J::Bool(false)
        }
        b'"' => J::Str(string(b, at)),
        b'[' => {
            *at += 1;
            let mut v = Vec::new();
            loop {
                ws(b, at);
                if b[*at] == b']' {
                    *at += 1;
                    return J::Arr(v);
                }
                v.push(value(b, at));
                ws(b, at);
                if b[*at] == b',' {
                    *at += 1;
                }
            }
        }
        b'{' => {
            *at += 1;
            let mut v = Vec::new();
            loop {
                ws(b, at);
                if b[*at] == b'}' {
                    *at += 1;
                    return J::Obj(v);
                }
                let k = string(b, at);
                ws(b, at);
                assert_eq!(b[*at], b':');
                *at += 1;
                v.push((k, value(b, at)));
                ws(b, at);
                if b[*at] == b',' {
                    *at += 1;
                }
            }
        }
        _ => {
            let s = *at;
            while *at < b.len() && matches!(b[*at], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                *at += 1;
            }
            J::Num(std::str::from_utf8(&b[s..*at]).unwrap().parse().unwrap())
        }
    }
}

fn string(b: &[u8], at: &mut usize) -> String {
    *at += 1;
    let s = *at;
    while b[*at] != b'"' {
        assert!(b[*at] != b'\\', "no escapes in the fixtures");
        *at += 1;
    }
    *at += 1;
    String::from_utf8(b[s..*at - 1].to_vec()).unwrap()
}

/// The lines of a gzipped JSON-lines fixture in tests/data.
pub fn fixture(name: &str) -> Vec<J> {
    let path = format!("{}/tests/data/{}", env!("CARGO_MANIFEST_DIR"), name);
    let file = std::fs::File::open(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut text = String::new();
    flate2::read::GzDecoder::new(file).read_to_string(&mut text).unwrap();
    text.lines().map(parse_json).collect()
}

// ---------------------------------------------------------------------------------------------
pub fn fnv(bytes: &[u8], mut h: u32) -> u32 {
    for &b in bytes {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}
pub const FNV0: u32 = 0x811c_9dc5;

pub fn proof_hash(p: &Proof) -> u32 {
    let mut h = FNV0;
    for c in &p.cells {
        for v in c {
            h = fnv(&v.to_le_bytes(), h);
        }
    }
    for v in &p.path_x {
        h = fnv(&v.to_le_bytes(), h);
    }
    for v in &p.path_y {
        h = fnv(&v.to_le_bytes(), h);
    }
    fnv(&p.path_h, h)
}

/// mulberry32, seeded
pub struct Rng(pub u32);
impl Rng {
    pub fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6D2B_79F5);
        let mut t = (self.0 ^ (self.0 >> 15)).wrapping_mul(1 | self.0);
        t = t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t)) ^ t;
        ((t ^ (t >> 14)) as f64) / 4294967296.0
    }
    pub fn range(&mut self, a: i64, b: i64) -> i64 {
        a + (self.next() * (b - a + 1) as f64).floor() as i64
    }
}

// ---------------------------------------------------------------------------------------------
// A fast hasher for the plain ant's map (the default one is slow for 5 million steps)
#[derive(Default, Clone, Copy)]
pub struct Fx(u64);
impl std::hash::Hasher for Fx {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0.rotate_left(5) ^ b as u64).wrapping_mul(0x517c_c1b7_2722_0a95);
        }
    }
    fn write_i64(&mut self, v: i64) {
        self.0 = (self.0.rotate_left(5) ^ v as u64).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}
pub type FxMap<K, V> = HashMap<K, V, std::hash::BuildHasherDefault<Fx>>;

pub fn key(x: i32, y: i32) -> i64 {
    (x as i64 + 2_097_152) * 4_194_304 + (y as i64 + 2_097_152)
}
pub fn key_x(k: i64) -> i32 {
    (k.div_euclid(4_194_304) - 2_097_152) as i32
}
pub fn key_y(k: i64) -> i32 {
    (k.rem_euclid(4_194_304) - 2_097_152) as i32
}

/// The plain ant. One map from cell to (visits, colour), the colour moved on by the plain rule.
/// It can go on from where it stopped. xs, ys, hs: position and heading BEFORE step t (index t =
/// now). col[t]: the colour read at step t. first_at[v]: the first step index at which some cell
/// got its v-th visit.
pub struct Plain {
    pub rule: Vec<u8>,
    pub n: u8,
    pub t: usize,
    pub x: i32,
    pub y: i32,
    pub h: u8,
    pub cells: FxMap<i64, (u32, u8)>,
    pub maxv: u32,
    pub first_at: Vec<usize>,
    pub xs: Vec<i32>,
    pub ys: Vec<i32>,
    pub hs: Vec<u8>,
    pub col: Vec<u8>,
}

impl Plain {
    pub fn new(rule: &str) -> Plain {
        Plain {
            rule: rule.bytes().collect(),
            n: rule.len() as u8,
            t: 0,
            x: 0,
            y: 0,
            h: 0,
            cells: FxMap::default(),
            maxv: 0,
            first_at: vec![0],
            xs: vec![0],
            ys: vec![0],
            hs: vec![0],
            col: Vec::new(),
        }
    }
    pub fn run(&mut self, to: usize) {
        while self.t < to {
            let e = self.cells.entry(key(self.x, self.y)).or_insert((0, 0));
            let c = e.1;
            e.0 += 1;
            e.1 = if c + 1 == self.n { 0 } else { c + 1 };
            let v = e.0;
            self.col.push(c);
            self.h = if self.rule[c as usize] == b'R' { (self.h + 1) % 4 } else { (self.h + 3) % 4 };
            if v > self.maxv {
                self.maxv = v;
                self.first_at.push(self.t);
            }
            match self.h {
                0 => self.y -= 1,
                1 => self.x += 1,
                2 => self.y += 1,
                _ => self.x -= 1,
            }
            self.t += 1;
            self.xs.push(self.x);
            self.ys.push(self.y);
            self.hs.push(self.h);
        }
    }
    /// how often the ant has stood on (0,0) up to step count N (the start counted), and when last
    pub fn home(&self, n: usize) -> (u64, u64) {
        let (mut visits, mut last) = (0, 0);
        for t in 0..=n {
            if self.xs[t] == 0 && self.ys[t] == 0 {
                visits += 1;
                last = t as u64;
            }
        }
        (visits, last)
    }
    /// the colour and visit count of a world cell
    pub fn cell(&self, x: i32, y: i32) -> (u32, u8) {
        self.cells.get(&key(x, y)).copied().unwrap_or((0, 0))
    }
}

/// grid and visit counts of an ant against the plain ant as it stands, cell by cell (a count stops at 255)
pub fn same_cells(ant: &Ant, a: &Plain) -> bool {
    let side = ant.side as i64;
    let mut live = 0;
    for (&k, &(v, c)) in &a.cells {
        let (gx, gy) = (key_x(k) as i64 + ant.off as i64, key_y(k) as i64 + ant.off as i64);
        let inside = gx >= 0 && gx < side && gy >= 0 && gy < side;
        let i = (gy * side + gx) as usize;
        let (gc, gv) = if inside { (ant.grid[i], ant.vis[i]) } else { (0, 0) };
        if gc != c || gv as u32 != v.min(255) {
            return false;
        }
        live += 1;
    }
    let seen = ant.vis.iter().filter(|&&v| v != 0).count();
    live == seen && ant.vis.len() == ant.grid.len() && ant.grid.iter().filter(|&&c| c != 0).count() == a.cells.values().filter(|v| v.1 != 0).count()
}

/// the first k letters of a rule written again and again
pub fn word(rule: &str, k: usize) -> String {
    rule.chars().cycle().take(k).collect()
}

pub fn flip(rule: &str) -> String {
    rule.chars().map(|c| if c == 'R' { 'L' } else { 'R' }).collect()
}

/// every rule of n letters in order, L = 0 and R = 1 (the most significant letter first)
pub fn all_rules(n: usize) -> impl Iterator<Item = String> {
    (0..(1u32 << n)).map(move |b| (0..n).rev().map(|k| if (b >> k) & 1 == 1 { 'R' } else { 'L' }).collect())
}

/// the state of an ant as one comparable text, grid, vis and ring left out
pub fn brief(a: &Ant) -> String {
    format!(
        "{:?}",
        (
            (a.state, a.steps, a.simulated, a.x, a.y, a.h, a.min_x, a.max_x, a.min_y, a.max_y, a.side, a.off),
            (a.lost_at, a.repeat, a.fake, a.next_check, a.next_full),
            (a.max_visits, a.home_visits, a.last_home, &a.copy_of, &a.proof)
        )
    )
}

/// The fastest way to a proved ant of a rule: run it with advance().
pub fn run(rule: &str, steps: u64) -> Ant {
    let mut a = Ant::new(rule);
    advance(&mut a, steps);
    a
}

// ---------------------------------------------------------------------------------------------
// The detector's timetables and repeats, written again from the description in the banner
pub const FULL_P: usize = 8192;

pub fn first_full_from(t: u64) -> u64 {
    let mut c = 64;
    while c < t {
        c += (c / 64).clamp(64, 8192);
    }
    c
}
/// the first look at or after step t that tries this period
pub fn first_look_from(t: u64, period: usize) -> u64 {
    let full = first_full_from(t);
    if period <= 128 {
        full.min(t.div_ceil(256) * 256)
    } else {
        full
    }
}
/// [step, longest period tried] for every look up to step n
pub fn timetable(n: u64) -> Vec<(u64, usize)> {
    let mut out = Vec::new();
    let (mut full, mut t) = (64u64, 64u64);
    while t <= n {
        let is_full = t == full;
        out.push((t, if is_full { FULL_P } else { 128 }));
        if is_full {
            full = t + (t / 64).clamp(64, 8192);
        }
        t = full.min((t / 256 + 1) * 256);
    }
    out
}
/// the last 2p colours before step t are the same p colours twice
pub fn is_candidate(col: &[u8], t: usize, p: usize) -> bool {
    (t - p..t).all(|i| col[i] == col[i - p])
}
/// the first step index >= t that reads another colour than the step p before it
pub fn true_end(r: &Plain, t: usize, p: usize) -> Option<usize> {
    (t..r.col.len()).find(|&i| r.col[i] != r.col[i - p])
}
/// the first step of the repeat with period p that holds just before step t, in the plain colours
pub fn ref_from(r: &Plain, t: usize, p: usize) -> usize {
    let mut i = t - p;
    while i > 0 && r.col[i - 1] == r.col[i - 1 + p] {
        i -= 1;
    }
    i
}
/// is R (an ant.repeat or an ant.fake) a true maximal repeat in the plain ant? It repeats from
/// `from`, not from `from - 1`, breaks exactly at `ends_at`, and one round hands the heading back
/// and moves by `shift`.
pub fn true_repeat(r: &Plain, x: &Repeat) -> bool {
    let (p, f, e) = (x.period, x.from as usize, x.ends_at as usize);
    if !(e < r.col.len() && e >= f + 2 * p) {
        return false;
    }
    if (f..e - p).any(|i| r.col[i] != r.col[i + p]) {
        return false;
    }
    (f == 0 || r.col[f - 1] != r.col[f - 1 + p])
        && r.col[e] != r.col[e - p]
        && r.hs[f + p] == r.hs[f]
        && r.xs[f + p] - r.xs[f] == x.shift[0]
        && r.ys[f + p] - r.ys[f] == x.shift[1]
        && x.rounds.is_none_or(|k| k == ((e - f) / p) as u64)
}
