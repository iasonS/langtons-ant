// check.js tableSame: proof.letters against a plain ant run on, for every census highway (the
// 4,094 rules of 2 to 12 letters that start with R, run to 300,000 steps); the same number from an
// earlier moment; copies against the plain ant of the copied rule; the number is tight; and the sum
// over the rounds to come in letters_read() on made-up footprints.
mod common;
use ant_core::*;
use common::*;

/// Run every census rule to 300,000 steps and give every proved highway to `f`. The ant is dropped
/// after its turn: 4,094 ants at once would not fit in memory.
fn each_highway(mut f: impl FnMut(&str, Ant)) -> (usize, usize, usize) {
    let (mut highways, mut loops, mut others) = (0, 0, 0);
    for n in 2..=12usize {
        for b in 0..(1u32 << (n - 1)) {
            let mut rule = String::from("R");
            rule.extend((0..n - 1).rev().map(|k| if (b >> k) & 1 == 1 { 'R' } else { 'L' }));
            let ant = run(&rule, 300_000);
            match ant.state {
                State::Highway => {
                    highways += 1;
                    f(&rule, ant)
                }
                State::Loop => {
                    loops += 1;
                    assert_eq!(
                        ant.proof.as_ref().unwrap().letters,
                        Letters::Infinite,
                        "{rule}: a loop reads letters without end"
                    );
                    assert!(copy_ant(&rule, &ant).is_none(), "{rule}: no copy of a loop");
                }
                _ => {
                    others += 1;
                    assert!(copy_ant(&rule, &ant).is_none(), "{rule}: no copy of an ant that is no proved highway");
                }
            }
        }
    }
    (highways, loops, others)
}

fn letters(a: &Ant) -> u32 {
    match a.proof.as_ref().unwrap().letters {
        Letters::Finite(l) => l,
        Letters::Infinite => panic!("infinite"),
    }
}

#[test]
fn proof_letters_equals_the_largest_visit_count_of_the_plain_ant_run_on() {
    let (mut sat, mut dist) = (0, Vec::new());
    let counts = each_highway(|rule, ant| {
        let p = ant.proof.clone().unwrap();
        let (at, per) = (p.at as usize, p.period);
        let l = letters(&ant);
        let mut x = Plain::new(rule);
        x.run(at);
        // the counts the ant kept, at the proof
        assert!(same_cells(&ant, &x) && ant.max_visits == x.maxv.min(255), "{rule}: visit counts at the proof");
        // How many rounds on? From the plain ant alone: until a round lies clear of all it visited before
        // the proof (J), plus the rounds whose cells still reach into one another (K), plus 2. At least 60.
        let (sx, sy) = (x.xs[at] - x.xs[at - per], x.ys[at] - x.ys[at - per]);
        assert_eq!([sx, sy], p.shift, "{rule}: shift");
        let boxof = |a: usize, b: usize| {
            let mut q = [x.xs[a], x.xs[a], x.ys[a], x.ys[a]];
            for i in a..b {
                q = [q[0].min(x.xs[i]), q[1].max(x.xs[i]), q[2].min(x.ys[i]), q[3].max(x.ys[i])];
            }
            q
        };
        let (fbox, vbox) = (boxof(at - per, at), boxof(0, at));
        let meets = |a: [i32; 4], b: [i32; 4], ox: i32, oy: i32| a[0] + ox <= b[1] && a[1] + ox >= b[0] && a[2] + oy <= b[3] && a[3] + oy >= b[2];
        let (mut jj, mut kk) = (1, 1);
        while meets(fbox, vbox, jj * sx, jj * sy) {
            jj += 1;
        }
        while meets(fbox, fbox, kk * sx, kk * sy) {
            kk += 1;
        }
        let r = 60usize.max((jj + kk + 2) as usize);
        x.run(at + r * per);
        let m1 = x.maxv;
        x.run(at + (r + 40) * per);
        let m2 = x.maxv;
        assert_eq!(m1, m2, "{rule}: still moving");
        assert_eq!(l, m2.min(255), "{rule}: page {l}, plain ant {m2}");
        if l == 255 {
            sat += 1;
        }
        dist.push(l);
    });
    assert_eq!(counts.0, 408);
    dist.sort();
    eprintln!(
        "highways {} loops {} others {}; proof.letters: smallest {}, median {}, {} are 255",
        counts.0,
        counts.1,
        counts.2,
        dist[0],
        dist[dist.len() / 2],
        sat
    );
}

