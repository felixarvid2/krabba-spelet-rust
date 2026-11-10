//! Text: fontdue med inbäddade typsnitt och glyfcache. Saknade glyfer ersätts med egenritade tecken.
use super::{Canvas, Color, FontKind};
use fontdue::{Font, FontSettings};
use std::collections::HashMap;
use std::sync::Arc;

static VT323: &[u8] = include_bytes!("../../assets/fonts/VT323.ttf");
static PRESS_START: &[u8] = include_bytes!("../../assets/fonts/PressStart2P.ttf");

/// En rastrerad glyf: täckning 0–255, placerad relativt baslinjen.
pub struct Glyph {
    pub w: usize,
    pub h: usize,
    pub xmin: i32,
    /// Avstånd från baslinjen upp till översta raden
    pub top: i32,
    pub adv: f32,
    pub cov: Vec<u8>,
}

/// Typsnitten med glyfcache per (typsnitt, storlek, tecken).
pub struct Fonts {
    mono: Font,
    pixel: Font,
    cache: HashMap<(FontKind, u32, char), Arc<Glyph>>,
    metrics: HashMap<(FontKind, u32), (i32, i32)>,
}

impl Fonts {
    /// Laddar VT323 och Press Start 2P (inbäddade med include_bytes! från assets/fonts)
    pub fn load() -> Fonts {
        let mono = Font::from_bytes(VT323, FontSettings::default()).expect("VT323");
        let pixel = Font::from_bytes(PRESS_START, FontSettings::default()).expect("PressStart2P");
        Fonts { mono, pixel, cache: HashMap::new(), metrics: HashMap::new() }
    }

    fn font(&self, kind: FontKind) -> &Font {
        match kind {
            FontKind::Mono => &self.mono,
            FontKind::Pixel => &self.pixel,
        }
    }

    /// (ascent, radhöjd) i heltalspixlar
    fn line(&mut self, size: f32, kind: FontKind) -> (i32, i32) {
        let key = (kind, size.to_bits());
        if let Some(m) = self.metrics.get(&key) {
            return *m;
        }
        let m = match self.font(kind).horizontal_line_metrics(size) {
            Some(l) => (l.ascent.round() as i32, (l.ascent - l.descent).ceil() as i32),
            None => ((size * 0.8).round() as i32, size.ceil() as i32),
        };
        self.metrics.insert(key, m);
        m
    }

    fn glyph(&mut self, kind: FontKind, size: f32, c: char) -> Arc<Glyph> {
        let key = (kind, size.to_bits(), c);
        if let Some(g) = self.cache.get(&key) {
            return g.clone();
        }
        let g = Arc::new(self.make_glyph(kind, size, c));
        self.cache.insert(key, g.clone());
        g
    }

    fn mono_adv(&self, size: f32) -> f32 {
        self.mono.metrics('M', size).advance_width
    }

    fn make_glyph(&mut self, kind: FontKind, size: f32, c: char) -> Glyph {
        let empty = |adv: f32| Glyph { w: 0, h: 0, xmin: 0, top: 0, adv, cov: vec![] };
        match c {
            '\u{200B}' | '\u{200D}' | '\u{FE0E}' | '\u{FE0F}' | '\r' => return empty(0.0),
            '\t' => {
                let a = self.glyph(kind, size, ' ').adv;
                return empty(a * 4.0);
            }
            _ => {}
        }
        if c.is_control() {
            return empty(0.0);
        }
        let has = self.font(kind).lookup_glyph_index(c) != 0 || c == ' ';
        if has {
            let (m, bmp) = self.font(kind).rasterize(c, size);
            let mut cov = bmp;
            let mut adv = m.advance_width;
            if kind == FontKind::Pixel {
                // Pixelfonten ska vara skarp
                for v in cov.iter_mut() {
                    *v = if *v >= 128 { 255 } else { 0 };
                }
                adv = adv.round();
            }
            return Glyph { w: m.width, h: m.height, xmin: m.xmin, top: m.ymin + m.height as i32, adv, cov };
        }
        let (asc, _) = self.line(size, kind);
        let cell = if kind == FontKind::Mono { self.mono_adv(size) } else { size.round().max(1.0) };
        let mut g = synth(c, size, cell, asc, if kind == FontKind::Pixel { 0.125 } else { 0.09 });
        if kind == FontKind::Pixel {
            for v in g.cov.iter_mut() {
                *v = if *v >= 128 { 255 } else { 0 };
            }
        }
        g
    }

