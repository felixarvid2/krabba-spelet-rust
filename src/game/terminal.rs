//! Terminalen: sessionsstack (laptop -> screen -> IOS, ssh, Windows), strömmad utskrift och tangenter.
//! Port av js/ui/terminal.js.
use super::levels::Week;
use super::ui_widgets::*;
use crate::cli::host::HostSession;
use crate::cli::ios::IosSession;
use crate::cli::lb::LbSession;
use crate::cli::wlc::WlcSession;
use crate::cli::{Chunk, Cli, CmdResult, PushReq};
use crate::engine::{Canvas, Color, FontKind, Fonts, Input, Key, rgb};
use crate::net::model::Os;
use crate::net::sim::{self as S, State};
use std::collections::{HashMap, VecDeque};

const GARBAGE: &str = "ÿþ~€¿x§¤ÆØ¶ßð½±¬¦";
const MAX_LINES: usize = 3000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EKind {
    Linux,
    Serial,
    Ssh,
    Win,
}

pub struct Entry {
    pub kind: EKind,
    pub session: Box<dyn Cli>,
    pub dev: String,
    pub title: String,
    pub speed: u32,
    pub ios: bool,
    pub started: bool,
    pub garbled: bool,
    pub wait_return: bool,
    pub hist_key: String,
}

/// Det terminalen behöver från spelet.
pub struct TermEnv<'a> {
    pub st: &'a mut State,
    pub consoles: &'a mut HashMap<String, Box<dyn Cli>>,
    pub console_target: Option<String>,
    pub week: Option<&'a Week>,
    pub week_n: u32,
    pub hist: &'a mut HashMap<String, Vec<String>>,
    pub fast: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TermEvent {
    /// Ett kommando kördes (enhet, rad)
    Command(String, String),
    /// Klart med ett kommando: räkna om och kolla uppgifterna
    AfterCommand,
    /// Konsolsessionen mot en enhet öppnades
    ConsoleConnected(String),
}

struct Finish {
    clear: bool,
    push: Option<PushReq>,
    close: bool,
    wait_return: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Cls {
    Plain,
    Err,
    Log,
    Warn,
    Bad,
    Ok,
    Mac,
    Ip,
    If,
}

pub struct Terminal {
    pub open: bool,
    pub maxed: bool,
    stack: Vec<Entry>,
    lines: Vec<String>,
    input: Vec<char>,
    cur: usize,
    hist_idx: HashMap<String, usize>,
    scroll: i32,
    busy: bool,
    chunks: VecDeque<Chunk>,
    wait: f64,
    wait_started: bool,
    finish: Option<Finish>,
    abort: bool,
    ctrl_a: bool,
    last_tab: f64,
    time: f64,
    ghost: String,
    ghost_dirty: bool,
    rng: u64,
    cols: usize,
    rows: usize,
    pub clip: Option<String>,
    pub font: f32,
    pub theme: String,
    /// Fönstrets rektangel från senaste ritningen (för musklick)
    chip_rects: Vec<(Rect, String)>,
    btn_close: Rect,
    btn_disc: Rect,
    btn_max: Rect,
    btn_copy: Rect,
    flash: f64,
}

fn motd(st: &State, dev: &str) -> Option<String> {
    st.devices.get(dev).and_then(|d| d.config.as_ref()).and_then(|c| c.banners.get("motd")).map(|b| b.text.clone())
}

fn host_name(st: &State, dev: &str) -> String {
    match st.devices.get(dev) {
        Some(d) => d.config.as_ref().map(|c| c.hostname.clone()).unwrap_or_else(|| if d.label.is_empty() { dev.to_string() } else { d.label.clone() }),
        None => dev.to_string(),
    }
}

impl Terminal {
    pub fn new() -> Terminal {
        Terminal {
            open: false,
            maxed: false,
            stack: vec![],
            lines: vec![String::new()],
            input: vec![],
            cur: 0,
            hist_idx: HashMap::new(),
            scroll: 0,
            busy: false,
            chunks: VecDeque::new(),
            wait: 0.0,
            wait_started: false,
            finish: None,
            abort: false,
            ctrl_a: false,
            last_tab: -10.0,
            time: 0.0,
            ghost: String::new(),
            ghost_dirty: true,
            rng: 0x2545F4914F6CDD1D,
            cols: 100,
            rows: 24,
            clip: None,
            font: 22.0,
            theme: "classic".into(),
            chip_rects: vec![],
            btn_close: Rect::default(),
            btn_disc: Rect::default(),
            btn_max: Rect::default(),
            btn_copy: Rect::default(),
            flash: 0.0,
        }
    }

