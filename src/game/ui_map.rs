//! Kartan (M) med flikar per plats och klick för att gå dit, samt minikartan (K) i hörnet.
use super::ui_widgets::*;
use super::world::World;
use crate::engine::{Canvas, FontKind, Fonts, Input, Key, rgb};

/// Vad kartan vill att spelet ska göra.
#[derive(Clone, Debug, PartialEq)]
pub enum MapAct {
    None,
    Close,
    /// Snabbresa till en plats
    Travel(String),
    /// Klick på kartan: (plats, pixel x, pixel y). Kartans rektangel anges i `MapView::map_rect`.
    Walk(String, i32, i32),
}

pub struct MapView {
    /// Platsen vars karta visas (flik)
    pub site: String,
    /// (nyckel, namn) för platserna som finns den här veckan
    pub sites: Vec<(String, String)>,
    /// Platsen spelaren står på
    pub here: String,
    pub hard: bool,
    pub closed: bool,
    pub map_rect: Rect,
    tabs: Vec<(Rect, String)>,
    travel: Vec<(Rect, String)>,
    close_btn: Rect,
}

impl MapView {
    pub fn new(sites: Vec<(String, String)>, here: &str, hard: bool) -> MapView {
        MapView { site: here.to_string(), sites, here: here.to_string(), hard, closed: false, map_rect: Rect::default(), tabs: vec![], travel: vec![], close_btn: Rect::default() }
    }

    fn step_tab(&mut self, d: i32) {
        let n = self.sites.len() as i32;
        if n == 0 {
            return;
        }
        let i = self.sites.iter().position(|s| s.0 == self.site).unwrap_or(0) as i32;
        self.site = self.sites[((i + d + n) % n) as usize].0.clone();
    }

    pub fn update(&mut self, input: &Input) -> MapAct {
        for k in &input.pressed {
            match k {
                Key::Escape | Key::M => {
                    self.closed = true;
                    return MapAct::Close;
                }
                Key::Left | Key::A => self.step_tab(-1),
                Key::Right | Key::D | Key::Tab => self.step_tab(1),
                Key::Num1 | Key::Num2 | Key::Num3 => {
                    let i = match k {
                        Key::Num1 => 0,
                        Key::Num2 => 1,
                        _ => 2,
                    };
                    if let Some(s) = self.sites.get(i) {
                        self.site = s.0.clone();
                    }
                }
                _ => {}
            }
        }
        if input.clicked {
            let (mx, my) = (input.mouse_x, input.mouse_y);
            if self.close_btn.contains(mx, my) {
                self.closed = true;
                return MapAct::Close;
            }
            if let Some((_, s)) = self.tabs.iter().find(|(r, _)| r.contains(mx, my)) {
                self.site = s.clone();
                return MapAct::None;
            }
            if let Some((_, s)) = self.travel.iter().find(|(r, _)| r.contains(mx, my)) {
                return MapAct::Travel(s.clone());
            }
            if self.map_rect.contains(mx, my) {
                return MapAct::Walk(self.site.clone(), mx, my);
            }
        }
        MapAct::None
    }

