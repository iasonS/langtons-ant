//! Drawing and egui: layout, the cells of each panel as a texture, the marker, the words, the
//! controls. Every decision about what is said or when is in `picture`.

use crate::picture::*;
use ant_core::*;
use eframe::egui::{
    self, epaint::Shape, Align, Align2, Color32, ColorImage, FontId, Layout, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions, Vec2,
};
use std::time::{Duration, Instant};

pub const TOP_H: f32 = 52.0;
pub const UI_H: f32 = 56.0;
pub const GAP: f32 = 8.0;
pub const HEAD: f32 = 20.0;
pub const FOOT: f32 = 42.0;

pub const BG: Color32 = Color32::from_rgb(0x0a, 0x0b, 0x0d);
pub const INK: Color32 = Color32::from_rgb(0xe8, 0xe6, 0xdf);
pub const MUTED: Color32 = Color32::from_rgb(0x8b, 0x8a, 0x84);
pub const LINE: Color32 = Color32::from_rgb(0x26, 0x28, 0x2d);
pub const ACCENT: Color32 = Color32::from_rgb(0x69, 0xa6, 0x21);
pub const WARN: Color32 = Color32::from_rgb(0xd8, 0x61, 0xaa);
pub const CALM: Color32 = Color32::from_rgb(0x25, 0x9c, 0xde);
pub const PANEL: Color32 = Color32::from_rgb(0x11, 0x13, 0x16);

const WHY_ALL: [&str; 3] = [
    "A panel says proved only after the page has checked that its ant repeats for ever. ",
    "From then on the ant is not simulated: its place is worked out. ",
    "Rules that start with L are mirror images, which is why the short ones all start with R.",
];
const NEVER_TIP: &str = "A 1995 theorem: an ant whose rule, read as a cycle, is made of runs of even length comes back to its start again and again for ever when it starts on the blank grid. So it never builds a highway";

fn tone_colour(t: Tone) -> Color32 {
    match t {
        Tone::Ok => ACCENT,
        Tone::Rep => WARN,
        Tone::Nev => CALM,
        Tone::Off => INK,
        Tone::None => MUTED,
    }
}
fn tone_border(t: Tone) -> Color32 {
    match t {
        Tone::Ok => Color32::from_rgb(0x34, 0x50, 0x0f),
        Tone::Rep => Color32::from_rgb(0x6b, 0x31, 0x55),
        Tone::Nev => Color32::from_rgb(0x17, 0x50, 0x6f),
        _ => LINE,
    }
}

// ---------------------------------------------------------------- the picture of one panel

#[derive(Clone, Copy, Default)]
pub struct View {
    s: i64,
    z: i64,
    bw: i64,
    bh: i64,
    x0: i64,
    y0: i64,
    dx: i64,
    dy: i64,
}

pub struct Gfx {
    pw: i64,
    ph: i64, // the picture in physical pixels
    pal: Vec<Color32>,
    view: Option<View>,
    drawn_view: Option<View>,
    img: Vec<Color32>,
    tex: Option<TextureHandle>,
    dirty: bool,
    drawn: i64,
    frozen: bool,
    copy: Option<Vec<u8>>,
    cw: i64,
    ch: i64,
    stamped: i64,
    stamps_done: bool,
}

impl Gfx {
    pub fn new(rule_len: usize) -> Gfx {
        let mut pal = vec![Color32::TRANSPARENT; rule_len.max(1)];
        for (i, c) in cell_colours(rule_len).into_iter().enumerate() {
            pal[i + 1] = Color32::from_rgb(c[0], c[1], c[2]);
        }
        Gfx {
            pw: 1,
            ph: 1,
            pal,
            view: None,
            drawn_view: None,
            img: vec![],
            tex: None,
            dirty: true,
            drawn: -1,
            frozen: false,
            copy: None,
            cw: 0,
            ch: 0,
            stamped: 0,
            stamps_done: false,
        }
    }

    /// Every picture is laid out again.
    fn reset(&mut self, pw: i64, ph: i64) {
        self.pw = pw;
        self.ph = ph;
        self.view = None;
        self.copy = None;
        self.frozen = false;
        self.dirty = true;
    }

