//! Spelet: huvudmeny, veckor, 2D-världen, terminalen, dialoger, felrapporter och sparning.
//! Port av js/game.js, js/ui/*.js (terminal, ui, handbook, more, v7, v8) – bara 2D.
#![allow(unused_variables, dead_code)]
pub mod handbook;
pub mod handbook_data;
pub mod art;
pub mod levels;
pub mod terminal;
pub mod ui_dialog;
pub mod ui_dialogs;
pub mod ui_rack;
pub mod ui_save;
pub mod ui_screens;
pub mod ui_widgets;
pub mod world;
pub mod world_map;
pub mod world_path;

use crate::cli::Cli;
use crate::engine::{self, Canvas, Fonts, Input, Key};
use crate::net::sim::{self as S, State};
use handbook::Handbook;
use levels::Week;
use std::collections::HashMap;
use terminal::{TermEnv, TermEvent, Terminal};
use ui_dialog::{Act, Dialog, DlgOut};
use ui_save::{SaveData, SavedRun, SavedTask};
use world::{Mark, World, WorldEvent};

/// Startar spelet i ett fönster.
pub fn run() {
    let mut app = App::new();
    engine::run("Krabba spelet", &mut app);
}

// ------------------------------------------------------------------ typer

#[derive(Clone, Debug, Default)]
pub struct ReportData {
    pub saw: String,
    pub first: String,
    pub checked: String,
    pub cause: String,
    pub fix: String,
    pub score: u32,
}

#[derive(Clone, Debug, Default)]
pub struct TaskState {
    pub fixed: bool,
    pub reported: bool,
    pub known: bool,
    pub hints: u32,
    pub score: u32,
    pub report: Option<ReportData>,
}

#[derive(Clone, Debug)]
pub struct CmdEntry {
    pub place: String,
    pub dev: String,
    pub line: String,
}

pub struct WeekResult {
    pub minutes: u32,
    pub hints: u32,
    pub report_score: u32,
    pub report_max: u32,
    pub stars: u32,
    pub commands: u32,
    pub badges: Vec<String>,
}

/// En pågående vecka
pub struct Run {
    pub n: u32,
    pub week: Option<Week>,
    pub st: State,
    pub world: World,
    pub ts: Vec<TaskState>,
    pub consoles: HashMap<String, Box<dyn Cli>>,
    pub console_target: Option<String>,
    pub cmd_log: Vec<CmdEntry>,
    pub commands: u32,
    pub elapsed: f64,
    pub done: bool,
    pub exam: bool,
    pub report_nag: Option<&'static str>,
    pub sec_acc: f64,
    pub sec_n: u64,
    pub prev_up: HashMap<String, bool>,
    pub prev_seg: HashMap<String, String>,
    pub guide: i32,
    pub console_opened: bool,
    pub last_progress: f64,
    pub nudges: u32,
    pub vpn_key: Option<String>,
    pub lb_key: Option<String>,
    pub site: String,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Screen {
    Menu,
    Play,
}

pub enum Overlay {
    Rack(ui_rack::RackView),
    Dlg(Dialog),
    Hand(Handbook),
}

#[derive(Clone, Copy, PartialEq)]
pub enum ToastKind {
    Info,
    Good,
    Warn,
}

pub struct Toast {
    pub text: String,
    pub ttl: f64,
    pub kind: ToastKind,
}

#[derive(Clone, Debug)]
pub enum Delayed {
    WeekDone,
    ReportNag(bool),
}

pub struct QItem {
    pub q: String,
    pub opts: Vec<String>,
    pub answer: usize,
    pub explain: String,
}

pub struct QuizRun {
    pub week: u32,
    pub items: Vec<QItem>,
    pub i: usize,
    pub right: u32,
}

/// Skript för --headless-demo
pub enum DemoCmd {
    ConsoleTo(String),
    Rack(String),
}

pub struct App {
    pub save: SaveData,
    pub screen: Screen,
    pub menu_sel: usize,
    pub menu_hits: Vec<(ui_widgets::Rect, MenuAct)>,
    pub weeks: Vec<Week>,
    pub run: Option<Run>,
    pub overlays: Vec<Overlay>,
    pub term: Terminal,
    pub toasts: Vec<Toast>,
    pub delayed: Vec<(f64, Delayed)>,
    pub clip: Option<String>,
    pub time: f64,
    pub quit: bool,
    pub mouse: (i32, i32),
    pub quiz: Option<QuizRun>,
    pub save_timer: f64,
    pub tip: usize,
    pub frame: u64,
    pub demo: Vec<(u64, DemoCmd)>,
    pub hud_report_rect: ui_widgets::Rect,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MenuAct {
    Start(u32, bool),
    Continue,
    Free,
    Exam,
    Settings,
    Quit,
}

impl App {
    pub fn new() -> App {
        let save = SaveData::load();
        let mut app = App {
            save,
            screen: Screen::Menu,
            menu_sel: 0,
            menu_hits: vec![],
            weeks: vec![],
            run: None,
            overlays: vec![],
            term: Terminal::new(),
            toasts: vec![],
            delayed: vec![],
            clip: None,
            time: 0.0,
            quit: false,
            mouse: (0, 0),
            quiz: None,
            save_timer: 0.0,
            tip: 0,
            frame: 0,
            demo: vec![],
            hud_report_rect: ui_widgets::Rect::default(),
        };
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos() as u64).unwrap_or(0);
        app.tip = (nanos % handbook_data::TIPS.len() as u64) as usize;
        app
    }

    pub fn weeks(&mut self) -> &Vec<Week> {
        if self.weeks.is_empty() {
            self.weeks = levels::all();
        }
        &self.weeks
    }

    pub fn toast(&mut self, text: impl Into<String>, kind: ToastKind) {
        self.toasts.push(Toast { text: text.into(), ttl: 5.0, kind });
        if self.toasts.len() > 5 {
            self.toasts.remove(0);
        }
    }

    pub fn dialog_open(&self) -> bool {
        !self.overlays.is_empty()
    }

    pub fn push_dlg(&mut self, d: Dialog) {
        self.overlays.push(Overlay::Dlg(d));
    }

    // ------------------------------------------------------------------ start av veckor

