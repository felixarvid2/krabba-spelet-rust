//! Handboken (H): kommandon, felbibliotek, adressplan, subnätkalkylator, OSI, ordlista m.m.
//! Port av js/ui/handbook.js. Innehållet är genererat till handbook_data.rs.
use super::handbook_data::{self as data, Block};
use super::ui_widgets::*;
use crate::engine::{Canvas, FontKind, Fonts, Input, Key};
use crate::net::util as U;

const TABS: &[(&str, &str)] = &[
    ("cmd", "Kommandon"),
    ("mine", "Mina kommandon"),
    ("fel", "Felbibliotek"),
    ("plan", "Adressplan"),
    ("calc", "Subnätsräknare"),
    ("bygg", "Bygg från grunden"),
    ("vpn", "VPN och LB"),
    ("osi", "OSI-modellen"),
    ("ord", "Ordlista"),
    ("keys", "Styrning"),
];

pub struct Handbook {
    pub tab: usize,
    pub search: String,
    pub calc: String,
    scroll: i32,
    max_scroll: i32,
    doc: Option<Doc>,
    doc_key: (usize, i32, String, String, usize),
    tab_rects: Vec<(Rect, usize)>,
    copy_rects: Vec<(Rect, String)>,
    /// Text som ska läggas i urklipp
    pub clip: Option<String>,
    /// Senaste unika kommandon (var, kommando) från spelet
    pub mine: Vec<(String, String)>,
    /// Veckotitlar för repetitionstabellen
    pub weeks: Vec<(u32, String, String)>,
    pub closed: bool,
}

impl Handbook {
    pub fn new(tab: &str, mine: Vec<(String, String)>, weeks: Vec<(u32, String, String)>) -> Handbook {
        let t = TABS.iter().position(|x| x.0 == tab).unwrap_or(0);
        Handbook {
            tab: t,
            search: String::new(),
            calc: "192.168.1.100/26".into(),
            scroll: 0,
            max_scroll: 0,
            doc: None,
            doc_key: (usize::MAX, 0, String::new(), String::new(), 0),
            tab_rects: vec![],
            copy_rects: vec![],
            clip: None,
            mine,
            weeks,
            closed: false,
        }
    }

