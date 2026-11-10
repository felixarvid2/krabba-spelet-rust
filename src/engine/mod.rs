//! Krabbmotorn: en liten 2D-spelmotor för just det här spelet.
//! Mjukvarurendering till en pixelbuffert (0xRRGGBB), text med VT323 och Press Start 2P (fontdue),
//! fönster och inmatning via minifb, och ett huvudlöst läge som sparar bilder som PNG (för tester).
#![allow(unused_variables, dead_code)]

mod text;
mod window;
#[cfg(target_arch = "wasm32")]
pub mod web;

pub type Color = u32;

pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
    ((r as u32) << 16) | ((g as u32) << 8) | b as u32
}

/// En bild med alfakanal: 0xAARRGGBB (A = 0 är helt genomskinlig).
#[derive(Clone, Debug)]
pub struct Sprite {
    pub w: usize,
    pub h: usize,
    pub px: Vec<u32>,
}

impl Sprite {
    pub fn new(w: usize, h: usize) -> Sprite {
        Sprite { w, h, px: vec![0; w * h] }
    }
    /// Bygg en sprite från en teckenkarta: varje tecken slås upp i palett (tecken -> 0xAARRGGBB), '.' eller ' ' = genomskinligt
    pub fn from_map(rows: &[&str], palette: &[(char, u32)]) -> Sprite {
        let w = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0);
        let mut sp = Sprite::new(w, rows.len());
        for (y, row) in rows.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                if let Some((_, col)) = palette.iter().find(|(c, _)| *c == ch) {
                    sp.px[y * w + x] = *col;
                }
            }
        }
        sp
    }
    pub fn set(&mut self, x: usize, y: usize, argb: u32) {
        if x < self.w && y < self.h {
            self.px[y * self.w + x] = argb;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FontKind {
    /// VT323 – terminaltext och brödtext
    Mono,
    /// Press Start 2P – rubriker och pixel-UI
    Pixel,
}

/// Pixelbufferten som allt ritas på. Koordinater i pixlar, (0,0) uppe till vänster.
pub struct Canvas {
    pub w: usize,
    pub h: usize,
    pub px: Vec<u32>,
    /// Klippruta (x0, y0, x1, y1), x1/y1 exklusiva
    pub clip: (i32, i32, i32, i32),
}

/// Blandar en kanal: s över d med alfa a (0–255)
#[inline(always)]
fn mix(s: u32, d: u32, a: u32) -> u32 {
    let ia = 255 - a;
    let r = (((s >> 16) & 255) * a + ((d >> 16) & 255) * ia + 127) / 255;
    let g = (((s >> 8) & 255) * a + ((d >> 8) & 255) * ia + 127) / 255;
    let b = ((s & 255) * a + (d & 255) * ia + 127) / 255;
    (r << 16) | (g << 8) | b
}

impl Canvas {
    pub fn new(w: usize, h: usize) -> Canvas {
        Canvas { w, h, px: vec![0; w * h], clip: (0, 0, w as i32, h as i32) }
    }
    pub fn clear(&mut self, c: Color) {
        let c = c & 0xFFFFFF;
        self.px.fill(c);
    }
    pub fn set_clip(&mut self, x: i32, y: i32, w: i32, h: i32) {
        let x0 = x.clamp(0, self.w as i32);
        let y0 = y.clamp(0, self.h as i32);
        let x1 = x.saturating_add(w).clamp(x0, self.w as i32);
        let y1 = y.saturating_add(h).clamp(y0, self.h as i32);
        self.clip = (x0, y0, x1, y1);
    }
    pub fn reset_clip(&mut self) {
        self.clip = (0, 0, self.w as i32, self.h as i32);
    }
    #[inline]
    pub fn pixel(&mut self, x: i32, y: i32, c: Color) {
        let (x0, y0, x1, y1) = self.clip;
        if x >= x0 && x < x1 && y >= y0 && y < y1 {
            self.px[y as usize * self.w + x as usize] = c & 0xFFFFFF;
        }
    }
    /// Blandar in färgen med alfa 0–255
    #[inline]
    pub fn pixel_a(&mut self, x: i32, y: i32, c: Color, a: u8) {
        let (x0, y0, x1, y1) = self.clip;
        if a == 0 || x < x0 || x >= x1 || y < y0 || y >= y1 {
            return;
        }
        let i = y as usize * self.w + x as usize;
        self.px[i] = if a == 255 { c & 0xFFFFFF } else { mix(c, self.px[i], a as u32) };
    }
    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Color) {
        let (cx0, cy0, cx1, cy1) = self.clip;
        let x0 = x.max(cx0);
        let y0 = y.max(cy0);
        let x1 = x.saturating_add(w).min(cx1);
        let y1 = y.saturating_add(h).min(cy1);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        let c = c & 0xFFFFFF;
        for yy in y0..y1 {
            let o = yy as usize * self.w;
            self.px[o + x0 as usize..o + x1 as usize].fill(c);
        }
    }
    pub fn fill_rect_a(&mut self, x: i32, y: i32, w: i32, h: i32, c: Color, a: u8) {
        if a == 255 {
            return self.fill_rect(x, y, w, h, c);
        }
        if a == 0 {
            return;
        }
        let (cx0, cy0, cx1, cy1) = self.clip;
        let x0 = x.max(cx0);
        let y0 = y.max(cy0);
        let x1 = x.saturating_add(w).min(cx1);
        let y1 = y.saturating_add(h).min(cy1);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        for yy in y0..y1 {
            let o = yy as usize * self.w;
            for p in &mut self.px[o + x0 as usize..o + x1 as usize] {
                *p = mix(c, *p, a as u32);
            }
        }
    }
    /// Ram med tjocklek t
    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, t: i32, c: Color) {
        if w <= 0 || h <= 0 || t <= 0 {
            return;
        }
        let t = t.min(w / 2 + 1).min(h / 2 + 1);
        self.fill_rect(x, y, w, t, c);
        self.fill_rect(x, y + h - t, w, t, c);
        self.fill_rect(x, y + t, t, h - 2 * t, c);
        self.fill_rect(x + w - t, y + t, t, h - 2 * t, c);
    }
    /// Fylld rektangel med rundade hörn (radie r)
    pub fn round_rect(&mut self, x: i32, y: i32, w: i32, h: i32, r: i32, c: Color, a: u8) {
        if w <= 0 || h <= 0 {
            return;
        }
        let r = r.clamp(0, w.min(h) / 2);
        for row in 0..h {
            // hur mycket raden dras in av hörnen
            let dy = if row < r { r - row } else if row >= h - r { row - (h - r) + 1 } else { 0 };
            let inset = if dy == 0 {
                0
            } else {
                let rr = (r * r - (dy - 1) * (dy - 1)) as f32;
                r - rr.max(0.0).sqrt().round() as i32
            };
            self.fill_rect_a(x + inset, y + row, w - 2 * inset, 1, c, a);
        }
    }
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Color) {
        let (mut x, mut y) = (x0, y0);
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            self.pixel(x, y, c);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }
    pub fn thick_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, t: i32, c: Color) {
        if t <= 1 {
            return self.line(x0, y0, x1, y1, c);
        }
        // Bresenham med fyrkantig penna
        let (mut x, mut y) = (x0, y0);
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let off = t / 2;
        loop {
            self.fill_rect(x - off, y - off, t, t, c);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }
    /// Fylld cirkel
    pub fn circle(&mut self, cx: i32, cy: i32, r: i32, c: Color, a: u8) {
        self.ellipse(cx, cy, r, r, c, a);
    }
    /// Fylld ellips
    pub fn ellipse(&mut self, cx: i32, cy: i32, rx: i32, ry: i32, c: Color, a: u8) {
        if rx < 0 || ry < 0 {
            return;
        }
        for dy in -ry..=ry {
            let t = if ry == 0 { 0.0 } else { dy as f32 / (ry as f32 + 0.5) };
            let hw = ((rx as f32 + 0.5) * (1.0 - t * t).max(0.0).sqrt()).round() as i32;
            let hw = hw.min(rx);
            self.fill_rect_a(cx - hw, cy + dy, 2 * hw + 1, 1, c, a);
        }
    }
    /// Ritar en sprite med heltalsskala (pixlarna blir scale×scale), flip = spegla horisontellt
    pub fn blit(&mut self, s: &Sprite, x: i32, y: i32, scale: i32, flip: bool) {
        self.blit_a(s, x, y, scale, flip, 255);
    }
    /// Som blit men med extra genomskinlighet 0–255
    pub fn blit_a(&mut self, s: &Sprite, x: i32, y: i32, scale: i32, flip: bool, alpha: u8) {
        let sc = scale.max(1);
        if alpha == 0 || s.w == 0 || s.h == 0 {
            return;
        }
        let (cx0, cy0, cx1, cy1) = self.clip;
        let (dw, dh) = (s.w as i32 * sc, s.h as i32 * sc);
        let dx0 = x.max(cx0);
        let dx1 = (x + dw).min(cx1);
        let dy0 = y.max(cy0);
        let dy1 = (y + dh).min(cy1);
        if dx0 >= dx1 || dy0 >= dy1 {
            return;
        }
        let al = alpha as u32;
        for yy in dy0..dy1 {
            let sy = ((yy - y) / sc) as usize;
            let srow = &s.px[sy * s.w..(sy + 1) * s.w];
            let drow = &mut self.px[yy as usize * self.w..(yy as usize + 1) * self.w];
            for xx in dx0..dx1 {
                let mut sx = ((xx - x) / sc) as usize;
                if flip {
                    sx = s.w - 1 - sx;
                }
                let p = srow[sx];
                let mut pa = p >> 24;
                if pa == 0 {
                    continue;
                }
                if al != 255 {
                    pa = (pa * al + 127) / 255;
                }
                let d = &mut drow[xx as usize];
                *d = if pa >= 255 { p & 0xFFFFFF } else if pa == 0 { continue } else { mix(p, *d, pa) };
            }
        }
    }
    /// Mörkar/tonar hela ytan mot färgen (för dialoger bakom)
    pub fn dim(&mut self, c: Color, a: u8) {
        if a == 0 {
            return;
        }
        let a = a as u32;
        if a == 255 {
            return self.clear(c);
        }
        // förräkna källans bidrag, per kanal
        let (sr, sg, sb) = (((c >> 16) & 255) * a, ((c >> 8) & 255) * a, (c & 255) * a);
        let ia = 255 - a;
        for p in &mut self.px {
            let d = *p;
            let r = (sr + ((d >> 16) & 255) * ia + 127) / 255;
            let g = (sg + ((d >> 8) & 255) * ia + 127) / 255;
            let b = (sb + (d & 255) * ia + 127) / 255;
            *p = (r << 16) | (g << 8) | b;
        }
    }

    /// Blandar in en täckningskarta (0–255 per pixel) i färgen c, med klippning (används av text).
    pub(crate) fn blend_cov(&mut self, x: i32, y: i32, w: usize, h: usize, cov: &[u8], c: Color) {
        let (cx0, cy0, cx1, cy1) = self.clip;
        for row in 0..h as i32 {
            let yy = y + row;
            if yy < cy0 || yy >= cy1 {
                continue;
            }
            for col in 0..w as i32 {
                let xx = x + col;
                if xx < cx0 || xx >= cx1 {
                    continue;
                }
                let a = cov[row as usize * w + col as usize] as u32;
                if a == 0 {
                    continue;
                }
                let i = yy as usize * self.w + xx as usize;
                self.px[i] = if a == 255 { c & 0xFFFFFF } else { mix(c, self.px[i], a) };
            }
        }
    }
}
pub use text::Fonts;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Up, Down, Left, Right,
    W, A, S, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, T, U, V, X, Y, Z, B, C,
    Num0, Num1, Num2, Num3, Num4, Num5, Num6, Num7, Num8, Num9,
    Enter, Escape, Tab, Backspace, Delete, Space, Home, End, PageUp, PageDown,
    Shift, Ctrl, Alt,
    F1, F2, F3, F4, F5, F11,
}

