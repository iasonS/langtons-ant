//! The decisions of the picture that are not drawing: what a panel says, the tooltip, the
//! half-second rule, the frame budget, the clock, the demo and which panel a typed rule becomes.
//! Plain functions over plain data (ant-core's `Ant`): no window, no egui. Everything here
//! follows the picture block of the original page (`index.html`, removed; in git history at
//! 8cb1e5c: `words`, `tipText`, `onShow`, `tick`, `frame`).

use ant_core::*;
use std::time::{Duration, Instant};

pub const MIN_TEXT_W: f64 = 128.0; // narrower: the lines do not fit at all
pub const FULL_TEXT_W: f64 = 170.0; // narrower: only with numbers cut short
pub const MIN_CELLS: f64 = 24.0; // a picture never zooms in further than this many cells across
pub const DEMO_STEPS: u64 = 60000;
pub const FAR: u64 = 1_000_000_000_000; // the far step in the tooltip
pub const DEMO_MAX: u64 = 50_000_000;
pub const SLICE: i64 = 4096; // ants take turns of SLICE steps until the frame's time for them is up
pub const SIM_SHARE: f64 = 0.6;
pub const SIM_MAX_MS: f64 = 10.0;
pub const PAINT_SHARE: f64 = 0.25;
pub const PAINT_MIN_MS: f64 = 1.5;
pub const STAMP_CELLS: usize = 200000; // highway work per panel and frame, in cells written
pub const FAKE_ROUNDS: u64 = 8;
pub const FAKE_STEPS: u64 = 512;
pub const SHOW_SECS: f64 = 0.5;
pub const SHOW_LETTERS: usize = 24;
pub const GAPS: usize = 32;
pub const PROBE: u64 = 32;

// ---------------------------------------------------------------- text helpers

