// Table 1 of check.js: every start rule run to 2,000,000 steps; position_at against the plain ant;
// the proof at the first look that could see it. And copy_ant against the plain ant of the copied rule.
mod common;
use ant_core::*;
use common::*;
use std::sync::Arc;

#[test]
fn start_rules_proved_and_placed_exactly() {
    let mut rules = start_rules();
    rules.extend(["RR", "LLRR", "RRRRRLRR"].map(String::from)); // reference rows that are not panels
    let spec = |r: &str| -> Option<(u64, usize, [i32; 2])> {
        match r {
            "RL" => Some((9977, 104, [-2, 2])),
            "RRL" => Some((39, 18, [1, 1])),
            "RRRL" => Some((79, 52, [2, 2])),
            "RRRRL" => Some((103, 68, [2, 2])),
            "RRLR" => Some((256241, 384, [-4, -4])),
            "LLRRRLRLRLLR" => Some((29898, 1968, [-16, 0])),
            _ => None,
        }
    };
    let none = ["LRRRRRLLR", "RRRRRLRR", "LLRR", "RLR", "RLL", "RRLL", "RRLLLRLLLRRR"];
    const LIMIT: u64 = 2_000_000;
    let (mut proved, mut quick) = (0, 0);
    for rule in &rules {
        let mut ant = Ant::new(rule);
        advance(&mut ant, LIMIT);
        if !matches!(ant.state, State::Highway | State::Loop) {
            assert_eq!(ant.steps, if ant.state == State::Lost { ant.lost_at.unwrap() } else { LIMIT }, "{rule}");
            assert_eq!(ant.simulated, ant.steps, "{rule}");
            assert!(position_at(&ant, LIMIT).is_none(), "{rule}");
            continue;
        }
        assert!(!none.contains(&rule.as_str()), "{rule} must never be proved");
        proved += 1;
        let p = ant.proof.clone().unwrap();
        let steps = [p.at + 1, 5_000_000, p.at + 1_000_003];
        let nmax = *steps.iter().max().unwrap() as usize;
        let mut r = Plain::new(rule);
        r.run(nmax);
        let same =
            |n: u64| matches!(position_at(&ant, n), Some(q) if q.x == r.xs[n as usize] as i64 && q.y == r.ys[n as usize] as i64 && q.h == r.hs[n as usize]);
        for n in steps {
            assert!(same(n), "{rule}: position_at {n}");
        }
        let wrong = (p.base..=nmax as u64).filter(|&n| !same(n)).count();
        assert_eq!(wrong, 0, "{rule}: wrong positions");
        // onset, period and shift once more, from the plain colours alone
        let (per, on, at, base) = (p.period, p.onset as usize, p.at as usize, p.base as usize);
        assert!((on..nmax - per).all(|i| r.col[i] == r.col[i + per]), "{rule}: periodic from onset");
        assert!(on == 0 || r.col[on - 1] != r.col[on - 1 + per], "{rule}: onset is sharp");
        let smaller = (1..per).find(|&q| (at..at + 2 * per).all(|i| r.col[i] == r.col[i + q]));
        assert_eq!(smaller, None, "{rule}: a shorter period");
        assert_eq!([r.xs[at] - r.xs[base], r.ys[at] - r.ys[base]], p.shift, "{rule}: shift");
        // the book: where and when it was proved
        assert_eq!(ant.steps, LIMIT, "{rule}");
        assert_eq!(ant.simulated, p.at, "{rule}");
        assert_eq!(p.base, p.at - per as u64, "{rule}");
        if p.base > 0 {
            assert!(position_at(&ant, p.base - 1).is_none(), "{rule}");
        }
        assert_eq!(
            p.at,
            first_look_from(p.onset + 2 * per as u64, per),
            "{rule}: the first look that tries the period"
        );
        assert_eq!(ant.state == State::Loop, p.shift == [0, 0], "{rule}");
        assert!(ant.repeat.is_none(), "{rule}");
        if p.at != first_full_from(p.at) {
            quick += 1;
        }
        // the record outlives the proof, and it must be a true repeat too
        if let Some(f) = &ant.fake {
            assert!(true_repeat(&r, f), "{rule}: ant.fake");
        }
        if let Some((o, per, sh)) = spec(rule) {
            assert_eq!((p.onset, p.period, p.shift), (o, per, sh), "{rule} against the numbers of the description");
        }
        if rule == "RR" {
            assert_eq!((ant.state, p.period, p.shift), (State::Loop, 8, [0, 0]));
        }
    }
    assert_eq!(proved, 7, "RL RRL RRRL RRRRL RRLR LLRRRLRLRLLR RR are proved, the rest not");
    let _ = quick; // how many proofs came at a quick look is only for the eye
}

