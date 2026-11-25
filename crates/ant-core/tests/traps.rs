// Table 2 of check.js: the two trap rules. They are never proved, every "ends at" is the true end
// in the plain ant, and ant.repeat / ant.fake at every look are what the banner says, worked out
// from the plain ant alone.
mod common;
use ant_core::*;
use common::*;

#[derive(Default)]
struct Stats {
    false_proofs: Vec<(u64, usize)>,
    wrong: Vec<(u64, usize, u64, Option<usize>)>,
    claims: usize,
    not_repeat: Vec<String>,
    not_fake: Vec<String>,
    bad_reports: Vec<String>,
    audits: usize,
    candidates: usize,
    model_repeat: Option<Repeat>,
    model_fake: Option<Repeat>,
}

/// One look of the page at step T that tried periods up to max_p. certify() is asked about every
/// candidate and compared with the plain ant. The model is what the banner defines for ant.repeat
/// and ant.fake, worked out from the plain ant alone: the ant must agree with it.
fn audit(ant: &mut Ant, r: &Plain, t: u64, max_p: usize, st: &mut Stats) {
    assert_eq!(ant.state, State::Running, "ant stopped running at {t}");
    let tt = t as usize;
    let mut best: Option<Repeat> = None; // of the candidates refused with an end step: the one that ends last, the shorter period on a tie
    for p in 1..=max_p.min(tt / 2) {
        if r.col[tt - 1] != r.col[tt - 1 - p] || !is_candidate(&r.col, tt, p) {
            continue;
        }
        st.candidates += 1;
        let e = true_end(r, tt, p);
        let (sx, sy) = (r.xs[tt] - r.xs[tt - p], r.ys[tt] - r.ys[tt - p]);
        if r.hs[tt] == r.hs[tt - p] && (sx != 0 || sy != 0) && best.is_none_or(|b| e.unwrap() as u64 > b.ends_at) {
            best = Some(Repeat {
                period: p,
                shift: [sx, sy],
                from: 0,
                ends_at: e.unwrap() as u64,
                rounds: None,
            });
        }
        match certify(ant, p) {
            Certify::Proved { .. } => st.false_proofs.push((t, p)),
            Certify::No => {}
            Certify::Refused { shift, ends_at } => {
                st.claims += 1;
                if Some(ends_at as usize) != e || shift != [sx, sy] {
                    st.wrong.push((t, p, ends_at, e));
                }
            }
        }
    }
    // The model. A repeat on show stays until the step it ends; a look replaces it only by the
    // look's best refused candidate, if that one ends later and still has max(8 periods, 256) steps to run.
    // The record is the noted refused repeat that covers the most steps (ends_at - from);
    // only a strictly larger span replaces it. rounds stays in the record.
    if matches!(st.model_repeat, Some(m) if t > m.ends_at) {
        st.model_repeat = None;
    }
    if let Some(b) = best {
        let from = ref_from(r, tt, b.period) as u64;
        let rounds = (b.ends_at - from) / b.period as u64;
        if b.ends_at - t >= (8 * b.period).max(256) as u64 && st.model_repeat.is_none_or(|m| b.ends_at > m.ends_at) {
            st.model_repeat = Some(Repeat { from, ..b });
        }
        if st.model_fake.is_none_or(|m| b.ends_at - from > m.ends_at - m.from) {
            st.model_fake = Some(Repeat {
                from,
                rounds: Some(rounds),
                ..b
            });
        }
    }
    st.audits += 1;
    if ant.repeat != st.model_repeat {
        st.not_repeat.push(format!("{t}: want {:?} got {:?}", st.model_repeat, ant.repeat));
    }
    if ant.fake != st.model_fake {
        st.not_fake.push(format!("{t}: want {:?} got {:?}", st.model_fake, ant.fake));
    }
    if let Some(x) = &ant.fake {
        if !true_repeat(r, x) {
            st.bad_reports.push(format!("{t}: fake {x:?}"));
        }
    }
    if let Some(x) = &ant.repeat {
        if t > x.ends_at || !true_repeat(r, x) {
            st.bad_reports.push(format!("{t}: repeat {x:?}"));
        }
    }
}

