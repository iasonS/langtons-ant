// =====================================================================================
// THE RULES. Ported one to one from the rules block of the original page (index.html, removed;
// in git history at 8cb1e5c): same names (in snake_case),
// same constants, same timetables, same order of operations, same tie-breaks. Nothing here
// draws or touches a page. An ant is plain data; advance(ant, k) moves its clock k steps on.
//
// The ant. A grid of cells, all colour 0 at the start. A rule is a string of L and R, one letter
// per colour. One step: read the colour under the ant, turn the way the rule says for that colour
// (R right, L left), give the cell its next colour (after the last colour comes 0 again), move
// one cell forward.
//
// The proof. Some ants end up doing the same p steps again and again, moved by a fixed shift
// each time: a highway. Seeing it repeat proves nothing: some ants do the same steps dozens of
// times in a row and then stop (the checks name one). So certify() does not count repeats. It
// takes the last p steps, lists the cells they read, and looks AHEAD along the shift: will the
// next round find on each of its cells the colour the last round found there? And the round
// after that, counting what the rounds before it leave behind? It follows the strip ahead until
// it is outside the box of everything the ant ever touched. Out there every cell is 0 and
// nothing can differ any more. If every round checks out, the ant repeats for ever: each round
// is the last one again, one shift further on. If a round does not check out, the same sum
// gives the exact step at which the repeat ends.
//
// The looks. The detector looks at set step counts, on two timetables: a quick look for short
// periods every few hundred steps, and a full look for every period, rarer as the ant gets
// older. A repeat that the check refused stays in ant.repeat until the step it ends. Each look
// notes one refused repeat (the one that ends latest), and ant.fake keeps the noted one that
// covers the most steps: what a detector that counts repeats would have fallen for. It is no
// record of the ant: a short repeat can begin and end between two looks, unseen.
//
// The jump. A proved ant is not simulated any more. Where it is after N steps is one line of
// arithmetic: position_at(ant, N).
//
// The same ant. Write a rule again and again without end: its endless word. A cell that was
// visited v times shows colour v mod n, so the next visit turns by letter number v of the
// endless word. Every cell's visits are counted (ant.vis). An ant that visits no cell more than
// V times only ever reads letters 0 to V-1, so any rule whose endless word starts with the same
// V letters makes the same moves. For a proved highway V is worked out for the whole endless run
// (proof.letters). copy_ant() then gives a rule with the same start the same proof, with no
// step simulated.
//
// Never a highway. For some rules part of the answer is known before the first step. Read a
// rule as a cycle (after its last letter comes its first). If it is made of runs of L and runs
// of R that all have even length (LLRR, RLLR, LLRRRR), a theorem says that its ant, started on
// the blank grid, comes back to its start cell again and again for ever (1995, "Further
// travels with my ant"). An ant on a highway leaves its start for ever, so such an ant never
// builds one. even_runs() reads this off the rule. It proves nothing by itself and changes
// nothing in the detector: the crate only counts how often the ant is back (ant.home_visits,
// ant.last_home).
//
// What differs from the JavaScript, and why. Steps are u64 (the page's numbers go to 2^53).
// The scratch arrays that the page keeps global (MATCH, CAND, REFUSED, F_..., H_...) live in a
// Scratch that each ant owns, so that detect() and certify() allocate nothing but the proof.
// The `state` is an enum. proof.letters can say "infinite" (a loop). A proof is shared by
// reference counting, as the page shares the object, so a copy has the very same proof.
// =====================================================================================
use std::collections::HashMap;
use std::sync::Arc;

// Headings: 0 up, 1 right, 2 down, 3 left. y grows down.
pub const HIST: usize = 32768; // steps an ant remembers (a power of two)
pub const HIST_MASK: usize = HIST - 1;
pub const MAX_PERIOD: usize = 8192; // longest period the detector looks for
pub const QUICK_EVERY: u64 = 256; // the quick look: every 256 steps, periods up to 128
pub const QUICK_PERIOD: usize = 128;
pub const START_SIDE: usize = 256; // the grid doubles from the first to the second
pub const MAX_SIDE: usize = 4096;
pub const BORDER: i64 = 2; // the ant never stands closer to the edge than this

const M: i64 = HIST_MASK as i64;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    Running,
    Highway,
    Loop,
    Lost,
}

impl State {
    /// The word the page uses.
    pub fn as_str(self) -> &'static str {
        match self {
            State::Running => "running",
            State::Highway => "highway",
            State::Loop => "loop",
            State::Lost => "lost",
        }
    }
}

/// A repeat that the check refused (ant.repeat), or the noted record (ant.fake).
/// `rounds` is set on ant.fake only.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Repeat {
    pub period: usize,
    pub shift: [i32; 2],
    pub from: u64,
    pub ends_at: u64,
    pub rounds: Option<u64>,
}

/// How many letters of the endless word an ant ever reads. 255 means "255 or more".
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Letters {
    Finite(u32),
    Infinite,
}