    pub fn start_week(&mut self, n: u32, exam: bool) {
        let (st, week) = if n == 0 { (levels::base_state(10), None) } else { levels::start(n) };
        let site = week.as_ref().map(|w| w.site).unwrap_or("gbg");
        let mut world = World::new(site, n);
        world.set_week(n);
        let ts: Vec<TaskState> = week.as_ref().map(|w| w.tasks.iter().map(|t| TaskState { known: t.build, ..Default::default() }).collect()).unwrap_or_default();
        let guide = if n == 1 && self.save.stars(1) == 0 && !exam { 0 } else { -1 };
        let mut run = Run {
            n,
            week,
            st,
            world,
            ts,
            consoles: HashMap::new(),
            console_target: None,
            cmd_log: vec![],
            commands: 0,
            elapsed: 0.0,
            done: false,
            exam,
            report_nag: None,
            sec_acc: 0.0,
            sec_n: 0,
            prev_up: HashMap::new(),
            prev_seg: HashMap::new(),
            guide,
            console_opened: false,
            last_progress: 0.0,
            nudges: 0,
            vpn_key: None,
            lb_key: None,
            site: site.to_string(),
        };
        S::refresh(&mut run.st);
        snapshot_up(&mut run);
        self.run = Some(run);
        self.term = {
            let mut t = Terminal::new();
            t.font = self.save.settings.term_font as f32;
            t.theme = self.save.settings.term_theme.clone();
            t
        };
        self.screen = Screen::Play;
        self.overlays.clear();
        self.toasts.clear();
        self.delayed.clear();
        self.save.last_week = Some(n);
        self.save.run = None;
        self.refresh_markers();
        self.save.save();
        let d = self.briefing_dialog(true);
        self.push_dlg(d);
    }

    pub fn resume_run(&mut self) -> bool {
        let sr = match self.save.run.clone() {
            Some(r) => r,
            None => return false,
        };
        let week = if sr.week == 0 { None } else { levels::def(sr.week) };
        if sr.week != 0 && week.is_none() {
            return false;
        }
        let mut st = sr.state;
        S::touch(&mut st);
        let ts: Vec<TaskState> = sr.tasks.iter().map(|t| TaskState { fixed: t.fixed, reported: t.reported, known: t.known, hints: t.hints, score: t.score, report: None }).collect();
        let mut world = World::new(&sr.site, sr.week);
        world.set_week(sr.week);
        world.set_site(&sr.site);
        let mut run = Run {
            n: sr.week,
            week,
            st,
            world,
            ts,
            consoles: HashMap::new(),
            console_target: sr.console,
            cmd_log: vec![],
            commands: sr.commands,
            elapsed: sr.elapsed,
            done: false,
            exam: sr.exam,
            report_nag: None,
            sec_acc: 0.0,
            sec_n: 0,
            prev_up: HashMap::new(),
            prev_seg: HashMap::new(),
            guide: -1,
            console_opened: true,
            last_progress: sr.elapsed,
            nudges: 0,
            vpn_key: None,
            lb_key: None,
            site: sr.site,
        };
        if let Some(w) = &run.week {
            if w.tasks.len() != run.ts.len() {
                return false;
            }
        }
        S::refresh(&mut run.st);
        snapshot_up(&mut run);
        self.run = Some(run);
        self.screen = Screen::Play;
        self.overlays.clear();
        self.refresh_markers();
        let name = levels::name(self.run.as_ref().and_then(|r| r.week.as_ref()));
        self.toast(format!("Välkommen tillbaka! {} fortsätter där du var.", name), ToastKind::Good);
        true
    }

    pub fn saved_run_of(&self) -> Option<SavedRun> {
        let run = self.run.as_ref()?;
        if run.done {
            return None;
        }
        Some(SavedRun {
            week: run.n,
            exam: run.exam,
            state: run.st.clone(),
            tasks: run.ts.iter().map(|t| SavedTask { fixed: t.fixed, reported: t.reported, known: t.known, hints: t.hints, score: t.score }).collect(),
            elapsed: run.elapsed,
            console: run.console_target.clone(),
            commands: run.commands,
            site: run.world.site.clone(),
        })
    }

    pub fn autosave(&mut self) {
        if self.screen == Screen::Play {
            self.save.run = self.saved_run_of();
        }
        self.save.save();
    }

    pub fn leave_to_menu(&mut self) {
        self.autosave();
        // terminalen lämnar tillbaka sina sessioner
        self.close_terminal();
        self.screen = Screen::Menu;
        self.overlays.clear();
    }

    pub fn close_terminal(&mut self) {
        let App { term, run, save, .. } = self;
        if let Some(run) = run {
            let mut env = TermEnv { st: &mut run.st, consoles: &mut run.consoles, console_target: run.console_target.clone(), week: run.week.as_ref(), week_n: run.n, hist: &mut save.history, fast: save.settings.fast_output };
            term.hide(&mut env);
        }
    }

    // ------------------------------------------------------------------ markeringar och mål

    pub fn refresh_markers(&mut self) {
        let hard = self.save.settings.difficulty == "hard";
        let run = match &mut self.run {
            Some(r) => r,
            None => return,
        };
        let week = match &run.week {
            Some(w) => w.clone(),
            None => {
                run.world.set_target(None);
                return;
            }
        };
        let mut npcs: Vec<&str> = vec![];
        for t in &week.tasks {
            if !npcs.contains(&t.npc) {
                npcs.push(t.npc);
            }
        }
        for n in npcs {
            let idx: Vec<usize> = week.tasks.iter().enumerate().filter(|(_, t)| t.npc == n).map(|(i, _)| i).collect();
            let open = idx.iter().any(|i| !run.ts[*i].fixed);
            let fixed = !idx.is_empty() && idx.iter().all(|i| run.ts[*i].reported);
            let m = if open && !hard {
                Mark::Ticket
            } else if !open && fixed {
                Mark::Done
            } else {
                Mark::None
            };
            run.world.set_mark(n, m);
        }
        // kompassmålet: nästa olösta uppgift
        let target = if hard {
            None
        } else {
            week.tasks.iter().enumerate().find(|(i, _)| !run.ts[*i].fixed).and_then(|(i, t)| if !run.ts[i].known { Some(t.npc.to_string()) } else if t.target.is_empty() { None } else { Some(t.target.to_string()) })
        };
        run.world.set_target(target);
    }

    // ------------------------------------------------------------------ uppgifter

    pub fn check_tasks(&mut self) {
        let mut toasts: Vec<(String, ToastKind)> = vec![];
        let mut fixed_now = false;
        let mut all_done = false;
        {
            let run = match &mut self.run {
                Some(r) => r,
                None => return,
            };
            let week = match &run.week {
                Some(w) => w.clone(),
                None => return,
            };
            if run.done {
                return;
            }
            for (i, t) in week.tasks.iter().enumerate() {
                if run.ts[i].fixed {
                    continue;
                }
                let ok = (t.check)(&mut run.st);
                if ok {
                    run.ts[i].fixed = true;
                    run.ts[i].known = true;
                    run.last_progress = run.elapsed;
                    if t.build {
                        run.ts[i].reported = true;
                        run.ts[i].score = 2;
                        let left = run.ts.iter().filter(|s| !s.fixed).count();
                        toasts.push((format!("{} är klart!{}", t.title, if left > 0 { format!(" {} steg kvar i bygget.", left) } else { String::new() }), ToastKind::Good));
                    } else {
                        toasts.push((format!("{} är löst! Skriv felrapporten med F.", t.title), ToastKind::Good));
                    }
                    fixed_now = true;
                }
            }
            if run.ts.iter().all(|s| s.reported) && !week.tasks.is_empty() {
                all_done = true;
            }
        }
        for (t, k) in toasts {
            self.toast(t, k);
        }
        if fixed_now {
            self.refresh_markers();
            self.remind_reports();
        }
        if all_done {
            let done_before = self.run.as_ref().map(|r| r.done).unwrap_or(true);
            if !done_before {
                if let Some(r) = &mut self.run {
                    r.done = true;
                }
                let res = self.result();
                self.save_week(&res);
                self.delayed.push((0.9, Delayed::WeekDone));
            }
        }
    }

