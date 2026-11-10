//! Karriären: erfarenhetspoäng, rang, prestationer och statistik. Port av js/ui/career.js
//! (och prestationerna från v7/v8/v10 som gäller 2D).
#![allow(unused_variables, dead_code)]
use crate::engine::{Canvas, Fonts, Input};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// En prestation (id, namn, beskrivning, ikon)
#[derive(Clone, Debug)]
pub struct Achievement {
    pub id: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    pub icon: &'static str,
}

/// Det spelet behöver veta om framstegen för prestationer som gäller veckor och byggen.
#[derive(Clone, Debug, Default)]
pub struct ProgressInfo {
    /// Klarade veckor/byggen (veckonummer -> stjärnor 0–3)
    pub weeks: BTreeMap<u32, u32>,
}

/// Sparas i save.json (serde).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Career {
    pub xp: i64,
    /// Statistik: "dist", "commands", "pings", "fixed", "reports", "perfect", "coffees", "crabs", "hints",
    /// "playSec", "cables", "bestCombo", "noHintWeeks", "fastWeeks", "exams", "trips", "dfPings",
    /// "cryptoShows", "curls", "animals" m.fl.
    pub stats: BTreeMap<String, f64>,
    /// Upplåsta prestationer: id -> sekunder sedan epoken
    pub ach: BTreeMap<String, u64>,
}

impl Career {
    /// Lägg till XP. Returnerar Some(ny rang) om rangen höjdes.
    pub fn add_xp(&mut self, n: i64, why: &str) -> Option<String> { todo!() }
    /// Öka en statistikräknare
    pub fn bump(&mut self, stat: &str, by: f64) { todo!() }
    /// Sätt en räknare till max(gammalt, v)
    pub fn max_stat(&mut self, stat: &str, v: f64) { todo!() }
    /// Kontrollera prestationerna. Returnerar de som just låstes upp (och ger deras XP).
    pub fn check(&mut self, p: &ProgressInfo) -> Vec<Achievement> { todo!() }
    /// (rangens namn, xp för nuvarande rang, xp för nästa rang eller None)
    pub fn rank(&self) -> (&'static str, i64, Option<i64>) { todo!() }
    pub fn all_achievements() -> Vec<Achievement> { todo!() }
}

/// Karriärdialogen (J): rang, XP-stapel, prestationer (låsta/upplåsta) och statistik.
pub struct CareerView {
    pub closed: bool,
}

impl CareerView {
    pub fn new() -> CareerView { CareerView { closed: false } }
    /// Esc/J/klick på Stäng sätter closed
    pub fn update(&mut self, input: &Input) { todo!() }
    pub fn draw(&mut self, c: &mut Canvas, fonts: &mut Fonts, career: &Career, p: &ProgressInfo) { todo!() }
}

/// XP-stapeln i HUD:en (liten, t.ex. under veckorubriken)
pub fn draw_xp_bar(c: &mut Canvas, fonts: &mut Fonts, x: i32, y: i32, w: i32, career: &Career) { todo!() }