/// One cell of one round: [dx, dy, visits], relative to where the round began.
pub type Cell = [i32; 3];

/// The ant is proved: all that is needed to place it at any later step.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Proof {
    pub at: u64,
    pub base: u64,
    pub period: usize,
    pub shift: [i32; 2],
    pub onset: u64,
    pub path_x: Vec<i32>,
    pub path_y: Vec<i32>,
    pub path_h: Vec<u8>,
    pub cells: Vec<Cell>,
    pub letters: Letters,
}

/// What certify() says.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Certify {
    /// no, and nothing to say about when
    No,
    /// no: step index `ends_at` is the first that reads another colour than the step p before it
    Refused { shift: [i32; 2], ends_at: u64 },
    /// proved. cells = [dx, dy, visits] per cell of one round, relative to where round 0 began
    Proved { shift: [i32; 2], cells: Vec<Cell> },
}

/// Position and heading of an ant.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Position {
    pub x: i64,
    pub y: i64,
    pub h: u8,
}

/// The notes of detect() and certify(), kept so that they allocate nothing. detect(): matches per
/// period, the candidate periods, and those the check refused. certify(): the cells one period
/// read, in order of first read (f_...), and a small hash table from grid index to place in that
/// list.
pub struct Scratch {
    matches: Vec<i32>,
    cand: Vec<i32>,
    refused: Vec<i32>,
    f_x: Vec<i32>,
    f_y: Vec<i32>,
    f_v: Vec<i32>,     // reads in the period
    f_first: Vec<i32>, // offset of the first read
    f_c0: Vec<i32>,    // colour at the first read
    f_sum: Vec<i32>,   // visits by later rounds
    h_key: Vec<i32>,
    h_at: Vec<i32>,
}

impl Scratch {
    fn new() -> Scratch {
        let v = |n: usize| vec![0i32; n];
        Scratch {
            matches: v(MAX_PERIOD + 1),
            cand: v(MAX_PERIOD),
            refused: v(MAX_PERIOD),
            f_x: v(MAX_PERIOD),
            f_y: v(MAX_PERIOD),
            f_v: v(MAX_PERIOD),
            f_first: v(MAX_PERIOD),
            f_c0: v(MAX_PERIOD),
            f_sum: v(MAX_PERIOD),
            h_key: v(4 * MAX_PERIOD),
            h_at: v(4 * MAX_PERIOD),
        }
    }
}

pub struct Ant {
    pub rule: String,
    pub n: usize,
    pub turn: [u8; 256], // added to the heading; only the first n are used
    pub grid: Vec<u8>,
    pub side: usize,
    pub off: i32, // world cell (x,y) is grid[(y+off)*side + (x+off)]
    pub vis: Vec<u8>,
    pub max_visits: u32, // visits of each cell (255 = 255 or more), and the largest
    pub x: i32,
    pub y: i32,
    pub h: u8,            // the simulated ant, after `simulated` steps
    pub home_visits: u64, // times it has stood on its start cell (0,0), the start counted
    pub last_home: u64,   // and the step count of the last time
    pub steps: u64,
    pub simulated: u64, // the clock, and how much of it was really simulated
    pub min_x: i32,
    pub max_x: i32,
    pub min_y: i32,
    pub max_y: i32, // box of every cell the ant has stood on
    // the last HIST steps, step i at index i % HIST: colour read, and position and heading BEFORE the step
    pub col: Box<[u8; HIST]>,
    pub px: Box<[i32; HIST]>,
    pub py: Box<[i32; HIST]>,
    pub hd: Box<[u8; HIST]>,
    pub next_check: u64, // step counts of the next look of either kind (u64::MAX: never)
    pub next_full: u64,  // and of the next full look
    pub state: State,
    pub proof: Option<Arc<Proof>>,
    pub repeat: Option<Repeat>,
    pub fake: Option<Repeat>,
    pub lost_at: Option<u64>,
    pub copy_of: Option<String>, // the rule of the proved ant this one was copied from (copy_ant), else None
    pub scratch: Box<Scratch>,
}

fn boxed<T: Clone + Default, const N: usize>() -> Box<[T; N]> {
    match vec![T::default(); N].into_boxed_slice().try_into() {
        Ok(b) => b,
        Err(_) => unreachable!(),
    }
}

impl Ant {
    /// makeAnt. The rule is a string of L and R, 1 to 255 letters (a hand-typed rule goes through
    /// valid_rule first).
    pub fn new(rule: &str) -> Ant {
        let n = rule.len();
        assert!((1..=255).contains(&n), "a rule has 1 to 255 letters");
        let side = START_SIDE;
        let mut turn = [0u8; 256];
        for (k, ch) in rule.bytes().enumerate() {
            turn[k] = if ch == b'R' { 1 } else { 3 };
        }
        Ant {
            rule: rule.to_string(),
            n,
            turn,
            grid: vec![0; side * side],
            side,
            off: (side >> 1) as i32,
            vis: vec![0; side * side],
            max_visits: 0,
            x: 0,
            y: 0,
            h: 0,
            home_visits: 1,
            last_home: 0,
            steps: 0,
            simulated: 0,
            min_x: 0,
            max_x: 0,
            min_y: 0,
            max_y: 0,
            col: boxed(),
            px: boxed(),
            py: boxed(),
            hd: boxed(),
            next_check: 64,
            next_full: 64,
            state: State::Running,
            proof: None,
            repeat: None,
            fake: None,
            lost_at: None,
            copy_of: None,
            scratch: Box::new(Scratch::new()),
        }
    }
}

