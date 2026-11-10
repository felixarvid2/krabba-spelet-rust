//! Gemensamma byggstenar för gränssnittet: färger, rektanglar, knappar och textdokument (Doc).
#![allow(dead_code)]
use crate::engine::{Canvas, Color, FontKind, Fonts, rgb};

pub const BG: Color = rgb(0x0e, 0x10, 0x1a);
pub const PANEL: Color = rgb(0x1a, 0x1d, 0x2e);
pub const PANEL2: Color = rgb(0x24, 0x28, 0x3e);
pub const BORDER: Color = rgb(0x5a, 0x62, 0x8c);
pub const TEXT: Color = rgb(0xe6, 0xe9, 0xf5);
pub const MUTED: Color = rgb(0x9a, 0xa1, 0xc0);
pub const ACCENT: Color = rgb(0xff, 0xd5, 0x4a);
pub const GOOD: Color = rgb(0x6e, 0xe7, 0x87);
pub const BAD: Color = rgb(0xff, 0x6b, 0x6b);
pub const WARN: Color = rgb(0xff, 0xb0, 0x20);
pub const LINK: Color = rgb(0x7f, 0xd4, 0xff);
pub const BTN: Color = rgb(0x32, 0x38, 0x5c);
pub const BTN_HI: Color = rgb(0x46, 0x4f, 0x80);
pub const BTN_PRIMARY: Color = rgb(0x2f, 0x6f, 0xd6);
pub const BTN_PRIMARY_HI: Color = rgb(0x4a, 0x8b, 0xf0);

pub const BODY: f32 = 22.0;
pub const SMALL: f32 = 19.0;
pub const TITLE: f32 = 14.0;

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect { x, y, w, h }
    }
    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && py >= self.y && px < self.x + self.w && py < self.y + self.h
    }
}

/// Rad- och teckenhöjd för brödtext
pub fn line_h(size: f32) -> i32 {
    (size * 1.15).ceil() as i32
}

pub fn text_w(fonts: &mut Fonts, s: &str, size: f32, kind: FontKind) -> i32 {
    fonts.measure(s, size, kind).0
}

/// Ritar text och trunkerar med … om den inte får plats på max_w
pub fn text_fit(c: &mut Canvas, fonts: &mut Fonts, x: i32, y: i32, s: &str, size: f32, color: Color, kind: FontKind, max_w: i32) {
    if fonts.measure(s, size, kind).0 <= max_w {
        fonts.text(c, x, y, s, size, color, kind);
        return;
    }
    let mut t: Vec<char> = s.chars().collect();
    while !t.is_empty() {
        t.pop();
        let cand: String = t.iter().collect::<String>() + "…";
        if fonts.measure(&cand, size, kind).0 <= max_w {
            fonts.text(c, x, y, &cand, size, color, kind);
            return;
        }
    }
}

/// Panel med ram
pub fn panel(c: &mut Canvas, r: Rect, fill: Color, border: Color) {
    c.fill_rect(r.x, r.y, r.w, r.h, fill);
    c.rect(r.x, r.y, r.w, r.h, 2, border);
}

pub fn button(c: &mut Canvas, fonts: &mut Fonts, r: Rect, label: &str, primary: bool, hover: bool, focus: bool) {
    let fill = match (primary, hover) {
        (true, true) => BTN_PRIMARY_HI,
        (true, false) => BTN_PRIMARY,
        (false, true) => BTN_HI,
        (false, false) => BTN,
    };
    c.fill_rect(r.x, r.y, r.w, r.h, fill);
    c.rect(r.x, r.y, r.w, r.h, if focus { 3 } else { 1 }, if focus { ACCENT } else { BORDER });
    let tw = text_w(fonts, label, BODY, FontKind::Mono);
    let ty = r.y + (r.h - line_h(BODY)) / 2;
    if tw <= r.w - 8 {
        fonts.text(c, r.x + (r.w - tw) / 2, ty, label, BODY, TEXT, FontKind::Mono);
    } else {
        text_fit(c, fonts, r.x + 4, ty, label, BODY, TEXT, FontKind::Mono, r.w - 8);
    }
}

// ------------------------------------------------------------------ Doc: förberäknade textblock

#[derive(Clone, Debug)]
pub enum DocItem {
    Text { x: i32, y: i32, s: String, size: f32, color: Color, kind: FontKind },
    Rect { x: i32, y: i32, w: i32, h: i32, color: Color, alpha: u8 },
}

/// Ett scrollbart dokument med rubriker, stycken, listor och tabeller, färdigt radbrutet för en bredd.
#[derive(Clone, Debug, Default)]
pub struct Doc {
    pub items: Vec<DocItem>,
    pub w: i32,
    pub h: i32,
}

