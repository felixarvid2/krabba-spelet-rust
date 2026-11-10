//! Frontpanelsvyn för ett rack: enheterna uppifrån och ned med portlampor. Hover visar portstatus,
//! klick på en switch/router/WLC/LB kopplar in konsolkabeln. Port av rackView i ui.js (2D-läget).
use super::ui_widgets::*;
use super::world::port_led;
use crate::engine::{Canvas, FontKind, Fonts, Input, Key, rgb};
use crate::net::model::Kind;
use crate::net::sim::{self as S, State};
use crate::net::util as U;

pub struct Unit {
    pub id: String,
    pub label: String,
    pub kind: Option<Kind>,
    pub ports: Vec<String>,
    pub console: bool,
}

pub struct RackView {
    pub title: String,
    pub units: Vec<Unit>,
    pub closed: bool,
    /// Enhet som spelaren klickade på (konsol)
    pub pick: Option<String>,
    info: String,
    rects: Vec<(Rect, usize)>,
}

impl RackView {
    pub fn new(title: &str, ids: &[String], st: &State) -> RackView {
        let mut units = vec![];
        for id in ids {
            match st.devices.get(id) {
                Some(d) => {
                    let mut ports: Vec<String> = vec![];
                    if let Some(c) = &d.config {
                        for k in c.ifaces.keys() {
                            if k.starts_with("GigabitEthernet") || k.starts_with("FastEthernet") || k.starts_with("Serial") {
                                if !k.contains('.') {
                                    ports.push(k.clone());
                                }
                            }
                        }
                    }
                    let label = d.config.as_ref().map(|c| c.hostname.clone()).unwrap_or_else(|| match d.kind {
                        Kind::Lb => "LB-Nordvik".into(),
                        Kind::Wlc => "WLC-Nordvik".into(),
                        _ => if d.label.is_empty() { id.clone() } else { d.label.clone() },
                    });
                    let console = matches!(d.kind, Kind::Router | Kind::Switch | Kind::Wlc | Kind::Lb);
                    units.push(Unit { id: id.clone(), label, kind: Some(d.kind), ports, console });
                }
                None => units.push(Unit { id: id.clone(), label: id.clone(), kind: None, ports: vec![], console: false }),
            }
        }
        RackView { title: title.to_string(), units, closed: false, pick: None, info: String::new(), rects: vec![] }
    }

    pub fn update(&mut self, input: &Input) {
        if input.was_pressed(Key::Escape) || input.was_pressed(Key::Enter) {
            self.closed = true;
        }
        if input.clicked {
            let hit = self.rects.iter().find(|(r, _)| r.contains(input.mouse_x, input.mouse_y)).map(|(_, i)| *i);
            match hit {
                Some(i) if self.units[i].console => {
                    self.pick = Some(self.units[i].id.clone());
                    self.closed = true;
                }
                Some(_) => {}
                None => {
                    // klick på Stäng-området längst ned
                    if input.mouse_y > 650 {
                        self.closed = true;
                    }
                }
            }
        }
    }