/// Inmatningen för en bildruta.
#[derive(Clone, Debug, Default)]
pub struct Input {
    /// Tangenter som hålls nere
    pub down: std::collections::HashSet<Key>,
    /// Tangenter som trycktes ned den här bildrutan (med tangentrepetition)
    pub pressed: Vec<Key>,
    /// Inskrivna tecken den här bildrutan (för textfält och terminalen), utan styrtecken
    pub chars: Vec<char>,
    pub mouse_x: i32,
    pub mouse_y: i32,
    pub mouse_down: bool,
    /// Vänsterklick den här bildrutan
    pub clicked: bool,
    pub right_clicked: bool,
    pub wheel: f32,
    pub ctrl: bool,
    pub shift: bool,
}

impl Input {
    pub fn is_down(&self, k: Key) -> bool { self.down.contains(&k) }
    pub fn was_pressed(&self, k: Key) -> bool { self.pressed.contains(&k) }
}

/// Spelet som motorn kör.
pub trait Game {
    /// Uppdatera med inmatning och tid (sekunder). Returnera false för att avsluta.
    fn update(&mut self, input: &Input, dt: f64) -> bool;
    fn draw(&mut self, c: &mut Canvas, fonts: &mut Fonts);
    /// Text att lägga i urklipp (sätts av spelet, töms av motorn)
    fn take_clipboard(&mut self) -> Option<String> { None }
}

/// Upplösningen som spelet ritar i. Fönstret skalar upp den.
pub const WIDTH: usize = 1280;
pub const HEIGHT: usize = 720;

/// Öppnar ett fönster, kör spelet (~60 bilder/s): se window::run.
/// Huvudlöst läge med PNG-skärmdumpar: window::run_headless.
pub use window::{run_headless, save_png};
#[cfg(not(target_arch = "wasm32"))]
pub use window::run;

