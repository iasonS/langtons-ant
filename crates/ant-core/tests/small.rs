// The small functions: valid_rule, even_runs, same_start, start_rules, and the refusals of
// certify() and position_at(). From check.js: tableEdge part 4, tableNever part 1, tableSame part 4.
mod common;
use ant_core::*;
use common::*;

#[test]
fn valid_rule_takes_2_to_24_letters_of_l_and_r_in_any_case() {
    let v = valid_rule;
    assert_eq!(v("rl").as_deref(), Some("RL"));
    assert_eq!(v(" rlR ").as_deref(), Some("RLR"));
    assert_eq!(v("R"), None);
    assert_eq!(v(""), None);
    assert_eq!(v("RLX"), None);
    assert_eq!(v(&"R".repeat(24)).as_deref(), Some("RRRRRRRRRRRRRRRRRRRRRRRR"));
    assert_eq!(v(&("RL".repeat(12) + "R")), None);
}

#[test]
fn position_at_and_certify_refuse_what_they_should() {
    let a = run("RLR", 1000);
    assert!(position_at(&a, 500).is_none());
    let mut a = a;
    for p in [0, 1001, 8193] {
        assert_eq!(certify(&mut a, p), Certify::No, "period {p}");
    }
}

fn even_runs_slow(rule: &str) -> bool {
    // a second way to read the runs: for every letter, walk left and right around the cycle to the
    // ends of its run and count
    let r = rule.as_bytes();
    let n = r.len();
    if !rule.contains('L') || !rule.contains('R') {
        return false;
    }
    for i in 0..n {
        let mut len = 1;
        let mut k = 1;
        while k < n && r[(i + k) % n] == r[i] {
            len += 1;
            k += 1;
        }
        k = 1;
        while k < n && r[(i + n * 2 - k) % n] == r[i] {
            len += 1;
            k += 1;
        }
        if len % 2 == 1 {
            return false;
        }
    }
    true
}

#[test]
fn even_runs_on_every_rule_of_1_to_14_letters() {
    let (mut all, mut even_all) = (0, 0);
    for n in 1..=14 {
        for rule in all_rules(n) {
            let e = even_runs(&rule);
            all += 1;
            even_all += e as usize;
            assert_eq!(e, even_runs_slow(&rule), "second reading: {rule}");
            assert_eq!(e, even_runs(&flip(&rule)), "mirror: {rule}");
            assert_eq!(e, even_runs(&format!("{}{}", &rule[1..], &rule[..1])), "turned by one letter: {rule}");
            if n % 2 == 1 {
                assert!(!e, "odd length: {rule}");
            }
            if rule.chars().all(|c| c == rule.as_bytes()[0] as char) {
                assert!(!e, "one letter only: {rule}");
            }
        }
    }
    assert_eq!(all, (1 << 15) - 2);
    assert!(even_all > 0);
}

#[test]
fn even_runs_on_made_up_cases() {
    let yes = [
        "LLRR",
        "RRLL",
        "RLLR",
        "LRRL",
        "LLRRRR",
        "LRRLLRRRRL",
        "RRLLRRLL",
        "RLLLLR",
        "RRRLLLLR",
        "RRRLLR",
        "LLLRRRRLLLLLLRRRRRRLLLLL",
        "RRRRRRRRRRRRRRRRRRRRRRLL",
    ];
    let no = [
        "RLLLR",
        "RRLLR",
        "LRRLL",
        "RLLRL",
        "RL",
        "LR",
        "RLR",
        "RRL",
        "RRRLLL",
        "RLRL",
        "RRLLRL",
        "LRRRLL",
        "RRRLLRL",
        "LRRRLLLR",
        "RR",
        "LL",
        "RRRR",
        "LLLLLL",
        "RRRRRRRRRRRRRRRRRRRRRRRR",
        "RRRRRRRRRRRRRRRRRRRRRRRL",
    ];
    for r in yes {
        assert!(even_runs(r), "{r}");
    }
    for r in no {
        assert!(!even_runs(r), "{r}");
    }
    let evens: Vec<String> = start_rules().into_iter().filter(|r| even_runs(r)).collect();
    assert_eq!(evens, ["RLLR", "RRLL"]);
}

#[test]
fn same_start_equals_the_words_written_out() {
    let mut rules = Vec::new();
    for len in 1..=7 {
        rules.extend(all_rules(len));
    }
    for a in &rules {
        for b in &rules {
            let equal = a.repeat(b.len()) == b.repeat(a.len());
            assert_eq!(same_start(a, b, u64::MAX), equal, "{a} {b} infinity");
            for k in 0..=30u64 {
                assert_eq!(same_start(a, b, k), word(a, k as usize) == word(b, k as usize), "{a} {b} {k}");
            }
        }
    }
    let cases: [(&str, &str, u64, bool); 17] = [
        ("RRL", "RRLRRLLRLR", 6, true),
        ("RRL", "RRLRRLLRLR", 7, false),
        ("RRLRRLLRLR", "RRL", 6, true),
        ("RRL", "RRLR", 5, true),
        ("RRL", "RRLR", 6, false),
        ("RL", "RLR", 3, true),
        ("RL", "RLR", 4, false),
        ("RL", "RLRLRL", 1000, true),
        ("RL", "RLRLRL", u64::MAX, true),
        ("RL", "RLRLRR", u64::MAX, false),
        ("RL", "RLRLRR", 5, true),
        ("RRL", "RRL", u64::MAX, true),
        ("RL", "LR", 0, true),
        ("RL", "LR", 1, false),
        ("LLRRRLRLRLLR", "LLRRRLRLRLLRLLRRRLRLRLLR", 66, true),
        ("LLRRRLRLRLLR", "LLRRRLRLRLLRLLRRRLRLRLLL", 23, true),
        ("LLRRRLRLRLLR", "LLRRRLRLRLLRLLRRRLRLRLLL", 24, false),
    ];
    for (a, b, k, want) in cases {
        assert_eq!(same_start(a, b, k), want, "{a} {b} {k}");
        assert_eq!(same_start(b, a, k), want, "{b} {a} {k}");
    }
}

#[test]
fn start_rules_match_the_list_written_again() {
    // check.js panelRules(): SPEC section 7, written again
    let mut out = vec!["RL".to_string()];
    for len in [3usize, 4, 5] {
        for r in all_rules(len) {
            if !r.starts_with('R') || !r.contains('L') {
                continue;
            }
            let rep = (1..len).any(|d| len % d == 0 && r == r[..d].repeat(len / d));
            if !rep {
                out.push(r);
            }
        }
    }
    out.extend(["LRRRRRLLR", "LLRRRLRLRLLR", "RRLLLRLLLRRR"].map(String::from));
    assert_eq!(start_rules(), out);
    assert_eq!(out.len(), 28);
}