    pub fn draw(&mut self, c: &mut Canvas, fonts: &mut Fonts, st: &mut State, mouse: (i32, i32), t: f64) {
        c.dim(0x000000, 170);
        let (sw, sh) = (c.w as i32, c.h as i32);
        let pw = 1180.min(sw - 20);
        let ph = sh - 30;
        let px = (sw - pw) / 2;
        let py = 15;
        panel(c, Rect::new(px, py, pw, ph), PANEL, BORDER);
        c.fill_rect(px, py, pw, 38, PANEL2);
        fonts.text(c, px + 16, py + 11, &self.title, TITLE, ACCENT, FontKind::Pixel);
        let hint = "Håll musen över en port · klicka på en enhet för konsolkabeln · Esc stänger";
        let hw = text_w(fonts, hint, SMALL, FontKind::Mono);
        fonts.text(c, px + pw - hw - 16, py + 10, hint, SMALL, MUTED, FontKind::Mono);
        let net = S::get(st);
        self.rects.clear();
        let mut y = py + 52;
        let mut info: Option<String> = None;
        let n = self.units.len().max(1) as i32;
        let uh = ((ph - 52 - 60) / n).min(96).max(52);
        for (i, u) in self.units.iter().enumerate() {
            let r = Rect::new(px + 20, y, pw - 40, uh - 6);
            let hover = r.contains(mouse.0, mouse.1);
            let base = match u.kind {
                Some(Kind::Router) => rgb(0xcf, 0xd2, 0xcf),
                Some(Kind::Switch) => rgb(0x4c, 0x5b, 0x6b),
                Some(Kind::Wlc) => rgb(0xb9, 0xbc, 0xbf),
                Some(Kind::Lb) => rgb(0x24, 0x34, 0x47),
                _ => rgb(0x1d, 0x20, 0x24),
            };
            c.fill_rect(r.x, r.y, r.w, r.h, base);
            c.rect(r.x, r.y, r.w, r.h, if hover && u.console { 3 } else { 1 }, if hover && u.console { ACCENT } else { rgb(0x10, 0x14, 0x1a) });
            let dark = matches!(u.kind, Some(Kind::Router) | Some(Kind::Wlc));
            let tc = if dark { rgb(0x1d, 0x22, 0x26) } else { TEXT };
            fonts.text(c, r.x + 12, r.y + 8, &u.label, BODY, tc, FontKind::Mono);
            if u.console {
                fonts.text(c, r.x + 12, r.y + 8 + line_h(BODY), "klicka: konsol", SMALL, if dark { rgb(0x44, 0x4a, 0x50) } else { MUTED }, FontKind::Mono);
            }
            // portar
            if !u.ports.is_empty() {
                let cell = 36;
                let per_row = (u.ports.len() as i32 + 1) / 2;
                let per_row = if u.ports.len() <= 6 { u.ports.len() as i32 } else { per_row };
                let start_x = r.x + 260.max(r.w - per_row * cell - 20);
                for (k, p) in u.ports.iter().enumerate() {
                    let row = if u.ports.len() <= 6 { 0 } else { k as i32 % 2 };
                    let col = if u.ports.len() <= 6 { k as i32 } else { k as i32 / 2 };
                    let (x, yy) = (start_x + col * cell, r.y + 8 + row * ((r.h - 16) / 2));
                    let led = port_led(st, Some(&net), &u.id, p, t);
                    c.fill_rect(x, yy + 12, 26, 14, rgb(0x0a, 0x0c, 0x10));
                    c.fill_rect(x + 2, yy + 14, 22, 10, rgb(0x1a, 0x1e, 0x24));
                    c.fill_rect(x + 9, yy, 8, 8, led);
                    let pr = Rect::new(x, yy, 26, 28);
                    if pr.contains(mouse.0, mouse.1) {
                        c.rect(x - 1, yy - 1, 28, 30, 1, ACCENT);
                        info = Some(port_info(st, &net, &u.id, &u.label, p));
                    }
                }
            }
            self.rects.push((r, i));
            y += uh;
        }
        self.info = info.unwrap_or_else(|| "Håll musen över en port för status".into());
        let by = py + ph - 44;
        c.fill_rect(px + 10, by, pw - 20, 34, rgb(0x0b, 0x0d, 0x16));
        fonts.text(c, px + 20, by + 6, &self.info, BODY, TEXT, FontKind::Mono);
    }
}

fn port_info(st: &State, net: &S::Derived, dev: &str, label: &str, port: &str) -> String {
    let short = U::short_if(port);
    let d = &st.devices[dev];
    let mut s = format!("{} {}: ", label, short);
    if let Some(c) = &d.config {
        if d.rt.errdisabled.contains_key(port) {
            return s + "err-disabled";
        }
        if let Some(i) = c.ifaces.get(port) {
            if i.shutdown {
                return s + "administratively down (shutdown)";
            }
        }
    }
    match net.ports.get(&S::key(dev, port)) {
        Some(p) if p.up => s += "upp",
        Some(_) => s += "ingen länk (notconnect)",
        None => s += "okänd",
    }
    s
}
