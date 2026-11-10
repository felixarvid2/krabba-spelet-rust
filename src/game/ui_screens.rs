//! Huvudmenyn, HUD och notiser.
use super::ui_widgets::*;
use super::{App, MenuAct, ToastKind};
use crate::engine::{Canvas, FontKind, Fonts, Input, Key, rgb};

impl App {
    // ------------------------------------------------------------------ meny

    pub fn update_menu(&mut self, input: &Input) {
        if !self.overlays.is_empty() {
            self.update_overlays_pub(input);
            return;
        }
        for k in &input.pressed {
            let n = match k {
                Key::Num1 => Some(1),
                Key::Num2 => Some(2),
                Key::Num3 => Some(3),
                Key::Num4 => Some(4),
                Key::Num5 => Some(5),
                Key::Num6 => Some(6),
                Key::Num7 => Some(7),
                Key::Num8 => Some(8),
                Key::Num9 => Some(9),
                Key::Num0 => Some(10),
                _ => None,
            };
            if let Some(n) = n {
                self.menu_act(MenuAct::Start(n, false));
                return;
            }
            let len = self.menu_hits.len();
            if len == 0 {
                continue;
            }
            match k {
                Key::Right => self.menu_sel = (self.menu_sel + 1) % len,
                Key::Left => self.menu_sel = (self.menu_sel + len - 1) % len,
                Key::Down => self.menu_sel = (self.menu_sel + 5).min(len - 1),
                Key::Up => self.menu_sel = self.menu_sel.saturating_sub(5),
                Key::Enter | Key::Space => {
                    let a = self.menu_hits[self.menu_sel.min(len - 1)].1.clone();
                    self.menu_act(a);
                    return;
                }
                Key::Escape => {}
                _ => {}
            }
        }
        if input.clicked {
            let hit = self.menu_hits.iter().position(|(r, _)| r.contains(input.mouse_x, input.mouse_y));
            if let Some(i) = hit {
                self.menu_sel = i;
                let a = self.menu_hits[i].1.clone();
                self.menu_act(a);
            }
        }
    }

    pub fn menu_act(&mut self, a: MenuAct) {
        match a {
            MenuAct::Start(n, exam) => {
                if self.weeks().iter().any(|w| w.week == n) {
                    self.start_week(n, exam);
                }
            }
            MenuAct::Free => self.start_week(0, false),
            MenuAct::Exam => {
                let ws: Vec<u32> = self.weeks().iter().filter(|w| w.build.is_none()).map(|w| w.week).collect();
                if !ws.is_empty() {
                    let ix = (self.frame as usize * 7 + (self.time * 1000.0) as usize) % ws.len();
                    self.start_week(ws[ix], true);
                }
            }
            MenuAct::Continue => {
                if self.save.run.is_some() && self.resume_run() {
                    return;
                }
                if let Some(n) = self.save.last_week {
                    self.start_week(n, false);
                }
            }
            MenuAct::Settings => {
                let d = self.settings_dialog();
                self.push_dlg(d);
            }
            MenuAct::Career => self.overlays.push(super::Overlay::Career(super::ui_career::CareerView::new())),
            MenuAct::Quit => self.quit = true,
        }
    }