    /// The view. One sample is one cell (drawn z pixels wide) or, when the picture is too big for
    /// the panel, every s-th cell (drawn as one pixel). room 1.15 leaves 15 % to grow into.
    fn fit(&mut self, x0: i64, x1: i64, y0: i64, y1: i64, room: f64) {
        let (pw, ph) = (self.pw as f64, self.ph as f64);
        let need_w = MIN_CELLS.max((x1 - x0 + 1) as f64 * room + 6.0);
        let need_h = (MIN_CELLS * ph / pw).max((y1 - y0 + 1) as f64 * room + 6.0);
        let mut z = (pw / need_w).min(ph / need_h).floor() as i64;
        let mut s = 1;
        if z < 1 {
            z = 1;
            s = (need_w / pw).max(need_h / ph).ceil() as i64;
        }
        let bw = (self.pw as f64 / z as f64).ceil() as i64;
        let bh = (self.ph as f64 / z as f64).ceil() as i64;
        let cx = ((x0 + x1 + 1) as f64 / 2.0 - (bw * s) as f64 / 2.0 + 0.5).floor() as i64;
        let cy = ((y0 + y1 + 1) as f64 / 2.0 - (bh * s) as f64 / 2.0 + 0.5).floor() as i64;
        self.view = Some(View {
            s,
            z,
            bw,
            bh,
            x0: cx,
            y0: cy,
            dx: (self.pw - bw * z).div_euclid(2),
            dy: (self.ph - bh * z).div_euclid(2),
        });
        if self.img.len() as i64 != bw * bh {
            self.img = vec![Color32::TRANSPARENT; (bw * bh) as usize];
        }
        self.dirty = true;
    }

    /// The view only changes when the ant's box comes close to its edge, so it does not jitter.
    fn outgrown(&self, a: &Ant) -> bool {
        let v = self.view.unwrap();
        let m = 2 * v.s;
        (a.min_x as i64) < v.x0 + m
            || (a.max_x as i64) > v.x0 + (v.bw - 1) * v.s - m
            || (a.min_y as i64) < v.y0 + m
            || (a.max_y as i64) > v.y0 + (v.bh - 1) * v.s - m
    }

    /// A proved ant: the view is fixed, with room on the side the highway runs to; the picture
    /// keeps its own copy of the cells in view and adds to it.
    fn freeze(&mut self, a: &Ant) {
        let pr = a.proof.as_ref().unwrap();
        let (sx, sy) = (pr.shift[0] as i64, pr.shift[1] as i64);
        let (mnx, mxx, mny, mxy) = (a.min_x as i64, a.max_x as i64, a.min_y as i64, a.max_y as i64);
        let ext = 12.max((0.6 * (mxx - mnx).max(mxy - mny) as f64).round() as i64);
        self.fit(
            mnx - if sx < 0 { ext } else { 0 },
            mxx + if sx > 0 { ext } else { 0 },
            mny - if sy < 0 { ext } else { 0 },
            mxy + if sy > 0 { ext } else { 0 },
            1.08,
        );
        let v = self.view.unwrap();
        let (cw, ch) = (v.bw * v.s, v.bh * v.s);
        let mut copy = vec![0u8; (cw * ch) as usize];
        let off = a.off as i64;
        let side = a.side as i64;
        let from = 0.max(-off - v.x0);
        let to = cw.min(side - off - v.x0);
        let mut y = 0;
        while y < ch && to > from {
            let gy = v.y0 + y + off;
            if gy >= 0 && gy < side {
                let g0 = gy * side + v.x0 + off;
                let src = &a.grid[(g0 + from) as usize..(g0 + to) as usize];
                copy[(y * cw + from) as usize..(y * cw + to) as usize].copy_from_slice(src);
            }
            y += 1;
        }
        self.copy = Some(copy);
        self.cw = cw;
        self.ch = ch;
        self.stamped = 0;
        self.stamps_done = sx == 0 && sy == 0;
        self.frozen = true;
    }

