# CLAUDE.md: langtons-ant

Contents: [Rust](#rust) · [What it is](#what-it-is) · [Rules](#rules) · [Checking](#checking) · [Git](#git)

## Rust

A Cargo workspace beside the page: `Cargo.toml` at the root, one crate, `crates/ant-core`, no dependencies (`flate2` in the tests only, to read the fixtures).
`ant-core` is a one-to-one port of the rules block of `index.html` (same names in snake_case, same constants, timetables, order of operations and tie-breaks, same comments). It must decide the same as the page, step for step. `index.html` stays the page for now and stays the source of the rules: change the rules there first, then the port.

- Run the checks: `cargo test --release` (about 20 s). Speed: `cargo run --release -p ant-core --example speed`. Before a commit: `cargo fmt`, `cargo clippy --all-targets`.
- The main proof is `tests/fixture.rs`: every rule of 2 to 12 letters that starts with R at 300,000 steps, and the 28 start rules at 2,000,000, equal the JavaScript field for field, with hashes of the whole grid and visit counts. If a field differs, the Rust is wrong: fix the port, never the fixture.
- The other tests port the rule checks of the list below (`small`, `jump`, `traps`, `edge`, `same`) against a plain ant with a hash map grid (`tests/common`).
- Regenerate the fixtures after any change to the rules block: `node tools/js-reference.js` (about 20 s). It cuts the block out of `index.html` and writes `crates/ant-core/tests/data/*.jsonl.gz`.
- `simulate()` checks once per call that the steps cannot reach the edge, then indexes the grid without bounds checks. Keep that check.

## What it is

One self-contained HTML file, `index.html`: no build, no libraries, no network requests. Open it
directly (`file://` works). It is the only source; nothing builds it.

Ant atlas: 28 ant rules side by side, one ant per panel. An ant walks on a grid of coloured cells; its rule (a string of L and R, one letter per colour) says which way it turns on each colour. Every panel simulates its ant and runs a detector on it. A panel says `proved` only when `certify()` has checked that the ant repeats for ever; it then shows the period, the shift and the proof step, the ant is no longer simulated, and `positionAt()` places it by arithmetic. A repeat that the check refused (`ant.repeat`) shows as `repeat` with the step at which it ends. A running page puts a repeat on show only if it will last half a second at the speed set, at the moment the picture first sees it (`endsAt - ant.steps >= 0.5 * rate`); it decides once per repeat, and a repeat on show stays until it ends. A paused page (and `#demo`) shows whatever `ant.repeat` holds. This is a rule of the picture: the rules block knows nothing of it. The rest say `not proved in N steps` (never "no repeat": the ant may be in a repeat the detector has not looked at, or in one that is not on show). An unproved panel with no repeat on show also shows `fake highway: N × period P`, when `ant.fake` covers at least 512 steps and has at least 8 rounds. `ant.fake`: each look notes one refused repeat (the one that ends latest), and `ant.fake` is the noted one that covers the most steps (`endsAt - from`). It is not the ant's record: a short repeat can begin and end between two looks, unseen. A panel whose rule is even-run says `no highway` from the first frame, in its own tone, with the lines `never a highway (theorem)` and `back at its start N times`. Even-run (`evenRuns()`): read as a cycle, the rule is made of runs of L and runs of R that all have even length (`LLRR`, `RLLR`, `LLRRRR`; `RLLR` and `RRLL` among the 28). A 1995 theorem ("Further travels with my ant") says that such an ant, started on the blank grid, comes back to its start cell again and again for ever, so it never builds a highway. The word comes from the rule alone, never from `certify()`, and the ant is simulated like any other. N is `ant.homeVisits - 1`, counted by the simulation (`ant.lastHome` is the step count of the last time; both are in the tooltip). A repeat on show takes the second line while it runs (`period P repeat · ends at step N`), and the end of the map takes it for good; the word stays `no highway`. The `fake highway` line is not on such a panel, only in its tooltip. `never a highway` is said ONLY for even-run rules and always with the word `theorem` near it. A proof would come first (none is known: the theorem rules out a highway). The top strip counts these panels (`never a highway (theorem) 2`). Counters per panel and in the top strip: steps simulated, steps skipped. Controls: a rule box (2 to 24 letters, becomes the first panel), speed, pause, restart. A typed rule that starts like a proved ant is that same ant: `copyAnt()` gives its panel a copy of the first proved panel that accepts it, and the panel says `proved` and `same ant as RRL` with `simulated 0`. `same ant as` is said only for a copy, never from comparing pictures. Why it holds: a cell visited v times makes the ant read letter v of its rule written again and again (counting from 0); `ant.vis` counts the visits, and at a proof `proof.letters` is the most visits any cell gets over the whole endless run (6 for `RRL`, 33 for `RL`; `Infinity` for a loop; 255 means 255 or more, and then nothing is copied). A rule whose first `proof.letters` letters are the same makes the same moves for ever. The tooltip of a proved panel states this with its number (more than 24 letters: `any rule that, written again and again, starts …`). On a copy the tooltip's `repeats from step` line names the source (`RRL repeats from step 40`): it is `proof.onset` of the shared proof, the step from which the colours the source read repeat. The copy's turns repeat from that step too, but its own colours can start to repeat at another step (`RRLRRLLRLR`: 56, which is what the panel says once it has proved itself after restart). `proof.onset` is not changed for a copy. Every ant stands at `clock - born` steps. A copy starts at its source's proof step (`positionAt()` knows no place before the proof), so it is born `proof.at` steps before the click. After restart a copy is an ordinary ant. Hover a proved panel for the ant's position at step 1,000,000,000,000. The tooltip of any panel whose `ant.fake` is set also gives that repeat: steps covered, rounds, period, first and last step (below the two limits too; a proved panel keeps the one from before its proof). The speed label says `set to N steps/s` while paused and right after load, play or restart, until the first measurement; then the speed really reached.

## Rules

- The page answers one question you can see. The numbers on screen come from the running sim, never typed in by hand.
- Few controls: rule box, speed, pause, restart. No scores, no win or lose.
- Rules and picture stay apart: the rules block is plain functions over plain data with no drawing and no page access.
- Check the rules with numbers before trusting the picture: cut the rules block out of the file and run it in `node`.
- Time-based, not frame-based (steps per second times frame time), so 60 Hz and 165 Hz behave the same.
- Colours that tell things apart (the cell ramp and the three status tones: proved `#69a621`, repeat `#d861aa`, no highway `#259cde`, all pairs) are checked with the dataviz skill's `validate_palette.js` against `#0a0b0d` before use. The ant ramp passes for rules of up to 11 letters. Longer rules FAIL the script's step check (steps closer than 0.06 in lightness), and that includes two of the 28 start panels (12 letters, step 0.057). This is an open exception, not a pass.
- Dark background `#0a0b0d`, text `#e8e6df`, muted `#8b8a84`.
- Nothing in this repo names an email or another account.

## Checking

`#demo` on the URL jumps to a fixed, seeded state for a screenshot, because a hidden browser
does not run animation frames: `#demo` (every ant at 60,000 steps, paused), `#demo=398000` (steps),
`#demo=60000,RLLRR` (also: that rule as the first panel), `#demo=60000,RRLRRLLRLR` (a rule that
starts like a proved ant: the first panel must say `proved`, `same ant as RRL` and `simulated 0`;
this is `shots/ant-same.png`). In all three ant shots the `RLLR` and `RRLL` panels must say `no highway`
and `never a highway (theorem)`, and the top strip `never a highway (theorem) 2`. In a demo the rule is added after the start ants have run, and
every panel shows the same step, a copy too. The step count stops at 50,000,000,
because demo mode runs every step before the first picture. Pausing never makes the detector
look: the picture never calls it. A paused picture shows every repeat that `ant.repeat` holds;
the running page leaves out those that are over in less than half a second (see What it is).
A panel's words are written again in the frame in which its ant's state, the repeat on show
or `ant.fake` changes, and otherwise ten times a second and at a pause click: the counters can
be about 0.1 s behind the ant, the status word never. The detector looks on two timetables: a quick look every 256 steps for periods up to
128, and a full look for periods up to 8192 that gets rarer as the ant gets older (64 steps
apart at first, 8192 at most). In `shots/ant-trap.png` (`#demo=398000`) the `LRRRRRLLR` panel must
say `repeat` and `ends at step 400,704`. Live, from `#demo=395000` at the slider's first setting
(about 5,000 steps per second), that panel must say `repeat` from the look at step 396,544 until
that end (read every 100 ms: `repeat` in all 8 readings from step 397,009 to 400,610). At the top
of the slider its status word must change rarely (measured twice over 5 s: no change; before the
half-second rule about 20 changes per second). `index.html` is the only source; nothing builds it.

```
timeout 90 brave --headless=new --disable-gpu --no-first-run --user-data-dir="$PWD/.shot-profile" \
  --hide-scrollbars --window-size=1600,900 --virtual-time-budget=3000 \
  --screenshot=shots/NAME.png "file://$PWD/index.html#demo"
```

- The first launch on a fresh profile folder can hang: run it under `timeout`, then retry.
- Never run two browsers on the same profile folder.
- In zsh, do not keep the command in a variable (`$B ...`): zsh does not split it into words.
- The hidden browser resizes once before the shot, so demo mode must keep its resize handler.
- A screenshot does not prove the live loop. To check that, start the same browser with
  `--remote-debugging-port`, load the page without `#demo`, and over the DevTools protocol read `clock` and, for each entry of `panels`, `ant.simulated`, `ant.steps` and
  `mark`: `clock` must move, a proved ant's `simulated` must stand still while its `steps`
  follow the clock, and `mark` (the cell the marker is on) must equal `positionAt(ant, mark.steps)`.
  For a copy: wait until `RRL` is proved, type `RRLRRLLRLR` into the rule box and press add. The
  first panel must say `proved` and `same ant as RRL` at once, its `ant.simulated` must stay 0,
  its `ant.steps` must start at `proof.at` (128) and follow the clock, and `mark` must equal
  `positionAt()`. `RRLRRR` (another sixth letter) must give an ordinary panel that simulates.
  For the theorem panels: `RLLR` and `RRLL` must say `no highway` before the first frame has run
  (hook `requestAnimationFrame` before the page loads), and their `back at its start` count must
  grow. Typing `LLRR` must give such a panel at once, at step 0; typing `RLR` must not.
  The top strip stacks its four totals below 1,560 px, and the sentence beside it loses its middle
  part below 1,000 px and its last sentence below 800 px. Measured at 15 widths from 1,600 to
  700 px with every ant at 2,000,000 steps: no line of the sentence is cut and the totals stay
  inside the strip.
  `frameMs` is the screen's frame interval: the shortest of the last 32 intervals between frames
  (a frame that ran late lasts a whole number of them, so an average would be too long).
  Simulating gets 60 % of it per frame (10 ms at most), drawing the cells 25 % (1.5 ms at least).
  The time is read after every ant's turn of 4,096 steps. A frame that writes the words simulates
  that much shorter. Every 32nd frame works half as long: if every frame ran late, that one shows
  the screen's real interval. The clock runs on real time, not on `frameMs`.
  To check that a fast screen stays smooth, call `frame(ts)` by hand with frames tied to a
  refresh (the next `ts` is the first refresh after the work is done). A plain timer cannot show
  a frame that misses its refresh.
- The rules block of `index.html` is everything from the `THE RULES` banner to the `THE PICTURE`
  banner. Cut it out and run it in `node`; it must run with no page. What must hold:
  on the trap rules `LRRRRRLLR` and `RRRRRLRR` `certify()` returns ok at no step, and every
  `endsAt` it reports is the true end of that repeat in a separate plain simulation;
  for every proved rule `positionAt()` equals a separate plain simulation at later steps;
  `advance()` in one call and in many small calls gives the same ant;
  a proof comes at the first look that tries its period, at or after the repeat's first step plus 2 periods;
  on its own timetable, `ant.repeat` of `LRRRRRLLR` is `{period 116, from 396128, endsAt 400703}` at
  every look from step 396,544 until the step the repeat ends, and is gone one step later (the same
  for the period-48 repeat of `RRRRRLRR`, `{from 457881, endsAt 460208}`, from step 457,984);
  on those two rules `ant.fake` is a true repeat in the separate simulation (it starts at `from`,
  not one step earlier, and breaks exactly at `endsAt`), and no repeat there with a period up to
  128 and at least 512 steps covers more steps (every repeat that long has a quick look inside it);
  `ant.vis` and `ant.maxVisits` equal the visit counts of a separate plain ant (`ant.vis` has the
  shape of `ant.grid`, so an ant takes twice the cells in memory);
  for every highway the page proves among the 4,094 census rules (408 in 300,000 steps),
  `proof.letters` equals the largest visit count of a separate plain ant run on for 60 rounds or
  more past the proof (255 where that is 255 or more), and equals it again when worked out at an
  earlier step of the repeat; on made-up footprints it equals the rounds laid down one by one
  (among the census highways the largest count is always there before the proof, so the made-up
  footprints are the ONLY check of the sum over the rounds to come in `lettersRead()`: that check
  stays a required one, and it must also find at least 1,000 footprints in which that sum decides);
  for a rule with the same first `proof.letters` letters `copyAnt()` gives a copy: the plain ant
  of that rule stands at `positionAt(copy, N)` with the same heading at every step from
  `proof.base` on, its cells at step `proof.at` are `copy.grid`, and `proof.cells` stamped 1, 2 and
  5 times (mod the copy's colours) gives its cells 1, 2 and 5 periods later;
  the number is tight: the rule with another letter at place `proof.letters` gets no copy, and its
  plain ant leaves the source's path at the step at which the source first reads that letter;
  `sameStart()` equals a comparison of the words written out;
  `certify()` and the proof steps, periods, shifts and onsets are as before the visit counts;
  `evenRuns()` equals the column `f_cyc_even` of the census table on all 4,094 rules (114 are
  even-run) and a second reading of the runs on every rule of 1 to 14 letters, and gives the same
  answer for the mirror rule and for the rule turned by one letter;
  the page proves no even-run rule a highway in the census (300,000 steps);
  `ant.homeVisits` and `ant.lastHome` equal the count of a separate plain ant at many step counts,
  through a growth of the grid (one index compare per step in the hot loop; the index of the start
  cell changes when the grid grows), and a copy carries its source's two numbers;
  the text of `certify()`, `detect()`, `setProof()`, `lettersRead()` and `firstStep()` and what they
  decide on every census rule are as before the theorem panels;
  `advance()` on `RLR` stays above 65 million steps per second in node.


The node check suite (`check.js`, with its reference library, census table and specs) is not in
this repo: it lives in the local research folder `~/tmp/ant-invent/build/`
(`node ~/tmp/ant-invent/build/check.js index.html all`).

## Git

Real-name account `iasonS`, its no-reply address (see the Git Rules in `~/CLAUDE.md`).
Feature branch and pull request for every change after the first commit.
Nothing here mentions any other account or repo.