    pub fn draw_menu(&mut self, c: &mut Canvas, fonts: &mut Fonts) {
        c.clear(BG);
        let weeks = self.weeks().clone();
        let w = c.w as i32;
        // bakgrundsmönster
        for i in 0..24 {
            let x = (i * 61 + (self.time * 8.0) as i32) % (w + 40) - 20;
            c.fill_rect_a(x, 0, 2, c.h as i32, rgb(0x20, 0x26, 0x44), 120);
        }
        let title = "Krabba spelet";
        let tw = text_w(fonts, title, 34.0, FontKind::Pixel);
        fonts.text(c, (w - tw) / 2 + 3, 23, title, 34.0, rgb(0x6a, 0x20, 0x20), FontKind::Pixel);
        fonts.text(c, (w - tw) / 2, 20, title, 34.0, ACCENT, FontKind::Pixel);
        let sub = "Du är nätverkstekniker på Nordvik. Felsök veckans fel eller bygg upp nätet från fabriksinställda lådor.";
        let sw = text_w(fonts, sub, SMALL, FontKind::Mono);
        fonts.text(c, (w - sw) / 2, 68, sub, SMALL, MUTED, FontKind::Mono);

        let mut hits: Vec<(Rect, MenuAct)> = vec![];
        let mx = 40;
        let cw = (w - 2 * mx - 4 * 12) / 5;
        let ch = 58;
        let mut y = 98;
        let sections: Vec<(&str, Vec<&super::levels::Week>)> = vec![
            ("Kurs 1: Nätverksteknik (vecka 1-10, siffertangent 1-9 startar)", weeks.iter().filter(|x| x.build.is_none() && x.kurs == 1).collect()),
            ("Bygg från grunden (byggen 1-5)", weeks.iter().filter(|x| x.build.is_some()).collect()),
            ("Kurs 2: Routing och switching (vecka 11-14)", weeks.iter().filter(|x| x.build.is_none() && x.kurs == 2).collect()),
        ];
        let sel = self.menu_sel;
        let mouse = self.mouse;
        for (label, list) in sections {
            if list.is_empty() {
                continue;
            }
            fonts.text(c, mx, y, label, 12.0, ACCENT, FontKind::Pixel);
            y += 22;
            for (i, wk) in list.iter().enumerate() {
                let col = (i % 5) as i32;
                let row = (i / 5) as i32;
                let r = Rect::new(mx + col * (cw + 12), y + row * (ch + 8), cw, ch);
                let idx = hits.len();
                hits.push((r, MenuAct::Start(wk.week, false)));
                let hover = r.contains(mouse.0, mouse.1);
                let focus = idx == sel;
                let stars = self.save.stars(wk.week);
                c.fill_rect(r.x, r.y, r.w, r.h, if hover { rgb(0x2c, 0x34, 0x58) } else { PANEL });
                c.rect(r.x, r.y, r.w, r.h, if focus { 3 } else { 1 }, if focus { ACCENT } else { BORDER });
                let head = match wk.build {
                    Some(b) => format!("Bygge {}", b),
                    None => format!("Vecka {}", wk.week),
                };
                fonts.text(c, r.x + 8, r.y + 7, &head, 10.0, if stars > 0 { GOOD } else { LINK }, FontKind::Pixel);
                text_fit(c, fonts, r.x + 8, r.y + 26, wk.title, BODY - 2.0, TEXT, FontKind::Mono, r.w - 16);
                for s in 0..3 {
                    let sx = r.x + r.w - 18 - s * 16;
                    let on = (2 - s) < stars as i32;
                    c.circle(sx, r.y + 14, 5, if on { ACCENT } else { rgb(0x3a, 0x40, 0x60) }, 255);
                }
            }
            y += ((list.len() as i32 + 4) / 5) * (ch + 8) + 8;
        }
        // knappar
        y += 6;
        let mut bx = mx;
        let cont_label = match (&self.save.run, self.save.last_week) {
            (Some(r), _) => format!("Fortsätt {}", super::levels::name(weeks.iter().find(|x| x.week == r.week)).to_lowercase()),
            (None, Some(n)) => format!("Fortsätt {}", super::levels::name(weeks.iter().find(|x| x.week == n)).to_lowercase()),
            _ => "Fortsätt".to_string(),
        };
        let can_cont = self.save.run.is_some() || self.save.last_week.is_some();
        let btns: Vec<(String, MenuAct, bool)> = vec![
            (cont_label, MenuAct::Continue, true),
            ("Fri träning".into(), MenuAct::Free, false),
            ("Examen".into(), MenuAct::Exam, false),
            ("Karriär".into(), MenuAct::Career, false),
            ("Inställningar".into(), MenuAct::Settings, false),
            ("Avsluta".into(), MenuAct::Quit, false),
        ];
        for (label, act, primary) in btns {
            let bw = text_w(fonts, &label, BODY, FontKind::Mono) + 40;
            let r = Rect::new(bx, y, bw, 42);
            let idx = hits.len();
            let enabled = act != MenuAct::Continue || can_cont;
            if enabled {
                hits.push((r, act));
            }
            let hover = enabled && r.contains(mouse.0, mouse.1);
            button(c, fonts, r, &label, primary && enabled, hover, enabled && idx == sel);
            if !enabled {
                c.fill_rect_a(r.x, r.y, r.w, r.h, BG, 150);
            }
            bx += bw + 12;
        }
        y += 62;
        let tip = handbook_tip(self.tip);
        fonts.text(c, mx, y, &format!("Tips: {}", tip), SMALL, GOOD, FontKind::Mono);
        let done = self.save.progress.values().filter(|p| p.stars > 0).count();
        fonts.text(c, mx, c.h as i32 - 30, &format!("{} veckor klarade · Piltangenter + Enter eller klick väljer · 1-9 startar vecka · Esc i spelet: paus", done), SMALL, MUTED, FontKind::Mono);
        self.menu_hits = hits;
        if self.menu_sel >= self.menu_hits.len() {
            self.menu_sel = 0;
        }
    }

    // ------------------------------------------------------------------ HUD