/// A rule typed by hand: 2 to 24 letters, only L and R.
pub fn valid_rule(text: &str) -> Option<String> {
    let rule = text.trim().to_uppercase();
    let ok = (2..=24).contains(&rule.chars().count()) && rule.chars().all(|c| c == 'L' || c == 'R');
    if ok {
        Some(rule)
    } else {
        None
    }
}

/// Is the rule, read as a cycle, made of runs of L and runs of R that all have even length?
/// (RLLR: the run RR goes around the end.) A rule with one letter only has no runs of both: false.
/// For such a rule a theorem says the ant never builds a highway from the blank grid (see the banner).
pub fn even_runs(rule: &str) -> bool {
    let rule = rule.as_bytes();
    let n = rule.len();
    let mut first = 0; // a place where a run begins: its letter differs from the one before it, around the end
    while first < n && rule[first] == rule[(first + n - 1) % n] {
        first += 1;
    }
    if first == n {
        return false;
    }
    let mut run = 0;
    for k in 0..n {
        run += 1;
        if rule[(first + k) % n] != rule[(first + k + 1) % n] {
            if run % 2 == 1 {
                return false;
            }
            run = 0;
        }
    }
    true
}

/// The start set: RL, then every rule of 3 to 5 letters that starts with R,
/// has both letters and is not a shorter rule written several times (RLRL is RL), then three
/// long ones. A rule that starts with L is the mirror image of one that starts with R.
pub fn start_rules() -> Vec<String> {
    let mut list = vec!["RL".to_string()];
    for len in 3..=5usize {
        for b in 0..(1u32 << (len - 1)) {
            let mut rule = String::from("R");
            for k in (0..=(len as i32 - 2)).rev() {
                rule.push(if (b >> k) & 1 == 1 { 'R' } else { 'L' });
            }
            let mut repeat = false;
            for d in 1..len {
                if len % d == 0 && rule[..d].repeat(len / d) == rule {
                    repeat = true;
                }
            }
            if rule.contains('L') && !repeat {
                list.push(rule);
            }
        }
    }
    for r in ["LRRRRRLLR", "LLRRRLRLRLLR", "RRLLLRLLLRRR"] {
        list.push(r.to_string());
    }
    list
}

/// The hot loop. The caller makes sure these steps cannot reach the edge of the grid (advance()
/// does). Here that is checked once per call, not once per step: it is what lets the loop below
/// index the grid without a bounds check.
pub fn simulate(ant: &mut Ant, count: u64) {
    if count == 0 {
        return;
    }
    let side = ant.side;
    assert!(ant.grid.len() == side * side && ant.vis.len() == side * side);
    let (gx, gy) = (ant.x as i64 + ant.off as i64, ant.y as i64 + ant.off as i64);
    let s = side as i64;
    let room = gx.min(gy).min(s - 1 - gx).min(s - 1 - gy);
    assert!(room >= 0 && count <= room as u64, "simulate: these steps could reach the edge");
    let grid = &mut ant.grid[..];
    let vis = &mut ant.vis[..];
    let turn = &ant.turn;
    let last = (ant.n - 1) as u8;
    let (col, px, py, hd) = (&mut *ant.col, &mut *ant.px, &mut *ant.py, &mut *ant.hd);
    let (mut x, mut y, mut h) = (ant.x, ant.y, ant.h);
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (ant.min_x, ant.max_x, ant.min_y, ant.max_y);
    let off = ant.off as usize;
    let mut i = (y as i64 + ant.off as i64) as usize * side + (x as i64 + ant.off as i64) as usize;
    let mut j = (ant.simulated as usize) & HIST_MASK;
    let mut most = ant.max_visits;
    let home = off * side + off; // the start cell (0,0); its index changes when the grid grows
    let (mut homes, mut back) = (0u64, 0u64); // times the ant came to stand on it in these steps, and after which of them last
    for step in 0..count {
        // SAFETY: the assert above gives room >= count, so for `count` steps the ant stays at
        // least 0 cells from every edge: the cell index i (and the one it moves to) is below
        // side * side, and grid and vis have exactly that length.
        let (c, w) = unsafe { (*grid.get_unchecked(i), *vis.get_unchecked(i)) };
        col[j] = c;
        px[j] = x;
        py[j] = y;
        hd[j] = h;
        j = (j + 1) & HIST_MASK;
        h = (h + turn[c as usize]) & 3;
        // SAFETY: same index i as above.
        unsafe {
            *grid.get_unchecked_mut(i) = if c == last { 0 } else { c + 1 };
            if w != 255 {
                // one more visit; a count stops at 255
                *vis.get_unchecked_mut(i) = w + 1;
                if w as u32 >= most {
                    most = w as u32 + 1;
                }
            }
        }
        if h == 0 {
            y -= 1;
            i = i.wrapping_sub(side);
            if y < min_y {
                min_y = y;
            }
        } else if h == 1 {
            x += 1;
            i = i.wrapping_add(1);
            if x > max_x {
                max_x = x;
            }
        } else if h == 2 {
            y += 1;
            i = i.wrapping_add(side);
            if y > max_y {
                max_y = y;
            }
        } else {
            x -= 1;
            i = i.wrapping_sub(1);
            if x < min_x {
                min_x = x;
            }
        }
        if i == home {
            homes += 1;
            back = step;
        }
    }
    if homes > 0 {
        ant.home_visits += homes;
        ant.last_home = ant.simulated + back + 1;
    }
    ant.x = x;
    ant.y = y;
    ant.h = h;
    ant.min_x = min_x;
    ant.max_x = max_x;
    ant.min_y = min_y;
    ant.max_y = max_y;
    ant.simulated += count;
    ant.steps += count;
    ant.max_visits = most;
}

