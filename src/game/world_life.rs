//! Liv i världen: kollegorna går omkring, djuren (katt, ankor, duvor, mås), Krabban och partiklar.
//! Barnmodul till world.rs (egna regler för schema och rörelse, JS-originalet har stillastående kollegor).
use super::*;
use std::collections::HashMap;

// ------------------------------------------------------------------ Kollegornas rörelse

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AiState {
    /// Vid sin plats (sitter eller står)
    Home,
    /// På väg till ett mål
    ToStop,
    /// Vid målet
    AtStop,
    /// På väg tillbaka
    ToHome,
}

/// Ett mål att gå till: (x, z, riktning vid målet, social = fika/kaffe)
pub type Stop = (f64, f64, u8, bool);

pub struct NpcAi {
    pub home: (f64, f64),
    pub home_sit: bool,
    pub stops: Vec<Stop>,
    pub state: AiState,
    pub path: Vec<(f64, f64)>,
    pub timer: f64,
    pub rng: Rng,
    pub stuck: f64,
    pub replans: u32,
    pub anim: f64,
    pub face: u8,
}

impl Default for NpcAi {
    fn default() -> NpcAi {
        NpcAi { home: (0.0, 0.0), home_sit: false, stops: vec![], state: AiState::Home, path: vec![], timer: 5.0, rng: Rng::new(1), stuck: 0.0, replans: 0, anim: 0.0, face: 0 }
    }
}

const COFFEE: Stop = (-2.8, -8.8, 0, true);
const WATER: Stop = (0.97, -8.8, 0, true);
const FIKA: Stop = (-1.5, -6.9, 1, true);
const FIKA2: Stop = (0.2, -5.6, 2, true);
const BOARD: Stop = (-5.2, -5.4, 2, false);
const PRINTER: Stop = (8.0, 8.3, 1, false);
const MONITOR: Stop = (-8.2, -3.0, 1, false);
const SERVER: Stop = (-12.0, -7.5, 1, false);
const SOFA: Stop = (12.8, -6.6, 1, true);

/// Möjliga mål per person (platsen är ur spelarens synvinkel; ogiltiga mål sorteras bort när de inte går att nå)
pub fn stops_for(name: &str) -> Vec<Stop> {
    match name {
        "Anna" => vec![COFFEE, PRINTER, BOARD, WATER],
        "Karim" => vec![SERVER, MONITOR, COFFEE, BOARD],
        "Sara" => vec![COFFEE, WATER, PRINTER, FIKA],
        "Lisa" => vec![PRINTER, WATER, FIKA2, BOARD],
        "Bo" => vec![COFFEE, FIKA, BOARD, MONITOR],
        "Maja" => vec![COFFEE, PRINTER, WATER, FIKA2],
        "Omar" => vec![SERVER, MONITOR, COFFEE, BOARD],
        "Linnea" => vec![WATER, COFFEE, SOFA],
        "Eva" => vec![FIKA, COFFEE, BOARD, FIKA2],
        "Nils" => vec![(100.0, -3.0, 1, false), (108.0, 2.0, 0, false), (96.0, 4.0, 1, false), (104.0, -9.0, 1, false)],
        "Elin" => vec![(166.0, -4.0, 1, false), (158.0, 4.0, 0, false), (168.0, 4.0, 0, false)],
        _ => vec![],
    }
}

pub fn dir_from(dx: f64, dz: f64) -> u8 {
    if dx.abs() > dz.abs() {
        if dx < 0.0 { 2 } else { 3 }
    } else if dz < 0.0 {
        0
    } else {
        1
    }
}

// ------------------------------------------------------------------ Djur

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AK {
    Cat,
    Duck(u8),
    Pigeon(u8),
    Gull,
}

pub struct Animal {
    pub kind: AK,
    pub site: usize,
    pub x: f64,
    pub z: f64,
    pub y: f64,
    pub hx: f64,
    pub hz: f64,
    pub tx: f64,
    pub tz: f64,
    pub wait: f64,
    pub left: bool,
    pub sit: bool,
    pub away: f64,
    pub ph: f64,
    pub a: f64,
    pub dive: f64,
    pub path: Vec<(f64, f64)>,
    pub moving: bool,
    pub peck: f64,
}