/// 1234567 -> "1,234,567"
pub fn full(v: u64) -> String {
    let s = v.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

pub fn full_i(v: i64) -> String {
    if v < 0 {
        format!("-{}", full(v.unsigned_abs()))
    } else {
        full(v as u64)
    }
}

/// 1,234,567 -> 1.23M, for a line that is too long for its panel
pub fn short(v: u64) -> String {
    if v < 10000 {
        return full(v);
    }
    let mut v = v as f64;
    let mut u = -1i32; // 'kMBT'[u], as in the page
    while v >= 1000.0 && u < 3 {
        v /= 1000.0;
        u += 1;
    }
    let num = if v >= 100.0 {
        format!("{}", v.floor())
    } else if v >= 10.0 {
        format!("{:.1}", (v * 10.0).floor() / 10.0)
    } else {
        format!("{:.2}", (v * 100.0).floor() / 100.0)
    };
    format!("{}{}", num, ['k', 'M', 'B', 'T'][u as usize])
}

/// The first text that fits in `max` characters, else the last one.
pub fn fit_text(max: usize, texts: &[String]) -> String {
    texts.iter().find(|t| t.chars().count() <= max).unwrap_or(&texts[texts.len() - 1]).clone()
}

pub fn pair(s: [i64; 2]) -> String {
    format!("({}, {})", full_i(s[0]), full_i(s[1]))
}
fn pair32(s: [i32; 2]) -> String {
    pair([s[0] as i64, s[1] as i64])
}

pub fn times(v: u64) -> String {
    format!("{} {}", full(v), if v == 1 { "time" } else { "times" })
}

/// The runs of a rule read as a cycle, from a place where a run begins: RLLR -> "LL RR"
pub fn runs_of(rule: &str) -> String {
    let b = rule.as_bytes();
    let n = b.len();
    let mut k = 0;
    while k < n && b[k] == b[(k + n - 1) % n] {
        k += 1;
    }
    let rot: Vec<u8> = b[k..].iter().chain(b[..k].iter()).copied().collect();
    let mut runs: Vec<String> = vec![];
    let mut i = 0;
    while i < rot.len() {
        let mut j = i;
        while j < rot.len() && rot[j] == rot[i] {
            j += 1;
        }
        runs.push(String::from_utf8_lossy(&rot[i..j]).into_owned());
        i = j;
    }
    runs.join(" ")
}

// ---------------------------------------------------------------- panels

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tone {
    None,
    Ok,
    Rep,
    Nev,
    Off,
}

/// A repeat that is on show: decided once per repeat (see `Judge::on_show`).
#[derive(Default, Clone)]
pub struct Judge {
    judged: Option<Repeat>,
    show: bool,
}

impl Judge {
    /// The repeat a panel has on show: ant.repeat or nothing. A paused page shows whatever
    /// ant.repeat holds. A running page decides once per repeat, at the moment it first sees it: on
    /// show only if it will last SHOW_SECS at the speed set. Once on show, a repeat stays on show
    /// until it ends, whatever the slider does after that.
    pub fn on_show(&mut self, a: &Ant, rate: f64, paused: bool) -> Option<Repeat> {
        let Some(r) = a.repeat else {
            self.judged = None;
            return None;
        };
        if self.judged != Some(r) {
            self.judged = Some(r);
            self.show = r.ends_at as f64 - a.steps as f64 >= SHOW_SECS * rate;
        }
        if paused {
            self.show = true;
        }
        if self.show {
            Some(r)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Words {
    pub word: &'static str,
    pub tone: Tone,
    pub l1: String,
    pub l2: String,
    pub count: [String; 2],
}

pub struct Panel {
    pub rule: String,
    pub custom: bool,
    pub born: i64, // every ant stands at clock - born steps
    pub ant: Ant,
    pub never: bool,
    pub judge: Judge,
    pub max_chars: usize,
    pub words: Words,
    said: Option<(State, Option<Repeat>, Option<Repeat>)>,
}

impl Panel {
    pub fn new(rule: &str, custom: bool, ant: Ant) -> Panel {
        Panel {
            rule: rule.to_string(),
            custom,
            born: 0,
            ant,
            never: even_runs(rule),
            judge: Judge::default(),
            max_chars: 30,
            words: Words {
                word: "",
                tone: Tone::None,
                l1: String::new(),
                l2: String::new(),
                count: [String::new(), String::new()],
            },
            said: None,
        }
    }
    /// A new ant for this panel, at step 0. A copy becomes an ordinary ant.
    pub fn fresh(&mut self) {
        self.ant = Ant::new(&self.rule);
        self.born = 0;
        self.judge = Judge::default();
        self.said = None;
    }
    /// Does this panel say `no highway`? Known from the rule alone (a theorem), until a proof.
    pub fn is_never(&self) -> bool {
        self.never && self.ant.proof.is_none()
    }
}

/// What a panel says. `proved` only ever comes from certify() (ant.proof). Steps on screen count from 1.
pub fn words(p: &mut Panel, rate: f64, paused: bool) -> Words {
    let max = p.max_chars;
    let r = p.judge.on_show(&p.ant, rate, paused);
    let never = p.is_never();
    let a = &p.ant;
    let sim = a.simulated;
    let skip = a.steps - sim;
    let count = fit_text(
        max,
        &[
            format!("simulated {}|skipped {}", full(sim), full(skip)),
            format!("sim {}|skip {}", full(sim), full(skip)),
            format!("sim {}|skip {}", full(sim), short(skip)),
            format!("sim {}|skip {}", short(sim), short(skip)),
        ],
    );
    let (c0, c1) = count.split_once('|').unwrap();
    let count = [c0.to_string(), c1.to_string()];
    let s = |x: String| x;
    if let Some(pr) = &a.proof {
        return Words {
            word: if a.state == State::Loop { "loop" } else { "proved" },
            tone: Tone::Ok,
            l1: fit_text(
                max,
                &[
                    format!("period {} · shift {}", full(pr.period as u64), pair32(pr.shift)),
                    format!("period {} · {}", full(pr.period as u64), pair32(pr.shift)),
                    format!("period {}", full(pr.period as u64)),
                ],
            ),
            l2: match &a.copy_of {
                Some(c) => fit_text(max, &[format!("same ant as {c}"), format!("same as {c}")]),
                None => fit_text(
                    max,
                    &[
                        format!("proved at step {}", full(pr.at)),
                        format!("proved at {}", full(pr.at)),
                        format!("proved at {}", short(pr.at)),
                    ],
                ),
            },
            count,
        };
    }
    let left = |a: &Ant| {
        let at = a.lost_at.unwrap_or(0);
        fit_text(
            max,
            &[
                format!("left the map at step {}", full(at)),
                format!("left at step {}", full(at)),
                format!("left at {}", full(at)),
                format!("left at {}", short(at)),
            ],
        )
    };
    if never {
        let back = a.home_visits - 1;
        return Words {
            word: "no highway",
            tone: Tone::Nev,
            l1: fit_text(
                max,
                &[
                    s("never a highway (theorem)".into()),
                    s("no highway (theorem)".into()),
                    s("by a theorem".into()),
                ],
            ),
            l2: if a.state == State::Lost {
                left(a)
            } else if let Some(r) = r {
                let e = r.ends_at + 1;
                fit_text(
                    max,
                    &[
                        format!("period {} repeat · ends at step {}", full(r.period as u64), full(e)),
                        format!("repeat ends at step {}", full(e)),
                        format!("repeat ends at {}", full(e)),
                        format!("repeat ends at {}", short(e)),
                    ],
                )
            } else {
                fit_text(
                    max,
                    &[
                        format!("back at its start {}", times(back)),
                        format!("back at start {}", times(back)),
                        format!("back {}", times(back)),
                        format!("back {} times", short(back)),
                    ],
                )
            },
            count,
        };
    }
    // what a detector that counts repeats would have fallen for; only once it is over
    let fake = match a.fake {
        Some(f) if a.steps > f.ends_at && f.rounds.unwrap_or(0) >= FAKE_ROUNDS && f.ends_at - f.from >= FAKE_STEPS => {
            let n = full(f.rounds.unwrap_or(0));
            let per = full(f.period as u64);
            fit_text(
                max,
                &[
                    format!("fake highway: {n} × period {per}"),
                    format!("fake: {n} × period {per}"),
                    format!("{n} × period {per}"),
                ],
            )
        }
        _ => String::new(),
    };
    if a.state == State::Lost {
        return Words {
            word: "off the map",
            tone: Tone::Off,
            l1: fake,
            l2: left(a),
            count,
        };
    }
    if let Some(r) = r {
        let e = r.ends_at + 1;
        return Words {
            word: "repeat",
            tone: Tone::Rep,
            l1: format!("period {} repeat", full(r.period as u64)),
            l2: fit_text(
                max,
                &[
                    format!("ends at step {}", full(e)),
                    format!("ends at {}", full(e)),
                    format!("ends at {}", short(e)),
                ],
            ),
            count,
        };
    }
    // "not proved", never "no repeat": the ant may be in a repeat that the detector has not looked at yet
    Words {
        word: "not proved",
        tone: Tone::None,
        l1: fake,
        l2: fit_text(
            max,
            &[
                format!("not proved in {} steps", full(a.steps)),
                format!("not proved in {}", full(a.steps)),
                format!("not proved in {}", short(a.steps)),
            ],
        ),
        count,
    }
}

pub fn tip_text(p: &mut Panel, rate: f64, paused: bool) -> String {
    let r = p.judge.on_show(&p.ant, rate, paused);
    let never = p.is_never();
    let a = &p.ant;
    let mut out = vec![format!("rule {} · {} colours", a.rule, a.n)];
    if never {
        let back = a.home_visits - 1;
        out.push(format!(
            "even runs ({}): a 1995 theorem says this ant comes back to its start again and again for ever,",
            runs_of(&a.rule)
        ));
        out.push(format!(
            "so it never leaves on a highway. {}",
            if back > 0 {
                format!("Seen back {}, last at step {}.", times(back), full(a.last_home))
            } else {
                "Seen back 0 times so far.".to_string()
            }
        ));
    }
    if let Some(pr) = &a.proof {
        let now = position_at(a, a.steps);
        let far = position_at(a, FAR);
        let cell = |p: Option<Position>| p.map(|p| pair([p.x, p.y])).unwrap_or_default();
        let src = a.copy_of.as_deref();
        out.push(match src {
            Some(c) => format!("same ant as {c} · proved there at step {}", full(pr.at)),
            None => format!("proved at step {}", full(pr.at)),
        });
        out.push(format!(
            "{}repeats from step {} · period {} · shift {}",
            src.map(|c| format!("{c} ")).unwrap_or_default(),
            full(pr.onset + 1),
            full(pr.period as u64),
            pair32(pr.shift)
        ));
        out.push(format!("at step {}: cell {}", full(a.steps), cell(now)));
        if far.is_some() {
            out.push(format!("at step {}: cell {}", full(FAR), cell(far)));
        }
        // No lines for a loop (Infinite) or when the number is not exact (255).
        if let Letters::Finite(l) = pr.letters {
            if l < 255 {
                let l = l as usize;
                out.push(format!("no cell is ever visited more than {} times:", full(l as u64)));
                out.push(if l <= SHOW_LETTERS {
                    format!("any rule that starts {} is this same ant", &a.rule.repeat(l.div_ceil(a.n))[..l])
                } else {
                    format!(
                        "any rule that, written again and again, starts {}… ({} letters) is this same ant",
                        &a.rule.repeat(SHOW_LETTERS)[..SHOW_LETTERS],
                        full(l as u64)
                    )
                });
            }
        }
    } else {
        if a.state == State::Lost {
            out.push(format!(
                "left the map at step {} · the map is {} cells wide",
                full(a.lost_at.unwrap_or(0)),
                full(a.side as u64)
            ));
        } else if let Some(r) = r {
            out.push(format!(
                "period {} repeat · shift {} · ends at step {}",
                full(r.period as u64),
                pair32(r.shift),
                full(r.ends_at + 1)
            ));
        } else {
            out.push(format!("not proved in {} steps", full(a.steps)));
        }
        out.push(format!("at step {}: cell {}", full(a.simulated), pair([a.x as i64, a.y as i64])));
    }
    if let Some(f) = a.fake {
        out.push(format!(
            "longest refused repeat the page noted: {} steps ({} rounds of period {}), steps {} to {}",
            full(f.ends_at - f.from),
            full(f.rounds.unwrap_or(0)),
            full(f.period as u64),
            full(f.from + 1),
            full(f.ends_at)
        ));
    }
    out.push(format!("simulated {} · skipped {}", full(a.simulated), full(a.steps - a.simulated)));
    out.join("\n")
}

/// Which panel a typed rule becomes: a copy of the first proved start panel that `copy_ant`
/// accepts (proved at once, no step simulated), else an ordinary ant. A copy starts at its
/// source's proof step, so it is born `proof.at` steps before now; any other is born now.
pub fn typed_panel(rule: &str, start: &[Panel], clock: f64) -> Panel {
    let mut ant = None;
    for q in start {
        ant = copy_ant(rule, &q.ant);
        if ant.is_some() {
            break;
        }
    }
    let ant = match ant {
        Some(mut a) => {
            let at = a.proof.as_ref().unwrap().at;
            advance(&mut a, at);
            a
        }
        None => Ant::new(rule),
    };
    let mut p = Panel::new(rule, true, ant);
    p.born = clock.floor() as i64 - p.ant.steps as i64;
    p
}

// ---------------------------------------------------------------- the frame clock and budget

/// frameMs is the frame interval of this screen: the shortest of the last 32 intervals between
/// frames (a frame that ran late lasts several of the screen's intervals; an average would be too
/// long). One interval is kept between 4 and 50 ms. Every PROBE-th frame works half as long.
pub struct FrameClock {
    gaps: [f64; GAPS],
    pub last: f64, // seconds; 0 before the first frame
    pub frames: u64,
    pub frame_ms: f64,
}

pub struct FrameStart {
    pub now: f64,
    pub dt: f64,
    pub part: f64,
    pub first: bool,
}

impl FrameClock {
    pub fn new() -> FrameClock {
        FrameClock {
            gaps: [1000.0 / 60.0; GAPS],
            last: 0.0,
            frames: 0,
            frame_ms: 1000.0 / 60.0,
        }
    }
    pub fn begin(&mut self, now: f64) -> FrameStart {
        let dt = {
            let d = now - self.last;
            if self.last == 0.0 || d == 0.0 {
                0.016
            } else {
                d.min(0.05)
            }
        };
        let first = self.last <= 0.0;
        if !first {
            self.gaps[(self.frames as usize) % GAPS] = ((now - self.last) * 1000.0).clamp(4.0, 50.0);
            self.frame_ms = self.gaps.iter().cloned().fold(f64::INFINITY, f64::min);
        }
        self.last = now;
        self.frames += 1;
        FrameStart {
            now,
            dt,
            part: if self.frames.is_multiple_of(PROBE) { 0.5 } else { 1.0 },
            first,
        }
    }
}

impl Default for FrameClock {
    fn default() -> Self {
        Self::new()
    }
}

/// ms for simulating: 60 % of the frame interval, 10 ms at most; a frame that writes the words
/// simulates that much shorter.
pub fn sim_ms(frame_ms: f64, part: f64, hud_due: bool, hud_ms: f64) -> f64 {
    part * SIM_MAX_MS.min(SIM_SHARE * frame_ms) - if hud_due { hud_ms } else { 0.0 }
}
/// ms for drawing the cells: 25 % of the frame interval, 1.5 ms at least.
pub fn paint_ms(frame_ms: f64, part: f64) -> f64 {
    PAINT_MIN_MS.max(part * PAINT_SHARE * frame_ms)
}

// ---------------------------------------------------------------- the model

pub fn rate_of(v: f64) -> f64 {
    20.0 * 1e5f64.powf(v / 1000.0) // 20 to 2,000,000 steps per second
}

pub struct Model {
    pub panels: Vec<Panel>,
    pub generation: u64, // bumped whenever the set of panels or their ants change under the drawing
    pub clock: f64,
    pub rate: f64,
    pub paused: bool,
    pub shown_rate: f64,
    pub measured: bool,
    rate_t: f64,
    rate_clock: f64,
    tick_from: usize,
    pub fc: FrameClock,
    pub hud_t: f64,
    pub hud_ms: f64,
}

impl Model {
    pub fn new(slider: f64) -> Model {
        let mut m = Model {
            panels: vec![],
            generation: 0,
            clock: 0.0,
            rate: rate_of(slider),
            paused: false,
            shown_rate: 0.0,
            measured: false,
            rate_t: 0.0,
            rate_clock: 0.0,
            tick_from: 0,
            fc: FrameClock::new(),
            hud_t: -1.0,
            hud_ms: 0.0,
        };
        m.set_rules(None);
        m
    }

    /// The start rules, with the typed rule (if any) in front.
    pub fn set_rules(&mut self, custom: Option<&str>) {
        let mut start: Vec<Panel> = std::mem::take(&mut self.panels).into_iter().filter(|p| !p.custom).collect();
        if start.is_empty() {
            for rule in start_rules() {
                let ant = Ant::new(&rule);
                start.push(Panel::new(&rule, false, ant));
            }
        }
        if let Some(rule) = custom {
            let p = typed_panel(rule, &start, self.clock);
            start.insert(0, p);
        }
        self.panels = start;
        self.generation += 1;
    }

    /// Demo (`--demo`): every ant at `steps`, paused; the rule comes last, when the start ants are
    /// where they will be shown (it may be a copy of one that is proved by now).
    pub fn demo(&mut self, steps: u64, rule: Option<&str>) {
        let steps = steps.min(DEMO_MAX);
        self.clock = steps as f64;
        for p in &mut self.panels {
            advance(&mut p.ant, steps);
        }
        if let Some(rule) = rule {
            self.set_rules(Some(rule));
            let p = &mut self.panels[0];
            p.born = 0;
            let k = steps.saturating_sub(p.ant.steps);
            advance(&mut p.ant, k);
        }
        self.set_paused(true);
    }

    pub fn restart(&mut self) {
        for p in &mut self.panels {
            p.fresh();
        }
        self.clock = 0.0;
        self.generation += 1;
        self.set_paused(false);
    }

    pub fn set_paused(&mut self, on: bool) {
        self.paused = on;
        self.rate_t = self.fc.last;
        self.rate_clock = self.clock;
        self.measured = false;
    }

    /// The clock: every ant should stand at clock - born steps. Running ants take turns of SLICE
    /// steps until they are there or the frame's `ms` for them are up (the time is read after every
    /// turn). Then the clock only goes as far as the slowest got. Proved ants follow for free.
    pub fn tick(&mut self, dt: f64, ms: f64) {
        let want = self.clock + self.rate * dt;
        let goal = want.floor() as i64;
        let until = Instant::now() + Duration::from_secs_f64(ms.max(0.0) / 1000.0);
        let count = self.panels.len();
        let mut idle = 0;
        while idle < count {
            if self.tick_from >= count {
                self.tick_from = 0;
            }
            let p = &mut self.panels[self.tick_from];
            self.tick_from += 1;
            let need = if p.ant.state == State::Running {
                goal - p.born - p.ant.steps as i64
            } else {
                0
            };
            if need <= 0 {
                idle += 1;
                continue;
            }
            idle = 0;
            advance(&mut p.ant, need.min(SLICE) as u64);
            if Instant::now() >= until {
                break;
            }
        }
        let mut reached = goal;
        for p in &self.panels {
            if p.ant.state == State::Running {
                reached = reached.min(p.ant.steps as i64 + p.born);
            }
        }
        self.clock = if reached < goal { self.clock.max(reached as f64) } else { want };
        let c = self.clock.floor() as i64;
        for p in &mut self.panels {
            let k = c - p.born - p.ant.steps as i64;
            if p.ant.proof.is_some() && k > 0 {
                advance(&mut p.ant, k as u64); // the skip
            }
        }
    }

    fn say(&mut self, i: usize) {
        let (rate, paused) = (self.rate, self.paused);
        let w = words(&mut self.panels[i], rate, paused);
        let p = &mut self.panels[i];
        p.words = w;
        p.said = Some((p.ant.state, p.judge.on_show(&p.ant, rate, paused), p.ant.fake));
    }

    fn stale(&mut self, i: usize) -> bool {
        let (rate, paused) = (self.rate, self.paused);
        let p = &mut self.panels[i];
        let now = (p.ant.state, p.judge.on_show(&p.ant, rate, paused), p.ant.fake);
        p.said != Some(now)
    }

    /// Write the words of every panel again.
    pub fn say_all(&mut self) {
        for i in 0..self.panels.len() {
            self.say(i);
        }
    }

    /// One frame of the page's `frame()` apart from drawing. Returns (hud_due). The words are
    /// written again in the frame in which a panel's state, repeat on show or fake changes, and
    /// otherwise ten times a second.
    pub fn frame(&mut self, now: f64) -> bool {
        let f = self.fc.begin(now);
        let hud_due = f.now - self.hud_t > 0.1;
        if f.first {
            self.rate_t = f.now;
            self.rate_clock = self.clock;
        }
        if !self.paused {
            let ms = sim_ms(self.fc.frame_ms, f.part, hud_due, self.hud_ms);
            self.tick(f.dt, ms);
            for i in 0..self.panels.len() {
                if self.stale(i) {
                    self.say(i);
                }
            }
            if f.now - self.rate_t >= 0.25 {
                self.shown_rate = ((self.clock - self.rate_clock) / (f.now - self.rate_t)).max(0.0);
                self.rate_t = f.now;
                self.rate_clock = self.clock;
                self.measured = true;
            }
        }
        if hud_due {
            self.hud_t = f.now;
            let t0 = Instant::now();
            self.say_all();
            self.hud_ms = (t0.elapsed().as_secs_f64() * 1000.0).min(SIM_MAX_MS);
        }
        hud_due
    }

    pub fn totals(&self) -> Totals {
        let mut t = Totals {
            proved: 0,
            of: self.panels.len(),
            never: 0,
            simulated: 0,
            skipped: 0,
        };
        for p in &self.panels {
            if p.ant.proof.is_some() {
                t.proved += 1;
            }
            if p.is_never() {
                t.never += 1;
            }
            t.simulated += p.ant.simulated;
            t.skipped += p.ant.steps - p.ant.simulated;
        }
        t
    }

    /// The speed label: `set to N steps/s` while paused or not measured yet, else the speed reached.
    pub fn rate_label(&self) -> (String, bool) {
        let set = self.paused || !self.measured;
        let r = if set { self.rate } else { self.shown_rate };
        (format!("{}{} steps/s", if set { "set to " } else { "" }, full(to_precision3(r) as u64)), set)
    }
}

pub struct Totals {
    pub proved: usize,
    pub of: usize,
    pub never: usize,
    pub simulated: u64,
    pub skipped: u64,
}

fn to_precision3(r: f64) -> f64 {
    if r <= 0.0 {
        return 0.0;
    }
    let mag = r.log10().floor() as i32;
    let q = 10f64.powi(mag - 2);
    (r / q).round() * q
}

// ---------------------------------------------------------------- the grid of panels

/// How many columns? The grid that gives a panel the largest picture, as long as its lines still fit.
pub fn choose_grid(count: usize, width: f64, height: f64) -> (usize, usize) {
    const TOP_H: f64 = 52.0;
    const UI_H: f64 = 56.0;
    const GAP: f64 = 8.0;
    const HEAD: f64 = 20.0;
    const FOOT: f64 = 42.0;
    let gw = width - 24.0;
    let gh = height - TOP_H - UI_H - 14.0;
    let mut best: Option<(usize, usize, f64)> = None;
    for cols in 1..=count {
        let rows = count.div_ceil(cols);
        let w = (gw - GAP * (cols as f64 - 1.0)) / cols as f64 - 18.0;
        let h = (gh - GAP * (rows as f64 - 1.0)) / rows as f64 - 13.0 - HEAD - FOOT;
        let score = (if w >= MIN_TEXT_W { 1e6 } else { 0.0 }) + w.min(h * 1.5) * (if w >= FULL_TEXT_W { 1.0 } else { 0.85 }) + w * h * 1e-6;
        if best.is_none_or(|b| score > b.2) {
            best = Some((cols, rows, score));
        }
    }
    let b = best.unwrap();
    (b.0, b.1)
}

/// Cell colours (OKLCH): colour 0 is not drawn. Two colours: one ink colour. More: one violet ramp.
pub fn cell_colours(n: usize) -> Vec<[u8; 3]> {
    const INK: [u8; 3] = [214, 211, 201];
    let k = n - 1;
    if k == 1 {
        return vec![INK];
    }
    let kf = k as f64;
    let hi = (0.94 + 0.006 * kf).min(0.985);
    let lo = (hi - 0.12 * kf).max(0.405);
    (0..k)
        .map(|i| {
            let l = lo + (hi - lo) * i as f64 / (kf - 1.0);
            oklch(l, ((1.0 - l) * 0.4).min(0.12), 290.0)
        })
        .collect()
}

fn oklch(l: f64, c: f64, hue: f64) -> [u8; 3] {
    let h = hue.to_radians();
    let (a, b) = (c * h.cos(), c * h.sin());
    let l_ = (l + 0.3963377774 * a + 0.2158037573 * b).powi(3);
    let m_ = (l - 0.1055613458 * a - 0.0638541728 * b).powi(3);
    let s_ = (l - 0.0894841775 * a - 1.2914855480 * b).powi(3);
    let lin = [
        4.0767416621 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_,
        -1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_,
        -0.0041960863 * l_ - 0.7034186147 * m_ + 1.7076147010 * s_,
    ];
    let enc = |v: f64| {
        let e = if v <= 0.0031308 {
            12.92 * v.max(0.0)
        } else {
            1.055 * v.min(1.0).powf(1.0 / 2.4) - 0.055
        };
        (255.0 * e).round() as u8
    };
    [enc(lin[0]), enc(lin[1]), enc(lin[2])]
}

// ---------------------------------------------------------------- command line

#[derive(Default, Debug, PartialEq)]
pub struct Args {
    pub demo: Option<(u64, Option<String>)>,
    pub shot: Option<String>,
    pub selftest: Option<f64>,
}

/// `--demo [N[,RULE]]`, `--shot FILE.png`, `--selftest-seconds N`.
pub fn parse_args(args: &[String]) -> Args {
    let mut out = Args::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--demo" => {
                let mut steps = DEMO_STEPS;
                let mut rule = None;
                if let Some(v) = args.get(i + 1).filter(|v| !v.starts_with("--")) {
                    i += 1;
                    let (n, r) = match v.split_once(',') {
                        Some((n, r)) => (n, Some(r)),
                        None => (v.as_str(), None),
                    };
                    if let Ok(n) = n.parse::<u64>() {
                        steps = n;
                    }
                    rule = r.and_then(valid_rule);
                }
                out.demo = Some((steps, rule));
            }
            "--shot" => {
                i += 1;
                out.shot = args.get(i).cloned();
            }
            "--selftest-seconds" => {
                i += 1;
                out.selftest = args.get(i).and_then(|v| v.parse().ok());
            }
            _ => {}
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rep(ends_at: u64) -> Repeat {
        Repeat {
            period: 24,
            shift: [0, 0],
            from: 0,
            ends_at,
            rounds: None,
        }
    }
    fn panel(rule: &str) -> Panel {
        Panel::new(rule, false, Ant::new(rule))
    }
    fn at(rule: &str, steps: u64) -> Panel {
        let mut p = panel(rule);
        advance(&mut p.ant, steps);
        p
    }

    #[test]
    fn half_second_rule_is_decided_once_per_repeat() {
        let mut a = Ant::new("RLR");
        a.steps = 1000;
        let mut j = Judge::default();
        // 400 steps left at 1,000 steps/s: under half a second, not on show
        a.repeat = Some(rep(1400));
        assert_eq!(j.on_show(&a, 1000.0, false), None);
        // a slower slider afterwards changes nothing: it was decided
        assert_eq!(j.on_show(&a, 10.0, false), None);
        // a new repeat lasting 600 steps: on show
        a.repeat = Some(rep(1600));
        assert!(j.on_show(&a, 1000.0, false).is_some());
        // a repeat on show stays until it ends, whatever the speed does
        assert!(j.on_show(&a, 1e6, false).is_some());
        a.steps = 1590;
        assert!(j.on_show(&a, 1e6, false).is_some());
        // it ended
        a.repeat = None;
        assert_eq!(j.on_show(&a, 1000.0, false), None);
    }

    #[test]
    fn paused_shows_whatever_ant_repeat_holds() {
        let mut a = Ant::new("RLR");
        a.steps = 1000;
        a.repeat = Some(rep(1010));
        let mut j = Judge::default();
        assert_eq!(j.on_show(&a, 1e6, false), None);
        assert!(j.on_show(&a, 1e6, true).is_some());
        let mut j = Judge::default();
        assert!(j.on_show(&a, 1e6, true).is_some());
    }

    #[test]
    fn words_of_the_states() {
        let mut p = at("RLLR", 0);
        let w = words(&mut p, 5000.0, false);
        assert_eq!((w.word, w.tone), ("no highway", Tone::Nev)); // an even-run panel from step 0
        assert_eq!(w.l1, "never a highway (theorem)");
        assert_eq!(w.l2, "back at its start 0 times");
        assert_eq!(w.count, ["simulated 0", "skipped 0"]);

        let mut p = at("RLR", 0);
        let w = words(&mut p, 5000.0, false);
        assert_eq!(
            (w.word, w.tone, w.l1.as_str(), w.l2.as_str()),
            ("not proved", Tone::None, "", "not proved in 0 steps")
        );

        let mut p = at("RRL", 1000);
        let w = words(&mut p, 5000.0, false);
        assert_eq!((w.word, w.tone), ("proved", Tone::Ok));
        assert_eq!(w.l1, "period 18 · shift (1, 1)");
        assert_eq!(w.l2, "proved at step 128");
        assert_eq!(w.count, ["simulated 128", "skipped 872"]);
    }

    #[test]
    fn trap_panel_at_398000() {
        let mut p = at("LRRRRRLLR", 398_000);
        let w = words(&mut p, 5000.0, true); // paused, as in the demo
        assert_eq!((w.word, w.tone), ("repeat", Tone::Rep));
        assert_eq!(w.l1, "period 116 repeat");
        assert_eq!(w.l2, "ends at step 400,704");
    }

    #[test]
    fn typed_rules() {
        let start: Vec<Panel> = vec![at("RRL", 1000)];
        let p = typed_panel("LLRR", &start, 0.0);
        assert!(p.custom && p.is_never());
        let mut p = p;
        assert_eq!(words(&mut p, 5000.0, false).word, "no highway");
        let p = typed_panel("RLR", &start, 0.0);
        assert!(!p.is_never());
        // starts like RRL (6 letters read): a copy, proved at once, nothing simulated
        let mut p = typed_panel("RRLRRLLRLR", &start, 0.0);
        assert_eq!(p.ant.copy_of.as_deref(), Some("RRL"));
        assert_eq!(p.born, -128);
        let w = words(&mut p, 5000.0, false);
        assert_eq!((w.word, w.l2.as_str()), ("proved", "same ant as RRL"));
        assert_eq!(w.count[0], "simulated 0");
        // another sixth letter: an ordinary panel that simulates
        let p = typed_panel("RRLRRR", &start, 0.0);
        assert!(p.ant.copy_of.is_none() && p.ant.proof.is_none());
        assert_eq!(p.born, 0);
    }

    #[test]
    fn frame_ms_is_the_shortest_of_32() {
        let mut c = FrameClock::new();
        let mut t = 1.0;
        c.begin(t);
        // 40 intervals of 16 ms with one of 8 ms among them, and some late ones of 32 ms
        for k in 0..40 {
            t += if k == 10 {
                0.008
            } else if k % 5 == 0 {
                0.032
            } else {
                0.016
            };
            c.begin(t);
        }
        assert!((c.frame_ms - 8.0).abs() < 1e-6);
        // 32 more intervals of 16 ms: the 8 ms one has dropped out of the last 32
        for _ in 0..32 {
            t += 0.016;
            c.begin(t);
        }
        assert!((c.frame_ms - 16.0).abs() < 1e-6);
        // kept between 4 and 50 ms
        let mut c = FrameClock::new();
        c.begin(1.0);
        for k in 1..=40 {
            c.begin(1.0 + k as f64 * 0.001);
        }
        assert!((c.frame_ms - 4.0).abs() < 1e-6);
        // the budget: 60 % of it, 10 ms at most; drawing 25 %, 1.5 ms at least
        assert!((sim_ms(16.0, 1.0, false, 0.0) - 9.6).abs() < 1e-9);
        assert!((sim_ms(33.0, 1.0, false, 0.0) - 10.0).abs() < 1e-9);
        assert!((sim_ms(16.0, 0.5, true, 1.0) - 3.8).abs() < 1e-9);
        assert!((paint_ms(4.0, 1.0) - 1.5).abs() < 1e-9);
        assert!((paint_ms(16.0, 1.0) - 4.0).abs() < 1e-9);
        // every 32nd frame works half as long
        let mut c = FrameClock::new();
        let parts: Vec<f64> = (0..64).map(|k| c.begin(1.0 + k as f64 * 0.016).part).collect();
        assert_eq!(parts.iter().filter(|&&p| p == 0.5).count(), 2);
        assert_eq!(parts[31], 0.5);
    }

    #[test]
    fn fake_highway_shows_at_512_steps_and_8_rounds() {
        let fake = |from: u64, ends_at: u64, rounds: u64| Repeat {
            period: 64,
            shift: [0, 0],
            from,
            ends_at,
            rounds: Some(rounds),
        };
        let line = |f: Repeat, steps: u64| {
            let mut p = panel("RLR");
            p.ant.steps = steps;
            p.ant.fake = Some(f);
            words(&mut p, 5000.0, false).l1
        };
        assert_eq!(line(fake(0, 512, 8), 1000), "fake highway: 8 × period 64");
        assert_eq!(line(fake(0, 511, 8), 1000), ""); // 1 step short
        assert_eq!(line(fake(0, 512, 7), 1000), ""); // 1 round short
        assert_eq!(line(fake(100, 611, 9), 1000), ""); // covers 511
        assert_eq!(line(fake(100, 612, 9), 1000), "fake highway: 9 × period 64");
        assert_eq!(line(fake(0, 512, 8), 512), ""); // only once it is over
        assert_eq!(line(fake(0, 512, 8), 513), "fake highway: 8 × period 64");
    }

    #[test]
    fn narrow_panels_shorten_their_lines() {
        let mut p = at("RLR", 1_234_567);
        p.max_chars = 22;
        let w = words(&mut p, 5000.0, false);
        assert_eq!(w.l2, "not proved in 1.23M");
        assert_eq!(short(1_234_567), "1.23M");
        assert_eq!(full(1_234_567), "1,234,567");
    }

    #[test]
    fn the_clock_moves_and_proved_ants_follow() {
        let mut m = Model::new(480.0);
        m.rate = 100_000.0;
        m.tick(0.05, 8.0);
        assert!(m.clock > 0.0);
        let c = m.clock.floor() as u64;
        for p in &m.panels {
            assert!(p.ant.steps >= c, "{} stands at {} of {}", p.rule, p.ant.steps, c);
        }
    }

    #[test]
    fn demo_and_args() {
        let a = parse_args(&["--demo".into(), "60000,RRLRRLLRLR".into(), "--shot".into(), "x.png".into()]);
        assert_eq!(a.demo, Some((60000, Some("RRLRRLLRLR".into()))));
        assert_eq!(a.shot.as_deref(), Some("x.png"));
        assert_eq!(parse_args(&["--demo".into()]).demo, Some((60000, None)));
        assert_eq!(parse_args(&["--demo".into(), "398000".into()]).demo, Some((398000, None)));
        let mut m = Model::new(480.0);
        m.demo(60000, Some("RRLRRLLRLR"));
        assert!(m.paused && m.panels.len() == 29);
        assert!(m.panels.iter().all(|p| p.ant.steps == 60000));
        assert_eq!(m.panels[0].ant.simulated, 0);
        assert_eq!(choose_grid(28, 1600.0, 900.0), (7, 4));
    }
}
