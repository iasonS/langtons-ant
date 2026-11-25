// Steps per second of advance() on RLR: 20,000,000 steps, best of 5 (the check of the page asks
// for 65 million per second in node).
// Run: cargo run --release -p ant-core --example speed
use ant_core::*;
use std::time::Instant;

fn best(detector: bool) -> (f64, u64) {
    const N: u64 = 20_000_000;
    let mut best = f64::INFINITY;
    let mut done = 0;
    for _ in 0..5 {
        let mut a = Ant::new("RLR");
        if !detector {
            a.next_check = u64::MAX;
        }
        let t0 = Instant::now();
        advance(&mut a, N);
        let s = t0.elapsed().as_secs_f64();
        assert!(a.state == State::Running && a.simulated == N, "RLR is unproved");
        done = a.simulated;
        best = best.min(s);
    }
    (done as f64 / best / 1e6, done)
}

fn main() {
    let (rate, done) = best(true);
    println!("advance() on RLR, {done} steps, best of 5: {rate:.1} M steps/s");
    let (rate, _) = best(false);
    println!("advance() with the detector switched off: {rate:.1} M steps/s");
}