fn animal(kind: AK, site: usize, x: f64, z: f64) -> Animal {
    Animal { kind, site, x, z, y: 0.0, hx: x, hz: z, tx: x, tz: z, wait: 3.0, left: false, sit: false, away: 0.0, ph: 0.0, a: 0.0, dive: 0.0, path: vec![], moving: false, peck: 0.0 }
}

const POND: (f64, f64) = (-18.3, -1.5);
const CAT_SPOTS: [(f64, f64); 9] = [(2.0, 4.0), (-3.0, 6.5), (6.0, 1.0), (10.0, 5.0), (12.5, 8.5), (-1.5, 0.5), (8.0, 8.6), (12.6, -8.75), (-12.4, -0.2)];

// ------------------------------------------------------------------ Krabban

pub struct CrabSt {
    pub site: usize,
    pub x: f64,
    pub z: f64,
    pub home: (f64, f64),
    pub target: Option<(f64, f64)>,
    pub t: f64,
    pub pause: f64,
    pub fleeing: bool,
    pub flee_t: f64,
    pub tired: f64,
    pub anim: f64,
    pub moving: bool,
    pub left: bool,
}

// ------------------------------------------------------------------ Partiklar

pub struct Part {
    pub x: f64,
    pub z: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub vz: f64,
    pub g: f64,
    pub age: f64,
    pub life: f64,
    pub size: i32,
    pub col: u32,
    pub a: f32,
}

pub struct Life {
    pub animals: Vec<Animal>,
    pub crab: Option<CrabSt>,
    pub crab_for: Option<String>,
    pub parts: Vec<Part>,
    pub rng: Rng,
    pub spr: HashMap<(u8, u8), Rc<Cv>>,
    pub steam_t: f64,
    pub meow_t: f64,
    pub quack_t: f64,
    pub inited: bool,
}

impl Life {
    pub fn new() -> Life {
        let mut animals = vec![animal(AK::Cat, 0, 2.0, 4.0)];
        for i in 0..2u8 {
            let mut d = animal(AK::Duck(i), 0, POND.0, POND.1);
            d.a = i as f64 * 2.6;
            d.ph = i as f64;
            animals.push(d);
        }
        for i in 0..3u8 {
            let mut p = animal(AK::Pigeon(i), 0, 10.0 + i as f64 * 0.5, -15.0 + (i % 2) as f64 * 0.6);
            p.peck = i as f64 * 1.3;
            animals.push(p);
        }
        let mut g = animal(AK::Gull, 1, 114.0, 3.6);
        g.tx = 114.0;
        g.wait = 2.0;
        animals.push(g);
        Life { animals, crab: None, crab_for: None, parts: vec![], rng: Rng::new(4711), spr: HashMap::new(), steam_t: 0.0, meow_t: 5.0, quack_t: 3.0, inited: false }
    }

    pub fn sprite(&mut self, kind: u8, var: u8, left: bool) -> (Rc<Cv>, i32, i32) {
        let key = (kind, var | if left { 128 } else { 0 });
        let (ax, ay) = match (kind, var) {
            (0, _) => (11, 13),
            (1, 2) => (8, 11),
            (1, _) => (9, 11),
            (2, _) => (7, 8),
            (3, 1) => (6, 8),
            (3, _) => (6, 9),
            _ => (8, 11),
        };
        let spr = self
            .spr
            .entry(key)
            .or_insert_with(|| {
                let s = match kind {
                    0 => crab_sprite(var & 1, var >= 2),
                    1 => cat_sprite(var),
                    2 => duck_sprite(),
                    3 => pigeon_sprite(var == 1),
                    _ => gull_sprite(),
                };
                Rc::new(if left { s.flipped() } else { s })
            })
            .clone();
        let ax = if left { spr.w - 1 - ax } else { ax };
        (spr, ax, ay)
    }