    pub fn pending_reports(&self) -> Vec<usize> {
        match &self.run {
            Some(r) => match &r.week {
                Some(w) if !w.tasks.iter().any(|t| t.build) => (0..w.tasks.len()).filter(|i| r.ts[*i].fixed && !r.ts[*i].reported).collect(),
                _ => vec![],
            },
            None => vec![],
        }
    }

    fn remind_reports(&mut self) {
        let pend = self.pending_reports();
        if pend.is_empty() {
            return;
        }
        let (all_fixed, crabs_fixed, nag) = {
            let run = self.run.as_ref().unwrap();
            let w = run.week.as_ref().unwrap();
            let all_fixed = run.ts.iter().all(|t| t.fixed);
            let crabs: Vec<usize> = (0..w.tasks.len()).filter(|i| w.tasks[*i].krabba).collect();
            let crabs_fixed = !crabs.is_empty() && crabs.iter().all(|i| run.ts[*i].fixed);
            (all_fixed, crabs_fixed, run.report_nag)
        };
        let key: Option<&'static str> = if all_fixed { Some("all") } else if crabs_fixed { Some("crabs") } else { None };
        let key = match key {
            Some(k) => k,
            None => return,
        };
        if nag == Some(key) {
            return;
        }
        if let Some(r) = &mut self.run {
            r.report_nag = Some(key);
        }
        self.delayed.push((1.4, Delayed::ReportNag(all_fixed)));
    }

    pub fn result(&self) -> WeekResult {
        let run = self.run.as_ref().unwrap();
        let minutes = ((run.elapsed / 60.0).round() as u32).max(1);
        let hints: u32 = run.ts.iter().map(|t| t.hints).sum();
        let rs: u32 = run.ts.iter().map(|t| t.score).sum();
        let max = run.ts.len() as u32 * 2;
        let stars = 1 + if rs + 1 >= max { 1 } else { 0 } + if hints <= 2 { 1 } else { 0 };
        let mut badges = vec![];
        if hints == 0 {
            badges.push("Utan ledtrådar".to_string());
        }
        if minutes <= 10 {
            badges.push("Snabb tekniker".to_string());
        }
        if rs == max {
            badges.push("Felfria rapporter".to_string());
        }
        if run.exam {
            badges.push("Examen klarad".to_string());
        }
        WeekResult { minutes, hints, report_score: rs, report_max: max, stars, commands: run.commands, badges }
    }

    fn save_week(&mut self, res: &WeekResult) {
        let n = match &self.run {
            Some(r) => r.n,
            None => return,
        };
        if n == 0 {
            return;
        }
        let old = self.save.progress.get(&n).cloned().unwrap_or_default();
        let mut badges = old.badges.clone();
        for b in &res.badges {
            if !badges.contains(b) {
                badges.push(b.clone());
            }
        }
        let p = ui_save::WeekProgress { stars: res.stars.max(old.stars), minutes: if old.minutes > 0 { old.minutes.min(res.minutes) } else { res.minutes }, badges };
        self.save.progress.insert(n, p);
        self.save.run = None;
        self.save.save();
    }

    pub fn submit_report(&mut self, idx: usize, r: ReportData) {
        if let Some(run) = &mut self.run {
            let s = &mut run.ts[idx];
            s.reported = true;
            s.score = r.score;
            s.report = Some(r);
            run.last_progress = run.elapsed;
        }
        self.refresh_markers();
        self.check_tasks();
    }

    pub fn commands_for(&self) -> Vec<String> {
        const REL: &[&str] = &["show", "sh", "ping", "tracert", "traceroute", "tracepath", "ipconfig", "nslookup", "arp", "ssh", "telnet", "ip ", "resolvectl", "config", "conf", "int", "do ", "nc ", "route", "test-net", "curl", "wget", "copy", "for ", "crypto", "clear crypto"];
        match &self.run {
            Some(r) => r
                .cmd_log
                .iter()
                .filter(|c| {
                    let l = c.line.trim().to_lowercase();
                    REL.iter().any(|p| l.starts_with(p))
                })
                .map(|c| format!("{}: {}", c.place, c.line.trim()))
                .collect(),
            None => vec![],
        }
    }

    pub fn report_markdown(&self) -> String {
        let run = match &self.run {
            Some(r) => r,
            None => return String::new(),
        };
        let w = match &run.week {
            Some(w) => w,
            None => return String::new(),
        };
        let mut lines: Vec<String> = vec![format!("# {} – {}: {}", if w.build.is_some() { "Byggrapport" } else { "Felrapport" }, levels::name(Some(w)).to_lowercase(), w.title), String::new(), "Krabba spelet.".into(), String::new()];
        for (i, t) in w.tasks.iter().enumerate() {
            let s = &run.ts[i];
            if !s.reported {
                continue;
            }
            let r = s.report.clone().unwrap_or_default();
            lines.push(format!("## Fel {}: {}{}", i + 1, t.title, if t.krabba { " (Krabba-fel)" } else { " (ärende)" }));
            lines.push(String::new());
            lines.push(format!("- **Vad vi såg:** {}", r.saw.replace('\n', " ")));
            lines.push(format!("- **Vad vi trodde först:** {}", r.first.replace('\n', " ")));
            lines.push("- **Vad vi kontrollerade:**".into());
            for l in r.checked.lines().filter(|l| !l.trim().is_empty()) {
                lines.push(format!("  - `{}`", l.replace('`', "")));
            }
            lines.push(format!("- **Vad felet var:** {}", r.cause));
            lines.push(format!("- **Hur det skulle rättas:** {}", r.fix));
            lines.push(String::new());
        }
        lines.join("\n")
    }

    // ------------------------------------------------------------------ sekundtick: klockan går

    fn housekeeping(&mut self) {
        let mut toasts: Vec<String> = vec![];
        if let Some(run) = &mut self.run {
            run.sec_n += 1;
            host_behaviour(run, &mut toasts);
            S::refresh(&mut run.st);
        }
        for t in toasts {
            self.toast(t, ToastKind::Info);
        }
        self.watch_week10();
        self.check_tasks();
        self.check_guide();
        self.nudge();
    }