    fn blocks(&self) -> Option<&'static [Block]> {
        match TABS[self.tab].0 {
            "cmd" => Some(data::CMD),
            "fel" => Some(data::FEL),
            "plan" => Some(data::PLAN),
            "bygg" => Some(data::BYGG),
            "vpn" => Some(data::VPN),
            "osi" => Some(data::OSI),
            "ord" => Some(data::ORD),
            "keys" => Some(data::KEYS),
            _ => None,
        }
    }

    pub fn update(&mut self, input: &Input) {
        // flikbyte
        for k in &input.pressed {
            match k {
                Key::Escape => {
                    if !self.search.is_empty() {
                        self.search.clear();
                    } else {
                        self.closed = true;
                    }
                }
                Key::Tab => {
                    let n = TABS.len();
                    self.tab = if input.shift { (self.tab + n - 1) % n } else { (self.tab + 1) % n };
                    self.scroll = 0;
                }
                Key::Left if input.ctrl => {
                    self.tab = (self.tab + TABS.len() - 1) % TABS.len();
                    self.scroll = 0;
                }
                Key::Right if input.ctrl => {
                    self.tab = (self.tab + 1) % TABS.len();
                    self.scroll = 0;
                }
                Key::Up => self.scroll = (self.scroll - 40).max(0),
                Key::Down => self.scroll = (self.scroll + 40).min(self.max_scroll),
                Key::PageUp => self.scroll = (self.scroll - 400).max(0),
                Key::PageDown => self.scroll = (self.scroll + 400).min(self.max_scroll),
                Key::Home => self.scroll = 0,
                Key::End => self.scroll = self.max_scroll,
                Key::Backspace => {
                    if TABS[self.tab].0 == "calc" {
                        self.calc.pop();
                    } else {
                        self.search.pop();
                    }
                }
                _ => {}
            }
        }
        if !input.ctrl {
            for ch in &input.chars {
                if printable(*ch) {
                    if TABS[self.tab].0 == "calc" {
                        self.calc.push(*ch);
                    } else {
                        self.search.push(*ch);
                    }
                }
            }
        }
        if input.wheel != 0.0 {
            self.scroll = (self.scroll - (input.wheel * 40.0) as i32).clamp(0, self.max_scroll);
        }
        if input.clicked {
            for (r, t) in &self.tab_rects {
                if r.contains(input.mouse_x, input.mouse_y) {
                    self.tab = *t;
                    self.scroll = 0;
                }
            }
            for (r, s) in &self.copy_rects {
                if r.contains(input.mouse_x, input.mouse_y) {
                    self.clip = Some(s.clone());
                }
            }
        }
    }

    fn build(&mut self, fonts: &mut Fonts, w: i32) {
        let key = (self.tab, w, self.search.clone(), self.calc.clone(), self.mine.len());
        if self.doc.is_some() && self.doc_key == key {
            return;
        }
        let q = self.search.to_lowercase();
        let mut d = Doc::new(w);
        match TABS[self.tab].0 {
            "calc" => {
                d.para(fonts, "Skriv en adress med prefix eller nätmask, till exempel 192.168.1.100/26 eller 10.0.0.2 255.255.255.192. Skriv direkt – raden nedan är textfältet.", MUTED);
                d.text(fonts, &format!("> {}_", self.calc), BODY + 2.0, ACCENT, FontKind::Mono, 0);
                d.gap(8);
                match calc_rows(&self.calc) {
                    Ok(rows) => {
                        d.table(fonts, &rows);
                        d.text(fonts, "Blocksteget: 256 minus masken i den oktett där masken slutar. Näten börjar på jämna multiplar av blocksteget.", SMALL, MUTED, FontKind::Mono, 0);
                    }
                    Err(e) => d.para(fonts, &e, BAD),
                }
            }
            "mine" => {
                if self.mine.is_empty() {
                    d.para(fonts, "Här samlas kommandona du skriver under veckan. Klicka på ett för att kopiera det.", MUTED);
                } else {
                    d.para(fonts, "Dina senaste unika kommandon. Klicka för att kopiera.", MUTED);
                    let rows: Vec<Vec<String>> = std::iter::once(vec!["Var".to_string(), "Kommando".to_string()]).chain(self.mine.iter().map(|(a, b)| vec![a.clone(), b.clone()])).collect();
                    d.table(fonts, &rows);
                }
            }
            _ => {
                if let Some(blocks) = self.blocks() {
                    let mut pending_head: Option<&str> = None;
                    for b in blocks {
                        match b {
                            Block::H(s) => pending_head = Some(s),
                            Block::P(s) => {
                                if q.is_empty() || s.to_lowercase().contains(&q) {
                                    if let Some(h) = pending_head.take() {
                                        d.heading(fonts, h);
                                    }
                                    d.para(fonts, s, MUTED);
                                }
                            }
                            Block::L(items) => {
                                let v: Vec<String> = items.iter().filter(|s| q.is_empty() || s.to_lowercase().contains(&q)).map(|s| s.to_string()).collect();
                                if !v.is_empty() {
                                    if let Some(h) = pending_head.take() {
                                        d.heading(fonts, h);
                                    }
                                    d.bullets(fonts, &v);
                                }
                            }
                            Block::T(rows) => {
                                let mut v: Vec<Vec<String>> = vec![];
                                for (i, r) in rows.iter().enumerate() {
                                    if i == 0 || q.is_empty() || r.iter().any(|c| c.to_lowercase().contains(&q)) {
                                        v.push(r.iter().map(|s| s.to_string()).collect());
                                    }
                                }
                                if v.len() > 1 || (q.is_empty() && !v.is_empty()) {
                                    if let Some(h) = pending_head.take() {
                                        d.heading(fonts, h);
                                    }
                                    d.table(fonts, &v);
                                }
                            }
                        }
                    }
                    if TABS[self.tab].0 == "vpn" && !self.weeks.is_empty() && q.is_empty() {
                        d.heading(fonts, "Veckorna i repris");
                        let mut rows = vec![vec!["Vecka".to_string(), "Tema".to_string(), "Kommandot att komma ihåg".to_string()]];
                        for (n, t, c) in &self.weeks {
                            rows.push(vec![n.to_string(), t.clone(), c.clone()]);
                        }
                        d.table(fonts, &rows);
                    }
                    if d.items.is_empty() {
                        d.para(fonts, "Inga träffar.", MUTED);
                    }
                }
            }
        }
        self.doc = Some(d);
        self.doc_key = key;
    }

    pub fn draw(&mut self, c: &mut Canvas, fonts: &mut Fonts, mouse: (i32, i32)) {
        c.dim(0x000000, 170);
        let (sw, sh) = (c.w as i32, c.h as i32);
        let pw = 1180.min(sw - 20);
        let ph = sh - 30;
        let px = (sw - pw) / 2;
        let py = 15;
        panel(c, Rect::new(px, py, pw, ph), PANEL, BORDER);
        c.fill_rect(px, py, pw, 38, PANEL2);
        fonts.text(c, px + 16, py + 11, "HANDBOK", TITLE, ACCENT, FontKind::Pixel);
        let hint = "Ctrl+←/→ eller Tab: flik · skriv för att söka · Esc: stäng";
        let hw = text_w(fonts, hint, SMALL, FontKind::Mono);
        fonts.text(c, px + pw - hw - 16, py + 10, hint, SMALL, MUTED, FontKind::Mono);
        // flikar
        self.tab_rects.clear();
        let mut tx = px + 12;
        let mut ty = py + 46;
        for (i, (_, name)) in TABS.iter().enumerate() {
            let w = text_w(fonts, name, SMALL, FontKind::Mono) + 20;
            if tx + w > px + pw - 12 {
                tx = px + 12;
                ty += 32;
            }
            let r = Rect::new(tx, ty, w, 28);
            let on = i == self.tab;
            let hover = r.contains(mouse.0, mouse.1);
            c.fill_rect(r.x, r.y, r.w, r.h, if on { BTN_PRIMARY } else if hover { BTN_HI } else { BTN });
            c.rect(r.x, r.y, r.w, r.h, 1, if on { ACCENT } else { BORDER });
            fonts.text(c, r.x + 10, r.y + 4, name, SMALL, TEXT, FontKind::Mono);
            self.tab_rects.push((r, i));
            tx += w + 6;
        }
        let top = ty + 38;
        // sökrad
        if TABS[self.tab].0 != "calc" {
            let r = Rect::new(px + 12, top, pw - 24, 30);
            c.fill_rect(r.x, r.y, r.w, r.h, 0x0b0d16);
            c.rect(r.x, r.y, r.w, r.h, 1, BORDER);
            let shown = if self.search.is_empty() { format!("Sök i {}…", TABS[self.tab].1.to_lowercase()) } else { format!("{}_", self.search) };
            fonts.text(c, r.x + 8, r.y + 4, &shown, BODY, if self.search.is_empty() { MUTED } else { TEXT }, FontKind::Mono);
        }
        let body_y = top + 40;
        let body = Rect::new(px + 14, body_y, pw - 40, py + ph - body_y - 10);
        self.build(fonts, body.w);
        let doc_h = self.doc.as_ref().map(|d| d.h).unwrap_or(0);
        self.max_scroll = (doc_h - body.h + 20).max(0);
        self.scroll = self.scroll.clamp(0, self.max_scroll);
        c.set_clip(body.x - 2, body.y, body.w + 4, body.h);
        if let Some(d) = &self.doc {
            d.draw(c, fonts, body.x, body.y - self.scroll, body.y - 40, body.y + body.h + 40);
        }
        c.reset_clip();
        // kopieringsknappar i "mina kommandon": klick på raden
        self.copy_rects.clear();
        if TABS[self.tab].0 == "mine" {
            // raderna ligger i tabellen; ungefärlig träff via radhöjd
            let lh = line_h(SMALL) + 8;
            for (i, (_, cmd)) in self.mine.iter().enumerate() {
                let y = body.y + lh * (i as i32 + 1) + 8 - self.scroll + 2;
                self.copy_rects.push((Rect::new(body.x, y, body.w, lh), cmd.clone()));
            }
        }
        if self.max_scroll > 0 {
            let th = (body.h * body.h / (doc_h + 20)).max(24);
            let ty2 = body.y + (body.h - th) * self.scroll / self.max_scroll.max(1);
            c.fill_rect(px + pw - 14, body.y, 5, body.h, 0x0b0d16);
            c.fill_rect(px + pw - 14, ty2, 5, th, BORDER);
        }
    }
}