#[test]
fn the_same_number_from_an_earlier_moment() {
    // an ant with its looks switched off, stopped at a random step of the second round after the
    // repeat began; certify() is asked there directly and letters_read() must give the same number
    let mut rng = Rng(20261122);
    let (mut early, mut ahead, mut before) = (0, 0, 0);
    each_highway(|rule, ant| {
        let p = ant.proof.clone().unwrap();
        let t = p.onset + p.period as u64 + rng.range(0, p.period as i64 - 1) as u64;
        let mut e = Ant::new(rule);
        e.next_check = u64::MAX;
        advance(&mut e, t);
        assert_eq!(e.state, State::Running);
        let (shift, cells) = match certify(&mut e, p.period) {
            Certify::Proved { shift, cells } => (shift, cells),
            other => panic!("{rule} at step {t}: certify said {other:?}"),
        };
        let j0 = ((t - p.period as u64) as usize) & HIST_MASK;
        let made = Proof {
            at: t,
            base: t - p.period as u64,
            period: p.period,
            shift,
            onset: 0,
            path_x: vec![e.px[j0]],
            path_y: vec![e.py[j0]],
            path_h: vec![],
            cells,
            letters: Letters::Finite(0),
        };
        let got = letters_read(&e, &made);
        early += 1;
        if let Letters::Finite(g) = got {
            if g > e.max_visits && g < 255 {
                ahead += 1;
            }
        }
        if t < p.at {
            before += 1;
        }
        assert_eq!(got, p.letters, "{rule} at step {t}");
    });
    eprintln!("{early} highways looked at early (before the proof step for {before}); the sum over the rounds to come decided for {ahead}");
}

#[test]
fn copies_walk_the_plain_ant_of_the_copied_rule() {
    let mut rng = Rng(7);
    let (mut sources, mut copies) = (0, 0);
    each_highway(|rule, ant| {
        let l = letters(&ant) as usize;
        if l > 20 {
            return;
        }
        sources += 1;
        let p = ant.proof.clone().unwrap();
        let (at, per, base) = (p.at as usize, p.period, p.base as usize);
        let start = word(rule, l);
        let mut ys: Vec<String> = Vec::new();
        for _ in 0..5 {
            let mut y = start.clone();
            let len = rng.range(2.max(l as i64), 24) as usize;
            while y.len() < len {
                y.push(if rng.next() < 0.5 { 'L' } else { 'R' });
            }
            ys.push(y);
        }
        // shorter than the letters, and its endless word still starts the same: a repeat inside the start
        let shorts: Vec<String> = (2..l.min(25)).map(|m| word(rule, m)).filter(|y| word(y, l) == start).collect();
        let others: Vec<&String> = shorts.iter().filter(|y| y.as_str() != rule).collect();
        if !others.is_empty() {
            ys.push(others[rng.range(0, others.len() as i64 - 1) as usize].clone());
        } else if !shorts.is_empty() {
            ys.push(rule.to_string());
        }
        // the plain ant of the source, for its path
        let mut x = Plain::new(rule);
        x.run(at + 40 * per);
        for y in ys {
            copies += 1;
            let mut copy = copy_ant(&y, &ant).unwrap_or_else(|| panic!("no copy of {y} from {rule}"));
            assert!(
                copy.rule == y && copy.n == y.len() && copy.state == State::Highway && copy.copy_of.as_deref() == Some(rule),
                "{y} from {rule}: fields"
            );
            assert!(std::sync::Arc::ptr_eq(copy.proof.as_ref().unwrap(), &p));
            assert!((copy.simulated, copy.steps) == (0, 0) && copy.fake.is_none() && copy.repeat.is_none());
            assert!((0..y.len()).all(|k| copy.turn[k] == if y.as_bytes()[k] == b'R' { 1 } else { 3 }));
            assert!(
                (copy.side, copy.off, copy.min_x, copy.max_x, copy.min_y, copy.max_y, copy.max_visits)
                    == (ant.side, ant.off, ant.min_x, ant.max_x, ant.min_y, ant.max_y, ant.max_visits),
                "{y} from {rule}: box"
            );
            assert!(position_at(&copy, p.base - 1).is_none());
            let mut a = Plain::new(&y);
            a.run(at);
            assert!(same_cells(&copy, &a), "{y} from {rule}: grid at step proof.at");
            assert_eq!((copy.home_visits, copy.last_home), a.home(at), "{y} from {rule}: home counts");
            // proof.cells stamped 1, 2 and 5 times, mod the copy's number of colours
            let mut want: FxMap<i64, u8> = a.cells.iter().map(|(&k, &(_, c))| (k, c)).collect();
            let mut done = 0;
            for k in [1usize, 2, 5] {
                while done < k {
                    done += 1;
                    for c in &p.cells {
                        let kk = key(p.path_x[0] + c[0] + done as i32 * p.shift[0], p.path_y[0] + c[1] + done as i32 * p.shift[1]);
                        let e = want.entry(kk).or_insert(0);
                        *e = ((*e as i32 + c[2]) % copy.n as i32) as u8;
                    }
                }
                a.run(at + k * per);
                let real: FxMap<i64, u8> = a.cells.iter().map(|(&k, &(_, c))| (k, c)).collect();
                let diff = real.iter().filter(|(k, &c)| want.get(k).copied().unwrap_or(0) != c).count()
                    + want.iter().filter(|(k, &c)| real.get(k).copied().unwrap_or(0) != c).count();
                assert_eq!(diff, 0, "{y} from {rule}: stamped {k} times");
            }
            // the path: position_at(copy, N) at every step from proof.base to proof.at + 40 periods, and the source's own path from 0
            a.run(at + 40 * per);
            for n in base..=at + 40 * per {
                let q = position_at(&copy, n as u64).unwrap();
                assert!(q.x == a.xs[n] as i64 && q.y == a.ys[n] as i64 && q.h == a.hs[n], "{y} from {rule}: step {n}");
            }
            for n in 0..=at + 40 * per {
                assert!(
                    a.xs[n] == x.xs[n] && a.ys[n] == x.ys[n] && a.hs[n] == x.hs[n],
                    "{y} walks the path of {rule}: step {n}"
                );
            }
            advance(&mut copy, 12345);
            assert_eq!((copy.steps, copy.simulated), (12345, 0));
        }
    });
    eprintln!("{sources} sources with 20 letters or fewer, {copies} copies");
    assert!(sources > 100);
}