    pub fn burst(&mut self, x: f64, z: f64, n: usize, col: u32) {
        for _ in 0..n {
            let a = self.rng.next() as f64 * std::f64::consts::TAU;
            let s = 0.6 + self.rng.next() as f64 * 1.6;
            let vy = 1.0 + self.rng.next() as f64 * 1.6;
            self.parts.push(Part { x, z, y: 0.2, vx: a.cos() * s, vy, vz: a.sin() * s * 0.6, g: 5.0, age: 0.0, life: 0.7 + self.rng.next() as f64 * 0.5, size: 1, col, a: 1.0 });
        }
    }
}

fn hyp(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

impl World {
    /// Väg för en figur som inte är spelaren (ingen väg = None, delvis väg räknas som misslyckad)
    pub(super) fn npc_plan(&self, from: (f64, f64), to: (f64, f64), exact: bool) -> Option<Vec<(f64, f64)>> {
        let nav = self.navs[self.cur].as_ref()?;
        let cols = &self.sites[self.cur].cols;
        let p = world_path::find_path(nav, cols, from.0, from.1, to.0, to.1, None)?;
        if p.partial {
            return None;
        }
        let mut pts = p.pts;
        if exact {
            pts.push(to);
        }
        Some(pts)
    }

    /// Sätter upp kollegornas schema (första anropet)
    pub(super) fn life_init(&mut self) {
        if self.life.inited {
            return;
        }
        self.life.inited = true;
        for n in self.npcs.iter_mut() {
            let seed = n.name.bytes().fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
            let mut rng = Rng::new(seed);
            let timer = 3.0 + rng.range(0.0, 20.0) as f64;
            n.ai = NpcAi { home: (n.x, n.z), home_sit: n.sit, stops: stops_for(n.name), state: AiState::Home, path: vec![], timer, rng, stuck: 0.0, replans: 0, anim: 0.0, face: 0 };
        }
    }

    fn next_interval(ai: &mut NpcAi, hour: f64, name: &str) -> f64 {
        let peak = (11.5..13.0).contains(&hour) || (15.0..15.6).contains(&hour);
        let lazy = if name == "Linnea" { 2.0 } else { 1.0 };
        if peak {
            ai.rng.range(8.0, 20.0) as f64 * lazy
        } else {
            ai.rng.range(22.0, 50.0) as f64 * lazy
        }
    }

    /// Kollegorna går sina rundor. Stannar när spelaren är nära eller en dialog är öppen.
    pub(super) fn update_npcs(&mut self, dt: f64, st: &State, blocked: bool) {
        self.life_init();
        let _ = self.nav();
        let hour = 8.0 + st.time / 3600.0;
        let cur = self.cur;
        let pl = self.pos;
        let blockers: Vec<(usize, (f64, f64))> = self.npcs.iter().enumerate().filter(|(_, n)| self.npc_visible(n)).map(|(i, n)| (i, (n.x, n.z))).collect();
        for k in 0..self.npcs.len() {
            if !self.npc_visible(&self.npcs[k]) {
                continue;
            }
            let mut ai = std::mem::take(&mut self.npcs[k].ai);
            let (name, mut pos, mut dir, mut sit) = {
                let n = &self.npcs[k];
                (n.name, (n.x, n.z), n.dir, n.sit)
            };
            let mark = self.marks.get(name).copied().unwrap_or(Mark::None);
            let wants_stay = mark == Mark::Ticket;
            let dd = hyp(pl, pos);
            let frozen = dd < 1.9 || (blocked && dd < 4.5);
            let mut moving = false;
            if frozen {
                if !sit && ai.state != AiState::AtStop {
                    dir = dir_from(pl.0 - pos.0, pl.1 - pos.1);
                } else if !sit {
                    dir = dir_from(pl.0 - pos.0, pl.1 - pos.1);
                }
            } else {
                // Den som har ett ärende går tillbaka till sin plats och väntar
                if wants_stay && matches!(ai.state, AiState::ToStop | AiState::AtStop) {
                    match self.npc_plan(pos, ai.home, true) {
                        Some(p) => {
                            ai.path = p;
                            ai.state = AiState::ToHome;
                        }
                        None => {
                            pos = ai.home;
                            sit = ai.home_sit;
                            ai.state = AiState::Home;
                            ai.path.clear();
                        }
                    }
                }
                match ai.state {
                    AiState::Home => {
                        ai.timer -= dt;
                        if wants_stay {
                            ai.timer = ai.timer.max(1.0);
                        } else if ai.timer <= 0.0 {
                            let peak = (11.5..13.0).contains(&hour) || (15.0..15.6).contains(&hour);
                            let soc: Vec<usize> = (0..ai.stops.len()).filter(|&i| ai.stops[i].3).collect();
                            let i = if peak && !soc.is_empty() { Some(soc[ai.rng.idx(soc.len())]) } else if ai.stops.is_empty() { None } else { Some(ai.rng.idx(ai.stops.len())) };
                            match i {
                                Some(i) => {
                                    let s = ai.stops[i];
                                    match self.npc_plan(pos, (s.0, s.1), false) {
                                        Some(p) if !p.is_empty() => {
                                            ai.path = p;
                                            ai.face = s.2;
                                            ai.state = AiState::ToStop;
                                            ai.replans = 0;
                                            sit = false;
                                        }
                                        _ => {
                                            ai.stops.remove(i);
                                            ai.timer = 2.0;
                                        }
                                    }
                                }
                                None => ai.timer = 30.0,
                            }
                        }
                    }
                    AiState::AtStop => {
                        dir = ai.face;
                        ai.timer -= dt;
                        if ai.timer <= 0.0 {
                            match self.npc_plan(pos, ai.home, true) {
                                Some(p) => {
                                    ai.path = p;
                                    ai.state = AiState::ToHome;
                                    ai.replans = 0;
                                }
                                None => {
                                    pos = ai.home;
                                    sit = ai.home_sit;
                                    ai.state = AiState::Home;
                                }
                            }
                        }
                    }
                    AiState::ToStop | AiState::ToHome => {
                        let speed = 1.15;
                        match ai.path.first().copied() {
                            None => {
                                if ai.state == AiState::ToStop {
                                    ai.state = AiState::AtStop;
                                    ai.timer = 5.0 + ai.rng.range(0.0, 9.0) as f64;
                                    dir = ai.face;
                                } else {
                                    pos = ai.home;
                                    sit = ai.home_sit;
                                    ai.state = AiState::Home;
                                    ai.timer = Self::next_interval(&mut ai, hour, name);
                                    dir = 0;
                                }
                            }
                            Some(t) => {
                                let d = hyp(t, pos);
                                let step = speed * dt;
                                let nxt = if d <= step { t } else { (pos.0 + (t.0 - pos.0) / d * step, pos.1 + (t.1 - pos.1) / d * step) };
                                let hit = hyp(pl, nxt) < 0.55 && hyp(pl, nxt) < hyp(pl, pos) || blockers.iter().any(|(i, b)| *i != k && hyp(*b, nxt) < 0.5 && hyp(*b, nxt) < hyp(*b, pos));
                                if hit {
                                    ai.stuck += dt;
                                    if ai.stuck > 1.5 {
                                        ai.stuck = 0.0;
                                        ai.replans += 1;
                                        let goal = if ai.state == AiState::ToHome { ai.home } else { *ai.path.last().unwrap_or(&ai.home) };
                                        match if ai.replans <= 3 { self.npc_plan(pos, goal, ai.state == AiState::ToHome) } else { None } {
                                            Some(p) => ai.path = p,
                                            None => {
                                                // Ger upp: tillbaka på platsen
                                                pos = ai.home;
                                                sit = ai.home_sit;
                                                ai.state = AiState::Home;
                                                ai.path.clear();
                                                ai.timer = 10.0;
                                            }
                                        }
                                    }
                                } else {
                                    ai.stuck = 0.0;
                                    if d > 0.001 {
                                        dir = dir_from(t.0 - pos.0, t.1 - pos.1);
                                    }
                                    let moved = hyp(nxt, pos);
                                    pos = nxt;
                                    moving = moved > 1e-6;
                                    ai.anim += moved * 3.6;
                                    if d <= step {
                                        ai.path.remove(0);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            let n = &mut self.npcs[k];
            n.x = pos.0;
            n.z = pos.1;
            n.col = Rect { x0: pos.0 - 0.25, x1: pos.0 + 0.25, z0: pos.1 - 0.2, z1: pos.1 + 0.25 };
            n.sit = sit;
            n.dir = dir;
            n.moving = moving;
            n.frame = if moving { (ai.anim as i64).rem_euclid(4) as u8 } else { 0 };
            n.ai = ai;
        }
    }

    // ------------------------------------------------------------------ Djur

    fn pop(&mut self, text: &str, x: f64, z: f64) {
        self.pops.push(Pop { text: text.into(), x, z, age: 0.0, life: 1.4 });
    }

    pub(super) fn update_animals(&mut self, dt: f64) {
        let _ = self.nav();
        let cur = self.cur;
        let me = self.pos;
        let mut animals = std::mem::take(&mut self.life.animals);
        let mut meow = self.life.meow_t - dt;
        let mut quack = self.life.quack_t - dt;
        for a in animals.iter_mut() {
            a.ph += dt;
            match a.kind {
                AK::Cat => {
                    if a.site != cur {
                        continue;
                    }
                    // Start på ett fritt ställe
                    if a.ph < dt * 1.5 && !self.navs[cur].as_ref().map(|n| n.free_at(a.x, a.z)).unwrap_or(true) {
                        if let Some(s) = CAT_SPOTS.iter().find(|s| self.navs[cur].as_ref().map(|n| n.free_at(s.0, s.1)).unwrap_or(true)) {
                            a.x = s.0;
                            a.z = s.1;
                        }
                    }
                    let near = hyp(me, (a.x, a.z));
                    a.moving = false;
                    if near < 0.7 && a.wait > 0.0 && a.path.is_empty() {
                        a.wait = 0.01;
                        a.sit = false;
                    }
                    if a.path.is_empty() {
                        if a.wait > 0.0 {
                            a.wait -= dt;
                        } else {
                            let s = CAT_SPOTS[self.life.rng.idx(CAT_SPOTS.len())];
                            a.sit = false;
                            a.wait = 1.0;
                            if let Some(p) = self.npc_plan((a.x, a.z), s, false) {
                                a.path = p;
                            }
                        }
                        if a.path.is_empty() && a.wait <= 0.0 {
                            a.wait = 4.0;
                            a.sit = true;
                        }
                    }
                    if let Some(t) = a.path.first().copied() {
                        let d = hyp(t, (a.x, a.z));
                        let sp = (if near < 0.9 { 2.2 } else { 0.9 }) * dt;
                        if d <= sp {
                            a.x = t.0;
                            a.z = t.1;
                            a.path.remove(0);
                            if a.path.is_empty() {
                                a.wait = 4.0 + (a.ph * 13.1) % 6.0;
                                a.sit = (a.ph * 3.7) % 1.0 < 0.7;
                            }
                        } else {
                            a.x += (t.0 - a.x) / d * sp;
                            a.z += (t.1 - a.z) / d * sp;
                            a.left = t.0 < a.x - 0.001 || (a.left && (t.0 - a.x).abs() < 0.02);
                            a.moving = true;
                        }
                    }
                    if meow <= 0.0 && near < 1.6 {
                        meow = 8.0 + (a.ph * 5.3) % 8.0;
                        crate::engine::audio::sfx("meow");
                        self.pops.push(Pop { text: "Mjau".into(), x: a.x, z: a.z, age: 0.0, life: 1.3 });
                    }
                }
                AK::Duck(i) => {
                    let (rx, rz, sp) = (1.1 + i as f64 * 0.3, 2.2 + i as f64 * 0.5, 0.18 + i as f64 * 0.07);
                    a.a += sp * dt;
                    a.x = POND.0 + a.a.cos() * rx;
                    a.z = POND.1 + a.a.sin() * rz;
                    a.left = -a.a.sin() * rx < 0.0;
                    a.dive = (a.dive - dt).max(0.0);
                    if a.dive <= 0.0 && (self.life.rng.next() as f64) < dt * 0.05 {
                        a.dive = 1.4;
                    }
                    if cur == 0 && quack <= 0.0 && i == 0 && hyp(me, POND) < 4.5 {
                        quack = 6.0 + (a.ph * 7.1) % 8.0;
                        crate::engine::audio::sfx("quack");
                        self.pops.push(Pop { text: "Kvack!".into(), x: a.x, z: a.z, age: 0.0, life: 1.4 });
                    }
                }
                AK::Pigeon(_) => {
                    a.peck += dt;
                    if a.away > 0.0 {
                        a.away -= dt;
                        a.y += dt * 2.5;
                        a.x += dt * 3.0 * if a.hx > 10.5 { 1.0 } else { -1.0 };
                        if a.away <= 0.0 {
                            a.x = a.hx;
                            a.z = a.hz;
                            a.y = 0.0;
                        }
                    } else if cur == 0 && hyp(me, (a.x, a.z)) < 1.8 {
                        a.away = 15.0 + (a.ph * 11.0) % 15.0;
                        crate::engine::audio::sfx("flap");
                        a.y = 0.1;
                        a.left = a.hx <= 10.5;
                    }
                }
                AK::Gull => {
                    if a.site != cur {
                        continue;
                    }
                    if a.wait > 0.0 {
                        a.wait -= dt;
                    } else {
                        let dx = a.tx - a.x;
                        if dx.abs() < 0.05 {
                            a.wait = 2.0 + (a.ph * 3.3) % 4.0;
                            a.tx = 112.6 + (a.ph * 1.7) % 2.6;
                        } else {
                            a.x += dx.signum() * dt * 0.6;
                            a.left = dx < 0.0;
                        }
                    }
                    if hyp(me, (a.x, a.z)) < 1.5 {
                        a.tx = (a.x + if a.x > me.0 { 1.2 } else { -1.2 }).clamp(112.6, 115.2);
                        a.wait = 0.0;
                    }
                    a.moving = a.wait <= 0.0;
                }
            }
        }
        self.life.meow_t = meow;
        self.life.quack_t = quack;
        self.life.animals = animals;
    }

    /// Djur som går att klappa/titta på
    pub(super) fn life_cands(&self, v: &mut Vec<Cand>) {
        for a in &self.life.animals {
            if a.site != self.cur {
                continue;
            }
            let (name, label, r, x, z) = match a.kind {
                AK::Cat => ("katten", "Klappa katten", 0.7, a.x, a.z),
                AK::Duck(0) => ("ankorna", "Mata ankorna", 1.3, a.x, a.z),
                AK::Pigeon(i) if i == 0 && a.away <= 0.0 => ("duvorna", "Titta på duvorna", 0.8, a.x, a.z),
                AK::Gull => ("masen", "Titta på måsen", 0.8, a.x, a.z),
                _ => continue,
            };
            v.push(Cand { x, z, r, kind: IKind::Animal(name), label: label.into(), item: None, npc: None });
        }
        if let Some(c) = &self.life.crab {
            if c.site == self.cur {
                v.push(Cand { x: c.x, z: c.z, r: 0.5, kind: IKind::Crab, label: "Fånga Krabban!".into(), item: None, npc: None });
            }
        }
    }

    // ------------------------------------------------------------------ Krabban

    /// Hitta ett ställe för krabban nära enheten/kollegan
    fn crab_home(&self, st: &State, name: &str) -> Option<(usize, f64, f64)> {
        for n in &self.npcs {
            if n.name == name {
                return Some((n.site, n.ai.home.0 + 0.9, n.ai.home.1 - 0.2));
            }
        }
        for (si, s) in self.sites.iter().enumerate() {
            if let Some(r) = s.racks.iter().find(|r| r.units.iter().any(|u| *u == name) || r.id == name) {
                return Some((si, r.x + 0.2, r.z + 1.5));
            }
        }
        for (si, s) in self.sites.iter().enumerate() {
            if let Some(sp) = s.spots.iter().find(|sp| sp.0 == name) {
                return Some((si, sp.1 + 1.0, sp.2 + 0.3));
            }
        }
        if let Some(d) = st.devices.get(name) {
            let si = site_index(&d.site);
            let b = self.sites[si].building;
            return Some((si, (b.x0 + b.x1) / 2.0, (b.z0 + b.z1) / 2.0));
        }
        None
    }

    fn collides_site(&self, site: usize, x: f64, z: f64, r: f64) -> bool {
        let s = &self.sites[site];
        for c in &s.cols {
            if x > c.x0 - r && x < c.x1 + r && z > c.z0 - r && z < c.z1 + r {
                return true;
            }
        }
        x < s.x0 + 0.5 || x > s.x1 - 0.5 || z < s.z0 + 0.5 || z > s.z1 - 0.5
    }

    pub(super) fn update_crab(&mut self, dt: f64, st: &State, events: &mut Vec<WorldEvent>) {
        // Ny plats för krabban?
        if self.life.crab_for != self.crab_near {
            self.life.crab_for = self.crab_near.clone();
            self.life.crab = None;
            if let Some(name) = self.crab_near.clone() {
                if let Some((si, hx, hz)) = self.crab_home(st, &name) {
                    // Närmaste fria punkt (spiral utåt)
                    let mut pos = None;
                    'o: for ri in 0..14 {
                        let r = ri as f64 * 0.25;
                        for ai in 0..12 {
                            let a = ai as f64 / 12.0 * std::f64::consts::TAU;
                            let (x, z) = (hx + a.cos() * r, hz + a.sin() * r);
                            let free = self.navs[si].as_ref().map(|n| n.free_at(x, z)).unwrap_or(true);
                            if free && !self.collides_site(si, x, z, 0.16) {
                                pos = Some((x, z));
                                break 'o;
                            }
                        }
                    }
                    let (x, z) = pos.unwrap_or((hx, hz));
                    self.life.crab = Some(CrabSt { site: si, x, z, home: (x, z), target: None, t: 0.0, pause: 0.5, fleeing: false, flee_t: 0.0, tired: 0.0, anim: 0.0, moving: false, left: false });
                }
            }
        }
        let Some(mut c) = self.life.crab.take() else { return };
        if c.site == self.cur {
            c.t -= dt;
            let me = self.pos;
            let (dx, dz) = (c.x - me.0, c.z - me.1);
            let d = dx.hypot(dz);
            c.tired = (c.tired - dt).max(0.0);
            let (mut vx, mut vz, mut speed) = (0.0, 0.0, 0.0);
            let mut skip = false;
            if d < 2.6 && c.tired <= 0.0 {
                if !c.fleeing {
                    c.fleeing = true;
                    c.flee_t = 0.0;
                }
                c.flee_t += dt;
                if c.flee_t > 1.8 {
                    c.fleeing = false;
                    c.tired = 1.4;
                }
                vx = dx / d.max(1e-6);
                vz = dz / d.max(1e-6);
                speed = 1.7;
            } else {
                c.fleeing = false;
                if c.t <= 0.0 || c.target.is_none() {
                    let a = ((self.t * 7.31 + c.x * 3.1).sin() * 43758.5).fract().abs() * std::f64::consts::TAU;
                    let r = ((self.t * 3.77 + c.z * 5.3).cos() * 24634.6).fract().abs() * 1.8;
                    c.target = Some((c.home.0 + a.cos() * r, c.home.1 + a.sin() * r));
                    c.t = 2.0 + ((self.t * 1.9).sin() * 997.0).fract().abs() * 3.0;
                    c.pause = ((self.t * 2.3).cos() * 311.0).fract().abs() * 1.5;
                }
                if c.pause > 0.0 {
                    c.pause -= dt;
                    skip = true;
                } else if let Some(t) = c.target {
                    let (ex, ez) = (t.0 - c.x, t.1 - c.z);
                    let el = ex.hypot(ez);
                    if el < 0.05 {
                        skip = true;
                    } else {
                        vx = ex / el;
                        vz = ez / el;
                        speed = 0.45;
                    }
                }
            }
            let mut moved = false;
            if !skip {
                let (nx, nz) = (c.x + vx * speed * dt, c.z + vz * speed * dt);
                if !self.collides_site(c.site, nx, c.z, 0.12) {
                    c.x = nx;
                    moved = true;
                }
                if !self.collides_site(c.site, c.x, nz, 0.12) {
                    c.z = nz;
                    moved = true;
                }
                if !moved {
                    c.target = None;
                    c.t = 0.0;
                    if c.fleeing {
                        c.fleeing = false;
                        c.tired = 1.2;
                    }
                }
            }
            c.moving = moved;
            if moved && vx.abs() > 0.05 {
                c.left = vx < 0.0;
            }
            c.anim += dt * if moved { if c.fleeing { 3.0 } else { 1.5 } } else { 0.0 };
            if moved && c.fleeing {
                let n = self.life.rng.next();
                if (n as f64) < dt * 8.0 {
                    self.life.parts.push(Part { x: c.x, z: c.z, y: 0.05, vx: 0.0, vy: 0.3, vz: 0.0, g: 0.0, age: 0.0, life: 0.5, size: 2, col: 0xd9c9a0, a: 0.5 });
                }
            }
            // Fångad när spelaren når den
            if hyp(me, (c.x, c.z)) < 0.5 {
                self.life.burst(c.x, c.z, 26, 0xff7a4a);
                self.pop("Fångad!", c.x, c.z);
                events.push(WorldEvent::Use("krabba".into()));
                self.crab_near = None;
                self.life.crab_for = None;
                return;
            }
        }
        self.life.crab = Some(c);
    }

    /// Krabban fångades genom klick/E (Use("krabba") skickades)
    pub(super) fn crab_caught(&mut self) {
        if let Some(c) = self.life.crab.take() {
            crate::engine::audio::sfx("squeak");
            self.life.burst(c.x, c.z, 26, 0xff7a4a);
            self.pop("Fångad!", c.x, c.z);
        }
        self.crab_near = None;
        self.life.crab_for = None;
    }

    // ------------------------------------------------------------------ Partiklar och kaffemaskinen

    pub(super) fn update_parts(&mut self, dt: f64) {
        for p in self.life.parts.iter_mut() {
            p.age += dt;
            p.x += p.vx * dt;
            p.z += p.vz * dt;
            p.y += p.vy * dt;
            p.vy -= p.g * dt;
            if p.y < 0.0 && p.g > 0.0 {
                p.y = 0.0;
                p.vy = 0.0;
                p.vx *= 0.5;
                p.vz *= 0.5;
            }
        }
        self.life.parts.retain(|p| p.age < p.life);
        if self.life.parts.len() > 400 {
            self.life.parts.drain(0..100);
        }
        // Kaffemaskinen ångar när man är i närheten
        if self.cur == 0 && hyp(self.pos, (-2.8, -9.2)) < 14.0 {
            self.life.steam_t -= dt;
            if self.life.steam_t <= 0.0 {
                self.life.steam_t = 0.35;
                let j = (self.t * 17.0).sin() * 0.05;
                self.life.parts.push(Part { x: -2.8 + j, z: -9.62, y: 1.02, vx: j, vy: 0.28, vz: 0.0, g: 0.0, age: 0.0, life: 1.5, size: 1, col: 0xffffff, a: 0.45 });
            }
        }
    }

    pub(super) fn draw_parts(&self, fb: &mut Cv, v: &View) {
        for p in &self.life.parts {
            let (sx, sy) = v.to_screen(p.x, p.z, p.y);
            let k = 1.0 - (p.age / p.life) as f32;
            let sz = if p.col == 0xffffff { 1 + (p.age * 1.5) as i32 } else { p.size };
            fb.rect(sx, sy, sz, sz, ca(p.col, p.a * k));
        }
    }
}

impl World {
    /// Kollegans aktuella position (för kompassen och test)
    pub fn npc_pos(&self, name: &str) -> Option<(f64, f64)> {
        self.npcs.iter().find(|n| n.name == name).map(|n| (n.x, n.z))
    }
    /// Krabbans position om den finns på platsen där spelaren är
    pub fn crab_pos(&self) -> Option<(f64, f64)> {
        self.life.crab.as_ref().filter(|c| c.site == self.cur).map(|c| (c.x, c.z))
    }
    /// Vilka mål en kollega har kvar efter att omöjliga sorterats bort (för test)
    pub fn npc_stops(&self, name: &str) -> usize {
        self.npcs.iter().find(|n| n.name == name).map(|n| n.ai.stops.len()).unwrap_or(0)
    }
}