    fn rnd(&mut self) -> f64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 11) as f64 / (1u64 << 53) as f64
    }

    fn garble(&mut self, text: &str) -> String {
        let g: Vec<char> = GARBAGE.chars().collect();
        let mut o = String::new();
        for ch in text.chars() {
            if ch == '\n' {
                o.push('\n');
                continue;
            }
            if self.rnd() < 0.7 {
                o.push(g[(self.rnd() * g.len() as f64) as usize % g.len()]);
            } else if self.rnd() < 0.5 {
                o.push((33 + (self.rnd() * 90.0) as u8) as char);
            }
        }
        o
    }

    fn top(&self) -> Option<&Entry> {
        self.stack.last()
    }

    pub fn line_text(&self) -> String {
        self.input.iter().collect()
    }

    // ------------------------------------------------------------------ utskrift

    pub fn print(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let text = if self.top().map(|t| t.garbled).unwrap_or(false) { self.garble(text) } else { text.to_string() };
        self.push_text(&text);
    }

    fn push_text(&mut self, text: &str) {
        let text = text.replace('\r', "").replace('\t', "        ");
        let mut first = true;
        for part in text.split('\n') {
            if !first {
                self.lines.push(String::new());
            }
            first = false;
            if let Some(l) = self.lines.last_mut() {
                l.push_str(part);
            }
        }
        if self.lines.len() > MAX_LINES {
            let n = self.lines.len() - MAX_LINES + 300;
            self.lines.drain(0..n);
        }
        if self.scroll > 0 {
            // läser längre upp: håll kvar vyn
        }
    }

    fn echo(&mut self, prompt: &str, cmd: &str) {
        // raden där prompten ska hamna måste börja på en ny rad
        if self.lines.last().map(|l| !l.is_empty()).unwrap_or(false) {
            self.push_text("\n");
        }
        let line = format!("{}{}\n", prompt, cmd);
        self.push_text(&line);
    }

    fn prompt(&mut self, env: &TermEnv) -> String {
        let (txt, garbled, started, kind) = match self.top() {
            Some(t) => (t.session.prompt_text(env.st), t.garbled, t.started, t.kind),
            None => return String::new(),
        };
        if kind == EKind::Serial && !started {
            return String::new();
        }
        if garbled && !txt.is_empty() {
            return self.garble(&txt);
        }
        txt
    }

    fn prompt_peek(&self, env: &TermEnv) -> String {
        match self.top() {
            Some(t) => {
                if t.kind == EKind::Serial && !t.started {
                    String::new()
                } else if t.garbled {
                    t.session.prompt_text(env.st).chars().map(|_| '§').collect()
                } else {
                    t.session.prompt_text(env.st)
                }
            }
            None => String::new(),
        }
    }

    // ------------------------------------------------------------------ öppna och stänga

    pub fn open_laptop(&mut self, env: &mut TermEnv, prefill: Option<&str>) {
        if !self.open || self.stack.is_empty() {
            self.release(env);
            let sh = HostSession::new(env.st, "Tekniker");
            let banner = sh.banner(env.st);
            self.stack.push(Entry { kind: EKind::Linux, session: Box::new(sh), dev: "Tekniker".into(), title: "tekniker@laptop".into(), speed: 0, ios: false, started: true, garbled: false, wait_return: false, hist_key: "linux".into() });
            self.print(&format!("{}\n", banner));
        }
        self.show(env);
        if let Some(p) = prefill {
            self.input = p.chars().collect();
            self.cur = self.input.len();
            self.ghost_dirty = true;
        }
    }

    pub fn open_windows(&mut self, env: &mut TermEnv, host: &str) {
        self.release(env);
        let sh = HostSession::new(env.st, host);
        let banner = sh.banner(env.st);
        let label = env.st.devices.get(host).map(|d| if d.label.is_empty() { host.to_string() } else { d.label.clone() }).unwrap_or_else(|| host.to_string());
        self.stack.push(Entry { kind: EKind::Win, session: Box::new(sh), dev: host.to_string(), title: format!("Kommandotolken – {}", label), speed: 0, ios: false, started: true, garbled: false, wait_return: false, hist_key: "win".into() });
        self.print(&format!("{}\n", banner));
        self.show(env);
    }

    fn show(&mut self, env: &TermEnv) {
        self.open = true;
        self.ghost_dirty = true;
        self.scroll = 0;
    }

    /// Stäng terminalen. Konsolsessioner lever kvar per enhet.
    pub fn hide(&mut self, env: &mut TermEnv) {
        self.release(env);
        self.open = false;
        self.lines = vec![String::new()];
        self.busy = false;
        self.chunks.clear();
        self.finish = None;
    }

    /// Lämna tillbaka konsolsessionerna till spelet och töm stacken
    pub fn release(&mut self, env: &mut TermEnv) {
        while let Some(e) = self.stack.pop() {
            if e.kind == EKind::Serial {
                env.consoles.insert(e.dev.clone(), e.session);
            }
        }
        self.input.clear();
        self.cur = 0;
        self.lines = vec![String::new()];
    }

    pub fn is_busy(&self) -> bool {
        self.busy
    }

    // ------------------------------------------------------------------ sessioner

    fn console_session(env: &mut TermEnv, dev: &str) -> Box<dyn Cli> {
        if let Some(s) = env.consoles.remove(dev) {
            return s;
        }
        Self::new_console(env.st, dev)
    }

    fn new_console(st: &State, dev: &str) -> Box<dyn Cli> {
        use crate::net::model::Kind;
        match st.devices.get(dev).map(|d| d.kind) {
            Some(Kind::Wlc) => Box::new(WlcSession::new(st, dev)),
            Some(Kind::Lb) => Box::new(LbSession::new(st, dev)),
            _ => Box::new(IosSession::console(st, dev)),
        }
    }

    fn push_session(&mut self, env: &mut TermEnv, p: PushReq, events: &mut Vec<TermEvent>) {
        match p {
            PushReq::Serial { target, speed } => {
                let sess = Self::console_session(env, &target);
                let ios = env.st.devices.get(&target).map(|d| d.os == Os::Ios).unwrap_or(false);
                let title = format!("screen /dev/ttyUSB0 {} — {}", speed, host_name(env.st, &target));
                let booting = sess.booting();
                self.stack.push(Entry { kind: EKind::Serial, session: sess, dev: target.clone(), title, speed, ios, started: false, garbled: false, wait_return: false, hist_key: format!("con:{}", target) });
                self.check_speed(env);
                self.print("\n");
                if booting {
                    self.print("\n");
                } else if let Some(m) = motd(env.st, &target) {
                    self.print(&format!("{}\n", m));
                }
                events.push(TermEvent::ConsoleConnected(target));
            }
            PushReq::Ios { dev, via, privileged, user } => {
                let s2 = IosSession::new(env.st, &dev, &via, privileged, user.clone());
                let title = format!("{} {}@{}", via, user.clone().unwrap_or_default(), host_name(env.st, &dev));
                self.stack.push(Entry { kind: EKind::Ssh, session: Box::new(s2), dev: dev.clone(), title, speed: 0, ios: true, started: true, garbled: false, wait_return: false, hist_key: format!("ssh:{}", dev) });
                if via == "ssh" {
                    self.print("\n");
                }
                if let Some(m) = motd(env.st, &dev) {
                    self.print(&format!("{}\n", m));
                }
            }
            PushReq::Wlc { dev } => {
                let w = WlcSession::new(env.st, &dev);
                self.stack.push(Entry { kind: EKind::Ssh, session: Box::new(w), dev: dev.clone(), title: "ssh admin@WLC-Nordvik".into(), speed: 0, ios: false, started: true, garbled: false, wait_return: false, hist_key: format!("ssh:{}", dev) });
                self.print("\n(Cisco Controller)\n");
            }
            PushReq::Lb { dev } => {
                let l = LbSession::new(env.st, &dev);
                self.stack.push(Entry { kind: EKind::Ssh, session: Box::new(l), dev: dev.clone(), title: "ssh admin@LB-Nordvik".into(), speed: 0, ios: false, started: true, garbled: false, wait_return: false, hist_key: format!("ssh:{}", dev) });
                self.print("\nLB-Nordvik 4.2 (lastbalanserare för tidrapporteringen)\nSkriv help för kommandon.\n");
            }
        }
        self.ghost_dirty = true;
    }

    fn close_top(&mut self, env: &mut TermEnv) {
        let t = match self.stack.pop() {
            Some(t) => t,
            None => return,
        };
        if t.kind == EKind::Ssh {
            let h = host_name(env.st, &t.dev);
            self.print(&format!("Connection to {} closed.\n", h));
        }
        if t.kind == EKind::Serial {
            let h = env.st.devices.get(&t.dev).and_then(|d| d.config.as_ref()).map(|c| c.hostname.clone()).unwrap_or_default();
            self.print(&format!("\n{} con0 is now available\n\n\n\nPress RETURN to get started.\n", h));
            let fresh = Self::new_console(env.st, &t.dev);
            self.stack.push(Entry { started: false, session: fresh, wait_return: false, ..t });
            return;
        }
        if self.stack.is_empty() {
            self.hide(env);
        }
        self.ghost_dirty = true;
    }

    /// Stänger screen (Ctrl+A K) eller ssh: översta serial/ssh-posten och allt ovanför
    fn pop_serial(&mut self, env: &mut TermEnv, announce: bool) {
        let idx = match self.stack.iter().rposition(|e| e.kind == EKind::Serial || e.kind == EKind::Ssh) {
            Some(i) => i,
            None => return,
        };
        let removed: Vec<Entry> = self.stack.drain(idx..).collect();
        let first_kind = removed[0].kind;
        for e in removed {
            if e.kind == EKind::Serial {
                env.consoles.insert(e.dev.clone(), e.session);
            }
        }
        if announce {
            self.print(&format!("\n{}\n", if first_kind == EKind::Serial { "[screen is terminating]" } else { "Connection closed." }));
        }
        self.busy = false;
        self.chunks.clear();
        self.finish = None;
        self.ghost_dirty = true;
    }

    fn check_speed(&mut self, env: &TermEnv) {
        let mut printed: Vec<usize> = vec![];
        for (i, t) in self.stack.iter_mut().enumerate() {
            if t.kind != EKind::Serial {
                continue;
            }
            let want = env.st.devices.get(&t.dev).and_then(|d| d.config.as_ref()).map(|c| c.lines.con.speed).unwrap_or(9600);
            let was = t.garbled;
            t.garbled = want != t.speed;
            if t.garbled && !was && t.started {
                printed.push(i);
            }
        }
        if !printed.is_empty() {
            let g = self.garble("\nspeed changed\n");
            self.push_text(&g);
        }
    }

    pub fn on_log(&mut self, dev: &str, line: &str) {
        if !self.open {
            return;
        }
        let show = match self.stack.last_mut() {
            Some(t) => {
                if !t.started {
                    false
                } else if t.kind == EKind::Serial && t.dev == dev {
                    true
                } else if t.kind == EKind::Ssh && t.dev == dev {
                    t.session.as_ios().map(|s| s.monitor).unwrap_or(false)
                } else {
                    false
                }
            }
            None => false,
        };
        if show {
            if self.lines.last().map(|l| !l.is_empty()).unwrap_or(false) {
                self.push_text("\n");
            }
            self.push_text(&format!("{}\n", line));
        }
    }

    // ------------------------------------------------------------------ kommandon

    fn submit(&mut self, env: &mut TermEnv, line: &str, events: &mut Vec<TermEvent>) {
        let (secret, garbled, kind, dev) = match self.top() {
            Some(t) => (t.session.secret_input(), t.garbled, t.kind, t.dev.clone()),
            None => return,
        };
        let prompt = self.prompt_peek(env);
        let shown = if secret { "" } else { line };
        self.echo(&prompt, shown);
        if !secret && !line.trim().is_empty() {
            let key = self.top().map(|t| t.hist_key.clone()).unwrap_or_default();
            let h = env.hist.entry(key.clone()).or_default();
            if h.last().map(|l| l != line).unwrap_or(true) {
                h.push(line.to_string());
            }
            if h.len() > 200 {
                let n = h.len() - 200;
                h.drain(0..n);
            }
            let n = h.len();
            self.hist_idx.insert(key, n);
        }
        // Seriell konsol: första Enter väcker konsolen
        if kind == EKind::Serial {
            if garbled {
                let g = self.garble(&format!("\n{}\n", ">"));
                self.push_text(&g);
                return;
            }
            let started = self.top().map(|t| t.started).unwrap_or(true);
            if !started {
                if let Some(t) = self.stack.last_mut() {
                    t.started = true;
                    if t.session.booting() {
                        t.session.finish_boot(env.st);
                    }
                }
                self.print("\n");
                return;
            }
            let wr = self.top().map(|t| t.wait_return).unwrap_or(false);
            if wr {
                if let Some(t) = self.stack.last_mut() {
                    t.wait_return = false;
                    t.session.finish_boot(env.st);
                }
                self.print("\n");
                return;
            }
        }
        if !secret && !line.trim().is_empty() {
            events.push(TermEvent::Command(dev, line.to_string()));
        }
        let r = match self.stack.last_mut() {
            Some(t) => t.session.handle(env.st, line),
            None => return,
        };
        self.consume(env, r, events);
    }

    fn consume(&mut self, env: &mut TermEnv, r: CmdResult, events: &mut Vec<TermEvent>) {
        let mut chunks: VecDeque<Chunk> = VecDeque::new();
        if !r.out.is_empty() {
            let mut t = r.out.clone();
            if !t.ends_with('\n') {
                t.push('\n');
            }
            chunks.push_back(Chunk { text: t, delay: r.delay });
        } else if r.delay > 0 {
            chunks.push_back(Chunk { text: String::new(), delay: r.delay });
        }
        if !r.stream.is_empty() {
            for c in r.stream.iter() {
                chunks.push_back(c.clone());
            }
            chunks.push_back(Chunk { text: "\n".into(), delay: 0 });
        }
        self.finish = Some(Finish { clear: r.clear, push: r.push.clone(), close: r.close, wait_return: r.wait_return });
        if chunks.is_empty() {
            self.do_finish(env, events);
            return;
        }
        self.busy = true;
        self.abort = false;
        self.scroll = 0;
        self.chunks = chunks;
        self.wait_started = false;
    }

    fn do_finish(&mut self, env: &mut TermEnv, events: &mut Vec<TermEvent>) {
        self.busy = false;
        self.chunks.clear();
        self.wait_started = false;
        let f = match self.finish.take() {
            Some(f) => f,
            None => return,
        };
        if f.clear {
            self.lines = vec![String::new()];
        }
        if let Some(p) = f.push {
            self.push_session(env, p, events);
            return;
        }
        if f.close {
            self.close_top(env);
            return;
        }
        if f.wait_return {
            if let Some(t) = self.stack.last_mut() {
                t.wait_return = true;
            }
        }
        self.ghost_dirty = true;
        events.push(TermEvent::AfterCommand);
    }

    fn run_stream(&mut self, env: &mut TermEnv, mut dt: f64, events: &mut Vec<TermEvent>) {
        let speed = if env.fast { 0.15 } else { 1.0 };
        loop {
            if !self.busy {
                return;
            }
            if self.abort {
                self.abort = false;
                self.do_finish(env, events);
                return;
            }
            let (delay, empty) = match self.chunks.front() {
                Some(c) => (c.delay.min(3200) as f64 / 1000.0 * speed, false),
                None => (0.0, true),
            };
            if empty {
                self.do_finish(env, events);
                return;
            }
            if !self.wait_started {
                self.wait = delay;
                self.wait_started = true;
            }
            if self.wait > dt {
                self.wait -= dt;
                return;
            }
            dt -= self.wait;
            self.wait_started = false;
            let c = self.chunks.pop_front().unwrap();
            self.print(&c.text);
            if !self.finish_is_some() {
                return;
            }
        }
    }

    fn finish_is_some(&self) -> bool {
        self.finish.is_some()
    }

    // ------------------------------------------------------------------ komplettering och hjälp

    fn update_ghost(&mut self, env: &mut TermEnv) {
        self.ghost_dirty = false;
        self.ghost.clear();
        let line = self.line_text();
        let (secret, garbled) = match self.top() {
            Some(t) => (t.session.secret_input(), t.garbled),
            None => return,
        };
        if secret || garbled || line.is_empty() || self.busy || !line.ends_with(|c: char| !c.is_whitespace()) {
            return;
        }
        if self.cur != self.input.len() {
            return;
        }
        if let Some(t) = self.stack.last_mut() {
            if let Some(c) = t.session.complete(env.st, &line) {
                if c.to_lowercase().starts_with(&line.to_lowercase()) && c.chars().count() > line.chars().count() {
                    self.ghost = c.chars().skip(line.chars().count()).collect();
                }
            }
        }
    }

    fn help_now(&mut self, env: &mut TermEnv) {
        let (ios, garbled, started) = match self.top() {
            Some(t) => (t.ios, t.garbled, t.started),
            None => return,
        };
        if !ios || garbled || !started {
            return;
        }
        let cur = self.line_text();
        let prompt = self.prompt_peek(env);
        self.echo(&prompt, &format!("{}?", cur));
        let h = match self.stack.last_mut() {
            Some(t) => t.session.help(env.st, &cur),
            None => return,
        };
        self.print(&format!("{}\n\n", h));
    }

    fn list_candidates(&mut self, env: &mut TermEnv) {
        let cur = self.line_text();
        let (ios, kind) = match self.top() {
            Some(t) => (t.ios, t.kind),
            None => return,
        };
        let words: Vec<String> = if ios {
            let h = match self.stack.last_mut() {
                Some(t) => t.session.help(env.st, &cur),
                None => return,
            };
            h.lines().filter_map(|l| l.trim().split_whitespace().next().map(|s| s.to_string())).filter(|w| !w.starts_with('<') && w != "%").collect()
        } else {
            let last = cur.rsplit(' ').next().unwrap_or("").to_string();
            let pool: &[&str] = if kind == EKind::Win {
                &["ipconfig", "ping", "tracert", "nslookup", "arp", "netsh", "curl", "copy", "route", "netstat", "getmac", "hostname", "whoami", "systeminfo", "test-netconnection", "pathping", "cls", "exit", "help"]
            } else {
                &["ssh", "screen", "ping", "traceroute", "tracepath", "curl", "nslookup", "dig", "resolvectl", "ip", "telnet", "nc", "man", "history", "clear", "exit", "nmap", "mtr", "ss", "netstat", "route", "ethtool", "nmcli", "arping", "hostnamectl", "uname", "help"]
            };
            pool.iter().filter(|w| w.starts_with(&last)).map(|s| s.to_string()).collect()
        };
        if words.is_empty() {
            return;
        }
        let prompt = self.prompt_peek(env);
        self.echo(&prompt, &cur);
        self.print(&format!("{}\n", words.join("   ")));
    }

    fn hist_move(&mut self, env: &mut TermEnv, up: bool) {
        let key = match self.top() {
            Some(t) => t.hist_key.clone(),
            None => return,
        };
        let list = env.hist.get(&key).cloned().unwrap_or_default();
        if list.is_empty() {
            return;
        }
        let idx = *self.hist_idx.get(&key).unwrap_or(&list.len());
        let ni = if up { idx.saturating_sub(1) } else { (idx + 1).min(list.len()) };
        self.hist_idx.insert(key, ni);
        self.input = list.get(ni).cloned().unwrap_or_default().chars().collect();
        self.cur = self.input.len();
        self.ghost_dirty = true;
    }

    // ------------------------------------------------------------------ uppdatera

    pub fn update(&mut self, input: &Input, dt: f64, env: &mut TermEnv) -> Vec<TermEvent> {
        let mut events = vec![];
        if !self.open {
            return events;
        }
        self.time += dt;
        self.flash = (self.flash - dt).max(0.0);
        self.check_speed(env);
        if self.busy {
            self.run_stream(env, dt, &mut events);
        }
        // musklick
        if input.clicked {
            let (mx, my) = (input.mouse_x, input.mouse_y);
            if self.btn_close.contains(mx, my) {
                self.hide(env);
                return events;
            }
            if self.btn_max.contains(mx, my) {
                self.maxed = !self.maxed;
            }
            if self.btn_disc.contains(mx, my) {
                self.pop_serial(env, true);
                if self.stack.is_empty() {
                    self.hide(env);
                    return events;
                }
            }
            if self.btn_copy.contains(mx, my) {
                self.clip = Some(self.lines.join("\n"));
            }
            let hit = self.chip_rects.iter().find(|(r, _)| r.contains(mx, my)).map(|(_, s)| s.clone());
            if let Some(s) = hit {
                if s == "?" {
                    self.help_now(env);
                } else {
                    self.input = s.chars().collect();
                    self.cur = self.input.len();
                    self.ghost_dirty = true;
                }
            }
        }
        if input.wheel != 0.0 {
            self.scroll = (self.scroll + (input.wheel * 3.0) as i32).max(0);
        }
        let cap_before = (self.input.clone(), self.cur);
        // tangenter
        for k in &input.pressed {
            if !self.open {
                break;
            }
            if self.stack.is_empty() {
                break;
            }
            let ctrl = input.ctrl;
            // Ctrl+A K
            if self.ctrl_a {
                self.ctrl_a = false;
                if *k == Key::K {
                    self.pop_serial(env, true);
                    if self.stack.is_empty() {
                        self.hide(env);
                    }
                    continue;
                }
            }
            let kind = self.top().map(|t| t.kind).unwrap_or(EKind::Linux);
            if ctrl {
                match k {
                    Key::A => {
                        if kind == EKind::Serial {
                            self.ctrl_a = true;
                        } else {
                            self.cur = 0;
                        }
                        continue;
                    }
                    Key::E => {
                        self.cur = self.input.len();
                        continue;
                    }
                    Key::P => {
                        self.hist_move(env, true);
                        continue;
                    }
                    Key::N => {
                        self.hist_move(env, false);
                        continue;
                    }
                    Key::U => {
                        self.input.drain(0..self.cur);
                        self.cur = 0;
                        self.ghost_dirty = true;
                        continue;
                    }
                    Key::K => {
                        self.input.truncate(self.cur);
                        self.ghost_dirty = true;
                        continue;
                    }
                    Key::W => {
                        let mut i = self.cur;
                        while i > 0 && self.input[i - 1].is_whitespace() {
                            i -= 1;
                        }
                        while i > 0 && !self.input[i - 1].is_whitespace() {
                            i -= 1;
                        }
                        self.input.drain(i..self.cur);
                        self.cur = i;
                        self.ghost_dirty = true;
                        continue;
                    }
                    Key::L => {
                        self.lines = vec![String::new()];
                        self.scroll = 0;
                        continue;
                    }
                    Key::C => {
                        if self.busy {
                            self.abort = true;
                            self.print("^C\n");
                            self.run_stream(env, 0.0, &mut events);
                        } else {
                            let p = self.prompt_peek(env);
                            let l = self.line_text();
                            self.echo(&p, &format!("{}^C", l));
                            self.input.clear();
                            self.cur = 0;
                            if let Some(t) = self.stack.last_mut() {
                                t.session.interrupt(env.st, false);
                            }
                            self.ghost_dirty = true;
                        }
                        continue;
                    }
                    Key::Z => {
                        if !self.busy {
                            let (ios, is_cfg) = match self.top() {
                                Some(t) => (t.ios, t.ios),
                                None => (false, false),
                            };
                            if ios && is_cfg {
                                let p = self.prompt_peek(env);
                                let in_cfg = p.contains("(config");
                                if in_cfg {
                                    let l = self.line_text();
                                    self.echo(&p, &format!("{}^Z", l));
                                    self.input.clear();
                                    self.cur = 0;
                                    if let Some(t) = self.stack.last_mut() {
                                        t.session.interrupt(env.st, true);
                                    }
                                    events.push(TermEvent::AfterCommand);
                                }
                            }
                        }
                        continue;
                    }
                    Key::D => {
                        if self.input.is_empty() && !self.busy && kind != EKind::Serial {
                            let ios = self.top().map(|t| t.ios).unwrap_or(false);
                            let cmd = if kind == EKind::Win || ios { "exit" } else { "logout" };
                            self.submit(env, cmd, &mut events);
                        }
                        continue;
                    }
                    Key::Left | Key::Right => {}
                    _ => continue,
                }
            }
            if *k == Key::Escape {
                self.hide(env);
                return events;
            }
            if *k == Key::F11 {
                self.maxed = !self.maxed;
                continue;
            }
            if *k == Key::PageUp {
                self.scroll += (self.rows as i32 * 85 / 100).max(1);
                continue;
            }
            if *k == Key::PageDown {
                self.scroll = (self.scroll - (self.rows as i32 * 85 / 100).max(1)).max(0);
                continue;
            }
            if self.busy {
                continue;
            }
            match k {
                Key::Enter => {
                    let line = self.line_text();
                    self.input.clear();
                    self.cur = 0;
                    self.scroll = 0;
                    self.ghost.clear();
                    self.submit(env, &line, &mut events);
                    if !self.open || self.stack.is_empty() {
                        return events;
                    }
                }
                Key::Tab => {
                    let line = self.line_text();
                    let garbled = self.top().map(|t| t.garbled).unwrap_or(true);
                    if !garbled {
                        let c = match self.stack.last_mut() {
                            Some(t) => t.session.complete(env.st, &line),
                            None => None,
                        };
                        if let Some(c) = c {
                            self.input = c.chars().collect();
                            self.cur = self.input.len();
                            self.last_tab = -10.0;
                        } else if self.time - self.last_tab < 0.7 {
                            self.last_tab = -10.0;
                            self.list_candidates(env);
                        } else {
                            self.last_tab = self.time;
                        }
                        self.ghost_dirty = true;
                    }
                }
                Key::Up => self.hist_move(env, true),
                Key::Down => self.hist_move(env, false),
                Key::Left => {
                    if ctrl {
                        while self.cur > 0 && self.input[self.cur - 1].is_whitespace() {
                            self.cur -= 1;
                        }
                        while self.cur > 0 && !self.input[self.cur - 1].is_whitespace() {
                            self.cur -= 1;
                        }
                    } else if self.cur > 0 {
                        self.cur -= 1;
                    }
                }
                Key::Right => {
                    if self.cur == self.input.len() && !self.ghost.is_empty() {
                        let g: Vec<char> = self.ghost.chars().collect();
                        self.input.extend(g);
                        self.cur = self.input.len();
                        self.ghost_dirty = true;
                    } else if self.cur < self.input.len() {
                        self.cur += 1;
                    }
                }
                Key::Home => self.cur = 0,
                Key::End => self.cur = self.input.len(),
                Key::Backspace => {
                    if self.cur > 0 {
                        self.input.remove(self.cur - 1);
                        self.cur -= 1;
                        self.ghost_dirty = true;
                    }
                }
                Key::Delete => {
                    if self.cur < self.input.len() {
                        self.input.remove(self.cur);
                        self.ghost_dirty = true;
                    }
                }
                _ => {}
            }
        }
        // inskrivna tecken
        if self.open && !self.stack.is_empty() && !input.ctrl {
            for ch in &input.chars {
                if self.busy {
                    break;
                }
                if *ch == '?' {
                    let can_help = self.top().map(|t| t.ios && !t.session.secret_input() && t.started && !t.garbled).unwrap_or(false);
                    if can_help {
                        self.help_now(env);
                        continue;
                    }
                }
                if printable(*ch) {
                    self.input.insert(self.cur, *ch);
                    self.cur += 1;
                    self.ghost_dirty = true;
                    self.scroll = 0;
                }
            }
        }
        if cap_before.0 != self.input || cap_before.1 != self.cur {
            self.ghost_dirty = true;
        }
        if self.open && self.ghost_dirty {
            self.update_ghost(env);
        }
        events
    }

    // ------------------------------------------------------------------ färgning

    fn theme_colors(&self) -> (Color, Color, bool) {
        match self.theme.as_str() {
            "green" => (rgb(0x00, 0x12, 0x04), rgb(0x3d, 0xff, 0x6a), false),
            "amber" => (rgb(0x16, 0x0d, 0x00), rgb(0xff, 0xb0, 0x20), false),
            "pixel" => (rgb(0x16, 0x0f, 0x2c), rgb(0xcf, 0xe3, 0xff), true),
            _ => (rgb(0x0b, 0x0c, 0x11), rgb(0xd7, 0xda, 0xe0), true),
        }
    }

    fn cls_color(&self, cls: Cls, base: Color, colorful: bool) -> Color {
        if !colorful {
            return base;
        }
        match cls {
            Cls::Plain => base,
            Cls::Err => rgb(0xff, 0x6b, 0x6b),
            Cls::Log => rgb(0xf0, 0xd0, 0x60),
            Cls::Warn => rgb(0xff, 0xb0, 0x20),
            Cls::Bad => rgb(0xff, 0x6b, 0x6b),
            Cls::Ok => rgb(0x6e, 0xe7, 0x87),
            Cls::Mac => rgb(0xc8, 0x9b, 0xff),
            Cls::Ip => rgb(0x7f, 0xd4, 0xff),
            Cls::If => rgb(0x8f, 0xb0, 0xff),
        }
    }

    // ------------------------------------------------------------------ rita

    fn chips(&self, env: &TermEnv) -> Vec<(String, String)> {
        let mut v: Vec<(String, String)> = vec![];
        let t = match self.top() {
            Some(t) => t,
            None => return v,
        };
        if t.kind == EKind::Linux {
            if env.console_target.is_some() {
                v.push(("screen 9600".into(), "sudo screen /dev/ttyUSB0 9600".into()));
            }
            for (a, b) in [("ssh R-Nordvik-1", "ssh drift@192.168.1.193"), ("ping gateway", "ping -c 4 192.168.1.193"), ("help", "help")] {
                v.push((a.into(), b.into()));
            }
            if env.week_n == 8 {
                v.push(("ssh WLC".into(), "ssh admin@192.168.1.196".into()));
            }
            if env.week_n == 10 {
                for (a, b) in [("ssh R-Boras-1", "ssh drift@192.168.2.193"), ("ssh LB", "ssh admin@192.168.1.13"), ("curl x4", "for i in 1 2 3 4; do curl -s http://tid/; done"), ("tracepath Borås", "tracepath 192.168.2.193")] {
                    v.push((a.into(), b.into()));
                }
            }
        } else if t.ios && (t.kind == EKind::Serial || t.kind == EKind::Ssh) {
            v.push(("?".into(), "?".into()));
            match env.week {
                Some(w) if !w.chips.is_empty() => {
                    for c in &w.chips {
                        v.push((c.to_string(), c.to_string()));
                    }
                }
                _ => {
                    v.push(("show ip int brief".into(), "show ip interface brief".into()));
                    v.push(("show run".into(), "show running-config".into()));
                }
            }
            v.push(("show logging".into(), "show logging".into()));
        }
        v
    }

    pub fn status_line(&self, env: &mut TermEnv) -> (String, String, String) {
        let t = match self.top() {
            Some(t) => t,
            None => return (String::new(), String::new(), String::new()),
        };
        let conn = match t.kind {
            EKind::Serial => format!("Konsol {} 8N1{}", t.speed, if t.garbled { " · fel hastighet?" } else { "" }),
            EKind::Ssh => if t.title.starts_with("telnet") { "Telnet (okrypterat!)".to_string() } else { "SSH".to_string() },
            EKind::Win => "Windows".into(),
            EKind::Linux => "Laptop (Linux)".into(),
        };
        let p = t.session.prompt_text(env.st);
        let mode = if t.ios || t.kind == EKind::Serial {
            if !t.started {
                "Tryck Enter".to_string()
            } else if p.contains("(config-if") {
                "Interfacekonfiguration".into()
            } else if p.contains("(config-line)") {
                "Linjekonfiguration".into()
            } else if p.contains("(config-isakmp)") {
                "ISAKMP-policy (fas 1)".into()
            } else if p.contains("(config-crypto-map)") {
                "Crypto map".into()
            } else if p.contains("(cfg-crypto-trans)") {
                "Transform-set (fas 2)".into()
            } else if p.contains("-nacl)") {
                "Åtkomstlista".into()
            } else if p.contains("(config") {
                "Konfigurationsläge".into()
            } else if p.trim_end().ends_with('#') {
                "Privilegierat läge (#)".into()
            } else if p.trim_end().ends_with('>') {
                "Användarläge (>)".into()
            } else if p.is_empty() {
                String::new()
            } else {
                "Inloggning".into()
            }
        } else if t.dev == "LB" {
            "Lastbalanserare".into()
        } else if t.dev == "WLC" {
            "Controller (AireOS)".into()
        } else if t.kind == EKind::Win {
            "cmd.exe".into()
        } else {
            "bash".into()
        };
        let clock = if env.st.devices.contains_key("R1") { S::device_clock(env.st, "R1").hms } else { String::new() };
        let mode = if self.busy { format!("arbetar… (Ctrl+C avbryter) · {}", mode) } else { mode };
        (conn, mode, clock)
    }

    pub fn draw(&mut self, c: &mut Canvas, fonts: &mut Fonts, env: &mut TermEnv, mouse: (i32, i32)) {
        if !self.open {
            return;
        }
        let (sw, sh) = (c.w as i32, c.h as i32);
        c.dim(0x000000, 130);
        let r = if self.maxed { Rect::new(8, 8, sw - 16, sh - 16) } else { Rect::new(60, 30, sw - 120, sh - 60) };
        let (bg, fg, colorful) = self.theme_colors();
        panel(c, r, bg, rgb(0x6a, 0x72, 0xa0));
        // titel
        c.fill_rect(r.x, r.y, r.w, 34, rgb(0x2b, 0x2e, 0x44));
        let (title, serial_like) = match self.top() {
            Some(t) => (t.title.clone(), self.stack.iter().any(|e| e.kind == EKind::Serial || e.kind == EKind::Ssh)),
            None => ("Terminal".into(), false),
        };
        text_fit(c, fonts, r.x + 12, r.y + 9, &title, 12.0, TEXT, FontKind::Pixel, r.w - 360);
        let mut bx = r.x + r.w - 8;
        let mut mkbtn = |c: &mut Canvas, fonts: &mut Fonts, label: &str, bx: &mut i32| -> Rect {
            let w = text_w(fonts, label, SMALL, FontKind::Mono) + 20;
            *bx -= w;
            let rr = Rect::new(*bx, r.y + 4, w, 26);
            button(c, fonts, rr, label, false, rr.contains(mouse.0, mouse.1), false);
            *bx -= 6;
            rr
        };
        self.btn_close = mkbtn(c, fonts, "Stäng", &mut bx);
        self.btn_max = mkbtn(c, fonts, if self.maxed { "Mindre" } else { "Större" }, &mut bx);
        self.btn_copy = mkbtn(c, fonts, "Kopiera", &mut bx);
        self.btn_disc = if serial_like { mkbtn(c, fonts, "Koppla från", &mut bx) } else { Rect::default() };
        // chips
        self.chip_rects.clear();
        let chips = self.chips(env);
        let mut cx = r.x + 10;
        let cy = r.y + 40;
        for (label, cmd) in chips {
            let w = text_w(fonts, &label, SMALL, FontKind::Mono) + 18;
            if cx + w > r.x + r.w - 10 {
                break;
            }
            let rr = Rect::new(cx, cy, w, 26);
            let hover = rr.contains(mouse.0, mouse.1);
            c.fill_rect(rr.x, rr.y, rr.w, rr.h, if hover { BTN_HI } else { rgb(0x24, 0x28, 0x3e) });
            c.rect(rr.x, rr.y, rr.w, rr.h, 1, rgb(0x4a, 0x50, 0x78));
            fonts.text(c, rr.x + 9, rr.y + 3, &label, SMALL, rgb(0xcf, 0xd6, 0xff), FontKind::Mono);
            self.chip_rects.push((rr, cmd));
            cx += w + 6;
        }
        // skärmyta
        let top = r.y + 72;
        let status_h = 26;
        let area = Rect::new(r.x + 10, top, r.w - 20, r.y + r.h - top - status_h - 6);
        let size = self.font;
        let adv = fonts.mono_advance(size).max(4.0);
        let lh = line_h(size) - 2;
        self.cols = (((area.w - 8) as f32 / adv) as usize).max(20);
        self.rows = ((area.h / lh) as usize).max(3);
        // visuella rader nerifrån och upp
        let prompt = self.prompt_peek(env);
        let secret = self.top().map(|t| t.session.secret_input()).unwrap_or(false);
        let show_prompt = !self.busy;
        let mut vis: Vec<(String, usize, usize, usize)> = vec![]; // (text, radindex, startkolumn, vissa)
        let want = self.rows as i32 + self.scroll + 2;
        let mut count = 0i32;
        let ncols = self.cols;
        // promptraden
        let input_str: String = if secret { "*".repeat(self.input.len()) } else { self.line_text() };
        let prompt_line = format!("{}{}", prompt, input_str);
        let mut prompt_rows: Vec<String> = vec![];
        if show_prompt {
            let chars: Vec<char> = prompt_line.chars().collect();
            let mut i = 0;
            loop {
                let end = (i + ncols).min(chars.len());
                prompt_rows.push(chars[i..end].iter().collect());
                if end >= chars.len() {
                    break;
                }
                i = end;
            }
            // markören ligger efter sista tecknet; ligger den vid radbrytning behövs en extra rad
            if chars.len() % ncols == 0 && !chars.is_empty() && self.cur == self.input.len() {
                prompt_rows.push(String::new());
            }
        }
        count += prompt_rows.len() as i32;
        let mut li = self.lines.len();
        let mut line_rows: Vec<(usize, Vec<String>)> = vec![];
        while li > 0 && count < want {
            li -= 1;
            let chars: Vec<char> = self.lines[li].chars().collect();
            if show_prompt && li == self.lines.len() - 1 && self.lines[li].is_empty() {
                // tom slutrad hör ihop med promptraden
                continue;
            }
            let mut rows_here: Vec<String> = vec![];
            if chars.is_empty() {
                rows_here.push(String::new());
            } else {
                let mut i = 0;
                while i < chars.len() {
                    let end = (i + ncols).min(chars.len());
                    rows_here.push(chars[i..end].iter().collect());
                    i = end;
                }
            }
            count += rows_here.len() as i32;
            line_rows.push((li, rows_here));
        }
        let _ = &mut vis;
        // bygg lista uppifrån och ned
        let mut all: Vec<(Option<usize>, usize, String)> = vec![]; // (radindex, radnr, text)
        for (li, rows_here) in line_rows.iter().rev() {
            for (k, t) in rows_here.iter().enumerate() {
                all.push((Some(*li), k, t.clone()));
            }
        }
        let n_prompt = prompt_rows.len();
        for t in &prompt_rows {
            all.push((None, 0, t.clone()));
        }
        let total = all.len() as i32;
        let max_scroll = (total - self.rows as i32).max(0);
        if self.scroll > max_scroll {
            self.scroll = max_scroll;
        }
        let bottom = total - self.scroll.min(max_scroll.max(0));
        let first = (bottom - self.rows as i32).max(0);
        c.set_clip(area.x, area.y, area.w, area.h);
        let mut y = area.y + 2;
        for idx in first..bottom {
            let (li, k, text) = &all[idx as usize];
            let mut x = area.x + 4;
            match li {
                Some(li) => {
                    // färga hela logiska raden och klipp ut denna radbit
                    let full = &self.lines[*li];
                    let runs = self.colorize(full);
                    let start = k * ncols;
                    let end = start + text.chars().count();
                    let mut pos = 0usize;
                    for (s, cls) in runs {
                        let n = s.chars().count();
                        let a = pos.max(start);
                        let b = (pos + n).min(end);
                        if a < b {
                            let piece: String = s.chars().skip(a - pos).take(b - a).collect();
                            let col = self.cls_color(cls, fg, colorful);
                            x += fonts.text(c, x, y, &piece, size, col, FontKind::Mono);
                        }
                        pos += n;
                    }
                }
                None => {
                    // promptraden: prompt i egen färg, sedan inmatningen
                    let rowno = idx - (total - n_prompt as i32);
                    let start = rowno as usize * ncols;
                    let pl = prompt.chars().count();
                    let chars: Vec<char> = text.chars().collect();
                    let mut cx2 = x;
                    for (i, ch) in chars.iter().enumerate() {
                        let abs = start + i;
                        let col = if abs < pl { self.prompt_color(&prompt, fg, colorful) } else { fg };
                        cx2 += fonts.text(c, cx2, y, &ch.to_string(), size, col, FontKind::Mono);
                    }
                    // spökkomplettering
                    if rowno as usize == n_prompt - 1 && !self.ghost.is_empty() && !secret && self.cur == self.input.len() {
                        fonts.text(c, cx2, y, &self.ghost, size, rgb(0x6a, 0x70, 0x88), FontKind::Mono);
                    }
                    // markör
                    if !self.busy {
                        let cpos = pl + self.cur;
                        let crow = cpos / ncols;
                        if crow == rowno as usize {
                            let pre: String = prompt_line.chars().skip(start).take(cpos - start).collect();
                            let px = x + text_w(fonts, &pre, size, FontKind::Mono);
                            if (self.time * 2.0) as i64 % 2 == 0 || true {
                                c.fill_rect_a(px, y + 1, (adv as i32).max(6), lh - 2, fg, 150);
                            }
                        }
                    }
                }
            }
            y += lh;
        }
        c.reset_clip();
        if self.scroll > 0 {
            fonts.text(c, area.x + area.w - 150, area.y + 2, "▲ rullat uppåt", SMALL, ACCENT, FontKind::Mono);
        }
        // statusrad
        let (conn, mode, clock) = self.status_line(env);
        let sy = r.y + r.h - status_h - 4;
        c.fill_rect(r.x + 4, sy, r.w - 8, status_h, rgb(0x24, 0x28, 0x3e));
        fonts.text(c, r.x + 14, sy + 4, &conn, SMALL, rgb(0xcf, 0xd6, 0xff), FontKind::Mono);
        fonts.text(c, r.x + 14 + 360, sy + 4, &mode, SMALL, MUTED, FontKind::Mono);
        let cw = text_w(fonts, &clock, SMALL, FontKind::Mono);
        fonts.text(c, r.x + r.w - cw - 14, sy + 4, &clock, SMALL, MUTED, FontKind::Mono);
    }

    fn prompt_color(&self, p: &str, fg: Color, colorful: bool) -> Color {
        if !colorful {
            return fg;
        }
        let t = p.trim_end();
        if t.contains("(config") && t.ends_with('#') {
            rgb(0xf0, 0xd0, 0x60)
        } else if t.ends_with('#') {
            rgb(0x6e, 0xe7, 0x87)
        } else if t.ends_with('>') {
            rgb(0x7f, 0xd4, 0xff)
        } else {
            fg
        }
    }

    /// Delar en rad i färgade bitar efter samma regler som terminal.js (RULES)
    pub fn colorize(&self, line: &str) -> Vec<(String, Cls)> {
        colorize_line(line)
    }
}