    pub fn draw(&mut self, c: &mut Canvas, fonts: &mut Fonts, world: &World, mouse: (i32, i32), tickets: &[String]) {
        c.dim(rgb(0, 0, 0), 150);
        let (sw, sh) = (c.w as i32, c.h as i32);
        let pw = 1000.min(sw - 30);
        let ph = 660.min(sh - 30);
        let (px, py) = ((sw - pw) / 2, (sh - ph) / 2);
        panel(c, Rect::new(px, py, pw, ph), PANEL, BORDER);
        fonts.text(c, px + 20, py + 16, "Karta", TITLE, ACCENT, FontKind::Pixel);
        // flikar
        self.tabs.clear();
        let mut tx = px + 140;
        for (key, name) in &self.sites {
            let w = text_w(fonts, name, BODY, FontKind::Mono) + 30;
            let r = Rect::new(tx, py + 10, w, 34);
            let sel = *key == self.site;
            let hover = r.contains(mouse.0, mouse.1);
            c.fill_rect(r.x, r.y, r.w, r.h, if sel { BTN_PRIMARY } else if hover { BTN_HI } else { BTN });
            c.rect(r.x, r.y, r.w, r.h, if sel { 2 } else { 1 }, if sel { ACCENT } else { BORDER });
            fonts.text(c, r.x + 15, r.y + 6, name, BODY, TEXT, FontKind::Mono);
            if *key == self.here {
                c.circle(r.x + r.w - 9, r.y + 9, 4, rgb(0xff, 0x4a, 0x3a), 255);
            }
            self.tabs.push((r, key.clone()));
            tx += w + 8;
        }
        // själva kartan
        let mr = Rect::new(px + 20, py + 56, pw - 40, ph - 56 - 120);
        c.fill_rect(mr.x, mr.y, mr.w, mr.h, rgb(0x10, 0x14, 0x20));
        c.rect(mr.x, mr.y, mr.w, mr.h, 1, BORDER);
        self.map_rect = mr;
        let (scale, ox, oy) = world.draw_map(c, &self.site, mr.x + 4, mr.y + 4, mr.w - 8, mr.h - 8);
        // utropstecken över kollegor med ärenden
        if let Some(info) = world.sites_info().into_iter().find(|s| s.0 == self.site) {
            for n in tickets {
                if let Some((nx, nz)) = world.npc_pos(n) {
                    if nx >= info.2 && nx <= info.3 && nz >= info.4 && nz <= info.5 {
                        let (bx, by) = (ox as i32 + ((nx - info.2) * scale) as i32, oy as i32 + ((info.5 - nz) * scale) as i32);
                        fonts.text(c, bx - 3, by - 24, "!", 18.0, rgb(0xff, 0xd5, 0x4a), FontKind::Pixel);
                    }
                }
            }
        }
        if self.map_rect.contains(mouse.0, mouse.1) {
            let name = self.sites.iter().find(|s| s.0 == self.site).map(|s| s.1.clone()).unwrap_or_default();
            let tip = if self.site == self.here { "Klicka för att gå dit".to_string() } else { format!("Klicka för att åka till {} och gå dit", name) };
            let w = text_w(fonts, &tip, SMALL, FontKind::Mono);
            let tx = (mouse.0 + 14).min(c.w as i32 - w - 14);
            c.fill_rect_a(tx - 6, mouse.1 + 14, w + 12, line_h(SMALL) + 6, rgb(0x0c, 0x0e, 0x18), 230);
            fonts.text(c, tx, mouse.1 + 17, &tip, SMALL, TEXT, FontKind::Mono);
        }
        // teckenförklaring
        let mut ly = mr.y + mr.h + 10;
        let mut lx = px + 24;
        let mut legend: Vec<(u32, &str, i32)> = vec![(0xff4a3a, "Du", 4), (0x9ab0c8, "Kollega", 3)];
        if !self.hard {
            legend.push((0xe8a317, "Kollega med ärende (!)", 3));
            legend.push((0xffd54a, "Målet", 5));
        }
        legend.push((0x3fae4f, "Har fått hjälp", 3));
        for (col, label, r) in legend {
            c.circle(lx + 6, ly + 9, r + 1, 0x101010, 255);
            c.circle(lx + 6, ly + 9, r, col, 255);
            fonts.text(c, lx + 18, ly, label, SMALL, MUTED, FontKind::Mono);
            lx += 18 + text_w(fonts, label, SMALL, FontKind::Mono) + 22;
        }
        ly += line_h(SMALL) + 12;
        // snabbresa
        self.travel.clear();
        fonts.text(c, px + 24, ly + 8, "Snabbresa:", SMALL, MUTED, FontKind::Mono);
        let mut bx = px + 24 + text_w(fonts, "Snabbresa:", SMALL, FontKind::Mono) + 14;
        for (key, name) in &self.sites {
            if *key == self.here {
                continue;
            }
            let label = format!("Åk till {}", name);
            let w = text_w(fonts, &label, BODY, FontKind::Mono) + 30;
            let r = Rect::new(bx, ly, w, 36);
            button(c, fonts, r, &label, false, r.contains(mouse.0, mouse.1), false);
            self.travel.push((r, key.clone()));
            bx += w + 10;
        }
        self.close_btn = Rect::new(px + pw - 130, py + ph - 52, 110, 38);
        button(c, fonts, self.close_btn, "Stäng", true, self.close_btn.contains(mouse.0, mouse.1), false);
        fonts.text(c, px + 24, py + ph - 40, "M/Esc stänger · pilarna eller 1-3 byter flik", SMALL, MUTED, FontKind::Mono);
    }
}