impl Doc {
    pub fn new(w: i32) -> Doc {
        Doc { items: vec![], w, h: 0 }
    }
    pub fn gap(&mut self, n: i32) {
        self.h += n;
    }
    /// Radbrutet stycke
    pub fn text(&mut self, fonts: &mut Fonts, s: &str, size: f32, color: Color, kind: FontKind, indent: i32) {
        let lh = line_h(size);
        for l in fonts.wrap(s, size, kind, (self.w - indent).max(40)) {
            self.items.push(DocItem::Text { x: indent, y: self.h, s: l, size, color, kind });
            self.h += lh;
        }
    }
    pub fn para(&mut self, fonts: &mut Fonts, s: &str, color: Color) {
        self.text(fonts, s, BODY, color, FontKind::Mono, 0);
        self.h += 4;
    }
    pub fn heading(&mut self, fonts: &mut Fonts, s: &str) {
        self.h += 8;
        self.text(fonts, s, 13.0, ACCENT, FontKind::Pixel, 0);
        self.h += 6;
    }
    pub fn bullets(&mut self, fonts: &mut Fonts, list: &[String]) {
        let lh = line_h(BODY);
        for it in list {
            let lines = fonts.wrap(it, BODY, FontKind::Mono, (self.w - 28).max(40));
            for (i, l) in lines.into_iter().enumerate() {
                if i == 0 {
                    self.items.push(DocItem::Text { x: 6, y: self.h, s: "•".into(), size: BODY, color: ACCENT, kind: FontKind::Mono });
                }
                self.items.push(DocItem::Text { x: 24, y: self.h, s: l, size: BODY, color: TEXT, kind: FontKind::Mono });
                self.h += lh;
            }
        }
        self.h += 4;
    }
    /// Tabell; första raden är rubrik. Kolumnbredder: vattenfyllning så att breda kolumner kortas först.
    pub fn table(&mut self, fonts: &mut Fonts, rows: &[Vec<String>]) {
        if rows.is_empty() {
            return;
        }
        let ncol = rows.iter().map(|r| r.len()).max().unwrap_or(1).max(1);
        let size = SMALL;
        let pad = 8;
        let mut nat = vec![40; ncol];
        for r in rows {
            for (i, cell) in r.iter().enumerate() {
                let w = text_w(fonts, cell, size, FontKind::Mono) + 2 * pad;
                if w > nat[i] {
                    nat[i] = w;
                }
            }
        }
        let avail = self.w;
        let mut widths = nat.clone();
        if nat.iter().sum::<i32>() > avail {
            // sök taket så att summan av min(nat, tak) ryms
            let (mut lo, mut hi) = (60, *nat.iter().max().unwrap());
            while lo < hi {
                let mid = (lo + hi + 1) / 2;
                let s: i32 = nat.iter().map(|n| (*n).min(mid)).sum();
                if s <= avail {
                    lo = mid;
                } else {
                    hi = mid - 1;
                }
            }
            widths = nat.iter().map(|n| (*n).min(lo)).collect();
        } else {
            // dela ut överskottet proportionellt
            let extra = avail - nat.iter().sum::<i32>();
            let tot: i32 = nat.iter().sum();
            for i in 0..ncol {
                widths[i] += extra * nat[i] / tot.max(1);
            }
        }
        let lh = line_h(size);
        for (ri, r) in rows.iter().enumerate() {
            let mut cells: Vec<Vec<String>> = vec![];
            let mut maxl = 1;
            for i in 0..ncol {
                let s = r.get(i).map(|s| s.as_str()).unwrap_or("");
                let lines = fonts.wrap(s, size, FontKind::Mono, (widths[i] - 2 * pad).max(20));
                maxl = maxl.max(lines.len().max(1));
                cells.push(lines);
            }
            let rh = maxl as i32 * lh + 8;
            let fill = if ri == 0 { rgb(0x2c, 0x33, 0x58) } else if ri % 2 == 0 { rgb(0x20, 0x24, 0x3a) } else { rgb(0x1a, 0x1d, 0x2e) };
            self.items.push(DocItem::Rect { x: 0, y: self.h, w: widths.iter().sum(), h: rh, color: fill, alpha: 255 });
            let mut x = 0;
            for (i, lines) in cells.into_iter().enumerate() {
                for (k, l) in lines.into_iter().enumerate() {
                    let col = if ri == 0 { ACCENT } else if i == 0 { LINK } else { TEXT };
                    self.items.push(DocItem::Text { x: x + pad, y: self.h + 4 + k as i32 * lh, s: l, size, color: col, kind: FontKind::Mono });
                }
                x += widths[i];
            }
            self.h += rh;
        }
        self.h += 8;
    }
    /// Ritar dokumentet med övre vänstra hörnet i (ox, oy), bara det som syns mellan y0 och y1 (skärmkoordinater)
    pub fn draw(&self, c: &mut Canvas, fonts: &mut Fonts, ox: i32, oy: i32, y0: i32, y1: i32) {
        for it in &self.items {
            match it {
                DocItem::Text { x, y, s, size, color, kind } => {
                    let sy = oy + y;
                    if sy + line_h(*size) < y0 || sy > y1 {
                        continue;
                    }
                    fonts.text(c, ox + x, sy, s, *size, *color, *kind);
                }
                DocItem::Rect { x, y, w, h, color, alpha } => {
                    let sy = oy + y;
                    if sy + h < y0 || sy > y1 {
                        continue;
                    }
                    if *alpha == 255 {
                        c.fill_rect(ox + x, sy, *w, *h, *color);
                    } else {
                        c.fill_rect_a(ox + x, sy, *w, *h, *color, *alpha);
                    }
                }
            }
        }
    }
}

/// Enkel textredigering för en rad/flera rader (tecken-index)
pub fn edit_insert(value: &mut String, cur: &mut usize, ch: char) {
    let mut v: Vec<char> = value.chars().collect();
    let at = (*cur).min(v.len());
    v.insert(at, ch);
    *value = v.into_iter().collect();
    *cur = at + 1;
}

pub fn edit_backspace(value: &mut String, cur: &mut usize) {
    let mut v: Vec<char> = value.chars().collect();
    let at = (*cur).min(v.len());
    if at > 0 {
        v.remove(at - 1);
        *cur = at - 1;
        *value = v.into_iter().collect();
    }
}

pub fn edit_delete(value: &mut String, cur: &mut usize) {
    let mut v: Vec<char> = value.chars().collect();
    let at = (*cur).min(v.len());
    if at < v.len() {
        v.remove(at);
        *value = v.into_iter().collect();
    }
}

/// Utesluter styrtecken
pub fn printable(ch: char) -> bool {
    !ch.is_control()
}

pub fn esc_time(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    format!("{}:{:02}", s / 60, s % 60)
}