const ERR_STARTS: &[&str] = &["Invalid input", "Incomplete command", "Ambiguous command", "Unknown command", "Unrecognized", "Bad", "Error"];
const WARN_PHRASES: &[&str] = &["Frag needed and DF set", "Paketet måste fragmenteras men DF har angetts.", "MM_KEY_EXCH", "DOWN-NEGOTIATING", "UP-IDLE", "late collision", "input errors", "Media disconnected"];
const BAD_PHRASES: &[&str] = &[
    "MM_NO_STATE", "502 Bad Gateway", "503 Service Unavailable", "proxy identities not supported", "Begäran gjorde time out", "0 fil(er) kopierade", "administratively down", "err-disabled", "notconnect", "not connected",
    "Request timed out", "Destination host unreachable", "unreachable", "timed out", "Success rate is 0 percent",
];
const OK_PHRASES: &[&str] = &["QM_IDLE", "UP-ACTIVE", "200 OK", "1 fil(er) kopierade", "Svar från", "Success rate is 100 percent", "Reply from", "bytes from"];
const BAD_WORDS: &[&str] = &["down", "DOWN", "disabled", "denied", "Denied", "refused", "failed", "FAILED"];
const WARN_WORDS: &[&str] = &["BLK", "blocking", "Blocking", "flapping", "mismatch", "CRC", "inactive", "Altn", "APIPA"];
const OK_WORDS: &[&str] = &["connected", "up", "UP", "FWD", "forwarding", "Established", "open", "Connected", "Enabled", "Registered"];

