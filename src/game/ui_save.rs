//! Sparning: framsteg, inställningar, anteckningar och pågående vecka som JSON.
//! Linux/macOS: $HOME/.krabba-spelet/save.json, Windows: %APPDATA%\krabba-spelet\save.json.
use crate::net::sim::State;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// "easy", "normal" eller "hard"
    pub difficulty: String,
    /// Terminalens textstorlek (pixlar)
    pub term_font: u32,
    /// "classic", "green", "amber" eller "pixel"
    pub term_theme: String,
    /// Kortare väntetider i strömmad utskrift (ping m.m.)
    pub fast_output: bool,
    /// Visa tangentraden längst ned i HUD
    pub show_keys: bool,
    /// Visa HUD
    pub hud: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings { difficulty: "normal".into(), term_font: 22, term_theme: "classic".into(), fast_output: false, show_keys: true, hud: true }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WeekProgress {
    pub stars: u32,
    pub minutes: u32,
    #[serde(default)]
    pub badges: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SaveData {
    pub progress: BTreeMap<u32, WeekProgress>,
    pub settings: Settings,
    /// Senast spelade vecka (för Fortsätt)
    pub last_week: Option<u32>,
    /// Bästa frågesportsresultat per vecka
    pub quiz: BTreeMap<u32, u32>,
    pub notes: String,
    /// Kommandohistorik per terminalsession (högst 60 rader)
    pub history: HashMap<String, Vec<String>>,
    /// Pågående vecka (Fortsätt)
    pub run: Option<SavedRun>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedTask {
    pub fixed: bool,
    pub reported: bool,
    pub known: bool,
    pub hints: u32,
    pub score: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedRun {
    pub week: u32,
    pub exam: bool,
    pub state: State,
    pub tasks: Vec<SavedTask>,
    pub elapsed: f64,
    pub console: Option<String>,
    pub commands: u32,
    pub site: String,
}

pub fn save_dir() -> PathBuf {
    if cfg!(windows) {
        let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
        PathBuf::from(base).join("krabba-spelet")
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        PathBuf::from(home).join(".krabba-spelet")
    }
}

pub fn save_path() -> PathBuf {
    if let Ok(p) = std::env::var("KRABBA_SAVE") {
        return PathBuf::from(p);
    }
    save_dir().join("save.json")
}

impl SaveData {
    pub fn load() -> SaveData {
        match std::fs::read_to_string(save_path()) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => SaveData::default(),
        }
    }
    pub fn save(&self) {
        let p = save_path();
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Ok(s) = serde_json::to_string(self) {
            let tmp = p.with_extension("tmp");
            if std::fs::write(&tmp, s).is_ok() {
                let _ = std::fs::rename(&tmp, &p);
            }
        }
    }
    pub fn stars(&self, week: u32) -> u32 {
        self.progress.get(&week).map(|p| p.stars).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip() {
        let mut d = SaveData::default();
        d.progress.insert(3, WeekProgress { stars: 2, minutes: 9, badges: vec!["x".into()] });
        d.last_week = Some(3);
        let s = serde_json::to_string(&d).unwrap();
        let e: SaveData = serde_json::from_str(&s).unwrap();
        assert_eq!(e.stars(3), 2);
        assert_eq!(e.settings.term_font, 22);
    }
}