    fn nudge(&mut self) {
        let hard = self.save.settings.difficulty == "hard";
        let msg = {
            let run = match &mut self.run {
                Some(r) => r,
                None => return,
            };
            if hard || run.exam || run.done || self.overlays.len() > 0 || run.elapsed - run.last_progress < 300.0 || run.nudges >= 3 {
                return;
            }
            let w = match &run.week {
                Some(w) => w,
                None => return,
            };
            let i = match (0..w.tasks.len()).find(|i| !run.ts[*i].fixed) {
                Some(i) => i,
                None => return,
            };
            let t = &w.tasks[i];
            let s = &run.ts[i];
            run.nudges += 1;
            run.last_progress = run.elapsed;
            if !s.known {
                format!("Har du pratat med {}? Det lyser ett utropstecken där.", t.npc)
            } else if s.hints == 0 {
                format!("Börja i symptomet: vilket show-kommando visar det {} beskrev? L ger en ledtråd.", t.npc)
            } else {
                "Jämför med ett ställe som fungerar. Vad skiljer sig i show-utskriften?".to_string()
            }
        };
        self.toast(format!("Omar: {}", msg), ToastKind::Info);
    }

    /// Kapitel 10: säg till när tunneln eller lastbalanserarens servrar ändrar läge
    fn watch_week10(&mut self) {
        let mut toasts: Vec<(String, ToastKind)> = vec![];
        if let Some(run) = &mut self.run {
            let has_crypto = run.st.devices.get("R1").and_then(|d| d.config.as_ref()).map(|c| c.crypto.is_some()).unwrap_or(false);
            if !has_crypto {
                run.vpn_key = None;
                run.lb_key = None;
                return;
            }
            let cs = S::crypto_status(&mut run.st, "R1");
            let up = cs.sas.iter().filter(|a| a.up).count();
            let key = format!("{}:{}/{}", cs.isakmp.first().map(|i| i.state.as_str()).unwrap_or("-"), up, cs.sas.len());
            if let Some(old) = &run.vpn_key {
                if *old != key {
                    let prev: usize = old.split(':').nth(1).and_then(|s| s.split('/').next()).and_then(|s| s.parse().ok()).unwrap_or(0);
                    if up == cs.sas.len() && up > 0 {
                        toasts.push((format!("Tunneln Göteborg <-> Borås är uppe för alla nät ({} par).", up), ToastKind::Good));
                    } else if up > prev {
                        toasts.push((format!("Ytterligare ett nät går nu genom tunneln ({} av {}).", up, cs.sas.len()), ToastKind::Info));
                    } else if up < prev {
                        toasts.push((format!("Ett nät tappade sin IPsec-SA ({} av {} uppe).", up, cs.sas.len()), ToastKind::Warn));
                    }
                }
            }
            run.vpn_key = Some(key);
            if run.st.devices.get("LB").map(|d| d.lb.is_some()).unwrap_or(false) {
                let mut parts: Vec<String> = vec![];
                for p in S::lb_status(&mut run.st, "LB") {
                    for m in p.members {
                        parts.push(format!("{}={}", m.m.name, if !m.m.enabled { "off" } else if m.up { "up" } else { "down" }));
                    }
                }
                let lk = parts.join(",");
                if let Some(old) = &run.lb_key {
                    if *old != lk {
                        let oldv: Vec<&str> = old.split(',').collect();
                        for (i, x) in parts.iter().enumerate() {
                            if oldv.get(i) == Some(&x.as_str()) {
                                continue;
                            }
                            let (nm, v) = x.split_once('=').unwrap_or((x, ""));
                            let (msg, k) = match v {
                                "up" => (format!("LB-Nordvik: {} är UP och får trafik.", nm), ToastKind::Good),
                                "down" => (format!("LB-Nordvik: {} markeras DOWN – tas ur poolen.", nm), ToastKind::Warn),
                                _ => (format!("LB-Nordvik: {} är avstängd.", nm), ToastKind::Warn),
                            };
                            toasts.push((msg, k));
                        }
                    }
                }
                run.lb_key = Some(lk);
            }
        }
        for (t, k) in toasts {
            self.toast(t, k);
        }
    }

    // ------------------------------------------------------------------ guide (vecka 1)

    pub fn guide_text(&self) -> Option<&'static str> {
        const G: [&str; 5] = [
            "Prata med Omar i fikarummet – följ pilen och tryck E.",
            "Gå till serverrummet och tryck E på SW-Nordvik-1 i Rack A.",
            "Kör kommandot som står i laptopen med Enter. Tryck sedan Enter igen för att väcka konsolen.",
            "Skräptecken? Hastigheten stämmer inte. Ctrl+A K stänger, prova sudo screen /dev/ttyUSB0 19200. (L = ledtråd)",
            "Bra! Tryck F och skriv felrapporten.",
        ];
        let run = self.run.as_ref()?;
        if run.guide >= 0 && (run.guide as usize) < G.len() {
            Some(G[run.guide as usize])
        } else {
            None
        }
    }

    fn check_guide(&mut self) {
        let mut finished = false;
        if let Some(run) = &mut self.run {
            if run.guide < 0 {
                return;
            }
            let k1 = run.week.as_ref().and_then(|w| w.tasks.iter().position(|t| t.id == "v1k1")).unwrap_or(0);
            loop {
                let ok = match run.guide {
                    0 => run.ts.get(k1).map(|t| t.known).unwrap_or(true),
                    1 => run.console_target.as_deref() == Some("SW1"),
                    2 => run.console_opened,
                    3 => run.ts.get(k1).map(|t| t.fixed).unwrap_or(true),
                    4 => run.ts.get(k1).map(|t| t.reported).unwrap_or(true),
                    _ => false,
                };
                if ok {
                    run.guide += 1;
                } else {
                    break;
                }
                if run.guide >= 5 {
                    break;
                }
            }
            if run.guide >= 5 {
                run.guide = -1;
                finished = true;
            }
        }
        if finished {
            self.toast("Guiden är klar. Resten klarar du själv – lycka till!", ToastKind::Good);
        }
    }

    // ------------------------------------------------------------------ kommandon från terminalen

    fn on_command(&mut self, dev: &str, line: &str) {
        let hist_where;
        if let Some(run) = &mut self.run {
            run.commands += 1;
            hist_where = if dev == "Tekniker" {
                "laptop".to_string()
            } else {
                run.st.devices.get(dev).map(|d| d.config.as_ref().map(|c| c.hostname.clone()).unwrap_or_else(|| dev.to_string())).unwrap_or_else(|| dev.to_string())
            };
            run.cmd_log.push(CmdEntry { place: hist_where, dev: dev.to_string(), line: line.to_string() });
            if run.cmd_log.len() > 400 {
                run.cmd_log.remove(0);
            }
            // Flaggan som vecka 4 kontrollerar: spelaren har tittat på spanning-tree på SW2
            if dev == "SW2" && is_show_sp(line) {
                run.st.flags.misc.insert("stpLooked".into(), true);
            }
        }
    }

    // ------------------------------------------------------------------ uppdatera