fn trap(rule: &str, n: u64, period: usize, shift: [i32; 2], start: usize, end: usize) {
    let mut r = Plain::new(rule);
    r.run(n as usize + 70_000);
    // the repeat named in the description, measured in the plain colours
    let (p, s) = (period, ref_from(&r, start + 2 * period, period));
    let e = true_end(&r, s + p, p).unwrap();
    let sh = [r.xs[s + p] - r.xs[s], r.ys[s + p] - r.ys[s]];
    let named = Repeat {
        period: p,
        shift: sh,
        from: s as u64,
        ends_at: e as u64,
        rounds: None,
    };
    assert_eq!((s, e, sh), (start, end, shift));
    assert!(true_repeat(&r, &named));

    // (a) the page's own timetable, to n steps
    let mut ant = Ant::new(rule);
    let mut st = Stats::default();
    let table = timetable(n);
    let mut named_at = Vec::new();
    for &(t, max_p) in &table {
        assert_eq!(ant.next_check, t, "looks on the timetable");
        assert_eq!(ant.next_full == t, max_p == FULL_P);
        let k = t - ant.steps;
        advance(&mut ant, k);
        if ant.state != State::Running {
            break;
        }
        audit(&mut ant, &r, t, max_p, &mut st);
        if ant.repeat == Some(named) {
            named_at.push(t);
        }
    }
    let k = n - ant.steps;
    advance(&mut ant, k);
    assert_eq!(st.audits, table.len());
    assert_eq!((ant.state, ant.proof.is_none()), (State::Running, true), "never proved");
    assert!(
        st.false_proofs.is_empty(),
        "certify() said ok on {} of {} candidates: {:?}",
        st.false_proofs.len(),
        st.candidates,
        &st.false_proofs[..st.false_proofs.len().min(3)]
    );
    assert!(
        st.wrong.is_empty(),
        "certify().ends_at not the true end (T, p, said, true): {:?}",
        &st.wrong[..st.wrong.len().min(3)]
    );
    assert!(st.claims > 0);
    assert!(st.bad_reports.is_empty(), "{:?}", &st.bad_reports[..st.bad_reports.len().min(3)]);
    assert!(
        st.not_repeat.is_empty(),
        "ant.repeat is not what the banner says: {:?}",
        &st.not_repeat[..st.not_repeat.len().min(3)]
    );
    assert!(
        st.not_fake.is_empty(),
        "ant.fake is not what the banner says: {:?}",
        &st.not_fake[..st.not_fake.len().min(3)]
    );

    // the named repeat: on show at every look from the first that can see it until the step it ends
    let expected: Vec<u64> = table
        .iter()
        .filter(|&&(t, mp)| t >= (s + 2 * p) as u64 && t <= e as u64 && mp >= p)
        .map(|l| l.0)
        .collect();
    assert!(!expected.is_empty());
    assert_eq!(named_at, expected, "ant.repeat is {named:?} at all those looks and at no other");
    assert!(expected[0] <= (s + 2 * p) as u64 + QUICK_EVERY);
    // and between the looks: one step at a time, with nothing forced
    let mut walk = Ant::new(rule);
    advance(&mut walk, s as u64);
    let (mut said_from, mut said_to, mut gaps) = (None, None::<u64>, 0);
    while walk.steps < e as u64 + 400 {
        advance(&mut walk, 1);
        if walk.repeat != Some(named) {
            continue;
        }
        if said_from.is_none() {
            said_from = Some(walk.steps);
        } else if said_to != Some(walk.steps - 1) {
            gaps += 1;
        }
        said_to = Some(walk.steps);
    }
    assert_eq!(
        (said_from, said_to, gaps),
        (Some(expected[0]), Some(e as u64), 0),
        "one step at a time; one step later it is gone"
    );

    // the grid, the ring and the box at step n, against the plain ant
    let mut at_n = Plain::new(rule);
    at_n.run(n as usize);
    assert!(same_cells(&ant, &at_n), "grid and visit counts at step {n}");
    let nn = n as usize;
    for i in nn - HIST..nn {
        let j = i % HIST;
        assert!(
            ant.col[j] == r.col[i] && ant.px[j] == r.xs[i] && ant.py[j] == r.ys[i] && ant.hd[j] == r.hs[i],
            "ring at {i}"
        );
    }
    let (mut bx0, mut bx1, mut by0, mut by1) = (0, 0, 0, 0);
    for i in 0..=nn {
        bx0 = bx0.min(r.xs[i]);
        bx1 = bx1.max(r.xs[i]);
        by0 = by0.min(r.ys[i]);
        by1 = by1.max(r.ys[i]);
    }
    assert_eq!((ant.min_x, ant.max_x, ant.min_y, ant.max_y), (bx0, bx1, by0, by1));
    assert_eq!((ant.x, ant.y, ant.h), (r.xs[nn], r.ys[nn], r.hs[nn]));
    assert!(ant.fake.is_some_and(|f| true_repeat(&r, &f)));

    // (b) a full look forced at EVERY step around the named repeat
    let (w0, w1) = (s as u64 - 3000, e as u64 + 3000);
    let mut dense = Ant::new(rule);
    advance(&mut dense, w0);
    let mut sd = Stats {
        model_repeat: dense.repeat,
        model_fake: dense.fake,
        ..Default::default()
    };
    let mut dense_named = Vec::new();
    while dense.steps < w1 && dense.state == State::Running {
        dense.next_check = dense.steps + 1;
        dense.next_full = dense.steps + 1;
        advance(&mut dense, 1);
        let t = dense.steps;
        audit(&mut dense, &r, t, FULL_P, &mut sd);
        if dense.repeat == Some(named) {
            dense_named.push(t);
        }
    }
    assert!(
        sd.false_proofs.is_empty() && sd.wrong.is_empty() && sd.bad_reports.is_empty() && sd.not_repeat.is_empty() && sd.not_fake.is_empty(),
        "{:?} {:?}",
        sd.not_repeat.first(),
        sd.not_fake.first()
    );
    let want: Vec<u64> = ((s + 2 * p) as u64..=e as u64).collect();
    assert_eq!(dense_named, want, "the named repeat on show at every step from its second round to its end");
}

