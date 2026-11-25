// check.js "extra" (tableEdge) and the home-visit part of tableNever: advance() in pieces,
// vis/maxVisits/homeVisits/lastHome against the plain ant through a growth of the grid,
// proof.cells stamped, positions far out, an ant that leaves the largest grid.
mod common;
use ant_core::*;
use common::*;

/// the plain ant's colours at its current step, as a map
fn colours(a: &Plain) -> FxMap<i64, u8> {
    a.cells.iter().map(|(&k, &(_, c))| (k, c)).collect()
}

fn same_ant(a: &Ant, b: &Ant, what: &str) {
    assert_eq!(brief(a), brief(b), "{what}: fields");
    assert!(a.grid == b.grid && a.vis == b.vis, "{what}: grid or visit counts");
    assert!(
        a.col[..] == b.col[..] && a.px[..] == b.px[..] && a.py[..] == b.py[..] && a.hd[..] == b.hd[..],
        "{what}: ring"
    );
}

/// chunk sizes for pattern `kind`; the sum is cut to the steps that are left
fn chunks(kind: u32, seed: &mut u64, steps: u64) -> u64 {
    *seed = (seed.wrapping_mul(1103515245).wrapping_add(12345)) % 2147483648;
    match kind {
        0 => {
            if steps < 3000 {
                1
            } else {
                1 + *seed % 2999
            }
        }
        1 => 4096, // a frame of the page
        2 => 1 + *seed % 50_000,
        3 => 1u64 << (*seed % 14), // 1 to 8192
        _ => 1 + *seed % 7,
    }
}

#[test]
fn advance_in_pieces_equals_advance_in_one_call_and_the_plain_ant() {
    let rules = ["RL", "RR", "RRLR", "RLR", "LRRRRRLLR", "RRLLLRLLLRRR", "RRLLLRLLLRRRRLRRLLRLRRLL"];
    const N: u64 = 400_000;
    for rule in rules {
        let one = run(rule, N);
        for kind in 0..5 {
            let mut many = Ant::new(rule);
            let mut seed = 12345 + kind as u64;
            while many.steps < N {
                let k = chunks(kind, &mut seed, many.steps).min(N - many.steps);
                advance(&mut many, k);
            }
            same_ant(&one, &many, &format!("{rule} pattern {kind}"));
        }
        // against the plain ant: cells, visit counts, box, place and heading
        let mut a = Plain::new(rule);
        a.run(one.simulated as usize);
        let s = one.simulated as usize;
        assert!(same_cells(&one, &a), "{rule}: cells and visits");
        assert_eq!(one.max_visits, a.maxv.min(255), "{rule}: max_visits");
        assert_eq!((one.x, one.y, one.h), (a.xs[s], a.ys[s], a.hs[s]), "{rule}");
        let bx = |f: fn(&i32, &i32) -> i32, v: &[i32]| v.iter().copied().fold(0, |m, x| f(&m, &x));
        assert_eq!(
            (one.min_x, one.max_x, one.min_y, one.max_y),
            (
                bx(|a, b| *a.min(b), &a.xs[..=s]),
                bx(|a, b| *a.max(b), &a.xs[..=s]),
                bx(|a, b| *a.min(b), &a.ys[..=s]),
                bx(|a, b| *a.max(b), &a.ys[..=s])
            ),
            "{rule}: box"
        );
        assert_eq!(one.steps, N);
    }
}

#[test]
fn home_visits_and_last_home_equal_the_plain_ant_through_a_growth_of_the_grid() {
    let rules = ["RLLR", "RRLL", "LLRR", "RLR", "RL", "RRL", "LRRRRRLLR", "RRLRR", "LLRRRLRLRLLR", "RRLLLRLLLRRR"];
    let marks: [u64; 17] = [0, 1, 2, 3, 4, 5, 8, 100, 1000, 9999, 60000, 100000, 300001, 777777, 1000000, 1500000, 2000000];
    let (mut grew, mut seed) = (0, 99u64);
    for rule in rules {
        let mut a = Ant::new(rule);
        let mut p = Plain::new(rule);
        let mut side_was = a.side;
        let mut grew_here = false;
        for n in marks {
            // in pieces of many sizes, so that a visit falls on the last step of a piece and on the first of the next
            while a.steps < n {
                seed = (seed * 1103515245 + 12345) % 2147483648;
                let k = if a.steps < 200 { 1 } else { 1 + seed % 50_000 };
                let k = k.min(n - a.steps);
                advance(&mut a, k);
            }
            grew_here |= a.side != side_was;
            side_was = a.side;
            p.run(a.simulated as usize); // a proved ant is no longer simulated: its counts stand at the proof step
            assert_eq!((a.home_visits, a.last_home), p.home(a.simulated as usize), "{rule} at {n}");
        }
        grew += grew_here as usize;
    }
    assert!(grew >= 1, "the grid grew for at least one rule");
}

