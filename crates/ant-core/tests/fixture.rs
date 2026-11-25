// The main proof of the port: the Rust ant, run the same number of steps in one call, equals the
// JavaScript ant field for field, down to the hashes of the whole grid and of the visit counts.
// The fixtures were recorded from the original page (index.html, removed) with tools/js-reference.js
// (removed too; both in git history at ecadee5). They are frozen: a change to the rules that is
// meant to change what an ant does needs a new fixture made by the Rust and a reason in the commit.
// Never edit a fixture just to make a test pass.
mod common;
use ant_core::*;
use common::*;

fn rep(r: &Option<Repeat>) -> String {
    match r {
        None => "null".into(),
        Some(r) => format!("{} {:?} {} {} {:?}", r.period, r.shift, r.from, r.ends_at, r.rounds),
    }
}

fn rep_j(j: &J) -> String {
    if j.is_null() {
        return "null".into();
    }
    let s = j.get("shift").arr();
    let rounds = j.get("rounds");
    format!(
        "{} {:?} {} {} {:?}",
        j.get("period").i(),
        [s[0].i() as i32, s[1].i() as i32],
        j.get("from").i(),
        j.get("endsAt").i(),
        if rounds.is_null() { None } else { Some(rounds.i() as u64) }
    )
}

/// every field of the record, as (name, rust, js) texts that must be equal
fn fields(a: &Ant, j: &J) -> Vec<(&'static str, String, String)> {
    let n = |x: i64| x.to_string();
    let opt = |x: Option<u64>| x.map_or("null".to_string(), |v| v.to_string());
    let jopt = |j: &J| if j.is_null() { "null".to_string() } else { j.i().to_string() };
    let mut v = vec![
        ("even", even_runs(&a.rule).to_string(), j.get("even").b().to_string()),
        ("state", a.state.as_str().to_string(), j.get("state").s().to_string()),
        ("steps", n(a.steps as i64), n(j.get("steps").i())),
        ("simulated", n(a.simulated as i64), n(j.get("simulated").i())),
        ("x", n(a.x as i64), n(j.get("x").i())),
        ("y", n(a.y as i64), n(j.get("y").i())),
        ("h", n(a.h as i64), n(j.get("h").i())),
        ("minX", n(a.min_x as i64), n(j.get("minX").i())),
        ("maxX", n(a.max_x as i64), n(j.get("maxX").i())),
        ("minY", n(a.min_y as i64), n(j.get("minY").i())),
        ("maxY", n(a.max_y as i64), n(j.get("maxY").i())),
        ("side", n(a.side as i64), n(j.get("side").i())),
        ("maxVisits", n(a.max_visits as i64), n(j.get("maxVisits").i())),
        ("homeVisits", n(a.home_visits as i64), n(j.get("homeVisits").i())),
        ("lastHome", n(a.last_home as i64), n(j.get("lastHome").i())),
        ("lostAt", opt(a.lost_at), jopt(j.get("lostAt"))),
        ("nextCheck", n(a.next_check as i64), n(j.get("nextCheck").i())),
        ("nextFull", n(a.next_full as i64), n(j.get("nextFull").i())),
        ("repeat", rep(&a.repeat), rep_j(j.get("repeat"))),
        ("fake", rep(&a.fake), rep_j(j.get("fake"))),
        ("gridHash", fnv(&a.grid, FNV0).to_string(), n(j.get("gridHash").i())),
        ("visHash", fnv(&a.vis, FNV0).to_string(), n(j.get("visHash").i())),
    ];
    let pj = j.get("proof");
    match (&a.proof, pj.is_null()) {
        (None, true) => v.push(("proof", "null".into(), "null".into())),
        (Some(p), false) => {
            let s = pj.get("shift").arr();
            let letters = match p.letters {
                Letters::Infinite => "inf".to_string(),
                Letters::Finite(l) => l.to_string(),
            };
            let jl = match pj.get("letters") {
                J::Str(s) => s.clone(),
                other => other.i().to_string(),
            };
            let mine = format!(
                "{} {} {} {:?} {} {} {} {}",
                p.at,
                p.base,
                p.period,
                p.shift,
                p.onset,
                letters,
                p.cells.len(),
                proof_hash(p)
            );
            let theirs = format!(
                "{} {} {} {:?} {} {} {} {}",
                pj.get("at").i(),
                pj.get("base").i(),
                pj.get("period").i(),
                [s[0].i() as i32, s[1].i() as i32],
                pj.get("onset").i(),
                jl,
                pj.get("cells").i(),
                pj.get("hash").i()
            );
            v.push(("proof", mine, theirs));
        }
        (a, b) => v.push(("proof", format!("{}", a.is_some()), format!("js null: {b}"))),
    }
    v.push(("copyOf", format!("{:?}", a.copy_of), "None".into()));
    v
}

fn check(name: &str, steps: u64) -> usize {
    let lines = fixture(name);
    let mut bad = Vec::new();
    for j in &lines {
        let rule = j.get("rule").s();
        let mut a = Ant::new(rule);
        advance(&mut a, steps);
        for (field, mine, theirs) in fields(&a, j) {
            if mine != theirs {
                bad.push(format!("{rule} {field}: rust {mine}, js {theirs}"));
            }
        }
    }
    assert!(bad.is_empty(), "{} fields differ, first: {:#?}", bad.len(), &bad[..bad.len().min(12)]);
    lines.len()
}

#[test]
fn every_rule_of_2_to_12_letters_at_300000_steps() {
    assert_eq!(check("js-300k.jsonl.gz", 300_000), 4094);
}

#[test]
fn the_28_start_rules_at_2000000_steps() {
    assert_eq!(check("js-demo.jsonl.gz", 2_000_000), 28);
}

#[test]
fn the_fixture_has_the_census_numbers() {
    // 4,094 rules, 114 even-run, 408 highways in 300,000 steps, none of them even-run
    let lines = fixture("js-300k.jsonl.gz");
    let even: Vec<_> = lines.iter().filter(|j| j.get("even").b()).collect();
    let highways = lines.iter().filter(|j| j.get("state").s() == "highway").count();
    assert_eq!(lines.len(), 4094);
    assert_eq!(even.len(), 114);
    assert_eq!(highways, 408);
    assert!(even.iter().all(|j| j.get("state").s() != "highway"));
    // the rules are the 4,094 that start with R, in the order of the script
    assert!(lines.iter().all(|j| j.get("rule").s().starts_with('R')));
}

#[test]
fn start_rules_are_the_28_of_the_demo() {
    let demo: Vec<String> = fixture("js-demo.jsonl.gz").iter().map(|j| j.get("rule").s().to_string()).collect();
    assert_eq!(start_rules(), demo);
    assert_eq!(demo.len(), 28);
}