fn find_all(hay: &[char], needle: &str) -> Vec<(usize, usize)> {
    let nd: Vec<char> = needle.chars().collect();
    let mut out = vec![];
    if nd.is_empty() || hay.len() < nd.len() {
        return out;
    }
    let mut i = 0;
    while i + nd.len() <= hay.len() {
        if hay[i..i + nd.len()] == nd[..] {
            out.push((i, i + nd.len()));
            i += nd.len();
        } else {
            i += 1;
        }
    }
    out
}

fn is_tok(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '.' || c == '/' || c == ':' || c == '-' || c == '%'
}

fn is_ip(s: &str) -> bool {
    let p: Vec<&str> = s.split('/').next().unwrap_or("").split('.').collect();
    p.len() == 4 && p.iter().all(|x| !x.is_empty() && x.len() <= 3 && x.chars().all(|c| c.is_ascii_digit()))
}

fn is_mac(s: &str) -> bool {
    let d: Vec<&str> = s.split('.').collect();
    if d.len() == 3 && d.iter().all(|x| x.len() == 4 && x.chars().all(|c| c.is_ascii_hexdigit())) {
        return true;
    }
    for sep in [':', '-'] {
        let p: Vec<&str> = s.split(sep).collect();
        if p.len() == 6 && p.iter().all(|x| x.len() == 2 && x.chars().all(|c| c.is_ascii_hexdigit())) {
            return true;
        }
    }
    false
}