/// Twice the side, the old picture in the middle. Positions are world cells, so only off changes.
pub fn grow(ant: &mut Ant) {
    let side = ant.side;
    let big = side * 2;
    let add = side >> 1;
    let wider = |old: &[u8]| {
        let mut cells = vec![0u8; big * big];
        for row in 0..side {
            let to = (row + add) * big + add;
            cells[to..to + side].copy_from_slice(&old[row * side..(row + 1) * side]);
        }
        cells
    };
    ant.grid = wider(&ant.grid);
    ant.vis = wider(&ant.vis);
    ant.side = big;
    ant.off = (big >> 1) as i32;
}

pub fn advance(ant: &mut Ant, k: u64) {
    // whole steps only; 0 changes nothing
    if k == 0 || ant.state == State::Lost {
        return;
    }
    let mut k = k;
    while k > 0 && ant.state == State::Running {
        // one step moves one cell, so this many steps are safe without looking at the edge again
        let (gx, gy) = (ant.x as i64 + ant.off as i64, ant.y as i64 + ant.off as i64);
        let s = ant.side as i64;
        let room = gx.min(gy).min(s - 1 - gx).min(s - 1 - gy) - BORDER;
        if room <= 0 {
            if ant.side >= MAX_SIDE {
                ant.state = State::Lost;
                ant.lost_at = Some(ant.steps);
                ant.repeat = None;
                return;
            }
            grow(ant);
            continue;
        }
        let run = k.min(room as u64).min(ant.next_check - ant.steps);
        simulate(ant, run);
        k -= run;
        let t = ant.steps;
        if let Some(r) = ant.repeat {
            if t > r.ends_at {
                ant.repeat = None; // that repeat is over
            }
        }
        if t < ant.next_check {
            continue;
        }
        // Two timetables. A full look tries every period. It costs the most, so it gets rarer as the
        // ant gets older (but never more than 8192 steps apart). A quick look tries the short periods
        // only, every QUICK_EVERY steps: without it a repeat of a few thousand steps can begin and end
        // between two full looks, unseen. A quick look never moves the full timetable.
        if t >= ant.next_full {
            detect(ant, MAX_PERIOD);
            ant.next_full = t + (t / 64).clamp(64, 8192);
        } else {
            detect(ant, QUICK_PERIOD);
        }
        ant.next_check = ant.next_full.min(t - t % QUICK_EVERY + QUICK_EVERY);
    }
    if ant.state != State::Running {
        ant.steps += k; // proved: the rest is skipped, not simulated
    }
}

/// Is X (ant.repeat or ant.fake) a repeat with period p that has not ended at step T?
fn running(x: Option<Repeat>, p: usize, t: u64) -> bool {
    matches!(x, Some(r) if r.period == p && t <= r.ends_at)
}