#[test]
fn the_number_is_tight() {
    let (mut tight, mut skipped) = (0, 0);
    each_highway(|rule, ant| {
        let l = letters(&ant) as usize;
        if !(2..=24).contains(&l) {
            skipped += 1;
            return;
        }
        tight += 1;
        let p = ant.proof.clone().unwrap();
        // the same first l-1 letters and another letter after them
        let w = word(rule, l);
        let y = format!("{}{}", &w[..l - 1], flip(&w[l - 1..]));
        assert!(copy_ant(&y, &ant).is_none(), "{y} from {rule} must get no copy");
        // the source's plain ant reads letter l-1 for the first time at step index `at`: there the two part
        let mut x = Plain::new(rule);
        x.run(p.at as usize + 100 * p.period);
        let at = *x
            .first_at
            .get(l)
            .unwrap_or_else(|| panic!("{rule}: the plain ant never made a visit number {l}"));
        let mut b = Plain::new(&y);
        b.run(at + 1);
        for n in 0..=at {
            assert!(
                b.xs[n] == x.xs[n] && b.ys[n] == x.ys[n] && b.hs[n] == x.hs[n],
                "{y} from {rule}: step {n} before the parting at {at}"
            );
        }
        assert!(
            b.hs[at + 1] != x.hs[at + 1] && (b.xs[at + 1] != x.xs[at + 1] || b.ys[at + 1] != x.ys[at + 1]),
            "{y} from {rule}: parts at {at}"
        );
    });
    eprintln!("{tight} highways with 2 to 24 letters, {skipped} left out");
    assert!(tight > 100);
}