#[test]
fn proof_cells_stamped_give_the_real_grid_later() {
    for rule in ["RL", "RRL", "RRLR", "LLRRRLRLRLLR", "RR", "RRR"] {
        let ant = run(rule, 2_000_000);
        let p = ant.proof.clone().unwrap_or_else(|| panic!("{rule} not proved"));
        let (at, per) = (p.at as usize, p.period);
        for k in [1usize, 2, 5] {
            let mut a = Plain::new(rule);
            a.run(at);
            let mut want = colours(&a);
            for m in 1..=k as i32 {
                for c in &p.cells {
                    let kk = key(p.path_x[0] + c[0] + m * p.shift[0], p.path_y[0] + c[1] + m * p.shift[1]);
                    let e = want.entry(kk).or_insert(0);
                    *e = ((*e as i32 + c[2]) % ant.n as i32) as u8;
                }
            }
            a.run(at + k * per);
            let real = colours(&a);
            let diff = real.iter().filter(|(k, &c)| want.get(k).copied().unwrap_or(0) != c).count()
                + want.iter().filter(|(k, &c)| real.get(k).copied().unwrap_or(0) != c).count();
            assert_eq!(diff, 0, "{rule} k={k}");
        }
    }
}

#[test]
fn position_at_far_out_is_exact_whole_number_arithmetic() {
    for rule in ["RL", "RRLR", "LLRRRLRLRLLR"] {
        let ant = run(rule, 2_000_000);
        let p = ant.proof.clone().unwrap();
        for n in [1_000_000_000_000u64, (1u64 << 53) - 1] {
            let q = position_at(&ant, n).unwrap();
            let big = (n - p.base) as i128;
            let (m, r) = (big / p.period as i128, (big % p.period as i128) as usize);
            assert_eq!(q.x as i128, p.path_x[r] as i128 + m * p.shift[0] as i128, "{rule} {n}");
            assert_eq!(q.y as i128, p.path_y[r] as i128 + m * p.shift[1] as i128, "{rule} {n}");
            assert_eq!(q.h, p.path_h[r]);
        }
        let b = position_at(&ant, p.base).unwrap();
        assert_eq!((b.x, b.y), (p.path_x[0] as i64, p.path_y[0] as i64));
        assert!(position_at(&ant, p.base - 1).is_none());
        assert_eq!(ant.steps - ant.simulated, 2_000_000 - p.at, "skipped = steps - simulated");
    }
}

/// An ant that leaves the largest grid (100,000,000 steps in the check; it is lost far sooner).
#[test]
fn an_ant_that_leaves_the_largest_grid_is_lost_and_stays_so() {
    let rule = "RRLLLRLLLRRR";
    let mut ant = run(rule, 100_000_000);
    assert_eq!(ant.state, State::Lost, "the ant must leave the map");
    let lost_at = ant.lost_at.unwrap();
    let (gx, gy) = (ant.x + ant.off, ant.y + ant.off);
    let side = ant.side as i32;
    let edge = gx.min(gy).min(side - 1 - gx).min(side - 1 - gy);
    let before = (ant.steps, ant.simulated, ant.x, ant.y, ant.h);
    advance(&mut ant, 12345);
    assert_eq!((ant.steps, ant.simulated, ant.x, ant.y, ant.h), before, "nothing moves after that");
    assert_eq!((lost_at, ant.simulated, ant.side, edge), (ant.steps, lost_at, 4096, 2));
    assert!(ant.proof.is_none() && ant.repeat.is_none() && position_at(&ant, lost_at).is_none());
    let mut p = Plain::new(rule);
    p.run(lost_at as usize);
    assert!(same_cells(&ant, &p), "the ant, grid and visit counts equal the plain ant at that step");
    let s = lost_at as usize;
    assert_eq!((ant.x, ant.y, ant.h), (p.xs[s], p.ys[s], p.hs[s]));
    // every cell's colour is its visit count mod n where the count is below 255
    for i in 0..ant.grid.len() {
        let w = ant.vis[i];
        if w < 255 {
            assert_eq!(ant.grid[i] as usize, w as usize % ant.n);
        }
    }
    assert_eq!(ant.vis.iter().copied().max().unwrap() as u32, ant.max_visits);
}
