//! Ant atlas as a native window. `cargo run --release -p atlas`
//! Flags: `--demo [N[,RULE]]`, `--shot FILE.png` (with --demo: one 1600x900 picture, no window),
//! `--selftest-seconds N` (run N seconds, print the clock and some ants, exit).

mod picture;
mod ui;

use eframe::egui;
use picture::*;

const SLIDER0: f64 = 480.0;

fn model_for(args: &Args) -> Model {
    let mut m = Model::new(SLIDER0);
    if let Some((steps, rule)) = &args.demo {
        m.demo(*steps, rule.as_deref());
    }
    m
}

fn shot(args: &Args, path: &str) {
    let model = model_for(args);
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::Vec2::new(1600.0, 900.0))
        .with_accessibility_check(false)
        .wgpu()
        .build_eframe(|cc| {
            let mut app = ui::App::new(&cc.egui_ctx, model, SLIDER0);
            app.shot_mode = true;
            app
        });
    for _ in 0..4 {
        harness.step();
    }
    let img = harness.render().expect("render");
    img.save(path).expect("save png");
    println!("wrote {path} ({}x{})", img.width(), img.height());
}

fn main() -> eframe::Result {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let args = parse_args(&argv);
    if let Some(path) = &args.shot.clone() {
        shot(&args, path);
        return Ok(());
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1600.0, 900.0]).with_title("ant atlas"),
        ..Default::default()
    };
    eframe::run_native(
        "ant atlas",
        options,
        Box::new(move |cc| {
            let model = model_for(&args);
            let mut app = ui::App::new(&cc.egui_ctx, model, SLIDER0);
            app.selftest = args.selftest;
            Ok(Box::new(app))
        }),
    )
}