/// The sum of the banner on made-up footprints, against the rounds laid down one by one. In the
/// census the largest count is nearly always reached before the proof, so the look ahead rarely
/// decides. Here it does: a random footprint, a random shift, random counts already there.
#[test]
fn letters_read_on_made_up_footprints_equals_the_rounds_laid_down_one_by_one() {
    let mut rng = Rng(20261122);
    let (mut cases, mut ahead, mut capped) = (0, 0, 0);
    let k = |x: i32, y: i32| (y as i64 + 4096) * 8192 + (x as i64 + 4096);
    for run in 0..4000 {
        let (w, h) = (rng.range(1, 9) as i32, rng.range(1, 9) as i32);
        let m = rng.range(1, (w * h) as i64) as usize;
        let heavy = rng.next() < 0.15;
        let (mut sx, mut sy) = (0, 0);
        while sx == 0 && sy == 0 {
            sx = rng.range(-3, 3) as i32;
            sy = rng.range(-3, 3) as i32;
        }
        let (px, py) = (rng.range(-20, 20) as i32, rng.range(-20, 20) as i32);
        let top_here = if heavy { 200 } else { 6 };
        let mut cells: Vec<Cell> = vec![[0, 0, rng.range(1, top_here) as i32]];
        let mut seen = std::collections::HashSet::from([(0, 0)]);
        while cells.len() < m {
            let (dx, dy) = (rng.range(-w as i64, w as i64) as i32, rng.range(-h as i64, h as i64) as i32);
            if !seen.insert((dx, dy)) {
                continue;
            }
            cells.push([dx, dy, rng.range(1, top_here) as i32]);
        }
        // counts so far: round 0's own visits on its cells, and a random cloud around the start
        let mut count: FxMap<i64, i64> = FxMap::default();
        let (mut min_x, mut max_x, mut min_y, mut max_y) = (0, 0, 0, 0);
        let mut add = |count: &mut FxMap<i64, i64>, x: i32, y: i32, v: i64| {
            *count.entry(k(x, y)).or_insert(0) += v;
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        };
        for c in &cells {
            add(&mut count, px + c[0], py + c[1], c[2] as i64);
        }
        let cloud = rng.range(0, 60);
        let top = if rng.next() < 0.5 { 3 } else { rng.range(1, if heavy { 250 } else { 30 }) };
        for _ in 0..cloud {
            let (x, y, v) = (rng.range(-30, 30) as i32, rng.range(-30, 30) as i32, rng.range(1, top));
            add(&mut count, x, y, v);
        }
        let mut a = Ant::new("RL"); // side 256, off 128
        let side = a.side as i64;
        let mut max_visits = 0u32;
        for (&kk, &c) in &count {
            let (x, y) = ((kk % 8192) as i32 - 4096, (kk / 8192) as i32 - 4096);
            let v = c.min(255) as u8;
            a.vis[((y as i64 + a.off as i64) * side + (x as i64 + a.off as i64)) as usize] = v;
            max_visits = max_visits.max(v as u32);
        }
        a.max_visits = max_visits;
        (a.min_x, a.max_x, a.min_y, a.max_y) = (min_x, max_x, min_y, max_y);
        let proof = Proof {
            at: 0,
            base: 0,
            period: 1,
            shift: [sx, sy],
            onset: 0,
            path_x: vec![px],
            path_y: vec![py],
            path_h: vec![0],
            cells: cells.clone(),
            letters: Letters::Finite(0),
        };
        let got = letters_read(&a, &proof);
        // the rounds one by one, far past every overlap: 200 rounds, each cell's count kept in a map
        for j in 1..=200 {
            for c in &cells {
                *count.entry(k(px + c[0] + j * sx, py + c[1] + j * sy)).or_insert(0) += c[2] as i64;
            }
        }
        let most = count.values().copied().max().unwrap();
        let want = if max_visits >= 255 { 255 } else { most.min(255) as u32 };
        cases += 1;
        if want > max_visits {
            ahead += 1;
        }
        if want == 255 {
            capped += 1;
        }
        assert_eq!(got, Letters::Finite(want), "run {run}");
    }
    eprintln!("{cases} cases, the look ahead decides in {ahead}, 255 in {capped}");
    assert!(ahead > 1000 && capped > 20, "the test must exercise the sum: {ahead} ahead, {capped} capped");
}

#[test]
fn a_loop_reads_letters_without_end() {
    let a = run("RR", 10_000);
    assert_eq!(a.state, State::Loop);
    assert_eq!(a.proof.unwrap().letters, Letters::Infinite);
}