/// The detector. A period p is a candidate when the last 2p colours read were the same p colours
/// twice. That alone means nothing (see the banner): certify() decides. max_p = the longest period to try.
pub fn detect(ant: &mut Ant, max_p: usize) {
    let t = ant.simulated;
    let end = t as i64 - 1;
    let max_p = max_p.min((t / 2) as usize);
    let span = 2 * max_p;
    let mut cands = 0usize;
    {
        let col = &ant.col;
        let sc = &mut *ant.scratch;
        let len = &mut sc.matches;
        // len[p] = how many of the newest colours, going back, equal the colour p steps before them
        // (inside the last 2*max_p steps). For all p in one pass: a stretch [from, to) that is known to
        // match the newest colours is not compared again (the "Z algorithm"), so the work stays small
        // even when the ant walks a long regular stretch.
        //
        // Nearly every period fails within the newest six colours, so those are tested first, in one go:
        // one test that nearly always says no costs much less than six that go either way. `newest` is
        // the newest six colours packed into one number, five bits each. `six` is the six colours p steps
        // before them, moved on by one colour per period. (& 31: a rule has at most 24 colours. With more
        // than 32, two packs could be equal by chance but never unequal by chance, and equal packs are
        // compared colour by colour anyway.)
        let mut newest = 0u32;
        for k in (0..=5i64).rev() {
            newest = (newest << 5) | (col[((end - k) & M) as usize] & 31) as u32;
        }
        let mut six = newest;
        let (mut from, mut to) = (0usize, 0usize);
        for p in 1..=max_p {
            six = (six >> 5) | (((col[((end - 5 - p as i64) & M) as usize] & 31) as u32) << 25);
            let mut z = if p < to { (to - p).min(len[p - from] as usize) } else { 0 };
            // len[p] is noted as 0 here though it may be up to 5. Too low is safe, and p >= 6 is then no candidate.
            if z == 0 && p >= 6 && six != newest {
                len[p] = 0;
                continue;
            }
            while p + z < span && col[((end - z as i64) & M) as usize] == col[((end - p as i64 - z as i64) & M) as usize] {
                z += 1;
            }
            len[p] = z as i32;
            if p + z > to {
                from = p;
                to = p + z;
            }
            if z >= p {
                sc.cand[cands] = p as i32;
                cands += 1;
            }
        }
    }
    // The candidates, shortest first. The first one that is proved ends the search. Of those the check
    // refuses, remember the one that will last longest (the shorter period on a tie).
    let (r, f) = (ant.repeat, ant.fake);
    let (mut refused, mut best_p, mut best_end, mut best_shift) = (0usize, 0usize, -1i64, [0i32; 2]);
    for c in 0..cands {
        let p = ant.scratch.cand[c] as usize;
        // If q was refused and all of the last 2p colours keep period q, then a multiple p of q is a
        // candidate with the same end as q (the colour that breaks q breaks p). Nothing new: skip it.
        // Without this an ant on a long regular stretch tries hundreds of multiples at every check.
        let mut known = false;
        for k in 0..refused {
            let q = ant.scratch.refused[k] as usize;
            if p.is_multiple_of(q) && ant.scratch.matches[q] as i64 >= 2 * p as i64 - q as i64 {
                known = true;
            }
        }
        if known {
            continue;
        }
        // A repeat refused at an earlier look and still running: its end is exact, so it is not worked
        // out again. (An ant on a long refused stretch would pay for that at every quick look.)
        let old = if running(r, p, t) {
            r
        } else if running(f, p, t) {
            f
        } else {
            None
        };
        let (ends_at, shift) = if let Some(o) = old {
            (o.ends_at as i64, o.shift)
        } else {
            match certify(ant, p) {
                Certify::Proved { shift, cells } => {
                    set_proof(ant, p, shift, cells);
                    return;
                }
                Certify::No => continue,
                Certify::Refused { shift, ends_at } => (ends_at as i64, shift),
            }
        };
        if ends_at > best_end {
            best_p = p;
            best_end = ends_at;
            best_shift = shift;
        }
        ant.scratch.refused[refused] = p as i32;
        refused += 1;
    }
    if best_p == 0 {
        return;
    }
    // Already noted at an earlier look: all that follows was done then, and its first step was found
    // then. (Walking back again later could come out wrong: the ring forgets.)
    let same = |x: Option<Repeat>| matches!(x, Some(x) if x.period == best_p && x.ends_at as i64 == best_end);
    if same(r) || same(f) {
        return;
    }
    let first = first_step(ant, best_p);
    let rounds = (best_end - first as i64) as u64 / best_p as u64;
    // On show: only with a good stretch still to run, and never in place of a repeat that ends later.
    // Once on show it stays until the step it ends (advance() clears it): its end is exact, so it stays true.
    if best_end - t as i64 >= (8 * best_p).max(256) as i64 && r.is_none_or(|r| best_end > r.ends_at as i64) {
        ant.repeat = Some(Repeat {
            period: best_p,
            shift: best_shift,
            from: first,
            ends_at: best_end as u64,
            rounds: None,
        });
    }
    // The record: of the repeats noted here so far (one per look), the one that covers the most steps,
    // from its first step to its end. Only a longer one takes its place. It outlives a proof.
    if f.is_none_or(|f| best_end - first as i64 > f.ends_at as i64 - f.from as i64) {
        ant.fake = Some(Repeat {
            period: best_p,
            shift: best_shift,
            from: first,
            ends_at: best_end as u64,
            rounds: Some(rounds),
        });
    }
}

/// The first step of the repeat with period p that the ant is in: walk back while a step read the
/// same colour as the step p after it. Not further back than the ring remembers; a repeat older
/// than that began at the step returned or before it.
pub fn first_step(ant: &Ant, p: usize) -> u64 {
    let t = ant.simulated as i64;
    let oldest = (t - HIST as i64).max(0);
    let p = p as i64;
    let mut i = t - p;
    while i > oldest && ant.col[((i - 1) & M) as usize] == ant.col[((i - 1 + p) & M) as usize] {
        i -= 1;
    }
    i as u64
}