/// Subnätsräknaren: tabellrader eller felmeddelande
pub fn calc_rows(inp: &str) -> Result<Vec<Vec<String>>, String> {
    let v = inp.trim().split_whitespace().collect::<Vec<_>>().join(" ");
    let err = || "Skriv t.ex. 192.168.1.100/26".to_string();
    let (ip, rest) = match v.split_once(|ch: char| ch == '/' || ch == ' ') {
        Some((a, b)) => (a.to_string(), (v.chars().nth(a.len()).unwrap_or(' '), b.to_string())),
        None => return Err(err()),
    };
    if !U::is_ip(&ip) {
        return Err(err());
    }
    let pfx: u32 = if rest.0 == '/' {
        match rest.1.parse::<u32>() {
            Ok(p) if p <= 32 => p,
            _ => return Err("Ogiltig nätmask.".into()),
        }
    } else {
        if !U::is_ip(&rest.1) {
            return Err(err());
        }
        match U::mask_to_prefix(&rest.1) {
            Some(p) => p,
            None => return Err("Ogiltig nätmask.".into()),
        }
    };
    let mask = U::prefix_to_mask(pfx);
    let net = U::network(&ip, &mask);
    let bc = U::broadcast(&ip, &mask);
    let size: u64 = 1u64 << (32 - pfx);
    let hosts = if pfx >= 31 { if pfx == 32 { 1 } else { 2 } } else { size - 2 };
    let octet = (pfx / 8).min(3) as usize;
    let mask_oct: i64 = mask.split('.').nth(octet).and_then(|s| s.parse().ok()).unwrap_or(0);
    let block = 256 - mask_oct;
    let wild = U::int_to_ip(!U::ipn(&mask));
    let first = if pfx >= 31 { net.clone() } else { U::int_to_ip(U::ipn(&net) + 1) };
    let last = if pfx >= 31 { bc.clone() } else { U::int_to_ip(U::ipn(&bc).wrapping_sub(1)) };
    Ok(vec![
        vec!["Vad".into(), "Värde".into()],
        vec!["Nätmask".into(), format!("{} (/{})", mask, pfx)],
        vec!["Wildcard (för ACL)".into(), wild],
        vec!["Blocksteg".into(), format!("{} i oktett {}", block, octet + 1)],
        vec!["Nätadress".into(), net],
        vec!["Första användbara".into(), first],
        vec!["Sista användbara".into(), last],
        vec!["Broadcast".into(), bc],
        vec!["Antal värdar".into(), hosts.to_string()],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn calc_26() {
        let r = calc_rows("192.168.1.100/26").unwrap();
        assert_eq!(r[1][1], "255.255.255.192 (/26)");
        assert_eq!(r[4][1], "192.168.1.64");
        assert_eq!(r[7][1], "192.168.1.127");
        assert_eq!(r[8][1], "62");
        assert_eq!(r[2][1], "0.0.0.63");
    }
    #[test]
    fn calc_mask_form() {
        let r = calc_rows("10.0.0.2 255.255.255.252").unwrap();
        assert_eq!(r[8][1], "2");
        assert!(calc_rows("hej").is_err());
    }
}