    fn update_play(&mut self, input: &Input, dt: f64) {
        // sim och sekundtick
        {
            let run = self.run.as_mut().unwrap();
            if !run.done || true {
                run.elapsed += dt;
            }
            S::tick(&mut run.st, dt);
            run.sec_acc += dt;
        }
        if self.run.as_ref().map(|r| r.sec_acc >= 1.0).unwrap_or(false) {
            self.run.as_mut().unwrap().sec_acc -= 1.0;
            self.housekeeping();
        }
        // loggrader till terminalen
        {
            let App { term, run, .. } = self;
            if let Some(run) = run {
                let evs: Vec<(String, String)> = run.st.log_events.drain(..).collect();
                for (d, l) in evs {
                    term.on_log(&d, &l);
                }
            }
        }
        // fördröjda dialoger
        let mut ready: Vec<Delayed> = vec![];
        for d in &mut self.delayed {
            d.0 -= dt;
        }
        let mut k = 0;
        while k < self.delayed.len() {
            if self.delayed[k].0 <= 0.0 && self.overlays.is_empty() && !self.term.open {
                ready.push(self.delayed.remove(k).1);
            } else {
                k += 1;
            }
        }
        for d in ready {
            match d {
                Delayed::WeekDone => {
                    let dl = self.week_done_dialog();
                    self.push_dlg(dl);
                }
                Delayed::ReportNag(all) => {
                    let dl = self.report_nag_dialog(all);
                    self.push_dlg(dl);
                }
            }
        }
        // demo-skript
        self.run_demo();

        // överlägg har företräde
        if !self.overlays.is_empty() {
            self.update_overlays(input);
            let App { run, .. } = self;
            let r = run.as_mut().unwrap();
            let _ = r.world.update(input, dt, &r.st, true);
            return;
        }
        // terminalen
        if self.term.open {
            let evs = {
                let App { term, run, save, .. } = self;
                let run = run.as_mut().unwrap();
                let mut env = TermEnv { st: &mut run.st, consoles: &mut run.consoles, console_target: run.console_target.clone(), week: run.week.as_ref(), week_n: run.n, hist: &mut save.history, fast: save.settings.fast_output };
                term.update(input, dt, &mut env)
            };
            if let Some(c) = self.term.clip.take() {
                self.clip = Some(c);
            }
            for e in evs {
                match e {
                    TermEvent::Command(dev, line) => self.on_command(&dev, &line),
                    TermEvent::AfterCommand => self.after_command(),
                    TermEvent::ConsoleConnected(_) => {
                        if let Some(r) = &mut self.run {
                            r.console_opened = true;
                        }
                    }
                }
            }
            let run = self.run.as_mut().unwrap();
            let _ = run.world.update(input, dt, &run.st, true);
            return;
        }
        // snabbtangenter
        for k in &input.pressed {
            match k {
                Key::T => self.open_laptop(None),
                Key::F => self.open_report_picker(),
                Key::L => self.open_hint_picker(),
                Key::H => self.open_handbook("cmd"),
                Key::Tab => {
                    let d = self.tasks_dialog();
                    self.push_dlg(d);
                }
                Key::M => {
                    let d = self.map_dialog();
                    self.push_dlg(d);
                }
                Key::N => {
                    let d = self.notes_dialog();
                    self.push_dlg(d);
                }
                Key::F1 => {
                    let d = self.help_dialog();
                    self.push_dlg(d);
                }
                Key::U => self.save.settings.hud = !self.save.settings.hud,
                Key::Escape => {
                    let d = self.pause_dialog();
                    self.push_dlg(d);
                }
                _ => {}
            }
        }
        if !self.overlays.is_empty() || self.term.open {
            let run = self.run.as_mut().unwrap();
            let _ = run.world.update(input, dt, &run.st, true);
            return;
        }
        // klick på gula rutan
        if input.clicked && self.hud_report_rect.contains(input.mouse_x, input.mouse_y) && !self.pending_reports().is_empty() {
            self.open_report_picker();
            return;
        }
        let events = {
            let run = self.run.as_mut().unwrap();
            run.world.update(input, dt, &run.st, false)
        };
        for e in events {
            self.handle_world_event(e);
        }
    }

    pub fn after_command(&mut self) {
        if let Some(run) = &mut self.run {
            S::refresh(&mut run.st);
        }
        self.watch_week10();
        self.check_tasks();
    }

    pub fn open_laptop(&mut self, prefill: Option<&str>) {
        let App { term, run, save, .. } = self;
        if let Some(run) = run {
            let mut env = TermEnv { st: &mut run.st, consoles: &mut run.consoles, console_target: run.console_target.clone(), week: run.week.as_ref(), week_n: run.n, hist: &mut save.history, fast: save.settings.fast_output };
            term.font = save.settings.term_font as f32;
            term.theme = save.settings.term_theme.clone();
            term.open_laptop(&mut env, prefill);
        }
    }

    pub fn open_windows(&mut self, host: &str) {
        let App { term, run, save, .. } = self;
        if let Some(run) = run {
            let mut env = TermEnv { st: &mut run.st, consoles: &mut run.consoles, console_target: run.console_target.clone(), week: run.week.as_ref(), week_n: run.n, hist: &mut save.history, fast: save.settings.fast_output };
            term.font = save.settings.term_font as f32;
            term.theme = save.settings.term_theme.clone();
            term.open_windows(&mut env, host);
        }
    }

    pub fn connect_console(&mut self, id: &str) {
        let name = {
            let run = match &mut self.run {
                Some(r) => r,
                None => return,
            };
            let d = match run.st.devices.get(id) {
                Some(d) => d,
                None => return,
            };
            let name = match &d.config {
                Some(c) => c.hostname.clone(),
                None => match d.kind {
                    crate::net::model::Kind::Lb => "LB-Nordvik".to_string(),
                    _ => "WLC-Nordvik".to_string(),
                },
            };
            run.console_target = Some(id.to_string());
            name
        };
        self.toast(format!("Den ljusblå konsolkabeln sitter nu i {}.", name), ToastKind::Info);
        self.close_terminal();
        self.open_laptop(Some("sudo screen /dev/ttyUSB0 9600"));
        self.check_guide();
    }