/// One round's cells as a list, for a proof: [dx, dy, visits] relative to where the round began.
fn footprint(sc: &Scratch, m: usize) -> Vec<Cell> {
    (0..m).map(|f| [sc.f_x[f] - sc.f_x[0], sc.f_y[f] - sc.f_y[0], sc.f_v[f]]).collect()
}

/// Does the ant repeat its last p steps for ever? T = ant.simulated; round 0 is steps T-p .. T-1.
///   Proved { shift, cells }     proved. cells = [dx, dy, visits] per cell of one round,
///                               relative to where round 0 began
///   Refused { shift, ends_at }  no: step index ends_at is the first that reads another colour
///                               than the step p before it
///   No                          no, and nothing to say about when
pub fn certify(ant: &mut Ant, p: usize) -> Certify {
    let t = ant.simulated;
    let n = ant.n as i32;
    let (side, off) = (ant.side as i64, ant.off as i64);
    if !(1..=MAX_PERIOD).contains(&p) || p as u64 > t {
        return Certify::No;
    }
    let start = (t - p as u64) as usize;
    let j0 = start & HIST_MASK;
    if ant.hd[j0] != ant.h {
        return Certify::No; // a round must hand the heading back
    }
    let (sx, sy) = (ant.x - ant.px[j0], ant.y - ant.py[j0]);

    // the cells round 0 read: how often (f_v), and at which offset first (f_first)
    let mut bits = 3;
    while (1usize << bits) < 4 * p {
        bits += 1;
    }
    let mask = (1usize << bits) - 1;
    let down = 32 - bits;
    let sc = &mut *ant.scratch;
    sc.h_key[..=mask].fill(-1);
    let (mut m, mut x0, mut y0) = (0usize, ant.px[j0], ant.py[j0]);
    let (mut x1, mut y1) = (x0, y0);
    for r in 0..p {
        let j = (start + r) & HIST_MASK;
        let (x, y) = (ant.px[j], ant.py[j]);
        let key = ((y as i64 + off) * side + (x as i64 + off)) as i32;
        let mut s = ((key as u32).wrapping_mul(0x9E37_79B1) >> down) as usize;
        while sc.h_key[s] != -1 && sc.h_key[s] != key {
            s = (s + 1) & mask;
        }
        if sc.h_key[s] == key {
            sc.f_v[sc.h_at[s] as usize] += 1;
            continue;
        }
        sc.h_key[s] = key;
        sc.h_at[s] = m as i32;
        sc.f_x[m] = x;
        sc.f_y[m] = y;
        sc.f_v[m] = 1;
        sc.f_first[m] = r as i32;
        m += 1;
        if x < x0 {
            x0 = x;
        } else if x > x1 {
            x1 = x;
        }
        if y < y0 {
            y0 = y;
        } else if y > y1 {
            y1 = y;
        }
    }

    if sx == 0 && sy == 0 {
        // Back on the same cell with the same heading. If every cell was read a multiple of n times
        // the whole grid is back too, and the same state can only do the same again.
        for f in 0..m {
            if sc.f_v[f] % n != 0 {
                return Certify::No;
            }
        }
        return Certify::Proved {
            shift: [0, 0],
            cells: footprint(sc, m),
        };
    }

    // Round j (j = 1, 2, ...) reads the cell f + j*shift where round 0 read f. Round 0 found f in the
    // colour it has now minus its own visits. Round j finds its cell as it is now, plus the visits of
    // rounds 1 .. j-1, which land on f + shift, f + 2*shift, ... as round 0's did on f. All mod n.
    for f in 0..m {
        let at = ((sc.f_y[f] as i64 + off) * side + (sc.f_x[f] as i64 + off)) as usize;
        sc.f_c0[f] = (((ant.grid[at] as i32 - sc.f_v[f]) % n) + n) % n;
        sc.f_sum[f] = 0;
    }
    // Follow the strip until the round's box no longer meets the box of all the ant ever stood on,
    // and one round more. From there on every cell is 0 and has no visits, so the sums stay as they are.
    let mut jj = 1i64;
    let (sx64, sy64) = (sx as i64, sy as i64);
    while x0 as i64 + jj * sx64 <= ant.max_x as i64
        && x1 as i64 + jj * sx64 >= ant.min_x as i64
        && y0 as i64 + jj * sy64 <= ant.max_y as i64
        && y1 as i64 + jj * sy64 >= ant.min_y as i64
    {
        jj += 1;
    }
    jj += 1;
    for j in 1..=jj {
        let (ox, oy) = (j * sx64, j * sy64);
        for f in 0..m {
            // in order of first read, so the first miss is the earliest step
            let x = sc.f_x[f] as i64 + ox;
            let y = sc.f_y[f] as i64 + oy;
            let (gx, gy) = (x + off, y + off);
            let (mut now, mut visits) = (0i32, 0i32); // a cell outside the grid has never been touched
            if gx >= 0 && gx < side && gy >= 0 && gy < side {
                let key = (gy * side + gx) as i32;
                now = ant.grid[key as usize] as i32;
                if x >= x0 as i64 && x <= x1 as i64 && y >= y0 as i64 && y <= y1 as i64 {
                    let mut s = ((key as u32).wrapping_mul(0x9E37_79B1) >> down) as usize;
                    while sc.h_key[s] != -1 && sc.h_key[s] != key {
                        s = (s + 1) & mask;
                    }
                    if sc.h_key[s] == key {
                        visits = sc.f_v[sc.h_at[s] as usize];
                    }
                }
            }
            if (now + sc.f_sum[f]) % n != sc.f_c0[f] {
                return Certify::Refused {
                    shift: [sx, sy],
                    ends_at: t + (j as u64 - 1) * p as u64 + sc.f_first[f] as u64,
                };
            }
            sc.f_sum[f] += visits;
        }
    }
    Certify::Proved {
        shift: [sx, sy],
        cells: footprint(sc, m),
    }
}

