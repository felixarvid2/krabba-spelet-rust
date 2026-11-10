//! 2D-världen: kontoret i Göteborg, lagret i Borås (och Mölndal i kurs 2) i 16-bitars pixelstil sett snett uppifrån.
//! Port av js/world/world2d.js, art2d.js, scene2d.js och path2d.js (bara 2D).
#![allow(unused_variables, dead_code)]
use crate::engine::{Canvas, Fonts, Input};
use crate::net::sim::State;

/// Något spelaren gjorde i världen som spelet (game::App) ska hantera.
#[derive(Clone, Debug, PartialEq)]
pub enum WorldEvent {
    /// Koppla in konsolkabeln i en IOS-enhet, WLC eller lastbalanseraren (enhetens id)
    Console(String),
    /// Använda en dator (öppnar kommandotolken/nätverksinställningarna), enhetens id
    UseHost(String),
    /// Öppna teknikerns laptop (T eller laptopen på bänken)
    Laptop,
    /// Prata med en kollega (namn, t.ex. "Anna")
    Talk(String),
    /// En kabel i racket eller vid en dator (länkens id): byta/dra ur/sätta i
    Cable(String),
    /// Titta på ett rack (frontpanelsvyn), rackets namn ("A", "B", "Boras")
    Rack(String),
    /// Åka till en annan plats ("gbg", "boras", "molndal")
    Travel(String),
    /// Ta upp gästmobilen (vecka 8), miniswitchen (vecka 9) m.m.
    Pickup(String),
    /// Kaffe, övervakningsskärmen och andra saker
    Use(String),
}

/// Markering ovanför en kollega
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    None,
    /// Har ett ärende (!)
    Ticket,
    /// Har fått hjälp (✓)
    Done,
}

pub struct World {
    pub site: String,
    pub week: u32,
}

impl World {
    pub fn new(site: &str, week: u32) -> World { todo!("World::new") }
    /// Byt plats (åk till Borås osv.)
    pub fn set_site(&mut self, site: &str) { todo!() }
    /// Markeringar ovanför kollegorna
    pub fn set_mark(&mut self, npc: &str, m: Mark) { todo!() }
    /// Målet som kompassen pekar på (enhetens id eller kollegans namn), None = inget
    pub fn set_target(&mut self, target: Option<String>) { todo!() }
    /// Rörelse, klick och E. `blocked` = en dialog är öppen (bara animationer).
    pub fn update(&mut self, input: &Input, dt: f64, st: &State, blocked: bool) -> Vec<WorldEvent> { todo!() }
    /// Ritar hela världen över canvasen (utom HUD och dialoger)
    pub fn draw(&mut self, c: &mut Canvas, fonts: &mut Fonts, st: &State) { todo!() }
    /// Tipset för det spelaren står vid, t.ex. "E – Koppla in konsolen i SW-Nordvik-1"
    pub fn hint(&self) -> Option<String> { todo!() }
}