    pub fn draw_hud(&mut self, c: &mut Canvas, fonts: &mut Fonts) {
        self.hud_report_rect = Rect::default();
        let run = match &self.run {
            Some(r) => r,
            None => return,
        };
        let pend = self.pending_reports().len();
        let w = 430;
        let mut y = 10;
        let x = 10;
        let mut lines: Vec<(String, super::levels::Task, usize)> = vec![];
        let _ = &mut lines;
        // titelpanel
        let title = match &run.week {
            Some(wk) => format!("{} · {}", super::levels::name(Some(wk)), wk.title),
            None => "Fri träning".to_string(),
        };
        let mut rows: Vec<(String, u32)> = vec![];
        let place = run.world.zone_name();
        let clock = if run.st.devices.contains_key("R1") { crate::net::sim::device_clock(&mut self.run.as_mut().unwrap().st, "R1").hms } else { String::new() };
        let run = self.run.as_ref().unwrap();
        let hard = self.save.settings.difficulty == "hard";
        let sub = format!("{} · {} · {}{}", place, clock.chars().take(5).collect::<String>(), esc_time(run.elapsed), if run.exam { " · EXAMEN" } else { "" });
        let mut task_lines: Vec<(String, Rgb)> = vec![];
        if let Some(wk) = &run.week {
            for (i, t) in wk.tasks.iter().enumerate() {
                let s = &run.ts[i];
                let (mark, col) = if s.reported { ("[x]", GOOD) } else if s.fixed { ("[~]", ACCENT) } else { ("[ ]", TEXT) };
                let who = if t.build { "" } else if t.krabba { "Krabba: " } else { "Ärende: " };
                let name = if s.known { t.title.to_string() } else if t.krabba { "Okänt Krabba-fel".to_string() } else { t.title.to_string() };
                let extra = if s.fixed && !s.reported { "  – skriv felrapport (F)" } else { "" };
                task_lines.push((format!("{} {}{}{}", mark, who, name, extra), col));
            }
            if !run.exam {
                if let Some(t) = super::levels::challenge_title(run.n) {
                    let done = self.challenge_marked(run.n);
                    task_lines.push((format!("{} Utmaning: {}", if done { "[x]" } else { "[ ]" }, t), if done { GOOD } else { MUTED }));
                }
            }
        }
        let _ = &mut rows;
        // höjd
        let mut hh = 38 + line_h(SMALL) + 6 + line_h(SMALL) + 14;
        let mut wrapped: Vec<(Vec<String>, Rgb)> = vec![];
        for (t, col) in &task_lines {
            let ls = fonts.wrap(t, SMALL, FontKind::Mono, w - 24);
            hh += ls.len() as i32 * line_h(SMALL);
            wrapped.push((ls, *col));
        }
        hh += 8;
        c.fill_rect_a(x, y, w, hh, rgb(0x0c, 0x0e, 0x18), 215);
        c.rect(x, y, w, hh, 1, BORDER);
        text_fit(c, fonts, x + 10, y + 9, &title, 11.0, ACCENT, FontKind::Pixel, w - if hard { 80 } else { 20 });
        y += 30;
        fonts.text(c, x + 10, y, &sub, SMALL, MUTED, FontKind::Mono);
        y += line_h(SMALL) + 6;
        super::ui_career::draw_xp_bar(c, fonts, x + 12, y, w - 24, &self.save.career);
        y += line_h(SMALL) + 14;
        if hard {
            let tw = text_w(fonts, "SVÅR", 18.0, FontKind::Mono);
            c.fill_rect(x + w - tw - 24, 8 + 6, tw + 12, 20, rgb(0x8a, 0x1f, 0x1f));
            fonts.text(c, x + w - tw - 18, 8 + 7, "SVÅR", 18.0, TEXT, FontKind::Mono);
        }
        for (ls, col) in &wrapped {
            for l in ls {
                fonts.text(c, x + 12, y, l, SMALL, *col, FontKind::Mono);
                y += line_h(SMALL);
            }
        }
        y = 10 + hh + 6;
        if let Some(t) = &run.console_target {
            let nm = run.st.devices.get(t).map(|d| d.config.as_ref().map(|c| c.hostname.clone()).unwrap_or_else(|| t.clone())).unwrap_or_default();
            fonts.text(c, x + 4, y, &format!("Konsolkabel i {}", nm), SMALL, LINK, FontKind::Mono);
            y += line_h(SMALL) + 4;
        }
        // gula rutan
        if pend > 0 && !hard {
            let txt = format!("{} felrapport{} att skicka in – tryck F eller klicka här", pend, if pend == 1 { "" } else { "er" });
            let ls = fonts.wrap(&txt, SMALL, FontKind::Mono, w - 20);
            let bh = ls.len() as i32 * line_h(SMALL) + 12;
            c.fill_rect(x, y, w, bh, rgb(0xff, 0xd5, 0x4a));
            c.rect(x, y, w, bh, 2, rgb(0x9a, 0x6a, 0x00));
            for (i, l) in ls.iter().enumerate() {
                fonts.text(c, x + 10, y + 6 + i as i32 * line_h(SMALL), l, SMALL, rgb(0x2a, 0x1a, 0x00), FontKind::Mono);
            }
            self.hud_report_rect = Rect::new(x, y, w, bh);
            y += bh + 6;
        }
        let guide = self.guide_text();
        if let Some(g) = guide {
            let ls = fonts.wrap(g, SMALL, FontKind::Mono, w - 20);
            let bh = ls.len() as i32 * line_h(SMALL) + 12;
            c.fill_rect_a(x, y, w, bh, rgb(0x1f, 0x4a, 0x8a), 230);
            c.rect(x, y, w, bh, 1, LINK);
            for (i, l) in ls.iter().enumerate() {
                fonts.text(c, x + 10, y + 6 + i as i32 * line_h(SMALL), l, SMALL, TEXT, FontKind::Mono);
            }
        }
        // tipsraden längst ned
        let hint = self.run.as_ref().and_then(|r| r.world.hint());
        let mut by = c.h as i32 - 14;
        if self.save.settings.show_keys {
            let keys = "WASD gå · E använd · T laptop · F felrapport · L ledtråd · H handbok · Tab ärenden · M karta · N anteckningar · U dölj · F1 hjälp · Esc paus";
            let kw = text_w(fonts, keys, SMALL, FontKind::Mono);
            by -= line_h(SMALL);
            c.fill_rect_a((c.w as i32 - kw) / 2 - 10, by - 2, kw + 20, line_h(SMALL) + 4, rgb(0x0c, 0x0e, 0x18), 190);
            fonts.text(c, (c.w as i32 - kw) / 2, by, keys, SMALL, MUTED, FontKind::Mono);
        }
        if let Some(h) = hint {
            let hw = text_w(fonts, &h, BODY + 2.0, FontKind::Mono);
            let bx = (c.w as i32 - hw) / 2;
            by -= line_h(BODY) + 12;
            c.fill_rect_a(bx - 14, by - 4, hw + 28, line_h(BODY) + 10, rgb(0x10, 0x14, 0x28), 235);
            c.rect(bx - 14, by - 4, hw + 28, line_h(BODY) + 10, 1, ACCENT);
            fonts.text(c, bx, by, &h, BODY + 2.0, ACCENT, FontKind::Mono);
        }
    }