    /// Ritar text och returnerar bredden i pixlar. size = pixelhöjd. Stöder svenska tecken (åäö) och de vanligaste symbolerna.
    pub fn text(&mut self, c: &mut Canvas, x: i32, y: i32, s: &str, size: f32, color: Color, kind: FontKind) -> i32 {
        let (asc, lh) = self.line(size, kind);
        let mut pen = 0.0f32;
        let mut maxw = 0.0f32;
        let mut base = y + asc;
        for ch in s.chars() {
            if ch == '\n' {
                maxw = maxw.max(pen);
                pen = 0.0;
                base += lh;
                continue;
            }
            let g = self.glyph(kind, size, ch);
            if g.w > 0 {
                let gx = x + (pen + g.xmin as f32).round() as i32;
                let gy = base - g.top;
                c.blend_cov(gx, gy, g.w, g.h, &g.cov, color);
            }
            pen += g.adv;
        }
        maxw.max(pen).round() as i32
    }

    /// (bredd, höjd) i pixlar
    pub fn measure(&mut self, s: &str, size: f32, kind: FontKind) -> (i32, i32) {
        let (_, lh) = self.line(size, kind);
        let mut maxw = 0.0f32;
        let mut pen = 0.0f32;
        let mut lines = 1;
        for ch in s.chars() {
            if ch == '\n' {
                maxw = maxw.max(pen);
                pen = 0.0;
                lines += 1;
                continue;
            }
            pen += self.glyph(kind, size, ch).adv;
        }
        (maxw.max(pen).round() as i32, lh * lines)
    }

    /// Bredden på ett tecken i monospace (VT323) vid storleken
    pub fn mono_advance(&mut self, size: f32) -> f32 {
        self.mono_adv(size)
    }

    /// Radbryter text till maxbredd (ord för ord, behåller \n)
    pub fn wrap(&mut self, s: &str, size: f32, kind: FontKind, max_w: i32) -> Vec<String> {
        let mut out = Vec::new();
        for para in s.split('\n') {
            let mut cur = String::new();
            for word in para.split(' ') {
                let cand = if cur.is_empty() { word.to_string() } else { format!("{} {}", cur, word) };
                if self.measure(&cand, size, kind).0 <= max_w || cur.is_empty() && word.is_empty() {
                    cur = cand;
                    continue;
                }
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                // för långt ord: bryt på tecken
                let mut piece = String::new();
                for ch in word.chars() {
                    let mut t = piece.clone();
                    t.push(ch);
                    if !piece.is_empty() && self.measure(&t, size, kind).0 > max_w {
                        out.push(std::mem::take(&mut piece));
                    }
                    piece.push(ch);
                }
                cur = piece;
            }
            out.push(cur);
        }
        out
    }
}

// ---------- egenritade tecken ----------

struct Shape {
    w: usize,
    h: usize,
    cov: Vec<f32>,
}

impl Shape {
    fn new(w: usize, h: usize) -> Shape {
        Shape { w, h, cov: vec![0.0; w * h] }
    }
    fn put(&mut self, x: usize, y: usize, v: f32) {
        let i = y * self.w + x;
        if v > self.cov[i] {
            self.cov[i] = v;
        }
    }
    /// Linjesegment med tjocklek t
    fn seg(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, t: f32) {
        let (dx, dy) = (x1 - x0, y1 - y0);
        let l2 = (dx * dx + dy * dy).max(1e-6);
        for y in 0..self.h {
            for x in 0..self.w {
                let (px, py) = (x as f32 + 0.5 - x0, y as f32 + 0.5 - y0);
                let u = ((px * dx + py * dy) / l2).clamp(0.0, 1.0);
                let d = ((px - u * dx).powi(2) + (py - u * dy).powi(2)).sqrt();
                self.put(x, y, (t / 2.0 - d + 0.5).clamp(0.0, 1.0));
            }
        }
    }
    fn disc(&mut self, cx: f32, cy: f32, r: f32) {
        for y in 0..self.h {
            for x in 0..self.w {
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                self.put(x, y, (r - d + 0.5).clamp(0.0, 1.0));
            }
        }
    }
    fn tri(&mut self, a: (f32, f32), b: (f32, f32), c: (f32, f32)) {
        let sgn = |p: (f32, f32), q: (f32, f32), r: (f32, f32)| (p.0 - r.0) * (q.1 - r.1) - (q.0 - r.0) * (p.1 - r.1);
        for y in 0..self.h {
            for x in 0..self.w {
                let p = (x as f32 + 0.5, y as f32 + 0.5);
                let (d1, d2, d3) = (sgn(p, a, b), sgn(p, b, c), sgn(p, c, a));
                let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
                let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
                if !(neg && pos) {
                    self.put(x, y, 1.0);
                }
            }
        }
    }
    fn outline_box(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, t: f32) {
        self.seg(x0, y0, x1, y0, t);
        self.seg(x1, y0, x1, y1, t);
        self.seg(x1, y1, x0, y1, t);
        self.seg(x0, y1, x0, y0, t);
    }
}