    /// Every round the ant has finished since the proof adds proof.cells once more, one shift
    /// further on. Stop for good once a round lies outside the view.
    fn stamp(&mut self, a: &Ant, limit: usize) -> bool {
        let pr = a.proof.as_ref().unwrap();
        let v = self.view.unwrap();
        let rounds = (a.steps as i64 - pr.base as i64).div_euclid(pr.period as i64) - 1;
        let mut work = 0usize;
        let mut changed = false;
        let n = a.n as i64;
        while !self.stamps_done && self.stamped < rounds && work < limit {
            self.stamped += 1;
            let j = self.stamped;
            let bx = pr.path_x[0] as i64 + j * pr.shift[0] as i64 - v.x0;
            let by = pr.path_y[0] as i64 + j * pr.shift[1] as i64 - v.y0;
            let mut inside = false;
            let copy = self.copy.as_mut().unwrap();
            for c in &pr.cells {
                let (x, y) = (bx + c[0] as i64, by + c[1] as i64);
                if x >= 0 && x < self.cw && y >= 0 && y < self.ch {
                    let i = (y * self.cw + x) as usize;
                    copy[i] = ((copy[i] as i64 + c[2] as i64) % n) as u8;
                    inside = true;
                }
            }
            work += pr.cells.len();
            if inside {
                changed = true;
            } else {
                self.stamps_done = true;
            }
        }
        changed
    }

    /// The cells in view, one pixel value per sample, uploaded as a texture.
    fn render(&mut self, a: &Ant, ctx: &egui::Context, id: usize) {
        let v = self.view.unwrap();
        let (src, sw, sh, sx0, sy0): (&[u8], i64, i64, i64, i64) = match &self.copy {
            Some(c) => (c, self.cw, self.ch, v.x0, v.y0),
            None => (&a.grid, a.side as i64, a.side as i64, -(a.off as i64), -(a.off as i64)),
        };
        let mut k = 0usize;
        for j in 0..v.bh {
            let gy = v.y0 + j * v.s - sy0;
            if gy < 0 || gy >= sh {
                self.img[k..k + v.bw as usize].fill(Color32::TRANSPARENT);
                k += v.bw as usize;
                continue;
            }
            let mut gx = v.x0 - sx0;
            let row = gy * sw;
            for _ in 0..v.bw {
                self.img[k] = if gx >= 0 && gx < sw {
                    self.pal[src[(row + gx) as usize] as usize]
                } else {
                    Color32::TRANSPARENT
                };
                k += 1;
                gx += v.s;
            }
        }
        let image = ColorImage::new([v.bw as usize, v.bh as usize], self.img.clone());
        match &mut self.tex {
            Some(t) => t.set(image, TextureOptions::NEAREST),
            None => self.tex = Some(ctx.load_texture(format!("panel{id}"), image, TextureOptions::NEAREST)),
        }
        self.drawn_view = Some(v);
        self.dirty = false;
        self.drawn = a.simulated as i64;
    }