    fn handle_world_event(&mut self, e: WorldEvent) {
        match e {
            WorldEvent::Console(id) => self.connect_console(&id),
            WorldEvent::UseHost(id) => {
                let d = self.pc_dialog(&id);
                self.push_dlg(d);
            }
            WorldEvent::Laptop => self.open_laptop(None),
            WorldEvent::Talk(name) => {
                let d = self.npc_dialog(&name);
                self.push_dlg(d);
            }
            WorldEvent::Cable(id) => self.cable_event(&id),
            WorldEvent::Rack(name) => {
                if let Some(run) = &self.run {
                    let ids = run.world.rack_units(&name);
                    let title = match name.as_str() {
                        "A" => "Rack A – nät",
                        "B" => "Rack B – servrar",
                        "Molndal" => "Mölndals rack",
                        _ => "Lagrets rack",
                    };
                    let rv = ui_rack::RackView::new(title, &ids, &run.st);
                    self.overlays.push(Overlay::Rack(rv));
                }
            }
            WorldEvent::Travel(site) => self.travel(&site),
            WorldEvent::Pickup(what) => {
                let w = what.to_lowercase();
                if w.contains("mobil") || w.contains("phone") {
                    let d = self.phone_dialog();
                    self.push_dlg(d);
                } else if w.contains("mini") || w.contains("switch") {
                    let removed = self.run.as_mut().map(|r| levels::remove_mini_switch(&mut r.st)).unwrap_or(false);
                    if removed {
                        if let Some(r) = &mut self.run {
                            S::refresh(&mut r.st);
                        }
                        self.toast("Du kopplade bort miniswitchen och satte tillbaka gästlaptopens kabel direkt i uttaget.", ToastKind::Info);
                        self.check_tasks();
                    }
                } else {
                    self.toast(format!("Du tog upp: {}", what), ToastKind::Info);
                }
            }
            WorldEvent::Use(what) => {
                let w = what.to_lowercase();
                if w.contains("whiteboard") || w.contains("tavla") {
                    let d = self.whiteboard_dialog();
                    self.push_dlg(d);
                } else if w.contains("monitor") || w.contains("övervak") {
                    let d = self.monitor_dialog();
                    self.push_dlg(d);
                } else if w.contains("coffee") || w.contains("kaffe") {
                    self.toast("Kaffe! Perfekt inför nästa felsökning.", ToastKind::Good);
                } else if w.contains("water") || w.contains("vatten") {
                    self.toast("Uppfriskande! Glöm inte att dricka vatten mellan felsökningarna.", ToastKind::Info);
                } else if w.contains("printer") || w.contains("skrivare") {
                    self.print_page();
                } else {
                    self.toast(what, ToastKind::Info);
                }
            }
        }
    }

    fn print_page(&mut self) {
        let msg = {
            let run = match &mut self.run {
                Some(r) => r,
                None => return,
            };
            let ok = if run.st.devices.contains_key("PC-Lisa") { S::ping(&mut run.st, "PC-Lisa", "192.168.1.11", Default::default()).ok } else { true };
            if !ok {
                "Skrivaren svarar inte. Något i nätet är trasigt mellan kontoret och skrivaren.".to_string()
            } else {
                let open = run.ts.iter().filter(|t| !t.reported).count();
                format!("Skrivaren skrev ut veckans ärendelista: {}.", if run.week.is_some() { format!("{} ärenden kvar", open) } else { "fri träning, inga ärenden".to_string() })
            }
        };
        self.toast(msg, ToastKind::Info);
    }

    fn travel(&mut self, site: &str) {
        if let Some(run) = &mut self.run {
            run.world.set_site(site);
            run.site = site.to_string();
        }
        let msg = match site {
            "boras" => "Du är framme vid lagret i Borås.",
            "molndal" => "Du är framme på Mölndalskontoret.",
            _ => "Tillbaka på huvudkontoret i Göteborg.",
        };
        self.toast(msg, ToastKind::Info);
    }

    fn cable_event(&mut self, id: &str) {
        let (state, label) = {
            let run = match &self.run {
                Some(r) => r,
                None => return,
            };
            let l = run.st.links.iter().find(|l| l.id == id).or_else(|| run.st.links.iter().find(|l| l.a.dev == id || l.b.dev == id));
            match l {
                Some(l) => (l.state, link_name(&run.st, l)),
                None => return,
            }
        };
        let link_id = self.run.as_ref().unwrap().st.links.iter().find(|l| l.id == id).or_else(|| self.run.as_ref().unwrap().st.links.iter().find(|l| l.a.dev == id || l.b.dev == id)).map(|l| l.id.clone()).unwrap();
        let d = self.cable_dialog(&link_id, state, &label);
        self.push_dlg(d);
    }

    pub fn set_cable(&mut self, link_id: &str, plugged: bool, replaced: bool) {
        let msg = {
            let run = match &mut self.run {
                Some(r) => r,
                None => return,
            };
            let name = match run.st.links.iter().find(|l| l.id == link_id) {
                Some(l) => link_name(&run.st, l),
                None => return,
            };
            let was = run.st.links.iter().find(|l| l.id == link_id).map(|l| l.state).unwrap();
            if let Some(l) = run.st.links.iter_mut().find(|l| l.id == link_id) {
                l.state = if plugged { crate::net::model::LinkState::Ok } else { crate::net::model::LinkState::Unplugged };
            }
            S::touch(&mut run.st);
            S::refresh(&mut run.st);
            if replaced {
                if was == crate::net::model::LinkState::Ok { format!("Du bytte kabeln {}. Ingen skillnad – den gamla var hel.", name) } else { format!("Du bytte patchkabeln {}.", name) }
            } else if plugged {
                format!("Du satte tillbaka kabeln {}.", name)
            } else {
                format!("Du drog ur kabeln {}.", name)
            }
        };
        self.toast(msg, ToastKind::Info);
        self.check_tasks();
    }

    // ------------------------------------------------------------------ överlägg

    pub fn update_overlays(&mut self, input: &Input) {
        let top = match self.overlays.last_mut() {
            Some(t) => t,
            None => return,
        };
        match top {
            Overlay::Rack(r) => {
                r.update(input);
                if r.closed {
                    let pick = r.pick.take();
                    self.overlays.pop();
                    if let Some(id) = pick {
                        self.connect_console(&id);
                    }
                }
            }
            Overlay::Hand(h) => {
                h.update(input);
                if let Some(c) = h.clip.take() {
                    self.clip = Some(c);
                }
                if h.closed {
                    self.overlays.pop();
                }
            }
            Overlay::Dlg(d) => {
                let out = d.update(input);
                match out {
                    DlgOut::None => {}
                    DlgOut::Cancel => {
                        // Esc stänger; i dialoger med enbart val tolkas det som sista knappen om den är Close
                        self.overlays.pop();
                        self.after_cancel();
                    }
                    DlgOut::Press(j) => {
                        let act = d.buttons[j].act.clone();
                        let keep = d.buttons[j].keep;
                        let mut dlg = match self.overlays.pop() {
                            Some(Overlay::Dlg(d)) => d,
                            _ => return,
                        };
                        let keep_open = self.run_act(act, &mut dlg);
                        if keep || keep_open {
                            self.overlays.push(Overlay::Dlg(dlg));
                        }
                    }
                }
            }
        }
    }

    fn after_cancel(&mut self) {
        // avbruten frågesport
        if self.overlays.is_empty() {
            self.quiz = None;
        }
    }

    pub fn open_handbook(&mut self, tab: &str) {
        let mine = {
            let mut seen: Vec<String> = vec![];
            let mut v: Vec<(String, String)> = vec![];
            if let Some(r) = &self.run {
                for c in r.cmd_log.iter().rev() {
                    let l = c.line.trim().to_string();
                    if !l.is_empty() && !seen.contains(&l) && v.len() < 40 {
                        seen.push(l.clone());
                        v.push((c.place.clone(), l));
                    }
                }
            }
            v
        };
        let weeks: Vec<(u32, String, String)> = self.weeks().iter().filter(|w| w.build.is_none()).map(|w| (w.week, w.title.to_string(), w.chips.first().map(|s| s.to_string()).unwrap_or_default())).collect();
        self.overlays.push(Overlay::Hand(Handbook::new(tab, mine, weeks)));
    }

