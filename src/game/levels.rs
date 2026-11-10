//! Veckorna (1–10), byggena (101–105) och kurs 2 (11–14). Port av js/levels.js, js/build.js och js/kurs2.js.
#![allow(unused_variables, dead_code)]
use crate::net::sim::State;

pub type CheckFn = fn(&mut State) -> bool;
pub type SetupFn = fn(&mut State);

#[derive(Clone, Debug)]
pub struct Report {
    pub cause: Vec<&'static str>,
    pub fix: Vec<&'static str>,
}

#[derive(Clone, Debug)]
pub struct Task {
    pub id: &'static str,
    /// Krabba-fel (true) eller kollegans ärende (false)
    pub krabba: bool,
    /// Byggsteg (bockas av automatiskt, ingen felrapport)
    pub build: bool,
    pub npc: &'static str,
    /// Enheten felet sitter i (för kompass/mål)
    pub target: &'static str,
    pub title: &'static str,
    pub ticket: &'static str,
    pub hints: Vec<&'static str>,
    /// Kontroll: är felet rättat? Spelets flaggor ligger i st.flags.misc (t.ex. "stpLooked").
    pub check: CheckFn,
    /// Svarsalternativ i felrapporten; första alternativet är rätt.
    pub report: Option<Report>,
    /// Flagga som spelet sätter när spelaren gjort något (t.ex. "stpLooked")
    pub flag: Option<&'static str>,
}

#[derive(Clone, Debug)]
pub struct Quiz {
    pub q: &'static str,
    pub options: Vec<&'static str>,
    /// Index för rätt svar
    pub answer: usize,
    pub explain: &'static str,
}

#[derive(Clone, Debug)]
pub struct Week {
    /// 1–14 för veckor, 101–105 för byggen
    pub week: u32,
    /// Byggets nummer (1–5) om det är ett bygge
    pub build: Option<u32>,
    /// 1 eller 2
    pub kurs: u32,
    pub title: &'static str,
    pub chapter: &'static str,
    /// Startplats: "gbg" eller "boras"
    pub site: &'static str,
    pub course: Option<&'static str>,
    pub intro: &'static str,
    pub learn: Vec<&'static str>,
    /// Förslag på kommandon i terminalen
    pub chips: Vec<&'static str>,
    pub tasks: Vec<Task>,
    pub quiz: Vec<Quiz>,
    /// Veckoutmaning (kurs 2), fritext
    pub challenge: Option<&'static str>,
}

pub const DHCP_HOSTS: &[&str] = &[
    "PC-Anna", "PC-Karim", "PC-Bo", "PC-Maja", "PC-Sara", "PC-Lisa", "Gast-laptop", "PC-Lager", "Gast-mobil", "Lager-skanner",
];

/// Alla veckor och byggen i menyordning
pub fn all() -> Vec<Week> { todo!("levels::all") }
pub fn def(n: u32) -> Option<Week> { all().into_iter().find(|w| w.week == n) }
/// "Vecka 3", "Bygge 2" eller "Fri träning"
pub fn name(w: Option<&Week>) -> String {
    match w {
        None => "Fri träning".into(),
        Some(w) => match w.build { Some(b) => format!("Bygge {}", b), None => format!("Vecka {}", w.week) },
    }
}
pub fn is_build(n: u32) -> bool { n > 100 }
/// Det felfria nätet för en vecka, med DHCP-adresser utdelade (baseState)
pub fn base_state(week: u32) -> State { todo!("levels::base_state") }
/// Startar en vecka eller ett bygge: bygger nätet, sätter in felen och kör after()
pub fn start(n: u32) -> (State, Option<Week>) { todo!("levels::start") }
/// Vecka 9: ta bort den okända miniswitchen i receptionen
pub fn remove_mini_switch(st: &mut State) -> bool { todo!("levels::remove_mini_switch") }