#[test]
fn proof_letters_of_rl_and_rrl() {
    assert_eq!(run("RRL", 2_000_000).proof.unwrap().letters, Letters::Finite(6));
    assert_eq!(run("RL", 2_000_000).proof.unwrap().letters, Letters::Finite(33));
    assert_eq!(run("RR", 1_000_000).proof.unwrap().letters, Letters::Infinite);
}

#[test]
fn copy_of_rrl_is_the_plain_ant_of_the_longer_rule() {
    let src = run("RRL", 1000);
    let p = src.proof.clone().unwrap();
    let y = "RRLRRLLRLR";
    let mut copy = copy_ant(y, &src).expect("a copy");
    assert_eq!((copy.rule.as_str(), copy.n, copy.state), (y, 10, State::Highway));
    assert!(Arc::ptr_eq(copy.proof.as_ref().unwrap(), &p), "the proof is shared");
    assert_eq!(copy.copy_of.as_deref(), Some("RRL"));
    assert_eq!((copy.simulated, copy.steps), (0, 0));
    assert!(copy.fake.is_none() && copy.repeat.is_none());
    assert!(!std::ptr::eq(copy.grid.as_ptr(), src.grid.as_ptr()), "own cells");
    assert_eq!(
        (copy.side, copy.off, copy.min_x, copy.max_x, copy.min_y, copy.max_y, copy.max_visits),
        (src.side, src.off, src.min_x, src.max_x, src.min_y, src.max_y, src.max_visits)
    );
    assert!(position_at(&copy, p.base - 1).is_none());

    // the plain ant of the copied rule: its cells at the proof step are copy.grid, and the stamps
    let (at, per, base) = (p.at as usize, p.period, p.base as usize);
    let mut a = Plain::new(y);
    a.run(at);
    assert!(same_cells(&copy, &a), "copy.grid and copy.vis at step proof.at");
    assert_eq!((copy.home_visits, copy.last_home), a.home(at), "a copy carries its source's counts");
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
        assert_eq!(diff, 0, "stamped {k} times");
    }
    // the path: position_at(copy, N) at every step from proof.base on, and the source's own path from step 0
    let end = at + 40 * per;
    a.run(end);
    let mut s = Plain::new("RRL");
    s.run(end);
    for n in base..=end {
        let q = position_at(&copy, n as u64).unwrap();
        assert!(q.x == a.xs[n] as i64 && q.y == a.ys[n] as i64 && q.h == a.hs[n], "step {n}");
    }
    for n in 0..=end {
        assert!(
            a.xs[n] == s.xs[n] && a.ys[n] == s.ys[n] && a.hs[n] == s.hs[n],
            "the copied rule walks the source's path: step {n}"
        );
    }
    // advance() only moves the copy's clock
    advance(&mut copy, 12345);
    assert_eq!((copy.steps, copy.simulated), (12345, 0));
    assert!(position_at(&copy, 12345).is_some());
}

#[test]
fn rrlrrr_gets_no_copy_and_leaves_the_path_where_the_sixth_letter_is_first_read() {
    let src = run("RRL", 1000);
    assert!(copy_ant("RRLRRR", &src).is_none());
    // the number is tight: the plain ant of the rule with another sixth letter leaves the source's
    // path at the step at which the source first reads its sixth letter
    let mut x = Plain::new("RRL");
    x.run(2000);
    let at = x.first_at[6];
    let mut b = Plain::new("RRLRRR");
    b.run(at + 1);
    for n in 0..=at {
        assert!(b.xs[n] == x.xs[n] && b.ys[n] == x.ys[n] && b.hs[n] == x.hs[n], "step {n}");
    }
    assert!(b.hs[at + 1] != x.hs[at + 1] && (b.xs[at + 1] != x.xs[at + 1] || b.ys[at + 1] != x.ys[at + 1]));
    // and no copy from an ant that is no proved highway
    let running = run("RLR", 10_000);
    assert!(copy_ant("RLR", &running).is_none());
    let lp = run("RR", 10_000);
    assert_eq!(lp.state, State::Loop);
    assert!(copy_ant("RR", &lp).is_none());
}

#[test]
fn a_copy_that_is_shorter_than_its_letters_and_the_source_itself() {
    // a rule that is the source written twice is the same endless word: it gets a copy
    let src = run("RRL", 1000);
    let c = copy_ant("RRLRRL", &src).expect("RRL written twice");
    let mut a = Plain::new("RRLRRL");
    let p = src.proof.clone().unwrap();
    a.run(p.at as usize + 3 * p.period);
    for n in p.base as usize..=a.t {
        let q = position_at(&c, n as u64).unwrap();
        assert!(q.x == a.xs[n] as i64 && q.y == a.ys[n] as i64 && q.h == a.hs[n], "step {n}");
    }
}
