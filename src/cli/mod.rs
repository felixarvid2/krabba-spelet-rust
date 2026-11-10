//! Kommandotolkar: Cisco IOS, Windows/Linux, WLC och lastbalanseraren.
//! Gemensamma typer som terminalen (game::terminal) använder.
use crate::net::sim::State;

pub mod host;
pub mod ios;
pub mod ios_cmds;
pub mod ios_dyn;
pub mod ios_help;
pub mod ios_more;
pub mod ios_show;
pub mod lb;
pub mod wlc;

/// En bit utskrift som visas efter en fördröjning (ms), t.ex. ping-tecken.
#[derive(Clone, Debug, Default)]
pub struct Chunk {
    pub text: String,
    pub delay: u32,
}

/// Terminalen ska öppna en ny session ovanpå den nuvarande (push i JS).
#[derive(Clone, Debug)]
pub enum PushReq {
    /// screen /dev/ttyUSB0 <speed> mot enheten som konsolkabeln sitter i
    Serial { target: String, speed: u32 },
    /// ssh/telnet in till en IOS-enhet
    Ios { dev: String, via: String, privileged: bool, user: Option<String> },
    Wlc { dev: String },
    Lb { dev: String },
}

/// Resultatet av ett kommando.
#[derive(Clone, Debug, Default)]
pub struct CmdResult {
    pub out: String,
    /// Fördröjning (ms) innan `out` visas
    pub delay: u32,
    /// Strömmad utskrift som visas bit för bit efter `out`
    pub stream: Vec<Chunk>,
    /// Sessionen stängs (exit/logout)
    pub close: bool,
    /// Vänta på Enter efter strömmen (omstart: "Press RETURN to get started!")
    pub wait_return: bool,
    pub push: Option<PushReq>,
    /// Rensa skärmen (cls/clear)
    pub clear: bool,
}

impl CmdResult {
    pub fn out(s: impl Into<String>) -> CmdResult {
        CmdResult { out: s.into(), ..Default::default() }
    }
    pub fn empty() -> CmdResult {
        CmdResult::default()
    }
    pub fn delayed(s: impl Into<String>, delay: u32) -> CmdResult {
        CmdResult { out: s.into(), delay, ..Default::default() }
    }
    pub fn stream(stream: Vec<Chunk>) -> CmdResult {
        CmdResult { stream, ..Default::default() }
    }
}

impl From<String> for CmdResult {
    fn from(s: String) -> Self {
        CmdResult::out(s)
    }
}
impl From<&str> for CmdResult {
    fn from(s: &str) -> Self {
        CmdResult::out(s)
    }
}

/// Gemensamt gränssnitt för alla sessioner som terminalen kan visa.
pub trait Cli {
    /// Prompten som visas före inmatningen (eller en väntande fråga, t.ex. "Password: ")
    fn prompt_text(&self, st: &State) -> String;
    fn handle(&mut self, st: &mut State, line: &str) -> CmdResult;
    /// Svar på "?" (raden fram till frågetecknet)
    fn help(&mut self, st: &mut State, line: &str) -> String {
        let _ = (st, line);
        String::new()
    }
    /// Tab-komplettering: hela den nya raden, eller None
    fn complete(&mut self, st: &mut State, line: &str) -> Option<String> {
        let _ = (st, line);
        None
    }
    /// Inmatningen ska döljas (lösenord)
    fn secret_input(&self) -> bool {
        false
    }
    fn closed(&self) -> bool;
    /// Enheten sessionen kör på
    fn dev_id(&self) -> &str;
    /// IOS: enheten håller på att starta om (väntar på Enter)
    fn booting(&self) -> bool {
        false
    }
    fn finish_boot(&mut self, st: &mut State) {
        let _ = st;
    }
    /// Ctrl+C / Ctrl+Z (IOS: tillbaka till exec, avbryt fråga)
    fn interrupt(&mut self, st: &mut State, ctrl_z: bool) {
        let _ = (st, ctrl_z);
    }
    /// För att kunna nå IOS-specifika saker
    fn as_ios(&mut self) -> Option<&mut ios::IosSession> {
        None
    }
}

/// Föreslår ett känt ord när ett kommando är felstavat (NV.suggestCmd): Levenshtein ≤ 2.
pub fn suggest_cmd(word: &str, list: &[&str]) -> Option<String> {
    if word.len() < 3 {
        return None;
    }
    let mut best: Option<(usize, &str)> = None;
    for k in list {
        let d = levenshtein(word, k);
        if d <= 2 && best.map(|b| d < b.0).unwrap_or(true) {
            best = Some((d, k));
        }
    }
    best.map(|b| b.1.to_string())
}

pub fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            let c = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + c);
        }
        prev = cur;
    }
    prev[b.len()]
}