    // ------------------------------------------------------------------ demo

    fn run_demo(&mut self) {
        if self.demo.is_empty() {
            return;
        }
        let f = self.frame;
        let mut i = 0;
        while i < self.demo.len() {
            if self.demo[i].0 <= f {
                let (_, cmd) = self.demo.remove(i);
                match cmd {
                    DemoCmd::Rack(n) => self.handle_world_event(WorldEvent::Rack(n)),
                    DemoCmd::ConsoleTo(id) => {
                        let id = if self.run.as_ref().map(|r| r.st.devices.contains_key(&id)).unwrap_or(false) { id } else { "SW1".to_string() };
                        self.connect_console(&id);
                    }
                }
            } else {
                i += 1;
            }
        }
    }
}

fn link_name(st: &State, l: &crate::net::model::Link) -> String {
    fn nm(st: &State, e: &crate::net::model::End) -> String {
        let n = st.devices.get(&e.dev).map(|d| d.config.as_ref().map(|c| c.hostname.clone()).unwrap_or_else(|| if d.label.is_empty() { e.dev.clone() } else { d.label.clone() })).unwrap_or_else(|| e.dev.clone());
        if e.port != "nic" { format!("{} {}", n, crate::net::util::short_if(&e.port)) } else { n }
    }
    format!("{} <-> {}", nm(st, &l.a), nm(st, &l.b))
}

fn is_show_sp(line: &str) -> bool {
    // ^\s*(do\s+)?sh\w*\s+sp
    let l = line.trim_start().to_lowercase();
    let l = match l.strip_prefix("do ") {
        Some(r) => r.trim_start().to_string(),
        None => l,
    };
    if !l.starts_with("sh") {
        return false;
    }
    let rest: Vec<&str> = l.splitn(2, char::is_whitespace).collect();
    if rest.len() < 2 {
        return false;
    }
    rest[0].chars().all(|c| c.is_alphanumeric() || c == '_') && rest[1].trim_start().starts_with("sp")
}

fn snapshot_up(run: &mut Run) {
    let d = S::get(&mut run.st);
    let ids: Vec<String> = run.st.devices.keys().cloned().collect();
    for id in ids {
        if let Some(e) = d.eps.get(&format!("E:{}:nic", id)) {
            run.prev_up.insert(id, e.up);
        }
    }
}

/// Windows ber om ny adress när kabeln kommer tillbaka, och försöker igen efter APIPA var 5:e sekund
fn host_behaviour(run: &mut Run, toasts: &mut Vec<String>) {
    let d = S::get(&mut run.st);
    let ids: Vec<String> = run.st.devices.iter().filter(|(_, h)| h.nic.as_ref().map(|n| n.dhcp).unwrap_or(false)).map(|(k, _)| k.clone()).collect();
    let mut changed = false;
    let retry = run.sec_n % 5 == 0;
    for id in ids {
        let key = format!("E:{}:nic", id);
        let ep = d.eps.get(&key);
        let up = ep.map(|e| e.up).unwrap_or(false);
        let was = run.prev_up.get(&id).copied();
        run.prev_up.insert(id.clone(), up);
        if up && was != Some(true) {
            S::dhcp(&mut run.st, &id);
            changed = true;
            continue;
        }
        let seg = if up { ep.and_then(|e| d.comp.get(&e.key)).copied() } else { None };
        let sig = match seg {
            Some(s) => {
                let mut ips: Vec<String> = d.by_seg.get(&s).map(|v| v.iter().filter_map(|k| d.eps.get(k)).filter(|x| x.up && run.st.devices.get(&x.dev).map(|dd| dd.kind == crate::net::model::Kind::Router).unwrap_or(false)).filter_map(|x| x.ip.clone()).collect()).unwrap_or_default();
                ips.sort();
                ips.join(",")
            }
            None => String::new(),
        };
        let seg_changed = run.prev_seg.get(&id).map(|p| *p != sig).unwrap_or(false);
        run.prev_seg.insert(id.clone(), sig);
        let label = run.st.devices.get(&id).map(|h| if h.label.is_empty() { id.clone() } else { h.label.clone() }).unwrap_or_else(|| id.clone());
        if up && seg_changed {
            let lease = S::dhcp(&mut run.st, &id);
            changed = true;
            toasts.push(format!("{} fick en ny adress av sig själv: {}", label, lease.ip));
            continue;
        }
        let apipa = run.st.devices.get(&id).and_then(|h| h.lease.as_ref()).map(|l| l.apipa);
        if up && apipa == Some(true) && retry {
            let lease = S::dhcp(&mut run.st, &id);
            changed = true;
            if !lease.apipa {
                toasts.push(format!("{} fick en ny adress av sig själv: {}", label, lease.ip));
            }
        }
        if up && apipa.is_none() {
            S::dhcp(&mut run.st, &id);
            changed = true;
        }
    }
    if changed {
        S::touch(&mut run.st);
    }
}

// ------------------------------------------------------------------ Game-trait

impl engine::Game for App {
    fn update(&mut self, input: &Input, dt: f64) -> bool {
        let dt = dt.min(0.1);
        self.time += dt;
        self.frame += 1;
        self.mouse = (input.mouse_x, input.mouse_y);
        for t in &mut self.toasts {
            t.ttl -= dt;
        }
        self.toasts.retain(|t| t.ttl > 0.0);
        match self.screen {
            Screen::Menu => self.update_menu(input),
            Screen::Play => {
                self.update_play(input, dt);
                self.save_timer += dt;
                if self.save_timer > 10.0 {
                    self.save_timer = 0.0;
                    self.autosave();
                }
            }
        }
        if self.quit {
            self.autosave_on_quit();
        }
        !self.quit
    }

    fn draw(&mut self, c: &mut Canvas, fonts: &mut Fonts) {
        match self.screen {
            Screen::Menu => self.draw_menu(c, fonts),
            Screen::Play => self.draw_play(c, fonts),
        }
        // överlägg
        let mouse = self.mouse;
        let t = self.time;
        for o in &mut self.overlays {
            match o {
                Overlay::Rack(r) => {
                    if let Some(run) = &mut self.run {
                        r.draw(c, fonts, &mut run.st, mouse, t);
                    }
                }
                Overlay::Dlg(d) => d.draw(c, fonts, mouse, t),
                Overlay::Hand(h) => h.draw(c, fonts, mouse),
            }
        }
        self.draw_toasts(c, fonts);
    }

    fn take_clipboard(&mut self) -> Option<String> {
        self.clip.take()
    }
}