/// The ant is proved: write down all that is needed to place it at any later step.
pub fn set_proof(ant: &mut Ant, p: usize, shift: [i32; 2], cells: Vec<Cell>) {
    let t = ant.simulated;
    let base = t - p as u64;
    let onset = first_step(ant, p); // the detector looks often enough for the ring to reach that far
    let (mut path_x, mut path_y, mut path_h) = (vec![0i32; p], vec![0i32; p], vec![0u8; p]);
    for r in 0..p {
        let j = (base as usize + r) & HIST_MASK;
        path_x[r] = ant.px[j];
        path_y[r] = ant.py[j];
        path_h[r] = ant.hd[j];
    }
    let mut proof = Proof {
        at: t,
        base,
        period: p,
        shift,
        onset,
        path_x,
        path_y,
        path_h,
        cells,
        letters: Letters::Finite(0),
    };
    proof.letters = letters_read(ant, &proof);
    ant.proof = Some(Arc::new(proof));
    ant.state = if shift == [0, 0] { State::Loop } else { State::Highway };
    ant.repeat = None;
}

/// The largest number of visits any cell gets over the whole endless run of a proved ant. That is
/// how many letters of its endless word the ant ever reads (see the banner). Infinite for a loop:
/// it visits the same cells without end. 255 means "255 or more": a count is one byte.
///
/// A cell that no later round touches keeps the count it has now, and ant.max_visits covers it.
/// Round j (j = 1, 2, ...) visits the cell f + j*shift as often as round 0 visited f. So the cell
/// y = f + j*shift ends with its count now, plus v(y - shift) + v(y - 2*shift) + ... for ALL the
/// rounds to come (v = visits by round 0, and 0 off its cells). That is `behind` below: the visits
/// by round 0 on f itself and on the cells behind f on its line, and on one more cell ahead of f
/// for every round j that has gone by.
/// The rounds are followed until one lies clear of the box of everything the ant ever stood on.
/// Round 0's own cells lie inside that box, so no cell of that round has a cell of round 0 ahead of
/// it on its line any more: each gets the whole sum along its line, on a count of 0. So does every
/// cell of every round after it, and nothing larger can come.
pub fn letters_read(ant: &Ant, proof: &Proof) -> Letters {
    let (sx, sy) = (proof.shift[0] as i64, proof.shift[1] as i64);
    let cells = &proof.cells;
    let m = cells.len();
    if sx == 0 && sy == 0 {
        return Letters::Infinite;
    }
    if ant.max_visits >= 255 {
        return Letters::Finite(255); // some count has stopped: no exact number
    }
    let (vis, side, off) = (&ant.vis, ant.side as i64, ant.off as i64);
    let (bx, by) = (proof.path_x[0] as i64 + off, proof.path_y[0] as i64 + off);
    // round 0's visits by cell (dx, dy from where the round began), and the box of its cells
    let mut visits: HashMap<i64, i64> = HashMap::new();
    let key = |dx: i64, dy: i64| dy * 65536 + dx;
    let (mut x0, mut x1, mut y0, mut y1) = (0i64, 0i64, 0i64, 0i64);
    for c in cells {
        let (dx, dy, v) = (c[0] as i64, c[1] as i64, c[2] as i64);
        visits.insert(key(dx, dy), v);
        if dx < x0 {
            x0 = dx;
        } else if dx > x1 {
            x1 = dx;
        }
        if dy < y0 {
            y0 = dy;
        } else if dy > y1 {
            y1 = dy;
        }
    }
    let v0 = |dx: i64, dy: i64| -> i64 {
        if dx >= x0 && dx <= x1 && dy >= y0 && dy <= y1 {
            visits.get(&key(dx, dy)).copied().unwrap_or(0)
        } else {
            0
        }
    };
    let mut behind = vec![0i64; m];
    for f in 0..m {
        let (mut dx, mut dy, mut sum) = (cells[f][0] as i64, cells[f][1] as i64, 0i64);
        while dx >= x0 && dx <= x1 && dy >= y0 && dy <= y1 {
            sum += v0(dx, dy);
            dx -= sx;
            dy -= sy;
        }
        behind[f] = sum;
    }
    let mut most = ant.max_visits as i64;
    let mut j = 1i64;
    loop {
        let (ox, oy) = (j * sx, j * sy);
        let (gx0, gx1, gy0, gy1) = (bx + x0 + ox, bx + x1 + ox, by + y0 + oy, by + y1 + oy); // this round's box, in grid cells
        let clear = gx0 > ant.max_x as i64 + off || gx1 < ant.min_x as i64 + off || gy0 > ant.max_y as i64 + off || gy1 < ant.min_y as i64 + off;
        let meets = x0 + ox <= x1 && x1 + ox >= x0 && y0 + oy <= y1 && y1 + oy >= y0; // does it still meet round 0's box?
        for f in 0..m {
            let (dx, dy) = (cells[f][0] as i64 + ox, cells[f][1] as i64 + oy);
            let (gx, gy) = (bx + dx, by + dy);
            // outside the grid: never touched
            let now = if clear || gx < 0 || gx >= side || gy < 0 || gy >= side {
                0
            } else {
                vis[(gy * side + gx) as usize] as i64
            };
            if now + behind[f] > most {
                most = now + behind[f];
            }
            if meets {
                behind[f] += v0(dx, dy);
            }
        }
        if clear {
            break;
        }
        j += 1;
    }
    Letters::Finite(most.min(255) as u32)
}