fn is_if(s: &str) -> bool {
    for pre in ["GigabitEthernet", "FastEthernet", "Port-channel", "Loopback", "Serial", "Vlan", "Tunnel", "Gi", "Fa", "Po", "Lo", "Se"] {
        if let Some(rest) = s.strip_prefix(pre) {
            let r = rest.trim_end_matches(|c: char| c == ',' || c == ':');
            if !r.is_empty() && r.chars().next().unwrap().is_ascii_digit() && r.chars().all(|c| c.is_ascii_digit() || c == '/' || c == '.') {
                return true;
            }
        }
    }
    false
}

fn is_log(s: &str) -> bool {
    if !s.starts_with('%') {
        return false;
    }
    let p: Vec<&str> = s[1..].split('-').collect();
    p.len() >= 3 && p[0].chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') && p[1].len() == 1 && p[1].chars().all(|c| c.is_ascii_digit())
}

pub fn colorize_line(line: &str) -> Vec<(String, Cls)> {
    let chars: Vec<char> = line.chars().collect();
    let n = chars.len();
    if n == 0 {
        return vec![];
    }
    if line.starts_with('%') && ERR_STARTS.iter().any(|e| line[1..].trim_start().starts_with(e)) {
        return vec![(line.to_string(), Cls::Err)];
    }
    let mut cls = vec![Cls::Plain; n];
    let apply = |cls: &mut Vec<Cls>, list: &[&str], k: Cls| {
        for p in list {
            for (a, b) in find_all(&chars, p) {
                if cls[a..b].iter().all(|x| *x == Cls::Plain) {
                    for x in &mut cls[a..b] {
                        *x = k;
                    }
                }
            }
        }
    };
    apply(&mut cls, WARN_PHRASES, Cls::Warn);
    apply(&mut cls, BAD_PHRASES, Cls::Bad);
    apply(&mut cls, OK_PHRASES, Cls::Ok);
    // ord för ord
    let mut i = 0;
    while i < n {
        if !is_tok(chars[i]) {
            i += 1;
            continue;
        }
        let mut j = i;
        while j < n && is_tok(chars[j]) {
            j += 1;
        }
        if cls[i..j].iter().all(|x| *x == Cls::Plain) {
            let tok: String = chars[i..j].iter().collect();
            let t = tok.trim_end_matches(|c: char| c == '.' || c == ':' || c == ',');
            let k = if is_log(t) {
                Cls::Log
            } else if BAD_WORDS.contains(&t) {
                Cls::Bad
            } else if WARN_WORDS.contains(&t) {
                Cls::Warn
            } else if OK_WORDS.contains(&t) || (t.len() >= 3 && t.chars().all(|c| c == '!')) {
                Cls::Ok
            } else if is_mac(t) {
                Cls::Mac
            } else if is_ip(t) {
                if t.starts_with("169.254.") { Cls::Warn } else { Cls::Ip }
            } else if is_if(t) {
                Cls::If
            } else {
                Cls::Plain
            };
            if k != Cls::Plain {
                for x in &mut cls[i..i + t.chars().count()] {
                    *x = k;
                }
            }
        }
        i = j;
    }
    // '!!!!!' i ping-rader
    for (a, b) in find_all(&chars, "!!!") {
        for x in &mut cls[a..b] {
            if *x == Cls::Plain {
                *x = Cls::Ok;
            }
        }
    }
    // slå ihop till bitar
    let mut out: Vec<(String, Cls)> = vec![];
    let mut s = String::new();
    let mut cur = cls[0];
    for k in 0..n {
        if cls[k] != cur {
            out.push((std::mem::take(&mut s), cur));
            cur = cls[k];
        }
        s.push(chars[k]);
    }
    out.push((s, cur));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colorize_status() {
        let r = colorize_line("Gi0/1   connected   10.0.0.1 err-disabled");
        let cls: Vec<Cls> = r.iter().map(|x| x.1).collect();
        assert!(cls.contains(&Cls::If));
        assert!(cls.contains(&Cls::Ok));
        assert!(cls.contains(&Cls::Ip));
        assert!(cls.contains(&Cls::Bad));
        let joined: String = r.iter().map(|x| x.0.clone()).collect();
        assert_eq!(joined, "Gi0/1   connected   10.0.0.1 err-disabled");
    }

    #[test]
    fn colorize_error_line() {
        let r = colorize_line("% Invalid input detected at '^' marker.");
        assert_eq!(r.len(), 1);
        assert!(r[0].1 == Cls::Err);
    }

    #[test]
    fn colorize_log() {
        let r = colorize_line("*Mar  1 00:00:01: %LINK-3-UPDOWN: Interface Gi0/1, changed state to down");
        assert!(r.iter().any(|x| x.1 == Cls::Log));
        assert!(r.iter().any(|x| x.1 == Cls::Bad));
    }

    #[test]
    fn apipa_is_warn() {
        let r = colorize_line("IPv4 Address. . . : 169.254.12.3");
        assert!(r.iter().any(|x| x.1 == Cls::Warn));
    }
}