impl App {
    fn autosave_on_quit(&mut self) {
        if self.screen == Screen::Play {
            self.save.run = self.saved_run_of();
        }
        self.save.save();
    }

    fn draw_play(&mut self, c: &mut Canvas, fonts: &mut Fonts) {
        if let Some(run) = &mut self.run {
            run.world.sync(&mut run.st);
            run.world.draw(c, fonts, &run.st);
        }
        if self.save.settings.hud {
            self.draw_hud(c, fonts);
        }
        if self.term.open {
            let mouse = self.mouse;
            let App { term, run, save, .. } = self;
            if let Some(run) = run {
                let mut env = TermEnv { st: &mut run.st, consoles: &mut run.consoles, console_target: run.console_target.clone(), week: run.week.as_ref(), week_n: run.n, hist: &mut save.history, fast: save.settings.fast_output };
                term.draw(c, fonts, &mut env, mouse);
            }
        }
    }
}

// ------------------------------------------------------------------ headless-demo

/// `krabba_spelet --headless-demo <katalog>`: kör menyn, vecka 1, en promenad, terminalen och dialoger med inspelad inmatning.
pub fn headless_demo(dir: &str) {
    let _ = std::fs::create_dir_all(dir);
    let mut app = App::new();
    // egen sparfil så att demon inte rör spelarens
    unsafe {
        std::env::set_var("KRABBA_SAVE", format!("{}/demo-save.json", dir));
    }
    app.save = SaveData::default();
    app.save.settings.fast_output = true;
    let mut inputs: Vec<Input> = vec![];
    let mut shots: Vec<(usize, String)> = vec![];
    let blank = || Input::default();
    let key = |k: Key| {
        let mut i = Input::default();
        i.pressed.push(k);
        i
    };
    fn typed(inputs: &mut Vec<Input>, s: &str) {
        for ch in s.chars() {
            let mut i = Input::default();
            i.chars.push(ch);
            inputs.push(i);
        }
    }
    let wait = |inputs: &mut Vec<Input>, n: usize| {
        for _ in 0..n {
            inputs.push(Input::default());
        }
    };
    let enter = || {
        let mut i = Input::default();
        i.pressed.push(Key::Enter);
        i
    };
    wait(&mut inputs, 10);
    shots.push((inputs.len().saturating_sub(1), format!("{}/01-meny.png", dir)));
    inputs.push(blank());
    inputs.push(key(Key::Num1));
    wait(&mut inputs, 10);
    shots.push((inputs.len().saturating_sub(1), format!("{}/02-intro.png", dir)));
    inputs.push(enter());
    wait(&mut inputs, 5);
    // gå runt
    for _ in 0..90 {
        let mut i = Input::default();
        i.down.insert(Key::D);
        inputs.push(i);
    }
    for _ in 0..60 {
        let mut i = Input::default();
        i.down.insert(Key::S);
        inputs.push(i);
    }
    shots.push((inputs.len().saturating_sub(1), format!("{}/03-varlden.png", dir)));
    inputs.push(key(Key::T));
    wait(&mut inputs, 5);
    typed(&mut inputs, "help");
    inputs.push(enter());
    wait(&mut inputs, 5);
    typed(&mut inputs, "ping -c 2 192.168.1.193");
    inputs.push(enter());
    wait(&mut inputs, 120);
    shots.push((inputs.len().saturating_sub(1), format!("{}/04-terminal.png", dir)));
    // konsolen mot SW1
    inputs.push(key(Key::Escape));
    wait(&mut inputs, 3);
    app.demo.push((inputs.len() as u64, DemoCmd::ConsoleTo("SW1".into())));
    wait(&mut inputs, 5);
    inputs.push(enter());
    wait(&mut inputs, 5);
    inputs.push(enter());
    wait(&mut inputs, 5);
    typed(&mut inputs, "enable");
    inputs.push(enter());
    wait(&mut inputs, 3);
    typed(&mut inputs, "show ip interface brief");
    inputs.push(enter());
    wait(&mut inputs, 60);
    typed(&mut inputs, "show vlan ");
    inputs.push(key(Key::Tab));
    shots.push((inputs.len().saturating_sub(1), format!("{}/05-konsol.png", dir)));
    wait(&mut inputs, 3);
    // Ctrl+A K stänger screen, sedan rätt hastighet
    let mut ca = Input::default();
    ca.ctrl = true;
    ca.down.insert(Key::Ctrl);
    ca.pressed.push(Key::A);
    inputs.push(ca);
    inputs.push(key(Key::K));
    wait(&mut inputs, 3);
    let mut cu = Input::default();
    cu.ctrl = true;
    cu.down.insert(Key::Ctrl);
    cu.pressed.push(Key::U);
    inputs.push(cu);
    typed(&mut inputs, "sudo screen /dev/ttyUSB0 19200");
    inputs.push(enter());
    wait(&mut inputs, 5);
    inputs.push(enter());
    wait(&mut inputs, 3);
    inputs.push(enter());
    wait(&mut inputs, 3);
    typed(&mut inputs, "enable");
    inputs.push(enter());
    typed(&mut inputs, "show ip interface brief");
    inputs.push(enter());
    wait(&mut inputs, 10);
    typed(&mut inputs, "conf");
    inputs.push(key(Key::Tab));
    typed(&mut inputs, "?");
    wait(&mut inputs, 3);
    shots.push((inputs.len().saturating_sub(1), format!("{}/05b-konsol-ok.png", dir)));
    let mut esc = Input::default();
    esc.pressed.push(Key::Escape);
    inputs.push(esc);
    wait(&mut inputs, 3);
    inputs.push(key(Key::F1));
    wait(&mut inputs, 3);
    shots.push((inputs.len().saturating_sub(1), format!("{}/06-hjalp.png", dir)));
    inputs.push(key(Key::Escape));
    wait(&mut inputs, 3);
    inputs.push(key(Key::H));
    wait(&mut inputs, 3);
    shots.push((inputs.len().saturating_sub(1), format!("{}/07-handbok.png", dir)));
    inputs.push(key(Key::Tab));
    wait(&mut inputs, 3);
    inputs.push(key(Key::Escape));
    wait(&mut inputs, 3);
    inputs.push(key(Key::Tab));
    wait(&mut inputs, 3);
    shots.push((inputs.len().saturating_sub(1), format!("{}/08-arenden.png", dir)));
    inputs.push(key(Key::Escape));
    wait(&mut inputs, 3);
    inputs.push(key(Key::L));
    wait(&mut inputs, 3);
    shots.push((inputs.len().saturating_sub(1), format!("{}/09-ledtrad.png", dir)));
    inputs.push(key(Key::Escape));
    wait(&mut inputs, 3);
    app.demo.push((inputs.len() as u64, DemoCmd::Rack("A".into())));
    wait(&mut inputs, 5);
    shots.push((inputs.len().saturating_sub(1), format!("{}/10-rack.png", dir)));
    wait(&mut inputs, 2);
    engine::run_headless(&mut app, &inputs, &shots);
}