/// Do the endless words of two rules agree on their first k letters? k may be larger than both
/// rules. Two endless words that agree on the first length_a + length_b letters are equal (both then
/// repeat the same shorter word), so more letters are never looked at, and k = u64::MAX asks just that.
pub fn same_start(rule_a: &str, rule_b: &str, k: u64) -> bool {
    let (a, b) = (rule_a.as_bytes(), rule_b.as_bytes());
    let (la, lb) = (a.len(), b.len());
    let up_to = k.min((la + lb) as u64) as usize;
    if la == 0 || lb == 0 {
        return up_to == 0; // no letter there to agree
    }
    (0..up_to).all(|i| a[i % la] == b[i % lb])
}

/// A rule that starts like a proved highway is the same ant: the highway never reads past its first
/// proof.letters letters. Returns that ant for `rule` with no step simulated, or None when the source
/// is no proved highway, its number is not exact (255), or the rule does not start the same.
/// The copy shares the source's proof (never written). Its cells are the source's visit counts
/// mod its own number of colours: the source stopped simulating at its proof, so these are the cells
/// at step proof.at. Its clock starts at 0 like any ant's; position_at() answers from proof.base on.
/// A loop is not copied: a loop rule proves itself within a few steps.
pub fn copy_ant(rule: &str, source: &Ant) -> Option<Ant> {
    if source.state != State::Highway {
        return None;
    }
    let p = source.proof.as_ref()?;
    let k = match p.letters {
        Letters::Finite(l) if l < 255 => l as u64,
        _ => return None,
    };
    if !same_start(rule, &source.rule, k) {
        return None;
    }
    let mut ant = Ant::new(rule);
    let n = ant.n;
    let vis = source.vis.clone();
    let grid: Vec<u8> = vis.iter().map(|&v| (v as usize % n) as u8).collect();
    ant.grid = grid;
    ant.vis = vis;
    ant.side = source.side;
    ant.off = source.off;
    ant.max_visits = source.max_visits;
    ant.x = source.x;
    ant.y = source.y;
    ant.h = source.h; // where the simulating stopped, as in the source
    ant.min_x = source.min_x;
    ant.max_x = source.max_x;
    ant.min_y = source.min_y;
    ant.max_y = source.max_y;
    ant.home_visits = source.home_visits;
    ant.last_home = source.last_home; // the same path, so the same times back at the start
    ant.state = State::Highway;
    ant.proof = Some(Arc::clone(p));
    ant.copy_of = Some(source.rule.clone());
    Some(ant)
}

/// Position and heading after N steps, for a proved ant: whole rounds times the shift, plus the
/// place inside the round. Exact for any N up to 2^53 (and beyond, to 2^63). None if not proved or
/// N is before the proof's base.
pub fn position_at(ant: &Ant, n: u64) -> Option<Position> {
    let p = ant.proof.as_ref()?;
    if n < p.base {
        return None;
    }
    let m = (n - p.base) / p.period as u64;
    let r = ((n - p.base) % p.period as u64) as usize;
    Some(Position {
        x: p.path_x[r] as i64 + m as i64 * p.shift[0] as i64,
        y: p.path_y[r] as i64 + m as i64 * p.shift[1] as i64,
        h: p.path_h[r],
    })
}
