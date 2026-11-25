# CLAUDE.md: langtons-ant

Contents: [Layout](#layout) · [What it is](#what-it-is) · [Rules](#rules) · [Checking](#checking) · [Git](#git)

## Layout

A Cargo workspace. No web version: the original page (`index.html`) was removed on 2026-11-25
(in git history at `8cb1e5c`). The Rust is the only source.

| Path | What |
|---|---|
| `crates/ant-core` | The rules. No dependencies (`flate2` in the tests only, to read the fixtures). No drawing. |
| `crates/atlas` | The window app (`eframe`/`egui`, wgpu). `src/picture.rs` decides, `src/ui.rs` draws. |
| `crates/ant-core/tests/data/*.jsonl.gz` | Fixtures recorded from the original page. Frozen. |
| `docs/*.drawio.svg` | README diagrams. Editable in draw.io (`~/Applications/drawio/squashfs-root/drawio --no-sandbox -x -f svg -e -o OUT IN`). |
| `shots/rust-*.png` | The three checked screenshots. |

- Run: `cargo run --release -p atlas`. Flags: `--demo [N[,RULE]]` (every ant at N steps, default 60,000, paused; then RULE as the first panel), `--shot FILE.png` (with `--demo`: one 1600x900 picture, drawn offscreen with `egui_kittest`, no window), `--selftest-seconds N` (run N s, print the clock and the first 8 ants, exit).
- Checks: `cargo test --release` (about 50 s). Speed: `cargo run --release -p ant-core --example speed`. Before a commit: `cargo fmt`, `cargo clippy --all-targets`.
- `ant-core` was ported one to one from the page's rules block (same names in snake_case, same constants, timetables, order of operations, tie-breaks and comments). The fixtures (`tests/fixture.rs`) hold what the page decided: every rule of 2 to 12 letters that starts with R at 300,000 steps, and the 28 start rules at 2,000,000, field for field, with hashes of the whole grid and visit counts. They were written by `tools/js-reference.js` (removed; in git history at `ecadee5`). A change to the rules must keep them passing. A change that is meant to change what an ant does needs new fixtures written by the Rust and the reason in the commit. Never edit a fixture just to make a test pass.
- `atlas/src/picture.rs` has every decision that is not drawing, as plain functions over `ant-core` data and no egui: `words`, `tip_text`, `Judge::on_show` (the half-second rule), `typed_panel` (which panel a typed rule becomes, `copy_ant` over the proved start panels), `FrameClock` (`frame_ms`, the 1-in-32 half frame), `sim_ms`/`paint_ms` (the budget), `Model::tick` (the clock, turns of 4,096 steps), `Model::demo`, `choose_grid`, `cell_colours`. `src/ui.rs` is drawing and controls only (`fit`, `outgrown`, `freeze`, `stamp`, `render`, marker, strip, rule box, speed, pause, restart). Change a decision in `picture.rs` and its tests, not in `ui.rs`.
- `simulate()` checks once per call that the steps cannot reach the edge, then indexes the grid without bounds checks. Keep that check.
- Window text uses JetBrains Mono from `/usr/share/fonts/TTF` if there, else egui's own font.

## What it is

Ant atlas: 28 ant rules side by side, one ant per panel. An ant walks on a grid of coloured cells; its rule (a string of L and R, one letter per colour) says which way it turns on each colour. Every panel simulates its ant and runs a detector on it. A panel says `proved` only when `certify()` has checked that the ant repeats for ever; it then shows the period, the shift and the proof step, the ant is no longer simulated, and `position_at()` places it by arithmetic. A repeat that the check refused (`ant.repeat`) shows as `repeat` with the step at which it ends. A running app puts a repeat on show only if it will last half a second at the speed set, at the moment the picture first sees it (`ends_at - ant.steps >= 0.5 * rate`); it decides once per repeat, and a repeat on show stays until it ends. A paused app (and `--demo`) shows whatever `ant.repeat` holds. This is a rule of the picture: ant-core knows nothing of it. The rest say `not proved in N steps` (never "no repeat": the ant may be in a repeat the detector has not looked at, or in one that is not on show). An unproved panel with no repeat on show also shows `fake highway: N × period P`, when `ant.fake` covers at least 512 steps and has at least 8 rounds. `ant.fake`: each look notes one refused repeat (the one that ends latest), and `ant.fake` is the noted one that covers the most steps (`ends_at - from`). It is not the ant's record: a short repeat can begin and end between two looks, unseen.

A panel whose rule is even-run says `no highway` from the first frame, in its own tone, with the lines `never a highway (theorem)` and `back at its start N times`. Even-run (`even_runs()`): read as a cycle, the rule is made of runs of L and runs of R that all have even length (`LLRR`, `RLLR`, `LLRRRR`; `RLLR` and `RRLL` among the 28). A 1995 theorem ("Further travels with my ant") says that such an ant, started on the blank grid, comes back to its start cell again and again for ever, so it never builds a highway. The word comes from the rule alone, never from `certify()`, and the ant is simulated like any other. N is `ant.home_visits - 1`, counted by the simulation (`ant.last_home` is the step count of the last time; both are in the tooltip). A repeat on show takes the second line while it runs (`period P repeat · ends at step N`), and the end of the map takes it for good; the word stays `no highway`. The `fake highway` line is not on such a panel, only in its tooltip. `never a highway` is said ONLY for even-run rules and always with the word `theorem` near it. The top strip counts these panels (`never a highway (theorem) 2`).

Counters per panel and in the top strip: steps simulated, steps skipped. Controls: a rule box (2 to 24 letters, becomes the first panel), speed, pause, restart. A typed rule that starts like a proved ant is that same ant: `copy_ant()` gives its panel a copy of the first proved panel that accepts it, and the panel says `proved` and `same ant as RRL` with `simulated 0`. `same ant as` is said only for a copy, never from comparing pictures. Why it holds: a cell visited v times makes the ant read letter v of its rule written again and again (counting from 0); `ant.vis` counts the visits, and at a proof `proof.letters` is the most visits any cell gets over the whole endless run (6 for `RRL`, 33 for `RL`; infinite for a loop; 255 means 255 or more, and then nothing is copied). A rule whose first `proof.letters` letters are the same makes the same moves for ever. The tooltip of a proved panel states this with its number. On a copy the tooltip's `repeats from step` line names the source (`RRL repeats from step 40`): it is `proof.onset` of the shared proof. The copy's own colours can start to repeat at another step (`RRLRRLLRLR`: 56, which its panel says once it has proved itself after restart). Every ant stands at `clock - born` steps. A copy starts at its source's proof step (`position_at()` knows no place before the proof), so it is born `proof.at` steps before the click. After restart a copy is an ordinary ant. Hover a proved panel for the ant's position at step 1,000,000,000,000. The tooltip of any panel whose `ant.fake` is set also gives that repeat. The speed label says `set to N steps/s` while paused and right after load, play or restart, until the first measurement; then the speed really reached.

## Rules

- The app answers one question you can see. The numbers on screen come from the running sim, never typed in by hand.
- Few controls: rule box, speed, pause, restart. No scores, no win or lose.
- Rules and picture stay apart: ant-core is plain functions over plain data with no drawing.
- Check the rules with numbers (`cargo test`) before trusting the picture.
- Time-based, not frame-based (steps per second times frame time), so 60 Hz and 165 Hz behave the same.
- Colours that tell things apart (the cell ramp and the three status tones: proved `#69a621`, repeat `#d861aa`, no highway `#259cde`, all pairs) are checked with the dataviz skill's `validate_palette.js` against `#0a0b0d` before use. The ant ramp passes for rules of up to 11 letters. Longer rules FAIL the script's step check (steps closer than 0.06 in lightness), and that includes two of the 28 start panels (12 letters, step 0.057). This is an open exception, not a pass.
- Dark background `#0a0b0d`, text `#e8e6df`, muted `#8b8a84`.
- Nothing in this repo names an email or another account.

## Checking

Shots: `cargo run --release -p atlas -- --demo [N[,RULE]] --shot shots/NAME.png`, then LOOK at the picture.

- `shots/rust-demo.png` (`--demo`): top strip `proved 5 of 28`, `simulated 1,424,823`, `skipped 255,177`.
- `shots/rust-trap.png` (`--demo 398000`): the `LRRRRRLLR` panel says `repeat` and `ends at step 400,704`.
- `shots/rust-same.png` (`--demo 60000,RRLRRLLRLR`): the first panel says `proved`, `same ant as RRL` and `simulated 0`.
- In all three the `RLLR` and `RRLL` panels say `no highway` and `never a highway (theorem)`, and the top strip `never a highway (theorem) 2`.

A shot does not prove the live loop. Live: `timeout 12 cargo run --release -p atlas -- --selftest-seconds 4`. The clock must be about 5,000 steps per second times the seconds at the first slider setting, and a proved ant's `simulated` must stand still while its `steps` follow the clock. Not yet checked in the app (the page had these): typing a rule into the live window, the top of the slider, narrow windows, frame misses on a fast screen.

The detector looks on two timetables: a quick look every 256 steps for periods up to 128, and a full look for periods up to 8192 that gets rarer as the ant gets older (64 steps apart at first, 8192 at most). `frame_ms` is the screen's frame interval: the shortest of the last 32 intervals between frames. Simulating gets 60 % of it per frame (10 ms at most), drawing the cells 25 % (1.5 ms at least). The time is read after every ant's turn of 4,096 steps. Every 32nd frame works half as long. The clock runs on real time, not on `frame_ms`.

What the ant-core tests hold (`crates/ant-core/tests/`):

- `fixture`: both fixtures, field for field; the census numbers (4,094 rules, 114 even-run, 408 highways, no even-run rule proved); `start_rules()` is the 28 of the demo.
- `traps`: on `LRRRRRLLR` and `RRRRRLRR` `certify()` returns ok at no step, and every `ends_at` is the true end of that repeat in a separate plain ant; `ant.repeat` of `LRRRRRLLR` is `{period 116, from 396128, ends_at 400703}` at every look from step 396,544 until it ends, and gone one step later (period 48 of `RRRRRLRR`: `{from 457881, ends_at 460208}`, from step 457,984); `ant.fake` is a true repeat there.
- `jump`: for every proved start rule `position_at()` equals a plain ant at later steps; a proof comes at the first look that tries its period; `proof.letters` is 6 for `RRL`, 33 for `RL`, infinite for a loop.
- `edge`: `advance()` in one call equals many small calls; `home_visits` and `last_home` equal a plain ant through a growth of the grid; `proof.cells` stamped 1, 2 and 5 times gives the cells 1, 2 and 5 periods later; positions at 10^12 and 2^53-1; an ant that leaves the map.
- `same`: over all 408 census highways `proof.letters` equals the largest visit count of a plain ant run on (255 where that is 255 or more), and again from an earlier step; copies (`copy_ant`) match the plain ant of the copied rule (path, grid, stamps, home counts); the number is tight (another letter at place `proof.letters` gets no copy); on made-up footprints it equals the rounds laid down one by one, and at least 1,000 footprints are ones where the sum over rounds to come decides (the ONLY check of that sum in `letters_read()`: keep it).
- `small`: `valid_rule`; `even_runs` against a second reading on every rule of 1 to 14 letters, mirror and rotation; `same_start` against written-out words.
- Speed: `advance()` on `RLR` measured 160 million steps per second (2026-11-25, `examples/speed.rs`).

The atlas tests (`cargo test --release -p atlas`) hold the picture rules: the half-second rule, the words of each state, typed rules (`LLRR` even-run at step 0, `RLR` not; `RRLRRLLRLR` a copy of `RRL`, `RRLRRR` not), `frame_ms`, the fake-highway limits.

## Git

Real-name account `iasonS`, its no-reply address (see the Git Rules in `~/CLAUDE.md`).
Feature branch and pull request for every change.
Nothing here mentions any other account or repo.