// ------------------------------------------------------------------ minikartan

/// Pixlar per meter i minikartan
pub const MINI_PPM: f64 = 5.0;
pub const MINI_W: i32 = 200;
pub const MINI_H: i32 = 140;

/// Minikartan: rummen runt spelaren i ett hörn. Ritas om några gånger i sekunden.
pub struct MiniMap {
    cv: Canvas,
    stamp: f64,
    site: String,
}

impl MiniMap {
    pub fn new() -> MiniMap {
        MiniMap { cv: Canvas::new(MINI_W as usize, MINI_H as usize), stamp: -1.0, site: String::new() }
    }

    /// Ritar minikartan på (x, y). Spelaren hamnar i mitten.
    pub fn draw(&mut self, c: &mut Canvas, fonts: &mut Fonts, world: &World, x: i32, y: i32, time: f64) {
        let site = world.site.clone();
        if time - self.stamp > 0.12 || self.site != site {
            self.stamp = time;
            self.site = site.clone();
            self.cv.clear(rgb(0x10, 0x14, 0x20));
            if let Some(info) = world.sites_info().into_iter().find(|s| s.0 == site) {
                let (ww, hh) = (info.3 - info.2, info.5 - info.4);
                // kartan ritas i full storlek (MINI_PPM px/m) med spelaren i mitten av rutan
                let (px, pz) = world.player_pos();
                let (fw, fh) = ((ww * MINI_PPM) as i32, (hh * MINI_PPM) as i32);
                let sc = (fw as f64 / ww).min(fh as f64 / hh);
                let (mw, mh) = ((ww * sc) as i32, (hh * sc) as i32);
                let ox = MINI_W / 2 - ((px - info.2) * sc) as i32 - (fw - mw) / 2;
                let oy = MINI_H / 2 - ((info.5 - pz) * sc) as i32 - (fh - mh) / 2;
                world.draw_map(&mut self.cv, &site, ox, oy, fw, fh);
            }
        }
        for row in 0..MINI_H {
            let yy = y + row;
            if yy < 0 || yy >= c.h as i32 {
                continue;
            }
            for col in 0..MINI_W {
                let xx = x + col;
                if xx < 0 || xx >= c.w as i32 {
                    continue;
                }
                c.px[yy as usize * c.w + xx as usize] = self.cv.px[(row * MINI_W + col) as usize];
            }
        }
        c.rect(x - 1, y - 1, MINI_W + 2, MINI_H + 2, 2, BORDER);
        let z = world.zone_name();
        c.fill_rect_a(x, y + MINI_H - line_h(SMALL) - 2, MINI_W, line_h(SMALL) + 2, rgb(0, 0, 0), 160);
        text_fit(c, fonts, x + 4, y + MINI_H - line_h(SMALL) - 1, &z, SMALL - 3.0, TEXT, FontKind::Mono, MINI_W - 8);
    }

    /// Världskoordinaten för en klickad pixel på minikartan som ritats på (x, y)
    pub fn to_world(world: &World, x: i32, y: i32, mx: i32, my: i32) -> (f64, f64) {
        let (px, pz) = world.player_pos();
        let mut sc = MINI_PPM;
        if let Some(info) = world.sites_info().into_iter().find(|s| s.0 == world.site) {
            let (ww, hh) = (info.3 - info.2, info.5 - info.4);
            sc = (((ww * MINI_PPM) as i32) as f64 / ww).min((((hh * MINI_PPM) as i32) as f64) / hh);
        }
        (px + (mx - (x + MINI_W / 2)) as f64 / sc, pz - (my - (y + MINI_H / 2)) as f64 / sc)
    }
}