#[test]
fn lrrrrrllr_is_never_proved_and_its_repeat_is_exact() {
    // {period 116, from 396128, endsAt 400703}, on show from the look at step 396,544
    trap("LRRRRRLLR", 6_000_000, 116, [-2, 2], 396_128, 400_703);
}

#[test]
fn rrrrrlrr_is_never_proved_and_its_repeat_is_exact() {
    // {period 48, from 457881, endsAt 460208}, on show from the look at step 457,984
    trap("RRRRRLRR", 3_000_000, 48, [-4, 0], 457_881, 460_208);
}

#[test]
fn the_named_repeats_are_on_show_from_the_stated_looks() {
    for (rule, p, from, ends_at, first_look) in [
        ("LRRRRRLLR", 116usize, 396_128u64, 400_703u64, 396_544u64),
        ("RRRRRLRR", 48, 457_881, 460_208, 457_984),
    ] {
        let mut a = Ant::new(rule);
        advance(&mut a, first_look - 1);
        assert!(a.repeat.is_none(), "{rule}: not before the look");
        advance(&mut a, 1);
        let r = a.repeat.unwrap_or_else(|| panic!("{rule}: on show at step {first_look}"));
        assert_eq!((r.period, r.from, r.ends_at), (p, from, ends_at));
        let k = ends_at - a.steps;
        advance(&mut a, k);
        assert_eq!(a.repeat.map(|r| r.ends_at), Some(ends_at), "{rule}: still there at its last step");
        advance(&mut a, 1);
        assert!(a.repeat.is_none(), "{rule}: gone one step later");
    }
}