    /// Cells, then the ant on top.
    fn compose(&self, painter: &egui::Painter, pic: Rect, ppp: f32, a: &Ant) {
        let (Some(v), Some(tex)) = (self.drawn_view, &self.tex) else { return };
        let to_pt = |x: f64, y: f64| Pos2::new(pic.min.x + x as f32 / ppp, pic.min.y + y as f32 / ppp);
        let img_rect = Rect::from_min_size(to_pt(v.dx as f64, v.dy as f64), Vec2::new((v.bw * v.z) as f32 / ppp, (v.bh * v.z) as f32 / ppp));
        painter.image(tex.id(), img_rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        let k = v.z as f64 / v.s as f64;
        let xf = |x: f64| v.dx as f64 + (x - v.x0 as f64 + 0.5) * k;
        let yf = |y: f64| v.dy as f64 + (y - v.y0 as f64 + 0.5) * k;
        let proof = a.proof.as_ref();
        let (ax, ay) = match proof.and_then(|_| position_at(a, a.steps)) {
            Some(p) => (p.x as f64, p.y as f64),
            None => (a.x as f64, a.y as f64),
        };
        let (mx, my) = (xf(ax), yf(ay));
        let stroke = Stroke::new(1.5, BG);
        if mx >= 0.0 && mx <= self.pw as f64 && my >= 0.0 && my <= self.ph as f64 {
            painter.circle(to_pt(mx, my), (3.5f32).max(0.6 * k as f32 / ppp), ACCENT, stroke);
            return;
        }
        let Some(p) = proof else { return };
        // the ant has left the picture: an arrow where its highway crosses the edge
        let (sx, sy) = (p.shift[0] as f64, p.shift[1] as f64);
        let len = sx.hypot(sy);
        let (ux, uy) = (sx / len, sy / len);
        let m = 9.0 * ppp as f64;
        let round = ((a.steps as i64 - p.base as i64).div_euclid(p.period as i64)) as f64;
        let (mx, my) = (xf(p.path_x[0] as f64 + round * sx), yf(p.path_y[0] as f64 + round * sy));
        let (pw, ph) = (self.pw as f64, self.ph as f64);
        let mut t: f64 = 0.0;
        if ux > 0.0 && mx > pw - m {
            t = t.max((mx - (pw - m)) / ux);
        }
        if ux < 0.0 && mx < m {
            t = t.max((mx - m) / ux);
        }
        if uy > 0.0 && my > ph - m {
            t = t.max((my - (ph - m)) / uy);
        }
        if uy < 0.0 && my < m {
            t = t.max((my - m) / uy);
        }
        let (cx, cy) = ((mx - t * ux).clamp(m, pw - m), (my - t * uy).clamp(m, ph - m));
        let (c, s) = (ux, uy); // rotation by the heading
        let pts: Vec<Pos2> = [(6.0, 0.0), (-5.0, -5.0), (-5.0, 5.0)]
            .iter()
            .map(|(px, py): &(f64, f64)| {
                let (px, py) = (px * ppp as f64, py * ppp as f64);
                to_pt(cx + px * c - py * s, cy + px * s + py * c)
            })
            .collect();
        painter.add(Shape::convex_polygon(pts, ACCENT, stroke));
    }
}

/// Bring the pictures up to date. A frame redraws only as many as fit in its `ms` of time (always
/// at least one) and goes on with the next ones in the next frame. `sync` draws everything now.
pub fn paint(model: &Model, gfx: &mut [Gfx], ctx: &egui::Context, paint_from: &mut usize, sync: bool, ms: f64) {
    let until = Instant::now() + Duration::from_secs_f64(ms / 1000.0);
    let count = model.panels.len();
    let mut drawn = 0;
    let mut next: Option<usize> = None;
    for n in 0..count {
        let i = (*paint_from + n) % count;
        let a = &model.panels[i].ant;
        let g = &mut gfx[i];
        if a.proof.is_some() {
            if !g.frozen {
                g.freeze(a);
            }
            if g.stamp(a, if sync { usize::MAX } else { STAMP_CELLS }) {
                g.dirty = true;
            }
        } else {
            if g.view.is_none() || g.outgrown(a) {
                g.fit(a.min_x as i64, a.max_x as i64, a.min_y as i64, a.max_y as i64, 1.15);
            }
            if a.simulated as i64 != g.drawn {
                g.dirty = true;
            }
        }
        if g.dirty {
            if drawn == 0 || Instant::now() < until {
                g.render(a, ctx, i);
                drawn += 1;
            } else if next.is_none() {
                next = Some(i);
            }
        }
    }
    if let Some(n) = next {
        *paint_from = n;
    }
}

// ---------------------------------------------------------------- the app

pub struct App {
    pub model: Model,
    gfx: Vec<Gfx>,
    gen: u64,
    rule_text: String,
    bad: bool,
    slider: f64,
    sync: bool,
    paint_from: usize,
    char_w10: f32,
    word_w: f32,
    sizes: Vec<(i64, i64, usize)>,
    pub selftest: Option<f64>,
    start: Option<f64>,
    pub shot_mode: bool,
}

pub fn setup(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    for (name, path, key) in [
        ("jb", "/usr/share/fonts/TTF/JetBrainsMonoNerdFontMono-Regular.ttf", "mono"),
        ("jbb", "/usr/share/fonts/TTF/JetBrainsMonoNerdFontMono-Bold.ttf", "bold"),
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            fonts.font_data.insert(name.to_string(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
            if key == "mono" {
                fonts.families.entry(egui::FontFamily::Monospace).or_default().insert(0, name.to_string());
                fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, name.to_string());
            } else {
                fonts.families.insert(egui::FontFamily::Name("bold".into()), vec![name.to_string()]);
            }
        }
    }
    if !fonts.families.contains_key(&egui::FontFamily::Name("bold".into())) {
        let mono = fonts.families[&egui::FontFamily::Monospace].clone();
        fonts.families.insert(egui::FontFamily::Name("bold".into()), mono);
    }
    ctx.set_fonts(fonts);
    ctx.set_visuals(egui::Visuals::dark());
    ctx.all_styles_mut(|s| {
        s.interaction.tooltip_delay = 0.0;
        s.interaction.show_tooltips_only_when_still = false;
        s.visuals.panel_fill = BG;
        s.visuals.window_fill = Color32::from_rgb(0x16, 0x18, 0x1c);
        s.visuals.window_stroke = Stroke::new(1.0, Color32::from_rgb(0x3a, 0x3d, 0x44));
        s.visuals.override_text_color = Some(INK);
        s.visuals.selection.bg_fill = ACCENT;
        s.visuals.selection.stroke = Stroke::new(1.0, ACCENT);
        s.visuals.extreme_bg_color = Color32::TRANSPARENT;
        for w in [&mut s.visuals.widgets.inactive, &mut s.visuals.widgets.hovered, &mut s.visuals.widgets.active] {
            w.weak_bg_fill = Color32::TRANSPARENT;
            w.bg_stroke = Stroke::new(1.0, LINE);
            w.corner_radius = 6.0.into();
        }
        s.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, MUTED);
        s.spacing.button_padding = Vec2::new(12.0, 7.0);
        s.spacing.item_spacing = Vec2::new(16.0, 0.0);
        s.spacing.slider_width = 160.0;
    });
}

fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

impl App {
    pub fn new(ctx: &egui::Context, model: Model, slider: f64) -> App {
        setup(ctx);
        let (char_w10, word_w) = (0.0, 0.0); // measured in the first frame: no fonts before that
        App {
            model,
            gfx: vec![],
            gen: u64::MAX,
            rule_text: String::new(),
            bad: false,
            slider,
            sync: true,
            paint_from: 0,
            char_w10,
            word_w,
            sizes: vec![],
            selftest: None,
            start: None,
            shot_mode: false,
        }
    }

    fn add_rule(&mut self) {
        match valid_rule(&self.rule_text) {
            None => self.bad = true,
            Some(r) => {
                self.bad = false;
                self.model.set_rules(Some(&r));
            }
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let screen = ctx.content_rect();
        let ppp = ctx.pixels_per_point();
        if self.char_w10 == 0.0 {
            self.char_w10 = ctx.fonts_mut(|f| f.layout_no_wrap("0".repeat(40), mono(10.0), INK).size().x / 40.0);
            self.word_w = ctx.fonts_mut(|f| f.layout_no_wrap("off the map".into(), mono(11.0), INK).size().x.ceil() + 10.0);
        }
        let now = ctx.input(|i| i.time) + 1.0;
        if self.start.is_none() {
            self.start = Some(now);
        }

        // gfx follows the set of panels
        if self.model.generation != self.gen {
            self.gen = self.model.generation;
            self.gfx = self.model.panels.iter().map(|p| Gfx::new(p.rule.len())).collect();
            self.sizes.clear();
            self.sync = true;
        }
        // layout: the grid that gives a panel the largest picture
        let count = self.model.panels.len();
        let (cols, rows) = choose_grid(count, screen.width() as f64, screen.height() as f64);
        let gx0 = 12.0;
        let gy0 = TOP_H + 2.0;
        let gw = screen.width() - 24.0;
        let gh = screen.height() - TOP_H - UI_H - 14.0;
        let cw = (gw - GAP * (cols as f32 - 1.0)) / cols as f32;
        let chh = (gh - GAP * (rows as f32 - 1.0)) / rows as f32;
        let mut rects = vec![];
        let mut pics = vec![];
        let mut said_again = false;
        if self.sizes.len() != count {
            self.sizes = vec![(0, 0, 0); count];
        }
        for i in 0..count {
            let (c, r) = (i % cols, i / cols);
            let rect = Rect::from_min_size(Pos2::new(gx0 + c as f32 * (cw + GAP), gy0 + r as f32 * (chh + GAP)), Vec2::new(cw, chh));
            let pic = Rect::from_min_size(
                Pos2::new(rect.min.x + 9.0, rect.min.y + 6.0 + HEAD),
                Vec2::new(rect.width() - 18.0, (rect.height() - 13.0 - HEAD - FOOT).max(1.0)),
            );
            let (pw, ph) = ((pic.width() * ppp).round().max(1.0) as i64, (pic.height() * ppp).round().max(1.0) as i64);
            let max_chars = (pic.width() / self.char_w10).floor().max(0.0) as usize;
            if self.sizes[i] != (pw, ph, max_chars) {
                self.sizes[i] = (pw, ph, max_chars);
                self.gfx[i].reset(pw, ph);
                self.model.panels[i].max_chars = max_chars;
                said_again = true;
                self.sync = true;
            }
            rects.push(rect);
            pics.push(pic);
        }
        if said_again {
            self.model.say_all();
        }

        // the clock
        let was = self.model.paused;
        let _ = was;
        self.model.frame(now);

        // the strip on top
        let totals = self.model.totals();
        egui::Panel::top("top").exact_size(TOP_H).frame(egui::Frame::new().fill(BG)).show(ui, |ui| {
            let rect = ui.max_rect();
            self.top_strip(ui, rect, &totals);
        });
        // the controls
        egui::Panel::bottom("ui")
            .exact_size(UI_H)
            .frame(egui::Frame::new().fill(PANEL).inner_margin(egui::Margin::symmetric(20, 0)))
            .show(ui, |ui| {
                let r = ui.max_rect();
                ui.painter().hline(r.x_range(), r.top(), Stroke::new(1.0, LINE));
                self.controls(ui, &ctx);
            });

        // the pictures
        let ms = paint_ms(self.model.fc.frame_ms, 1.0);
        let sync = std::mem::take(&mut self.sync);
        if self.gen == self.model.generation {
            paint(&self.model, &mut self.gfx, &ctx, &mut self.paint_from, sync, ms);
        }

        egui::CentralPanel::default().frame(egui::Frame::new().fill(BG)).show(ui, |ui| {
            for i in 0..self.model.panels.len().min(self.gfx.len()) {
                self.draw_panel(ui, i, rects[i], pics[i], ppp);
            }
        });

        if let Some(secs) = self.selftest {
            if now - self.start.unwrap() >= secs {
                self.print_selftest();
                self.selftest = None;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        if !self.model.paused || self.selftest.is_some() {
            ctx.request_repaint(); // running: every frame; paused: idle
        }
    }

    fn print_selftest(&self) {
        let m = &self.model;
        let (label, _) = m.rate_label();
        println!(
            "selftest: clock {:.0} steps, {} (shown rate {:.0}/s), frames {}, frame_ms {:.2}",
            m.clock, label, m.shown_rate, m.fc.frames, m.fc.frame_ms
        );
        for p in m.panels.iter().take(8) {
            println!(
                "  {:<14} state {:<8} steps {:>10} simulated {:>10} word {}",
                p.rule,
                p.ant.state.as_str(),
                p.ant.steps,
                p.ant.simulated,
                p.words.word
            );
        }
        let t = m.totals();
        println!(
            "  totals: proved {} of {}, never {}, simulated {}, skipped {}",
            t.proved, t.of, t.never, t.simulated, t.skipped
        );
    }

    fn top_strip(&mut self, ui: &mut egui::Ui, rect: Rect, t: &Totals) {
        let p = ui.painter().clone();
        let w = rect.width();
        let stacked = w < 1560.0;
        let (fs, lh) = if stacked { (11.0, 13.0) } else { (13.0, 15.0) };
        let items: [(&str, String); 4] = [
            ("proved", format!("{} of {}", t.proved, t.of)),
            ("never a highway (theorem)", t.never.to_string()),
            ("simulated", full(t.simulated)),
            ("skipped", full(t.skipped)),
        ];
        let galleys: Vec<_> = items
            .iter()
            .map(|(l, v)| {
                let mut job = egui::text::LayoutJob::default();
                job.append(&format!("{l} "), 0.0, egui::TextFormat::simple(mono(fs), MUTED));
                job.append(v, 0.0, egui::TextFormat::simple(mono(fs), INK));
                ui.fonts_mut(|f| f.layout_job(job))
            })
            .collect();
        let right = rect.right() - 20.0;
        let mut left = right;
        let mut rects = vec![];
        if stacked {
            for (k, g) in galleys.iter().enumerate() {
                let pos = Pos2::new(right - g.size().x, rect.top() + (TOP_H - 4.0 * lh) / 2.0 + k as f32 * lh);
                rects.push(Rect::from_min_size(pos, g.size()));
                left = left.min(pos.x);
            }
        } else {
            let mut x = right;
            for g in galleys.iter().rev() {
                x -= g.size().x;
                let pos = Pos2::new(x, rect.center().y - g.size().y / 2.0);
                rects.insert(0, Rect::from_min_size(pos, g.size()));
                x -= 20.0;
            }
            left = x + 20.0;
        }
        for (g, r) in galleys.into_iter().zip(&rects) {
            p.galley(r.min, g, INK);
        }
        let resp = ui.interact(rects[1], ui.scope_id().with("nevtip"), Sense::hover());
        resp.on_hover_ui_at_pointer(|ui| {
            ui.set_max_width(520.0);
            ui.label(egui::RichText::new(NEVER_TIP).monospace().size(12.0));
        });
        // the name and the sentence
        let name = ui.fonts_mut(|f| f.layout_no_wrap("ant atlas".into(), mono(20.0), INK));
        p.galley(Pos2::new(rect.left() + 20.0, rect.center().y - name.size().y / 2.0), name.clone(), INK);
        let x0 = rect.left() + 20.0 + name.size().x + 24.0;
        let avail = left - 24.0 - x0;
        let text: String = if w < 800.0 {
            WHY_ALL[0].to_string()
        } else if w < 1000.0 {
            format!("{}{}", WHY_ALL[0], WHY_ALL[2])
        } else {
            WHY_ALL.concat()
        };
        let mut job = egui::text::LayoutJob::simple(text, mono(11.0), MUTED, avail.max(10.0));
        job.wrap.max_rows = 3;
        job.wrap.overflow_character = None;
        for s in &mut job.sections {
            s.format.line_height = Some(14.0);
        }
        let g = ui.fonts_mut(|f| f.layout_job(job));
        p.with_clip_rect(Rect::from_min_size(Pos2::new(x0, rect.top()), Vec2::new(avail.max(0.0), TOP_H)))
            .galley(Pos2::new(x0, rect.center().y - g.size().y / 2.0), g, MUTED);
    }

    fn controls(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let width = ctx.content_rect().width();
        ui.horizontal_centered(|ui| {
            ui.label(egui::RichText::new("rule").size(13.0).color(MUTED));
            let tw = (width - 760.0).clamp(27.0 * 7.8, 58.0 * 7.8);
            let bad = self.bad;
            let resp = ui
                .scope(|ui| {
                    if bad {
                        let v = ui.visuals_mut();
                        for w in [
                            &mut v.widgets.inactive,
                            &mut v.widgets.hovered,
                            &mut v.widgets.active,
                            &mut v.widgets.noninteractive,
                        ] {
                            w.bg_stroke = Stroke::new(1.0, WARN);
                        }
                    }
                    ui.add(
                        egui::TextEdit::singleline(&mut self.rule_text)
                            .hint_text(
                                egui::RichText::new("L and R, 2 to 24 letters. Try one that starts like a proved ant")
                                    .size(11.0)
                                    .color(MUTED),
                            )
                            .char_limit(24)
                            .desired_width(tw)
                            .margin(Vec2::new(10.0, 7.0))
                            .font(mono(13.0))
                            .background_color(Color32::TRANSPARENT),
                    )
                })
                .inner;
            if resp.changed() {
                self.rule_text = self.rule_text.to_uppercase();
                self.bad = false;
            }
            let enter = resp.lost_focus() && ctx.input(|i| i.key_pressed(egui::Key::Enter));
            let add = ui.button("add");
            if add.clicked() || enter {
                self.add_rule();
            }
            if add.clicked() {
                add.surrender_focus();
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let again = ui.button("restart");
                if again.clicked() {
                    self.model.restart();
                    self.model.say_all();
                    again.surrender_focus();
                }
                let pause = ui.button(if self.model.paused { "play" } else { "pause" });
                if pause.clicked() {
                    self.toggle_pause();
                    pause.surrender_focus();
                }
                let (label, set) = self.model.rate_label();
                ui.allocate_ui_with_layout(Vec2::new(160.0, 20.0), Layout::left_to_right(Align::Center), |ui| {
                    ui.label(egui::RichText::new(label).size(13.0).color(if set { MUTED } else { INK }));
                });
                let before = self.slider;
                ui.add(egui::Slider::new(&mut self.slider, 0.0..=1000.0).show_value(false).trailing_fill(true));
                if before != self.slider {
                    self.model.rate = rate_of(self.slider);
                }
                ui.label(egui::RichText::new("speed").size(13.0).color(MUTED));
            });
        });
        // Space is pause everywhere but in the rule box
        if ctx.input(|i| i.key_pressed(egui::Key::Space)) && !ctx.egui_wants_keyboard_input() {
            self.toggle_pause();
        }
    }

    fn toggle_pause(&mut self) {
        let on = !self.model.paused;
        self.model.set_paused(on);
        self.model.say_all();
    }

    fn draw_panel(&mut self, ui: &mut egui::Ui, i: usize, rect: Rect, pic: Rect, ppp: f32) {
        let p = ui.painter().clone();
        let (rate, paused) = (self.model.rate, self.model.paused);
        let w = self.model.panels[i].words.clone();
        let rule = self.model.panels[i].rule.clone();
        p.rect_stroke(rect, 6.0, Stroke::new(1.0, tone_border(w.tone)), StrokeKind::Inside);
        // header: the rule (smaller for a long one, two small lines for a very long one), the word
        let hy = rect.min.y + 6.0;
        let right = pic.right();
        let word = ui.fonts_mut(|f| f.layout_no_wrap(w.word.to_string(), mono(11.0), tone_colour(w.tone)));
        p.galley(Pos2::new(right - word.size().x, hy + (HEAD - word.size().y) / 2.0), word, tone_colour(w.tone));
        let bold = FontId::new(13.0, egui::FontFamily::Name("bold".into()));
        let size = (((pic.width() - self.word_w) / (rule.len() as f32 * 0.69) * 2.0).floor() / 2.0).min(13.0);
        if size < 9.0 {
            let f = FontId::new(9.0, egui::FontFamily::Name("bold".into()));
            let cw = ui.fonts_mut(|fo| fo.layout_no_wrap("0".into(), f.clone(), INK).size().x);
            let per = (((pic.width() - self.word_w) / cw).floor() as usize).max(1);
            let chars: Vec<char> = rule.chars().collect();
            for (r, chunk) in chars.chunks(per).take(2).enumerate() {
                p.text(
                    Pos2::new(pic.left(), hy + r as f32 * 10.0),
                    Align2::LEFT_TOP,
                    chunk.iter().collect::<String>(),
                    f.clone(),
                    INK,
                );
            }
        } else {
            let f = FontId::new(size.max(9.0), bold.family.clone());
            let adv = ui.fonts_mut(|fo| fo.layout_no_wrap("0".into(), f.clone(), INK).size().x);
            let mut x = pic.left();
            for c in rule.chars() {
                p.text(Pos2::new(x, hy + HEAD / 2.0), Align2::LEFT_CENTER, c, f.clone(), INK);
                x += adv + 0.08 * f.size;
            }
        }
        // the picture
        if let Some(g) = self.gfx.get(i) {
            let clip = p.with_clip_rect(pic);
            g.compose(&clip, pic, ppp, &self.model.panels[i].ant);
        }
        // the lines, from the bottom
        let foot_bottom = rect.max.y - 7.0;
        let lines: Vec<&String> = [&w.l1, &w.l2].into_iter().filter(|s| !s.is_empty()).collect();
        let fb = mono(10.0);
        let row = |k: usize| foot_bottom - 14.0 * (k as f32 + 1.0);
        p.text(Pos2::new(pic.left(), row(0)), Align2::LEFT_TOP, &w.count[0], fb.clone(), MUTED);
        p.text(Pos2::new(pic.right(), row(0)), Align2::RIGHT_TOP, &w.count[1], fb.clone(), MUTED);
        for (k, l) in lines.iter().rev().enumerate() {
            p.text(Pos2::new(pic.left(), row(k + 1)), Align2::LEFT_TOP, l, fb.clone(), MUTED);
        }
        // the tooltip
        let resp = ui.interact(rect, ui.scope_id().with(("panel", i)), Sense::hover());
        if resp.hovered() {
            let text = tip_text(&mut self.model.panels[i], rate, paused);
            resp.on_hover_ui_at_pointer(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                ui.label(egui::RichText::new(text).monospace().size(12.0));
            });
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        App::ui(self, ui);
    }
    fn clear_color(&self, _v: &egui::Visuals) -> [f32; 4] {
        BG.to_normalized_gamma_f32()
    }
}