    pub fn draw_toasts(&mut self, c: &mut Canvas, fonts: &mut Fonts) {
        // Längst ner till höger, ovanför tangentraden, så att de inte skymmer rubriker
        let w = 440;
        let x = c.w as i32 - w - 12;
        let wrapped: Vec<Vec<String>> = self.toasts.iter().map(|t| fonts.wrap(&t.text, SMALL, FontKind::Mono, w - 20)).collect();
        let total: i32 = wrapped.iter().map(|ls| ls.len() as i32 * line_h(SMALL) + 18).sum();
        // Med en dialog öppen hamnar notiserna överst så att de inte täcker knapparna
        // Med en dialog öppen väntar notiserna tills den stängs
        if !self.overlays.is_empty() {
            return;
        }
        let mut y = c.h as i32 - 48 - total;
        for (t, ls) in self.toasts.iter().zip(wrapped.iter()) {
            let h = ls.len() as i32 * line_h(SMALL) + 12;
            let a = if t.ttl < 0.6 { (t.ttl / 0.6 * 235.0) as u8 } else { 235 };
            let (bg, bd) = match t.kind {
                ToastKind::Info => (rgb(0x16, 0x1a, 0x30), BORDER),
                ToastKind::Good => (rgb(0x12, 0x30, 0x20), GOOD),
                ToastKind::Warn => (rgb(0x3a, 0x26, 0x10), WARN),
            };
            c.fill_rect_a(x, y, w, h, bg, a);
            c.rect(x, y, w, h, 1, bd);
            for (i, l) in ls.iter().enumerate() {
                fonts.text(c, x + 10, y + 6 + i as i32 * line_h(SMALL), l, SMALL, TEXT, FontKind::Mono);
            }
            y += h + 6;
        }
    }

    pub fn update_overlays_pub(&mut self, input: &Input) {
        self.update_overlays(input);
    }
}

type Rgb = u32;

fn handbook_tip(i: usize) -> &'static str {
    super::handbook_data::TIPS[i % super::handbook_data::TIPS.len()]
}