/// Bygger en ersättningsglyf. cell = tecknets bredd, asc = baslinjens rad.
fn synth(c: char, size: f32, cell: f32, asc: i32, thick: f32) -> Glyph {
    let w = cell.ceil().max(2.0) as usize;
    let h = (size * 1.25).ceil().max(4.0) as usize;
    let base = (asc as usize).min(h - 1) as f32;
    let mut s = Shape::new(w, h);
    let t = (size * thick).max(1.0);
    let cx = w as f32 / 2.0;
    let cy = base - size * 0.33;
    let l = (size * 0.38).min(cx - 1.0).max(1.5);
    let hd = l * 0.6;
    let mut adv = cell;
    match c {
        '✓' | '✔' => {
            s.seg(cx - l, cy, cx - l * 0.3, cy + l * 0.7, t * 1.2);
            s.seg(cx - l * 0.3, cy + l * 0.7, cx + l, cy - l * 0.8, t * 1.2);
        }
        '✗' | '✘' | '×' | '✕' => {
            s.seg(cx - l * 0.8, cy - l * 0.8, cx + l * 0.8, cy + l * 0.8, t);
            s.seg(cx - l * 0.8, cy + l * 0.8, cx + l * 0.8, cy - l * 0.8, t);
        }
        '•' | '·' | '●' | '∙' => s.disc(cx, cy, (size * 0.13).max(1.0)),
        '→' | '➜' | '➔' | '⇒' => {
            s.seg(cx - l, cy, cx + l, cy, t);
            s.seg(cx + l, cy, cx + l - hd, cy - hd, t);
            s.seg(cx + l, cy, cx + l - hd, cy + hd, t);
        }
        '←' | '⇐' => {
            s.seg(cx - l, cy, cx + l, cy, t);
            s.seg(cx - l, cy, cx - l + hd, cy - hd, t);
            s.seg(cx - l, cy, cx - l + hd, cy + hd, t);
        }
        '↑' => {
            s.seg(cx, cy - l, cx, cy + l, t);
            s.seg(cx, cy - l, cx - hd, cy - l + hd, t);
            s.seg(cx, cy - l, cx + hd, cy - l + hd, t);
        }
        '↓' => {
            s.seg(cx, cy - l, cx, cy + l, t);
            s.seg(cx, cy + l, cx - hd, cy + l - hd, t);
            s.seg(cx, cy + l, cx + hd, cy + l - hd, t);
        }
        '⏎' | '↵' | '↩' => {
            s.seg(cx + l, cy - l, cx + l, cy + l * 0.4, t);
            s.seg(cx + l, cy + l * 0.4, cx - l, cy + l * 0.4, t);
            s.seg(cx - l, cy + l * 0.4, cx - l + hd, cy + l * 0.4 - hd, t);
            s.seg(cx - l, cy + l * 0.4, cx - l + hd, cy + l * 0.4 + hd, t);
        }
        '▶' | '►' | '▸' => s.tri((cx - l * 0.6, cy - l), (cx - l * 0.6, cy + l), (cx + l, cy)),
        '◀' | '◄' | '◂' => s.tri((cx + l * 0.6, cy - l), (cx + l * 0.6, cy + l), (cx - l, cy)),
        '▲' | '▴' => s.tri((cx - l, cy + l * 0.6), (cx + l, cy + l * 0.6), (cx, cy - l)),
        '▼' | '▾' => s.tri((cx - l, cy - l * 0.6), (cx + l, cy - l * 0.6), (cx, cy + l)),
        '🦀' => return crab(size, base as i32),
        _ => {
            // okänt tecken: ihålig ruta
            s.outline_box(cx - l * 0.8, cy - l * 1.1, cx + l * 0.8, cy + l * 1.1, t.max(1.0));
            adv = cell;
        }
    }
    let cov: Vec<u8> = s.cov.iter().map(|v| (v * 255.0).round() as u8).collect();
    Glyph { w: s.w, h: s.h, xmin: 0, top: base as i32, adv, cov }
}

/// Liten pixelkrabba (enfärgad) som ersättning för 🦀.
fn crab(size: f32, _base: i32) -> Glyph {
    const MAP: [&str; 7] = [
        "#.........#",
        "##.......##",
        "#.#######.#",
        ".#.#####.#.",
        "..#######..",
        "..#.#.#.#..",
        ".#.......#.",
    ];
    let p = ((size * 0.9 / 11.0).floor() as usize).max(1);
    let (w, h) = (11 * p + p, 7 * p);
    let mut cov = vec![0u8; w * h];
    for (ry, row) in MAP.iter().enumerate() {
        for (rx, ch) in row.chars().enumerate() {
            if ch == '#' {
                for dy in 0..p {
                    for dx in 0..p {
                        cov[(ry * p + dy) * w + rx * p + dx] = 255;
                    }
                }
            }
        }
    }
    Glyph { w, h, xmin: 0, top: h as i32, adv: w as f32 + p as f32, cov }
}
