//! Dialogrutor: text, tabeller, textfält, radioval, kryssrutor och knappar med tangentbords- och musstyrning.
use super::ui_widgets::*;
use crate::engine::{Canvas, FontKind, Fonts, Input, Key};

/// Vad en knapp gör. App tolkar dem (se App::run_act).
#[derive(Clone, Debug, PartialEq)]
pub enum Act {
    Close,
    Menu,
    StartWeek(u32, bool),
    ConfirmStart(u32, bool),
    OpenHint(usize),
    NextHint(usize),
    HintPicker,
    ReportPick(usize),
    ReportSubmit(usize),
    ViewReport(usize),
    Tasks,
    CopyReport,
    OpenWin(String),
    OpenIpv4(String),
    ApplyIpv4(String),
    ReplaceCable(String),
    SetCable(String, bool),
    ConnectConsole(String),
    Travel(String),
    QuizStart(u32),
    QuizAnswer,
    QuizNext,
    SettingsOpen,
    SettingsApply,
    NotesSave,
    Whiteboard,
    Briefing,
    PhoneJoin(String),
    PhoneWeb,
    PhoneScan,
    Phone,
    OpenLaptop,
    ReportNow,
    ResetProgress,
    Quit,
    Continue,
    Handbook,
    Help,
    MapOpen,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tone {
    Normal,
    Muted,
    Quote,
    Good,
    Bad,
    Head,
    Code,
    Big,
}

#[derive(Clone, Debug)]
pub enum Item {
    Text(String, Tone),
    Table(Vec<Vec<String>>),
    Gap(i32),
    Input { id: &'static str, label: String, value: String, rows: usize, hint: String, cur: usize },
    Radio { id: &'static str, legend: String, options: Vec<String>, sel: Option<usize> },
    Check { id: &'static str, label: String, on: bool },
    Choice { id: &'static str, label: String, options: Vec<String>, sel: usize },
}

#[derive(Clone, Debug)]
pub struct Btn {
    pub label: String,
    pub primary: bool,
    pub act: Act,
    /// Dialogen stängs inte av knappen
    pub keep: bool,
}

impl Btn {
    pub fn new(label: &str, act: Act) -> Btn {
        Btn { label: label.into(), primary: false, act, keep: false }
    }
    pub fn primary(label: &str, act: Act) -> Btn {
        Btn { label: label.into(), primary: true, act, keep: false }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Hit {
    Btn(usize),
    Item(usize),
    Opt(usize, usize),
    ChoiceDec(usize),
    ChoiceInc(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub enum DlgOut {
    None,
    /// Esc eller klick utanför: stäng
    Cancel,
    /// Knapp nummer i trycktes
    Press(usize),
}

#[derive(Clone, Debug)]
pub struct Dialog {
    pub title: String,
    pub wide: bool,
    pub items: Vec<Item>,
    pub buttons: Vec<Btn>,
    pub focus: usize,
    pub scroll: i32,
    /// Felmeddelande under formuläret
    pub error: String,
    hits: Vec<(Rect, Hit)>,
    focus_rect: Option<Rect>,
    max_scroll: i32,
    pub body_h: i32,
    /// Data som dialogen bär (t.ex. vilket ärende)
    pub tag: usize,
}

impl Dialog {
    pub fn new(title: &str) -> Dialog {
        Dialog { title: title.into(), wide: false, items: vec![], buttons: vec![], focus: usize::MAX, scroll: 0, error: String::new(), hits: vec![], focus_rect: None, max_scroll: 0, body_h: 0, tag: 0 }
    }
    pub fn wide(mut self) -> Dialog {
        self.wide = true;
        self
    }
    pub fn text(mut self, s: impl Into<String>) -> Dialog {
        self.items.push(Item::Text(s.into(), Tone::Normal));
        self
    }
    pub fn tone(mut self, s: impl Into<String>, t: Tone) -> Dialog {
        self.items.push(Item::Text(s.into(), t));
        self
    }
    pub fn item(mut self, i: Item) -> Dialog {
        self.items.push(i);
        self
    }
    pub fn btn(mut self, b: Btn) -> Dialog {
        self.buttons.push(b);
        self
    }
    pub fn tag(mut self, t: usize) -> Dialog {
        self.tag = t;
        self
    }

    // ----- värden
    pub fn input(&self, id: &str) -> String {
        for it in &self.items {
            if let Item::Input { id: i, value, .. } = it {
                if *i == id {
                    return value.clone();
                }
            }
        }
        String::new()
    }
    pub fn radio(&self, id: &str) -> Option<usize> {
        for it in &self.items {
            if let Item::Radio { id: i, sel, .. } = it {
                if *i == id {
                    return *sel;
                }
            }
        }
        None
    }
    pub fn checked(&self, id: &str) -> bool {
        for it in &self.items {
            if let Item::Check { id: i, on, .. } = it {
                if *i == id {
                    return *on;
                }
            }
        }
        false
    }
    pub fn choice(&self, id: &str) -> usize {
        for it in &self.items {
            if let Item::Choice { id: i, sel, .. } = it {
                if *i == id {
                    return *sel;
                }
            }
        }
        0
    }

    /// Fokusordning: först interaktiva poster, sedan knapparna
    fn focusables(&self) -> Vec<Hit> {
        let mut v = vec![];
        for (i, it) in self.items.iter().enumerate() {
            match it {
                Item::Input { .. } | Item::Radio { .. } | Item::Check { .. } | Item::Choice { .. } => v.push(Hit::Item(i)),
                _ => {}
            }
        }
        for j in 0..self.buttons.len() {
            v.push(Hit::Btn(j));
        }
        v
    }

    fn init_focus(&mut self) {
        if self.focus != usize::MAX {
            return;
        }
        let f = self.focusables();
        // första textfältet om det finns, annars primärknappen
        let first_item = f.iter().position(|h| matches!(h, Hit::Item(_)));
        let prim = self.buttons.iter().position(|b| b.primary);
        self.focus = match (first_item, prim) {
            (Some(i), _) => i,
            (None, Some(p)) => f.len() - self.buttons.len() + p,
            _ => 0,
        };
    }

    fn primary_idx(&self) -> Option<usize> {
        self.buttons.iter().position(|b| b.primary).or(if self.buttons.is_empty() { None } else { Some(0) })
    }

    pub fn update(&mut self, input: &Input) -> DlgOut {
        self.init_focus();
        let f = self.focusables();
        if f.is_empty() {
            return DlgOut::Cancel;
        }
        if self.focus >= f.len() {
            self.focus = f.len() - 1;
        }
        // mushjul
        if input.wheel != 0.0 {
            self.scroll = (self.scroll - (input.wheel * 40.0) as i32).clamp(0, self.max_scroll);
        }
        // klick
        if input.clicked {
            let (mx, my) = (input.mouse_x, input.mouse_y);
            let hit = self.hits.iter().rev().find(|(r, _)| r.contains(mx, my)).cloned();
            if let Some((r, h)) = hit {
                if let Some(pos) = f.iter().position(|x| *x == h || matches!((x, &h), (Hit::Item(a), Hit::Opt(b, _)) if a == b) || matches!((x, &h), (Hit::Item(a), Hit::ChoiceDec(b)) if a == b) || matches!((x, &h), (Hit::Item(a), Hit::ChoiceInc(b)) if a == b)) {
                    self.focus = pos;
                }
                match h {
                    Hit::Btn(j) => return DlgOut::Press(j),
                    Hit::Opt(i, k) => {
                        if let Item::Radio { sel, .. } = &mut self.items[i] {
                            *sel = Some(k);
                        }
                    }
                    Hit::Item(i) => {
                        if let Item::Check { on, .. } = &mut self.items[i] {
                            *on = !*on;
                        }
                        if let Item::Input { value, cur, .. } = &mut self.items[i] {
                            *cur = value.chars().count();
                        }
                    }
                    Hit::ChoiceDec(i) => self.choice_step(i, -1),
                    Hit::ChoiceInc(i) => self.choice_step(i, 1),
                }
                let _ = r;
            }
        }
        let cur_focus = f[self.focus];
        // tecken
        if !input.ctrl {
            if let Hit::Item(i) = cur_focus {
                for ch in &input.chars {
                    if let Item::Input { value, cur, .. } = &mut self.items[i] {
                        if printable(*ch) {
                            edit_insert(value, cur, *ch);
                        }
                    }
                }
            }
        }
        for k in &input.pressed {
            match k {
                Key::Escape => return DlgOut::Cancel,
                Key::Tab => {
                    let n = f.len();
                    self.focus = if input.shift { (self.focus + n - 1) % n } else { (self.focus + 1) % n };
                }
                Key::PageDown => self.scroll = (self.scroll + 240).min(self.max_scroll),
                Key::PageUp => self.scroll = (self.scroll - 240).max(0),
                Key::Up | Key::Down => {
                    let d = if *k == Key::Down { 1 } else { -1 };
                    match cur_focus {
                        Hit::Item(i) => match &mut self.items[i] {
                            Item::Radio { options, sel, .. } => {
                                let n = options.len() as i32;
                                let cur = sel.map(|s| s as i32).unwrap_or(if d > 0 { -1 } else { n });
                                *sel = Some((cur + d).clamp(0, n - 1) as usize);
                            }
                            Item::Input { .. } | Item::Check { .. } | Item::Choice { .. } => {
                                self.focus = (self.focus as i32 + d).clamp(0, f.len() as i32 - 1) as usize;
                            }
                            _ => {}
                        },
                        Hit::Btn(_) => self.scroll = (self.scroll + d * 40).clamp(0, self.max_scroll),
                        _ => {}
                    }
                }
                Key::Left | Key::Right => {
                    let d = if *k == Key::Right { 1 } else { -1 };
                    match cur_focus {
                        Hit::Item(i) => match &mut self.items[i] {
                            Item::Choice { .. } => self.choice_step(i, d),
                            Item::Input { value, cur, .. } => {
                                let n = value.chars().count() as i32;
                                *cur = (*cur as i32 + d).clamp(0, n) as usize;
                            }
                            Item::Check { on, .. } => *on = d > 0,
                            _ => {}
                        },
                        Hit::Btn(_) => {
                            let n = f.len();
                            let nb = self.buttons.len();
                            let first_btn = n - nb;
                            let ni = (self.focus as i32 + d).clamp(first_btn as i32, n as i32 - 1);
                            self.focus = ni as usize;
                        }
                        _ => {}
                    }
                }
                Key::Home | Key::End => {
                    if let Hit::Item(i) = cur_focus {
                        if let Item::Input { value, cur, .. } = &mut self.items[i] {
                            *cur = if *k == Key::Home { 0 } else { value.chars().count() };
                        }
                    }
                }
                Key::Backspace => {
                    if let Hit::Item(i) = cur_focus {
                        if let Item::Input { value, cur, .. } = &mut self.items[i] {
                            edit_backspace(value, cur);
                        }
                    }
                }
                Key::Delete => {
                    if let Hit::Item(i) = cur_focus {
                        if let Item::Input { value, cur, .. } = &mut self.items[i] {
                            edit_delete(value, cur);
                        }
                    }
                }
                Key::Space => {
                    if let Hit::Item(i) = cur_focus {
                        if let Item::Check { on, .. } = &mut self.items[i] {
                            *on = !*on;
                        }
                    }
                    if let Hit::Btn(j) = cur_focus {
                        return DlgOut::Press(j);
                    }
                }
                Key::Enter => match cur_focus {
                    Hit::Btn(j) => return DlgOut::Press(j),
                    Hit::Item(i) => {
                        if let Item::Input { value, cur, rows, .. } = &mut self.items[i] {
                            if *rows > 1 {
                                edit_insert(value, cur, '\n');
                                continue;
                            }
                        }
                        if let Item::Radio { .. } | Item::Check { .. } | Item::Input { .. } | Item::Choice { .. } = &self.items[i] {
                            if let Some(p) = self.primary_idx() {
                                return DlgOut::Press(p);
                            }
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        DlgOut::None
    }

    fn choice_step(&mut self, i: usize, d: i32) {
        if let Item::Choice { options, sel, .. } = &mut self.items[i] {
            let n = options.len() as i32;
            *sel = ((*sel as i32 + d).rem_euclid(n.max(1))) as usize;
        }
    }

    pub fn draw(&mut self, c: &mut Canvas, fonts: &mut Fonts, mouse: (i32, i32), t: f64) {
        self.init_focus();
        let f = self.focusables();
        let screen_w = c.w as i32;
        let screen_h = c.h as i32;
        c.dim(rgb_black(), 150);
        let pw = if self.wide { 960.min(screen_w - 40) } else { 660.min(screen_w - 40) };
        let inner_w = pw - 40;
        // layout
        let mut y = 0;
        struct Place {
            i: usize,
            y: i32,
            h: i32,
        }
        let mut places: Vec<Place> = vec![];
        let mut docs: Vec<(usize, Doc)> = vec![];
        for (i, it) in self.items.iter().enumerate() {
            let h;
            match it {
                Item::Text(s, tone) => {
                    let mut d = Doc::new(inner_w);
                    let (size, col, kind) = match tone {
                        Tone::Normal => (BODY, TEXT, FontKind::Mono),
                        Tone::Muted => (SMALL, MUTED, FontKind::Mono),
                        Tone::Quote => (BODY + 2.0, ACCENT, FontKind::Mono),
                        Tone::Good => (BODY, GOOD, FontKind::Mono),
                        Tone::Bad => (BODY, BAD, FontKind::Mono),
                        Tone::Head => (13.0, ACCENT, FontKind::Pixel),
                        Tone::Code => (BODY, LINK, FontKind::Mono),
                        Tone::Big => (28.0, ACCENT, FontKind::Pixel),
                    };
                    d.text(fonts, s, size, col, kind, 0);
                    d.h += 6;
                    h = d.h;
                    docs.push((i, d));
                }
                Item::Table(rows) => {
                    let mut d = Doc::new(inner_w);
                    d.table(fonts, rows);
                    h = d.h;
                    docs.push((i, d));
                }
                Item::Gap(n) => h = *n,
                Item::Input { label, value, rows, .. } => {
                    let lines = if *rows > 1 { fonts.wrap(&(value.clone() + "_"), BODY, FontKind::Mono, inner_w - 16).len().max(*rows) } else { 1 };
                    let lab = if label.is_empty() { 0 } else { line_h(SMALL) + 2 };
                    h = lab + lines as i32 * line_h(BODY) + 14;
                }
                Item::Radio { legend, options, .. } => {
                    let mut hh = line_h(SMALL) + 6;
                    for o in options {
                        hh += fonts.wrap(o, BODY, FontKind::Mono, inner_w - 44).len() as i32 * line_h(BODY) + 8;
                    }
                    let _ = legend;
                    h = hh + 6;
                }
                Item::Check { .. } => h = line_h(BODY) + 12,
                Item::Choice { .. } => h = line_h(BODY) + 14,
            }
            places.push(Place { i, y, h });
            y += h;
        }
        if !self.error.is_empty() {
            y += line_h(BODY) + 6;
        }
        let content_h = y;
        let chrome = 56 + 64;
        let ph = (content_h + chrome + 16).min(screen_h - 40).max(200);
        let px = (screen_w - pw) / 2;
        let py = (screen_h - ph) / 2;
        let body = Rect::new(px + 20, py + 56, inner_w, ph - chrome);
        self.body_h = body.h;
        self.max_scroll = (content_h - body.h + 8).max(0);
        self.scroll = self.scroll.clamp(0, self.max_scroll);
        // fokuserad post i sikte
        if let Some(fr) = self.focus_rect.take() {
            if fr.y - self.scroll < 0 {
                self.scroll = (fr.y - 6).max(0);
            } else if fr.y + fr.h - self.scroll > body.h {
                self.scroll = (fr.y + fr.h - body.h + 6).min(self.max_scroll);
            }
        }
        panel(c, Rect::new(px, py, pw, ph), PANEL, BORDER);
        c.fill_rect(px, py, pw, 40, PANEL2);
        text_fit(c, fonts, px + 16, py + 12, &self.title, TITLE, ACCENT, FontKind::Pixel, pw - 32);
        c.set_clip(body.x - 4, body.y, body.w + 8, body.h);
        self.hits.clear();
        let focus_hit = if self.focus < f.len() { Some(f[self.focus]) } else { None };
        let mut new_focus_rect = None;
        for p in &places {
            let top = body.y + p.y - self.scroll;
            let it = self.items[p.i].clone();
            match it {
                Item::Text(..) | Item::Table(..) => {
                    if let Some((_, d)) = docs.iter().find(|(i, _)| *i == p.i) {
                        d.draw(c, fonts, body.x, top, body.y - 40, body.y + body.h + 40);
                    }
                }
                Item::Gap(_) => {}
                Item::Input { label, value, rows, hint, cur, .. } => {
                    let focused = focus_hit == Some(Hit::Item(p.i));
                    let mut yy = top;
                    if !label.is_empty() {
                        fonts.text(c, body.x, yy, &label, SMALL, MUTED, FontKind::Mono);
                        yy += line_h(SMALL) + 2;
                    }
                    let box_h = p.h - (yy - top) - 4;
                    let r = Rect::new(body.x, yy, inner_w, box_h);
                    c.fill_rect(r.x, r.y, r.w, r.h, rgb_dark());
                    c.rect(r.x, r.y, r.w, r.h, if focused { 2 } else { 1 }, if focused { ACCENT } else { BORDER });
                    let shown = if value.is_empty() && !focused { hint.clone() } else { value.clone() };
                    let col = if value.is_empty() { MUTED } else { TEXT };
                    if rows > 1 {
                        let mut txt = shown.clone();
                        if focused && !value.is_empty() {
                            let chars: Vec<char> = value.chars().collect();
                            let at = cur.min(chars.len());
                            txt = chars[..at].iter().collect::<String>() + "|" + &chars[at..].iter().collect::<String>();
                        } else if focused {
                            txt = "|".into();
                        }
                        let lines = fonts.wrap(&txt, BODY, FontKind::Mono, inner_w - 16);
                        for (k, l) in lines.iter().enumerate() {
                            fonts.text(c, r.x + 8, r.y + 6 + k as i32 * line_h(BODY), l, BODY, col, FontKind::Mono);
                        }
                    } else {
                        fonts.text(c, r.x + 8, r.y + 6, &shown, BODY, col, FontKind::Mono);
                        if focused && (t * 2.0) as i64 % 2 == 0 {
                            let chars: Vec<char> = value.chars().collect();
                            let at = cur.min(chars.len());
                            let pre: String = chars[..at].iter().collect();
                            let cw = text_w(fonts, &pre, BODY, FontKind::Mono);
                            c.fill_rect(r.x + 8 + cw, r.y + 6, 2, line_h(BODY) - 2, TEXT);
                        }
                    }
                    self.hits.push((r, Hit::Item(p.i)));
                    if focused {
                        new_focus_rect = Some(Rect::new(0, p.y, 0, p.h));
                    }
                }
                Item::Radio { legend, options, sel, .. } => {
                    let focused = focus_hit == Some(Hit::Item(p.i));
                    let mut yy = top;
                    fonts.text(c, body.x, yy, &legend, SMALL, ACCENT, FontKind::Mono);
                    yy += line_h(SMALL) + 6;
                    for (k, o) in options.iter().enumerate() {
                        let lines = fonts.wrap(o, BODY, FontKind::Mono, inner_w - 44);
                        let oh = lines.len() as i32 * line_h(BODY) + 8;
                        let r = Rect::new(body.x, yy, inner_w, oh);
                        let hover = r.contains(mouse.0, mouse.1);
                        if hover {
                            c.fill_rect(r.x, r.y, r.w, r.h, rgb_hover());
                        }
                        let on = sel == Some(k);
                        c.circle(r.x + 14, r.y + 4 + line_h(BODY) / 2, 8, BORDER, 255);
                        c.circle(r.x + 14, r.y + 4 + line_h(BODY) / 2, 6, rgb_dark(), 255);
                        if on {
                            c.circle(r.x + 14, r.y + 4 + line_h(BODY) / 2, 4, ACCENT, 255);
                        }
                        for (li, l) in lines.iter().enumerate() {
                            fonts.text(c, r.x + 34, r.y + 4 + li as i32 * line_h(BODY), l, BODY, if on { TEXT } else { rgb_soft() }, FontKind::Mono);
                        }
                        if focused && on {
                            c.rect(r.x, r.y, r.w, r.h, 1, ACCENT);
                        }
                        self.hits.push((r, Hit::Opt(p.i, k)));
                        yy += oh;
                    }
                    if focused {
                        new_focus_rect = Some(Rect::new(0, p.y, 0, p.h));
                    }
                }
                Item::Check { label, on, .. } => {
                    let focused = focus_hit == Some(Hit::Item(p.i));
                    let r = Rect::new(body.x, top, inner_w, p.h - 2);
                    if r.contains(mouse.0, mouse.1) {
                        c.fill_rect(r.x, r.y, r.w, r.h, rgb_hover());
                    }
                    c.fill_rect(r.x + 4, r.y + 4, 20, 20, rgb_dark());
                    c.rect(r.x + 4, r.y + 4, 20, 20, if focused { 2 } else { 1 }, if focused { ACCENT } else { BORDER });
                    if on {
                        c.fill_rect(r.x + 9, r.y + 9, 10, 10, GOOD);
                    }
                    fonts.text(c, r.x + 36, r.y + 4, &label, BODY, TEXT, FontKind::Mono);
                    self.hits.push((r, Hit::Item(p.i)));
                    if focused {
                        new_focus_rect = Some(Rect::new(0, p.y, 0, p.h));
                    }
                }
                Item::Choice { label, options, sel, .. } => {
                    let focused = focus_hit == Some(Hit::Item(p.i));
                    fonts.text(c, body.x, top + 3, &label, BODY, TEXT, FontKind::Mono);
                    let bw = 300.min(inner_w / 2);
                    let bx = body.x + inner_w - bw;
                    let r = Rect::new(bx, top, bw, p.h - 2);
                    c.fill_rect(r.x, r.y, r.w, r.h, rgb_dark());
                    c.rect(r.x, r.y, r.w, r.h, if focused { 2 } else { 1 }, if focused { ACCENT } else { BORDER });
                    let name = options.get(sel).cloned().unwrap_or_default();
                    let tw = text_w(fonts, &name, BODY, FontKind::Mono);
                    fonts.text(c, r.x + (r.w - tw) / 2, r.y + 4, &name, BODY, TEXT, FontKind::Mono);
                    fonts.text(c, r.x + 8, r.y + 4, "<", BODY, ACCENT, FontKind::Mono);
                    fonts.text(c, r.x + r.w - 20, r.y + 4, ">", BODY, ACCENT, FontKind::Mono);
                    self.hits.push((Rect::new(r.x, r.y, r.w / 4, r.h), Hit::ChoiceDec(p.i)));
                    self.hits.push((Rect::new(r.x + r.w * 3 / 4, r.y, r.w / 4, r.h), Hit::ChoiceInc(p.i)));
                    if focused {
                        new_focus_rect = Some(Rect::new(0, p.y, 0, p.h));
                    }
                }
            }
        }
        if !self.error.is_empty() {
            let ey = body.y + content_h - line_h(BODY) - 6 - self.scroll;
            fonts.text(c, body.x, ey, &self.error, BODY, BAD, FontKind::Mono);
        }
        c.reset_clip();
        self.focus_rect = new_focus_rect;
        // rullningslist
        if self.max_scroll > 0 {
            let th = (body.h * body.h / (content_h + 8)).max(24);
            let ty = body.y + (body.h - th) * self.scroll / self.max_scroll.max(1);
            c.fill_rect(px + pw - 10, body.y, 4, body.h, rgb_dark());
            c.fill_rect(px + pw - 10, ty, 4, th, BORDER);
        }
        // knappar
        let n = self.buttons.len() as i32;
        if n > 0 {
            let gap = 10;
            let mut bws = vec![];
            let mut total = 0;
            for b in &self.buttons {
                let w = (text_w(fonts, &b.label, BODY, FontKind::Mono) + 36).max(110);
                bws.push(w);
                total += w;
            }
            total += gap * (n - 1);
            let mut bx = px + pw - 20 - total;
            if bx < px + 10 {
                bx = px + 10;
            }
            let by = py + ph - 56;
            for (j, b) in self.buttons.iter().enumerate() {
                let r = Rect::new(bx, by, bws[j], 40);
                let focused = focus_hit == Some(Hit::Btn(j));
                button(c, fonts, r, &b.label, b.primary, r.contains(mouse.0, mouse.1), focused);
                self.hits.push((r, Hit::Btn(j)));
                bx += bws[j] + gap;
            }
        }
    }
}

fn rgb_black() -> u32 {
    0x000000
}
fn rgb_dark() -> u32 {
    0x0b0d16
}
fn rgb_hover() -> u32 {
    0x262b45
}
fn rgb_soft() -> u32 {
    0xc4c9e0
}
