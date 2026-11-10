//! 2D-världen: kontoret i Göteborg, lagret i Borås (och Mölndal i kurs 2) i 16-bitars pixelstil sett snett uppifrån.
//! Port av js/world/world2d.js, art2d.js, scene2d.js och path2d.js (bara 2D).
//!
//! Koordinater: x åt höger, z uppåt på skärmen (norr), i meter. Allt ritas i en liten pixelbuffert
//! (1 bildpunkt = 1 konstpixel) som sedan förstoras med heltalsskala (zoom) till canvasen.
#![allow(unused_variables, dead_code)]
use super::art::*;
use super::world_map::*;
use super::world_path::{self, Nav};
use crate::engine::{Canvas, FontKind, Fonts, Input, Key};
use crate::net::model::LinkState;
use crate::net::sim::{self, Derived, State};
use std::collections::HashMap;
use std::rc::Rc;

#[path = "world_life.rs"]
mod life;
#[path = "world_fx.rs"]
mod fx;
use life::{Life, NpcAi};
use fx::Fx;

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

// ------------------------------------------------------------------ Kollegorna

struct NpcDef {
    name: &'static str,
    site: usize,
    desk: Option<&'static str>,
    stand: (f64, f64),
    look: Look,
    weeks: (u32, u32),
}

fn npc_defs() -> Vec<NpcDef> {
    let l = |skin, hair, shirt, pants| Look { skin, hair, shirt, pants, ..Default::default() };
    vec![
        NpcDef { name: "Anna", site: 0, desk: Some("PC-Anna"), stand: (0.0, 0.0), look: Look { long: true, ..l(0xf1c7a5, 0x6b3e26, 0xc8553d, 0x2d3142) }, weeks: (0, 99) },
        NpcDef { name: "Karim", site: 0, desk: Some("PC-Karim"), stand: (0.0, 0.0), look: Look { glasses: true, beard: true, ..l(0xc68e62, 0x1d1d1d, 0x3c6e71, 0x353535) }, weeks: (0, 99) },
        NpcDef { name: "Sara", site: 0, desk: Some("PC-Sara"), stand: (0.0, 0.0), look: Look { long: true, lanyard: true, ..l(0xe8b894, 0xd9a441, 0x7b9e3a, 0x3b3b4f) }, weeks: (0, 99) },
        NpcDef { name: "Lisa", site: 0, desk: Some("PC-Lisa"), stand: (0.0, 0.0), look: Look { long: true, lanyard: true, ..l(0xf3d0b5, 0x2b1b12, 0x8e5ea2, 0x2f3e46) }, weeks: (0, 99) },
        NpcDef { name: "Bo", site: 0, desk: Some("PC-Bo"), stand: (0.0, 0.0), look: Look { tie: true, glasses: true, beard: true, ..l(0xefc4a4, 0x9a9a9a, 0x5a7fa6, 0x3a3a3a) }, weeks: (0, 99) },
        NpcDef { name: "Maja", site: 0, desk: Some("PC-Maja"), stand: (0.0, 0.0), look: Look { long: true, ..l(0xa8734f, 0x1a1a1a, 0xd9a441, 0x283044) }, weeks: (0, 99) },
        NpcDef { name: "Omar", site: 0, desk: None, stand: (-3.4, -7.8), look: Look { beard: true, ..l(0xb5835a, 0x121212, 0x2f2f2f, 0x1f2a44) }, weeks: (0, 99) },
        NpcDef { name: "Linnea", site: 0, desk: None, stand: (7.2, -7.1), look: Look { long: true, headset: true, ..l(0xf2d1b8, 0xb5651d, 0x3c7478, 0x2d2d2d) }, weeks: (0, 99) },
        NpcDef { name: "Eva", site: 0, desk: None, stand: (0.2, -4.6), look: Look { long: true, glasses: true, ..l(0xe0ac8a, 0x5a5a5a, 0x6a4c93, 0x222222) }, weeks: (6, 6) },
        NpcDef { name: "Nils", site: 1, desk: None, stand: (92.2, -7.2), look: Look { vest: true, cap: Some(0x2c3e50), ..l(0xeec09c, 0x8a5a2b, 0xe07b24, 0x2c3e50) }, weeks: (0, 99) },
        NpcDef { name: "Elin", site: 2, desk: Some("PC-Molndal"), stand: (0.0, 0.0), look: Look { long: true, ..l(0xe6b890, 0x3a2a1c, 0x4f8f8a, 0x2c3a4a) }, weeks: (11, 99) },
    ]
}

struct Npc {
    name: &'static str,
    site: usize,
    x: f64,
    z: f64,
    sit: bool,
    look: usize,
    weeks: (u32, u32),
    phase: f64,
    col: Rect,
    /// Riktning (0 = mot oss, 1 = bort, 2 = vänster, 3 = höger), gångbild och om figuren går
    dir: u8,
    frame: u8,
    moving: bool,
    ai: NpcAi,
}

// ------------------------------------------------------------------ Vy

#[derive(Clone, Copy, Debug, Default)]
struct View {
    z: i32,
    vw: i32,
    vh: i32,
    hw: i32,
    hh: i32,
    cx0: i32,
    cz0: i32,
    /// Skärmposition för bildpunkt (0,0) i pixelbufferten
    ox: i32,
    oy: i32,
    cw: i32,
    ch: i32,
}

impl View {
    fn to_screen(&self, x: f64, z: f64, y: f64) -> (i32, i32) {
        (
            (x * PPM as f64).round() as i32 - self.cx0 + self.hw,
            self.cz0 - (z * (PPM * ZS) as f64).round() as i32 + self.hh - (y * (PPM * HS) as f64).round() as i32,
        )
    }
    /// Skärmpunkt (mus) till världen
    fn to_world(&self, sx: i32, sy: i32) -> (f64, f64) {
        let fx = (sx - self.ox).div_euclid(self.z.max(1));
        let fy = (sy - self.oy).div_euclid(self.z.max(1));
        (((fx - self.hw + self.cx0) as f64 + 0.5) / PPM as f64, (self.cz0 - (fy - self.hh)) as f64 / (PPM * ZS) as f64)
    }
    /// Pixelbufferten till skärmen (för text)
    fn hi(&self, fx: i32, fy: i32) -> (i32, i32) {
        (self.ox + fx * self.z, self.oy + fy * self.z)
    }
}

struct Label {
    fx: i32,
    fy: i32,
    text: String,
    bg: u32,
    fg: u32,
    scale: f32,
}

struct Walk {
    x: f64,
    z: f64,
    then: Option<Cand>,
}

#[derive(Clone, Debug)]
struct Cand {
    x: f64,
    z: f64,
    r: f64,
    kind: IKind,
    label: String,
    item: Option<usize>,
    npc: Option<usize>,
}

struct Pop {
    text: String,
    x: f64,
    z: f64,
    age: f64,
    life: f64,
}

pub struct World {
    pub site: String,
    pub week: u32,
    sites: Vec<Site>,
    navs: Vec<Option<Nav>>,
    cur: usize,
    pos: (f64, f64),
    dir: u8,
    frame: u8,
    anim_t: f64,
    accel: f64,
    path: Vec<(f64, f64)>,
    goal: Option<Walk>,
    run_path: bool,
    path_stuck: f64,
    replans: u32,
    cam: Option<(f64, f64)>,
    look: (f64, f64),
    zoom: i32,
    t: f64,
    npcs: Vec<Npc>,
    looks: Vec<Look>,
    marks: HashMap<String, Mark>,
    target: Option<String>,
    hover: Option<Cand>,
    hint_text: Option<String>,
    net: Option<Rc<Derived>>,
    fb: Cv,
    view: View,
    screen: (i32, i32),
    cache: HashMap<(u8, u8, u8, bool, bool), Rc<Cv>>,
    labels: Vec<Label>,
    door_open: f64,
    pops: Vec<Pop>,
    link_snap: Vec<(String, String, LinkState)>,
    has_mini: bool,
    last_use: Vec<WorldEvent>,
    /// Gånghastighetens faktor (kaffe ger > 1)
    pub speed_mult: f64,
    /// Var Krabban gömmer sig: enhetens id eller kollegans namn (None = ingen krabba)
    pub crab_near: Option<String>,
    /// Gången sträcka i meter (för statistik)
    pub walked: f64,
    life: Life,
    fx: Fx,
}

const SITE_KEYS: [&str; 3] = ["gbg", "boras", "molndal"];

fn site_index(key: &str) -> usize {
    SITE_KEYS.iter().position(|k| *k == key).unwrap_or(0)
}

impl World {
    pub fn new(site: &str, week: u32) -> World {
        let sites: Vec<Site> = SITE_KEYS.iter().map(|k| build_site(k)).collect();
        let cur = site_index(site);
        let mut looks = vec![];
        let mut npcs = vec![];
        for d in npc_defs() {
            let (x, z, sit) = match d.desk {
                Some(id) => {
                    let s = sites[d.site].spots.iter().find(|s| s.0 == id).map(|s| (s.1, s.2)).unwrap_or((0.0, 0.0));
                    (s.0 + 0.35, s.1 + 0.62, true)
                }
                None => (d.stand.0, d.stand.1, false),
            };
            looks.push(d.look.clone());
            npcs.push(Npc {
                name: d.name,
                site: d.site,
                x,
                z,
                sit,
                look: looks.len() - 1,
                weeks: d.weeks,
                phase: (d.name.len() as f64 * 1.7 + x * 0.3).abs() % 6.0,
                col: Rect { x0: x - 0.25, x1: x + 0.25, z0: z - 0.2, z1: z + 0.25 },
                dir: 0,
                frame: 0,
                moving: false,
                ai: NpcAi::default(),
            });
        }
        let mut w = World {
            site: SITE_KEYS[cur].to_string(),
            week,
            navs: (0..sites.len()).map(|_| None).collect(),
            sites,
            cur,
            pos: (0.0, 0.0),
            dir: 0,
            frame: 0,
            anim_t: 0.0,
            accel: 0.0,
            path: vec![],
            goal: None,
            run_path: false,
            path_stuck: 0.0,
            replans: 0,
            cam: None,
            look: (0.0, 0.0),
            zoom: 3,
            t: 0.0,
            npcs,
            looks,
            marks: HashMap::new(),
            target: None,
            hover: None,
            hint_text: None,
            net: None,
            fb: Cv::new(10, 10),
            view: View::default(),
            screen: (1280, 720),
            cache: HashMap::new(),
            labels: vec![],
            door_open: 0.0,
            pops: vec![],
            link_snap: vec![],
            has_mini: false,
            last_use: vec![],
            speed_mult: 1.0,
            crab_near: None,
            walked: 0.0,
            life: Life::new(),
            fx: Fx::new(),
        };
        w.teleport(cur);
        w
    }

    /// Byt plats (åk till Borås osv.)
    pub fn set_site(&mut self, site: &str) {
        self.teleport(site_index(site));
    }

    fn teleport(&mut self, idx: usize) {
        self.cur = idx;
        self.site = SITE_KEYS[idx].to_string();
        let sp = self.sites[idx].spawn;
        self.pos = (sp.0, sp.1);
        self.dir = sp.2;
        self.goal = None;
        self.path.clear();
        self.cam = None;
        self.hover = None;
    }

    /// Markeringar ovanför kollegorna
    pub fn set_mark(&mut self, npc: &str, m: Mark) {
        self.marks.insert(npc.to_string(), m);
    }

    /// Målet som kompassen pekar på (enhetens id eller kollegans namn), None = inget
    pub fn set_target(&mut self, target: Option<String>) {
        self.target = target;
    }

    /// Veckan (kollegor och bilar beror på den)
    /// Gånghastighetens faktor (1.0 normalt, kaffe ger t.ex. 1.35 en stund)
    pub fn set_speed_mult(&mut self, m: f64) {
        self.speed_mult = m;
    }
    /// Gömmer Krabban nära en enhet eller kollega (None = ingen krabba). När spelaren fångar den
    /// skickar update() WorldEvent::Use("krabba") och crab_near blir None.
    pub fn set_crab(&mut self, near: Option<String>) {
        self.crab_near = near;
    }
    /// Gången sträcka i meter sedan förra anropet (nollställs)
    pub fn take_walked(&mut self) -> f64 {
        std::mem::take(&mut self.walked)
    }
    pub fn set_week(&mut self, week: u32) {
        self.week = week;
    }

    /// Ger världen nätets härledda status (anropa med sim::get(st) varje bildruta, eller bara ibland)
    pub fn sync(&mut self, st: &mut State) {
        self.net = Some(sim::get(st));
    }
    pub fn set_net(&mut self, d: Option<Rc<Derived>>) {
        self.net = d;
    }

    /// Spelarens position i världen (x, z) och vilken plats
    pub fn player_pos(&self) -> (f64, f64) {
        self.pos
    }
    pub fn site_name(&self) -> &'static str {
        self.sites[self.cur].name
    }
    /// Zonens namn där spelaren står ("Serverrummet", "Utanför kontoret" ...)
    pub fn zone_name(&self) -> String {
        let s = &self.sites[self.cur];
        for z in &s.zones {
            if self.pos.0 >= z.r.x0 && self.pos.0 <= z.r.x1 && self.pos.1 >= z.r.z0 && self.pos.1 <= z.r.z1 {
                return z.name.to_string();
            }
        }
        match self.cur {
            1 => "Utanför lagret",
            2 => "Utanför kontoret i Mölndal",
            _ => "Utanför kontoret",
        }
        .to_string()
    }
    pub fn zoom_in(&mut self) {
        self.zoom = (self.zoom + 1).min(7);
    }
    pub fn zoom_out(&mut self) {
        self.zoom = (self.zoom - 1).max(1);
    }
    pub fn zoom_level(&self) -> i32 {
        self.zoom
    }

    /// Gå till en punkt i världen (minikartan)
    pub fn walk_to_point(&mut self, x: f64, z: f64) {
        self.walk_to(x, z, None, false);
    }
    /// Rackets enheter i ordning uppifrån och ned (för frontpanelsvyn)
    pub fn rack_units(&self, rack: &str) -> Vec<String> {
        for s in &self.sites {
            for r in &s.racks {
                if r.id == rack {
                    return r.units.iter().map(|u| u.to_string()).collect();
                }
            }
        }
        vec![]
    }

    // ------------------------------------------------------------------ Hjälpare

    fn site(&self) -> &Site {
        &self.sites[self.cur]
    }

    fn npc_visible(&self, n: &Npc) -> bool {
        n.site == self.cur && self.week >= n.weeks.0 && self.week <= n.weeks.1
    }

    fn collides(&self, x: f64, z: f64, r: f64) -> bool {
        let s = &self.sites[self.cur];
        for c in &s.cols {
            if x > c.x0 - r && x < c.x1 + r && z > c.z0 - r && z < c.z1 + r {
                return true;
            }
        }
        for n in &self.npcs {
            if self.npc_visible(n) && x > n.col.x0 - r && x < n.col.x1 + r && z > n.col.z0 - r && z < n.col.z1 + r {
                return true;
            }
        }
        x < s.x0 + 0.5 || x > s.x1 - 0.5 || z < s.z0 + 0.5 || z > s.z1 - 0.5
    }

    fn face_to(&mut self, x: f64, z: f64) {
        let (dx, dz) = (x - self.pos.0, z - self.pos.1);
        self.dir = if dx.abs() > dz.abs() {
            if dx < 0.0 { 2 } else { 3 }
        } else if dz < 0.0 {
            0
        } else {
            1
        };
    }

    fn nav(&mut self) -> &Nav {
        if self.navs[self.cur].is_none() {
            let s = &self.sites[self.cur];
            self.navs[self.cur] = Some(Nav::build(s.x0, s.x1, s.z0, s.z1, &s.cols));
        }
        self.navs[self.cur].as_ref().unwrap()
    }

    /// Alla saker man kan använda just nu
    fn cands(&self, st: &State) -> Vec<Cand> {
        let s = &self.sites[self.cur];
        let mut v = vec![];
        for i in &s.inters {
            if i.min_week > self.week {
                continue;
            }
            match &i.kind {
                IKind::MiniSwitch if !st.devices.contains_key("Mini-switch") => continue,
                IKind::Cable(dev, port) if find_link(st, dev, port).is_none() => continue,
                IKind::Pc(id) if !st.devices.contains_key(id.as_str()) => continue,
                _ => {}
            }
            v.push(Cand { x: i.x, z: i.z, r: i.r, kind: i.kind.clone(), label: i.label.clone(), item: i.item, npc: None });
        }
        self.life_cands(&mut v);
        for (k, n) in self.npcs.iter().enumerate() {
            if self.npc_visible(n) {
                v.push(Cand { x: n.x, z: n.z, r: 1.1, kind: IKind::Npc(n.name.to_string()), label: format!("Prata med {}", n.name), item: None, npc: Some(k) });
            }
        }
        v
    }

    fn event_for(&self, st: &State, c: &Cand) -> Option<WorldEvent> {
        Some(match &c.kind {
            IKind::Rack(id) => WorldEvent::Rack(id.clone()),
            IKind::Laptop => WorldEvent::Laptop,
            IKind::Pc(id) => WorldEvent::UseHost(id.clone()),
            IKind::Phone => WorldEvent::Pickup("Gast-mobil".into()),
            IKind::MiniSwitch => WorldEvent::Pickup("Mini-switch".into()),
            IKind::Monitor => WorldEvent::Use("monitor".into()),
            IKind::Whiteboard => WorldEvent::Use("whiteboard".into()),
            IKind::Printer => WorldEvent::Use("printer".into()),
            IKind::Water => WorldEvent::Use("water".into()),
            IKind::Coffee => WorldEvent::Use("coffee".into()),
            IKind::Travel(to) => WorldEvent::Travel(to.to_string()),
            IKind::Npc(n) => WorldEvent::Talk(n.clone()),
            IKind::Animal(a) => WorldEvent::Use(format!("djur:{}", a)),
            IKind::Crab => WorldEvent::Use("krabba".into()),
            IKind::Cable(dev, port) => WorldEvent::Cable(find_link(st, dev, port)?),
        })
    }

    /// Närmaste sak framför spelaren (E)
    fn pick_facing(&self, st: &State) -> Option<Cand> {
        let (fx, fz) = match self.dir {
            0 => (0.0, -1.0),
            1 => (0.0, 1.0),
            2 => (-1.0, 0.0),
            _ => (1.0, 0.0),
        };
        let mut best: Option<Cand> = None;
        let mut bs = -1e9;
        for c in self.cands(st) {
            let (dx, dz) = (c.x - self.pos.0, c.z - self.pos.1);
            let d = dx.hypot(dz);
            if d > c.r + 0.6 {
                continue;
            }
            let dot = if d > 0.01 { (dx * fx + dz * fz) / d } else { 1.0 };
            let score = dot * 1.2 - d;
            if dot > -0.2 && score > bs {
                bs = score;
                best = Some(c);
            }
        }
        best
    }

    /// Sak under muspekaren (skärmpunkt)
    fn pick_at_screen(&self, st: &State, mx: i32, my: i32) -> Option<Cand> {
        let v = &self.view;
        let (wx, wz) = v.to_world(mx, my);
        let fx = (mx - v.ox).div_euclid(v.z.max(1));
        let fy = (my - v.oy).div_euclid(v.z.max(1));
        let s = &self.sites[self.cur];
        let mut best: Option<Cand> = None;
        let mut bd = 99.0;
        for c in self.cands(st) {
            // Träff på föremålets eller personens bild
            let mut hit = false;
            if let Some(ii) = c.item {
                let it = &s.items[ii];
                let (sx, sy) = v.to_screen(it.x, it.z, it.y);
                let (px, py) = (fx - (sx - it.ox), fy - (sy - it.oy));
                hit = it.spr.get(px, py) >> 24 > 40;
            } else if let Some(k) = c.npc {
                let n = &self.npcs[k];
                let (sx, sy) = v.to_screen(n.x, n.z, 0.0);
                let (px, py) = (fx - (sx - FW / 2 - 1), fy - (sy - FH - 2 + if n.sit { 4 } else { 0 }));
                hit = px >= 0 && py >= 0 && px < FW + 2 && py < FH + 2 && py < 33;
            }
            let d = (c.x - wx).hypot((c.z - wz) * 1.2);
            if hit {
                return Some(c);
            }
            if d < 0.9 && d < bd {
                bd = d;
                best = Some(c);
            }
        }
        best
    }

    fn walk_to(&mut self, x: f64, z: f64, then: Option<Cand>, replan: bool) -> bool {
        let pos = self.pos;
        let site = self.cur;
        // Kollegor och bilar finns som hinder i rutnätet
        let mut navg = self.nav().clone();
        for n in &self.npcs {
            if n.site == site && self.week >= n.weeks.0 && self.week <= n.weeks.1 {
                navg.stamp(&n.col);
            }
        }
        let cols = self.sites[site].cols.clone();
        let mut p = None;
        if let Some(th) = &then {
            p = world_path::find_path(&navg, &cols, pos.0, pos.1, th.x, th.z, Some(th.r + 1.1));
            if p.as_ref().map(|q| q.partial).unwrap_or(true) {
                p = None;
            }
        }
        if p.is_none() {
            p = world_path::find_path(&navg, &cols, pos.0, pos.1, x, z, None);
        }
        match p {
            Some(p) if !p.pts.is_empty() => {
                if p.partial && !replan {
                    self.pops.push(Pop { text: "Så nära det går".into(), x: p.end.0, z: p.end.1, age: 0.0, life: 1.2 });
                }
                self.path = p.pts;
                self.goal = Some(Walk { x: p.end.0, z: p.end.1, then });
                if !replan {
                    self.run_path = p.len > 14.0;
                }
                self.path_stuck = 0.0;
                self.replans = if replan { self.replans + 1 } else { 0 };
                true
            }
            _ => {
                self.goal = None;
                self.path.clear();
                if !replan {
                    self.pops.push(Pop { text: "Dit kommer du inte".into(), x, z, age: 0.0, life: 1.2 });
                }
                false
            }
        }
    }

    // ------------------------------------------------------------------ Uppdatering

    /// Rörelse, klick och E. `blocked` = en dialog är öppen (bara animationer).
    pub fn update(&mut self, input: &Input, dt: f64, st: &State, blocked: bool) -> Vec<WorldEvent> {
        let dt = dt.min(0.1);
        self.t += dt;
        let mut events = vec![];
        self.has_mini = st.devices.contains_key("Mini-switch");
        self.link_snap = st.links.iter().flat_map(|l| [(l.a.dev.clone(), l.a.port.clone(), l.state), (l.b.dev.clone(), l.b.port.clone(), l.state)]).collect();
        for p in self.pops.iter_mut() {
            p.age += dt;
        }
        self.pops.retain(|p| p.age < p.life);
        // Zoom
        if !blocked {
            if input.wheel > 0.0 || input.was_pressed(Key::Z) {
                self.zoom_in();
            }
            if input.wheel < 0.0 || input.was_pressed(Key::X) {
                self.zoom_out();
            }
        }
        // Nuvarande tips och markering
        let mut hover: Option<Cand> = None;
        if !blocked {
            if let Some(c) = self.pick_at_screen(st, input.mouse_x, input.mouse_y) {
                if (c.x - self.pos.0).hypot(c.z - self.pos.1) < c.r + 1.6 && input.mouse_x >= 0 {
                    hover = Some(c);
                }
            }
        }
        let facing = if blocked { None } else { self.pick_facing(st) };
        let shown = hover.clone().or(facing.clone());
        self.hint_text = shown.as_ref().map(|c| format!("E – {}", c.label));
        self.hover = shown;
        if !blocked {
            // Använd det närmaste
            if input.was_pressed(Key::E) || input.was_pressed(Key::Space) || input.was_pressed(Key::Enter) {
                if let Some(c) = self.hover.clone() {
                    if let Some(e) = self.event_for(st, &c) {
                        self.face_to(c.x, c.z);
                        events.push(e);
                    }
                }
            } else if input.was_pressed(Key::T) {
                events.push(WorldEvent::Laptop);
            }
            // Klick: gå dit / använd
            if input.clicked {
                let hit = self.pick_at_screen(st, input.mouse_x, input.mouse_y);
                if let Some(c) = hit {
                    let dist = (c.x - self.pos.0).hypot(c.z - self.pos.1);
                    if dist < c.r + 1.6 {
                        self.face_to(c.x, c.z);
                        if let Some(e) = self.event_for(st, &c) {
                            events.push(e);
                        }
                    } else {
                        let (tx, tz) = (c.x, c.z - 0.6);
                        self.walk_to(tx, tz, Some(c), false);
                    }
                } else {
                    let (wx, wz) = self.view.to_world(input.mouse_x, input.mouse_y);
                    if input.shift && self.goal.is_some() {
                        self.queue_walk(wx, wz);
                    } else {
                        self.walk_to(wx, wz, None, false);
                    }
                }
            }
            if input.right_clicked || input.was_pressed(Key::Escape) {
                self.goal = None;
                self.path.clear();
            }
        }
        // Rörelse
        let mv = if blocked { None } else { self.step_move(input, dt, st) };
        if let Some(e) = mv {
            events.push(e);
        }
        // Kollegor, djur, Krabban och väder
        self.update_npcs(dt, st, blocked);
        self.update_animals(dt);
        self.update_parts(dt);
        self.fx.update(dt, self.t, self.week);
        self.update_crab(dt, st, &mut events);
        if events.iter().any(|e| matches!(e, WorldEvent::Use(s) if s == "krabba")) && self.crab_near.is_some() {
            self.crab_caught();
        }
        // Kameran
        self.update_camera(dt);
        // Dörrarna glider isär när du närmar dig
        let dd = (self.pos.0 - 10.5).hypot(self.pos.1 + 10.1);
        let want = if self.cur == 0 && dd < 2.2 { 1.0 } else { 0.0 };
        self.door_open += (want - self.door_open) * (dt * 6.0).min(1.0);
        events
    }

    fn queue_walk(&mut self, x: f64, z: f64) {
        let from = match &self.goal {
            Some(g) => (g.x, g.z),
            None => return,
        };
        let mut navg = self.nav().clone();
        let site = self.cur;
        for n in &self.npcs {
            if n.site == site && self.week >= n.weeks.0 && self.week <= n.weeks.1 {
                navg.stamp(&n.col);
            }
        }
        let cols = self.sites[site].cols.clone();
        if let Some(p) = world_path::find_path(&navg, &cols, from.0, from.1, x, z, None) {
            if !p.pts.is_empty() {
                self.path.extend(p.pts);
                self.goal = Some(Walk { x: p.end.0, z: p.end.1, then: None });
                self.pops.push(Pop { text: "+ mellanstation".into(), x, z, age: 0.0, life: 1.0 });
            }
        }
    }

    fn step_move(&mut self, input: &Input, dt: f64, st: &State) -> Option<WorldEvent> {
        let down = |k: Key| input.is_down(k);
        let mut dx = (if down(Key::D) || down(Key::Right) { 1.0 } else { 0.0 }) - (if down(Key::A) || down(Key::Left) { 1.0 } else { 0.0 });
        let mut dz = (if down(Key::W) || down(Key::Up) { 1.0 } else { 0.0 }) - (if down(Key::S) || down(Key::Down) { 1.0 } else { 0.0 });
        let manual = dx != 0.0 || dz != 0.0;
        if manual {
            self.goal = None;
            self.path.clear();
        }
        let mut step_cap = f64::INFINITY;
        let mut arrived: Option<Cand> = None;
        if !manual && self.goal.is_some() {
            while self.path.len() > 1 && (self.path[0].0 - self.pos.0).hypot(self.path[0].1 - self.pos.1) < 0.02 {
                self.path.remove(0);
            }
            let g = self.goal.as_ref().unwrap();
            let wp = self.path.first().copied().unwrap_or((g.x, g.z));
            let (tx, tz) = (wp.0 - self.pos.0, wp.1 - self.pos.1);
            let d = tx.hypot(tz);
            let last = self.path.len() <= 1;
            if last && d < 0.15 {
                let g = self.goal.take().unwrap();
                self.path.clear();
                if let Some(th) = g.then {
                    if (th.x - self.pos.0).hypot(th.z - self.pos.1) < th.r + 1.6 {
                        arrived = Some(th);
                    }
                }
            } else if d > 0.001 {
                dx = tx / d;
                dz = tz / d;
                step_cap = d;
            }
        }
        let mut speed = if input.is_down(Key::Shift) || input.shift || (self.goal.is_some() && self.run_path) { 5.0 } else { 3.2 };
        speed *= self.speed_mult.max(0.0);
        let len = dx.hypot(dz);
        let mut moving = false;
        if len > 0.0 {
            dx /= len;
            dz /= len;
            let (ox, oz) = self.pos;
            self.accel = (self.accel + dt * 8.0).min(1.0);
            speed *= 0.35 + 0.65 * self.accel;
            let step = (speed * dt).min(step_cap);
            let (nx, nz) = (self.pos.0 + dx * step, self.pos.1 + dz * step);
            let r = 0.22;
            let mut moved_x = false;
            let mut moved_z = false;
            let trapped = self.collides(self.pos.0, self.pos.1, r);
            if trapped || !self.collides(nx, self.pos.1, r) {
                self.pos.0 = nx;
                moved_x = true;
            }
            if trapped || !self.collides(self.pos.0, nz, r) {
                self.pos.1 = nz;
                moved_z = true;
            }
            // Hörnhjälp: går du rakt mot ett hörn glider du runt det
            if self.goal.is_none() && !moved_x && !moved_z {
                let side = step * 0.8;
                if dx.abs() > dz.abs() {
                    for o in [0.18, -0.18] {
                        if !self.collides(nx, self.pos.1 + o, r) {
                            self.pos.1 += o.signum() * side;
                            moved_z = true;
                            break;
                        }
                    }
                } else {
                    for o in [0.18, -0.18] {
                        if !self.collides(self.pos.0 + o, nz, r) {
                            self.pos.0 += o.signum() * side;
                            moved_x = true;
                            break;
                        }
                    }
                }
            }
            moving = moved_x || moved_z;
            // Fastnar du planeras vägen om, högst tre gånger
            let got = (self.pos.0 - ox).hypot(self.pos.1 - oz);
            self.walked += got;
            if self.goal.is_some() && got < step * 0.3 {
                self.path_stuck += dt;
            } else {
                self.path_stuck = 0.0;
            }
            if self.goal.is_some() && self.path_stuck > 0.3 {
                self.path_stuck = 0.0;
                let g = self.goal.take().unwrap();
                if self.replans >= 3 || !self.walk_to(g.x, g.z, g.then, true) {
                    self.goal = None;
                    self.path.clear();
                }
            }
            if dx.abs() > dz.abs() {
                self.dir = if dx < 0.0 { 2 } else { 3 };
            } else {
                self.dir = if dz < 0.0 { 0 } else { 1 };
            }
        }
        // Kameran tittar lite framåt
        let lk = 1.0 - (-dt * 2.5).exp();
        let (lx, lz) = if moving { (dx * 1.1, dz * 0.8) } else { (0.0, 0.0) };
        self.look.0 += (lx - self.look.0) * lk;
        self.look.1 += (lz - self.look.1) * lk;
        if !moving {
            self.accel = 0.0;
            self.frame = 0;
            self.anim_t = 0.0;
        } else {
            self.anim_t += dt * speed * 2.2;
            self.frame = (self.anim_t.floor() as i64).rem_euclid(4) as u8;
        }
        if let Some(c) = arrived {
            self.face_to(c.x, c.z);
            return self.event_for(st, &c);
        }
        None
    }

    fn update_camera(&mut self, dt: f64) {
        let s = &self.sites[self.cur];
        let z = self.zoom as f64;
        let (cw, ch) = (self.screen.0 as f64, self.screen.1 as f64);
        let half_w = (cw / z) / 2.0 / PPM as f64;
        let half_h = (ch / z) / 2.0 / (PPM * ZS) as f64;
        let mut wx = self.pos.0 + self.look.0;
        let mut wz = self.pos.1 + self.look.1;
        wx = wx.max(s.x0 + half_w).min(s.x1 - half_w);
        wz = wz.max(s.z0 + half_h).min(s.z1 - half_h);
        if s.x1 - s.x0 < half_w * 2.0 {
            wx = (s.x0 + s.x1) / 2.0;
        }
        if s.z1 - s.z0 < half_h * 2.0 {
            wz = (s.z0 + s.z1) / 2.0;
        }
        let cam = self.cam.get_or_insert((wx, wz));
        let k = 1.0 - (-dt * 9.0).exp();
        cam.0 += (wx - cam.0) * k;
        cam.1 += (wz - cam.1) * k;
        let cam = *cam;
        self.view = self.make_view(self.screen.0, self.screen.1, cam);
    }

    fn make_view(&self, cw: i32, ch: i32, cam: (f64, f64)) -> View {
        let z = self.zoom.max(1);
        let vw = (cw + z - 1) / z + 2;
        let vh = (ch + z - 1) / z + 2;
        let hw = vw / 2;
        let hh = vh / 2;
        View {
            z,
            vw,
            vh,
            hw,
            hh,
            cx0: (cam.0 * PPM as f64).floor() as i32,
            cz0: (cam.1 * (PPM * ZS) as f64).floor() as i32,
            ox: cw / 2 - hw * z,
            oy: ch / 2 - hh * z,
            cw,
            ch,
        }
    }

    /// Tipset för det spelaren står vid, t.ex. "E – Koppla in konsolen i SW-Nordvik-1"
    pub fn hint(&self) -> Option<String> {
        self.hint_text.clone()
    }

    // ------------------------------------------------------------------ Ritning

    /// Ritar hela världen över canvasen (utom HUD och dialoger)
    pub fn draw(&mut self, c: &mut Canvas, fonts: &mut Fonts, st: &State) {
        self.screen = (c.w as i32, c.h as i32);
        let cam = match self.cam {
            Some(cam) => cam,
            None => {
                self.update_camera(0.0);
                self.cam.unwrap_or(self.pos)
            }
        };
        let v = self.make_view(c.w as i32, c.h as i32, cam);
        self.view = v;
        if self.fb.w != v.vw || self.fb.h != v.vh {
            self.fb = Cv::new(v.vw, v.vh);
        }
        self.labels.clear();
        let net = self.net.clone().or_else(|| st.cache.clone());
        let t = self.t;
        let mut fb = std::mem::replace(&mut self.fb, Cv::new(1, 1));
        fb.fill(c_(0x3f8a25));
        self.draw_scene(&mut fb, &v, st, net.as_deref());
        self.fb = fb;
        self.present(c);
        self.draw_labels(c, fonts, st);
    }

    fn draw_scene(&mut self, fb: &mut Cv, v: &View, st: &State, net: Option<&Derived>) {
        let t = self.t;
        let cur = self.cur;
        // Marken
        {
            let s = &self.sites[cur];
            let gx = v.cx0 - v.hw - (s.x0 * PPM as f64).round() as i32;
            let gy = (s.z1 * (PPM * ZS) as f64).round() as i32 - v.cz0 - v.hh;
            // Rader av marken som syns
            let sx0 = gx.max(0);
            let sy0 = gy.max(0);
            let sx1 = (gx + v.vw).min(s.ground.w);
            let sy1 = (gy + v.vh).min(s.ground.h);
            for sy in sy0..sy1 {
                let dy = sy - gy;
                for sx in sx0..sx1 {
                    fb.d[(dy * fb.w + (sx - gx)) as usize] = s.ground.d[(sy * s.ground.w + sx) as usize];
                }
            }
        }
        // Vägen som prickar
        if !self.path.is_empty() && self.goal.is_some() {
            let mut pts = vec![self.pos];
            pts.extend(self.path.iter().copied());
            let mut acc = 0.0;
            for i in 0..pts.len() - 1 {
                let (a, b) = (pts[i], pts[i + 1]);
                let l = (b.0 - a.0).hypot(b.1 - a.1);
                let mut d = 0.5 - acc % 0.5;
                while d < l {
                    let q = d / l;
                    let (sx, sy) = v.to_screen(a.0 + (b.0 - a.0) * q, a.1 + (b.1 - a.1) * q, 0.0);
                    let pulse = 0.55 + 0.35 * (t * 5.0 - (acc + d) * 2.0).sin();
                    fb.put(sx, sy, ca(0x281e0a, (pulse * 0.5) as f32));
                    fb.put(sx - 1, sy, ca(0x281e0a, (pulse * 0.5) as f32));
                    fb.put(sx + 1, sy, ca(0x281e0a, (pulse * 0.5) as f32));
                    fb.rect(sx - 1, sy - 1, 2, 2, ca(0xfff0aa, pulse as f32));
                    d += 0.5;
                }
                acc += l;
            }
        }
        // Vad ska ritas, bakifrån och framåt
        enum Dr {
            Item(usize),
            Npc(usize),
            Player,
            Animal(usize),
            Crab,
        }
        let mut list: Vec<(f64, Dr)> = vec![];
        {
            let s = &self.sites[cur];
            let (hwm, hhm) = (v.vw as f64 / 2.0 / PPM as f64, v.vh as f64 / 2.0 / (PPM * ZS) as f64);
            let (cxm, czm) = (v.cx0 as f64 / PPM as f64, v.cz0 as f64 / (PPM * ZS) as f64);
            let (min_x, max_x, min_z, max_z) = (cxm - hwm - 4.0, cxm + hwm + 4.0, czm - hhm - 6.0, czm + hhm + 9.0);
            for (i, it) in s.items.iter().enumerate() {
                if it.min_week > self.week {
                    continue;
                }
                let w = it.spr.w as f64 / PPM as f64;
                if it.x + w < min_x || it.x - it.ox as f64 / PPM as f64 > max_x || it.z + 3.0 < min_z || it.z - 8.0 > max_z {
                    continue;
                }
                list.push((it.key, Dr::Item(i)));
            }
            for (k, n) in self.npcs.iter().enumerate() {
                if self.npc_visible(n) && n.x > min_x && n.x < max_x {
                    list.push((n.z - 0.001, Dr::Npc(k)));
                }
            }
            for (i, a) in self.life.animals.iter().enumerate() {
                if a.site == cur && !(matches!(a.kind, life::AK::Duck(_)) && a.dive > 0.0) && a.y < 6.0 {
                    list.push((a.z - 0.001, Dr::Animal(i)));
                }
            }
            if let Some(cb) = &self.life.crab {
                if cb.site == cur {
                    list.push((cb.z - 0.003, Dr::Crab));
                }
            }
            list.push((self.pos.1 - 0.002, Dr::Player));
        }
        list.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        // Skuggor
        {
            let s = &self.sites[cur];
            for (_, d) in &list {
                match d {
                    Dr::Item(i) => {
                        let it = &s.items[*i];
                        match &it.shadow {
                            Shadow::Ell { rx, rz, dx } => {
                                let (sx, sy) = v.to_screen(it.x + dx, it.z - dx * 0.4, 0.0);
                                dither_ellipse(fb, sx, sy, rx * PPM as f64, rz * (PPM * ZS) as f64, 0.26);
                            }
                            Shadow::Sil(sil) => {
                                let (sx, sy) = v.to_screen(it.x, it.z, it.y);
                                fb.blit(sil, sx - it.ox + 3, sy - it.oy + 2);
                            }
                            Shadow::None => {}
                        }
                    }
                    Dr::Npc(k) => {
                        let n = &self.npcs[*k];
                        let (sx, sy) = v.to_screen(n.x, n.z, 0.0);
                        dither_ellipse(fb, sx + 1, sy, 7.0, 2.6, 0.3);
                    }
                    Dr::Player => {
                        let (sx, sy) = v.to_screen(self.pos.0, self.pos.1, 0.0);
                        dither_ellipse(fb, sx + 1, sy, 7.0, 2.6, 0.3);
                    }
                    Dr::Animal(i) => {
                        let a = &self.life.animals[*i];
                        if !matches!(a.kind, life::AK::Duck(_)) {
                            let (sx, sy) = v.to_screen(a.x, a.z, 0.0);
                            let r = if a.y > 0.05 { 2.0 } else { 4.5 };
                            dither_ellipse(fb, sx, sy, r, r * 0.4, 0.28);
                        }
                    }
                    Dr::Crab => {
                        let cb = self.life.crab.as_ref().unwrap();
                        let (sx, sy) = v.to_screen(cb.x, cb.z, 0.0);
                        dither_ellipse(fb, sx, sy, 7.0, 2.0, 0.25);
                    }
                }
            }
        }
        let mut late: Vec<(Rc<Cv>, i32, i32)> = vec![];
        // Själva ritningen
        let hover_item = self.hover.as_ref().and_then(|h| h.item);
        let hover_npc = self.hover.as_ref().and_then(|h| h.npc);
        for (_, d) in &list {
            match d {
                Dr::Item(i) => {
                    let it = &self.sites[cur].items[*i];
                    let (sx, sy) = v.to_screen(it.x, it.z, it.y);
                    match &it.dynk {
                        Dyn::None => {
                            let mut alpha = 1.0f32;
                            if let Some((x0, x1, h)) = it.fade {
                                let top = it.z + h * HS as f64 / ZS as f64;
                                if self.pos.0 > x0 - 0.2 && self.pos.0 < x1 + 0.2 && self.pos.1 > it.z && self.pos.1 < top + 0.6 {
                                    alpha = 0.45;
                                }
                            }
                            fb.blit_a(&it.spr, sx - it.ox, sy - it.oy, alpha);
                            if hover_item == Some(*i) {
                                highlight(fb, &it.spr, sx - it.ox, sy - it.oy, t);
                            }
                        }
                        Dyn::Pc(id) => draw_pc(fb, sx, sy, id, st, net, t),
                        Dyn::Laptop => draw_laptop(fb, sx, sy),
                        Dyn::Monitor => {
                            fb.blit(&it.spr, sx - it.ox, sy - it.oy);
                            draw_monitor(fb, sx, sy, net, t);
                            if hover_item == Some(*i) {
                                highlight(fb, &it.spr, sx - it.ox, sy - it.oy, t);
                            }
                        }
                        Dyn::Rack(ri) => {
                            fb.blit(&it.spr, sx - it.ox, sy - it.oy);
                            let r = &self.sites[cur].racks[*ri];
                            draw_rack_front(fb, sx, sy, it.spr.h, r, st, net, t);
                            if hover_item == Some(*i) {
                                highlight(fb, &it.spr, sx - it.ox, sy - it.oy, t);
                            }
                        }
                        Dyn::Doors => draw_doors(fb, &it.spr, sx, sy, self.door_open),
                        Dyn::Ap(id) => {
                            let (gx, gy) = v.to_screen(it.x, it.z, 0.0);
                            draw_ap(fb, sx, sy, gx, gy, id, st, net, t);
                            self.labels.push(Label { fx: sx, fy: sy - 6, text: id.to_string(), bg: 0x141820, fg: 0xffffff, scale: 0.75 });
                        }
                        Dyn::Cable(idx) => {
                            let cs = &self.sites[cur].cable_spots[*idx];
                            let hot = self.hover.as_ref().map(|h| matches!(&h.kind, IKind::Cable(d, p) if *d == cs.2 && *p == cs.3)).unwrap_or(false);
                            draw_cable_plate(fb, sx, sy, &cs.2, &cs.3, st, net, t, hot);
                        }
                        Dyn::Truck => {}
                    }
                }
                Dr::Npc(k) => {
                    let (x, z, sit, look, phase, name) = {
                        let n = &self.npcs[*k];
                        (n.x, n.z, n.sit, n.look, n.phase, n.name)
                    };
                    let (sx, sy) = v.to_screen(x, z, 0.0);
                    // Riktning och andning
                    let dd = (self.pos.0 - x).hypot(self.pos.1 - z);
                    let (n_dir, n_frame, n_moving) = {
                        let n = &self.npcs[*k];
                        (n.dir, n.frame, n.moving)
                    };
                    let dir = if n_moving {
                        n_dir
                    } else if !sit {
                        if dd < 2.5 {
                            let (dxp, dzp) = (self.pos.0 - x, self.pos.1 - z);
                            if dxp.abs() > dzp.abs() {
                                if dxp < 0.0 { 2 } else { 3 }
                            } else if dzp < 0.0 {
                                0
                            } else {
                                1
                            }
                        } else {
                            n_dir
                        }
                    } else if dd < 2.2 && (self.pos.0 - x).abs() > 0.5 {
                        if self.pos.0 < x { 2 } else { 3 }
                    } else {
                        0
                    };
                    let frame = if n_moving { n_frame } else if (t * 2.1 + phase).sin() > 0.55 { 4 } else { 0 };
                    let blink = (t + phase * 1.7) % 4.3 < 0.14;
                    let spr = self.person_sprite(look as u8, dir, frame, sit, blink);
                    let yoff = if sit { 4 } else { 0 };
                    let bx = sx - spr.w / 2;
                    let by = sy - spr.h + yoff;
                    fb.blit(&spr, bx, by);
                    if hover_npc == Some(*k) {
                        highlight(fb, &spr, bx, by, t);
                    }
                    // Namn och pratbubbla
                    let top = by - 2;
                    if dd < 4.5 {
                        self.labels.push(Label { fx: sx, fy: top - 2, text: name.to_string(), bg: 0x2b1a10, fg: 0xfff6dc, scale: 0.8 });
                    }
                    let mark = self.marks.get(name).copied().unwrap_or(Mark::None);
                    if mark != Mark::None {
                        let bob = ((t * 3.0 + phase).sin() * 1.5).round() as i32;
                        let b = Rc::new(bubble(mark == Mark::Done));
                        late.push((b.clone(), sx - b.w / 2, top - 18 + bob));
                    }
                }
                Dr::Player => {
                    let (sx, sy) = v.to_screen(self.pos.0, self.pos.1, 0.0);
                    let frame = if self.frame == 0 && (t * 2.1).sin() > 0.55 { 4 } else { self.frame };
                    let spr = self.person_sprite(255, self.dir, frame, false, t % 3.7 < 0.13);
                    fb.blit(&spr, sx - spr.w / 2, sy - spr.h);
                }
                Dr::Animal(i) => {
                    let (kind, x, z, y, left, sit, moving, ph, away, hover_a) = {
                        let a = &self.life.animals[*i];
                        (a.kind, a.x, a.z, a.y, a.left, a.sit, a.moving, a.ph, a.away, self.hover.as_ref().map(|h| matches!(&h.kind, IKind::Animal(_)) && (h.x - a.x).hypot(h.z - a.z) < 0.01).unwrap_or(false))
                    };
                    let (sx, sy) = v.to_screen(x, z, y);
                    let walk = ((ph * 6.0) as i64 % 2) as u8;
                    let (spr, ax, ay, dy) = match kind {
                        life::AK::Cat => {
                            let (s, ax, ay) = self.life.sprite(1, if sit { 2 } else if moving { walk } else { 0 }, left);
                            (s, ax, ay, 0)
                        }
                        life::AK::Duck(_) => {
                            let (s, ax, ay) = self.life.sprite(2, 0, left);
                            // Ringar i vattnet
                            let rr = 5.0 + (ph * 2.0).sin() * 1.0;
                            ring(fb, sx, sy, rr, rr * 0.4, ca(0xffffff, 0.35));
                            (s, ax, ay, ((ph * 3.0).sin() * 0.6).round() as i32)
                        }
                        life::AK::Pigeon(_) => {
                            let flying = away > 0.0;
                            let peck = !flying && (ph * 2.2).sin() > 0.6;
                            let (s, ax, ay) = self.life.sprite(3, if flying && walk == 0 { 1 } else { 0 }, left);
                            (s, ax, ay, if peck { 1 } else { 0 })
                        }
                        life::AK::Gull => {
                            let (s, ax, ay) = self.life.sprite(4, 0, left);
                            (s, ax, ay, if moving && walk == 1 { -1 } else { 0 })
                        }
                    };
                    fb.blit(&spr, sx - ax, sy - ay + dy);
                    if hover_a {
                        highlight(fb, &spr, sx - ax, sy - ay + dy, t);
                    }
                }
                Dr::Crab => {
                    let (x, z, anim, fleeing, left, moving) = {
                        let c = self.life.crab.as_ref().unwrap();
                        (c.x, c.z, c.anim, c.fleeing, c.left, c.moving)
                    };
                    let (sx, sy) = v.to_screen(x, z, 0.0);
                    let var = ((anim * 8.0) as i64 % 2) as u8 + if fleeing { 2 } else { 0 };
                    let (spr, ax, ay) = self.life.sprite(0, var, false);
                    let _ = (left, moving);
                    fb.blit(&spr, sx - ax, sy - ay);
                    if self.hover.as_ref().map(|h| matches!(h.kind, IKind::Crab)).unwrap_or(false) {
                        highlight(fb, &spr, sx - ax, sy - ay, t);
                    }
                }
            }
        }
        self.draw_parts(fb, v);
        self.apply_light(fb, v, st);
        for (b, x, y) in &late {
            fb.blit(b, *x, *y);
        }
        // Ringar på sådant utan bild som går att använda
        if let Some(h) = &self.hover {
            if h.item.is_none() && h.npc.is_none() {
                let (sx, sy) = v.to_screen(h.x, h.z, 0.0);
                let pulse = 0.5 + 0.3 * (t * 5.0).sin();
                ring(fb, sx, sy, 9.0, 3.5, ca(0xffd54a, pulse as f32));
            }
        }
        // Kompassen
        self.draw_compass(fb, v, st, t);
        for p in &self.pops {
            let (sx, sy) = v.to_screen(p.x, p.z, 0.4 + p.age * 0.3);
            self.labels.push(Label { fx: sx, fy: sy, text: p.text.clone(), bg: 0x3a2818, fg: 0xffe1a8, scale: 0.8 });
        }
        // Skyltar
        let s = &self.sites[cur];
        for sg in &s.signs {
            let (sx, sy) = v.to_screen(sg.x, sg.z, sg.y);
            if sx < -40 || sy < -10 || sx > v.vw + 40 || sy > v.vh + 10 {
                continue;
            }
            let hide = sg.y > 2.0 && (self.pos.0 - sg.x).hypot(self.pos.1 - sg.z) > 5.0;
            if hide {
                continue;
            }
            self.labels.push(Label { fx: sx, fy: sy, text: sg.text.clone(), bg: 0x3a2818, fg: 0xffe9b8, scale: 0.85 });
        }
    }

    fn labels_push_ap(&mut self, sx: i32, sy: i32, id: &str) {
        self.labels.push(Label { fx: sx, fy: sy - 6, text: id.to_string(), bg: 0x141820, fg: 0xffffff, scale: 0.75 });
    }

    fn person_sprite(&mut self, who: u8, dir: u8, frame: u8, sit: bool, blink: bool) -> Rc<Cv> {
        let key = (who, dir, frame, sit, blink);
        if let Some(s) = self.cache.get(&key) {
            return s.clone();
        }
        let look = if who == 255 {
            Look { skin: 0xf0c8a0, hair: 0x5a3a22, shirt: 0x2f6fb0, pants: 0x33405a, cap: Some(0xd64b3a), ..Default::default() }
        } else {
            self.looks[who as usize].clone()
        };
        let s = Rc::new(person(&look, dir, frame, sit, blink));
        self.cache.insert(key, s.clone());
        s
    }

    fn target_pos(&self, st: &State) -> Option<(usize, f64, f64, f64)> {
        let id = self.target.as_ref()?;
        // Kollega?
        for n in &self.npcs {
            if n.name == id && self.week >= n.weeks.0 && self.week <= n.weeks.1 {
                return Some((n.site, n.x, n.z, 1.9));
            }
        }
        // Plats i någon byggnad
        for (si, s) in self.sites.iter().enumerate() {
            if let Some(sp) = s.spots.iter().find(|sp| sp.0 == *id) {
                return Some((si, sp.1, sp.2, sp.3));
            }
        }
        // Okänd enhet: platsen från enheten, mitt i byggnaden
        if let Some(d) = st.devices.get(id.as_str()) {
            let si = site_index(&d.site);
            let b = self.sites[si].building;
            return Some((si, (b.x0 + b.x1) / 2.0, (b.z0 + b.z1) / 2.0, 1.0));
        }
        None
    }

    fn draw_compass(&mut self, fb: &mut Cv, v: &View, st: &State, t: f64) {
        let Some((si, mut x, mut z, mut y)) = self.target_pos(st) else { return };
        if si != self.cur {
            // Målet finns på en annan plats: peka på bilen dit
            let key = if self.cur == 0 { SITE_KEYS[si] } else { "gbg" };
            let s = &self.sites[self.cur];
            match s.spots.iter().find(|sp| sp.0 == key) {
                Some(sp) => {
                    x = sp.1;
                    z = sp.2;
                    y = 1.0;
                }
                None => return,
            }
        }
        let (sx, sy) = v.to_screen(x, z, y + 0.9);
        let bob = ((t * 4.0).sin() * 2.0).round() as i32;
        let arrow = arrow_down(0xffd54a);
        let margin = 14;
        if sx >= margin && sy >= margin && sx <= v.vw - margin && sy <= v.vh - margin {
            fb.blit(&arrow, sx - 5, sy - 12 + bob);
            let (gx, gy) = v.to_screen(x, z, 0.0);
            ring(fb, gx, gy, 10.0 + (t * 4.0).sin() * 1.5, 4.0, ca(0xffd54a, 0.55));
        } else {
            // Pil vid skärmkanten mot målet
            let (cx, cy) = (v.vw / 2, v.vh / 2);
            let (dx, dy) = ((sx - cx) as f64, (sy - cy) as f64);
            let k = ((v.vw as f64 / 2.0 - 16.0) / dx.abs().max(1e-6)).min((v.vh as f64 / 2.0 - 16.0) / dy.abs().max(1e-6));
            let (ex, ey) = (cx + (dx * k) as i32, cy + (dy * k) as i32);
            let ang = dy.atan2(dx);
            for i in 0..7 {
                let r = i as f64;
                let (px, py) = (ex as f64 - ang.cos() * r * 1.6, ey as f64 - ang.sin() * r * 1.6);
                let wdt = (7 - i) as f64 / 2.0;
                let (nx, ny) = (-ang.sin(), ang.cos());
                for q in -(wdt as i32)..=(wdt as i32) {
                    fb.put((px + nx * q as f64).round() as i32, (py + ny * q as f64).round() as i32, c(if i == 0 { 0x2b1a10 } else { 0xffd54a }));
                }
            }
        }
    }

    /// Förstorar pixelbufferten till canvasen
    fn present(&self, cnv: &mut Canvas) {
        let v = &self.view;
        let z = v.z;
        let fb = &self.fb;
        let (cw, ch) = (cnv.w as i32, cnv.h as i32);
        for fy in 0..fb.h {
            let sy0 = v.oy + fy * z;
            if sy0 + z <= 0 || sy0 >= ch {
                continue;
            }
            // En skalad rad
            let mut row = vec![0u32; cw as usize];
            for fx in 0..fb.w {
                let sx0 = v.ox + fx * z;
                if sx0 + z <= 0 || sx0 >= cw {
                    continue;
                }
                let px = fb.d[(fy * fb.w + fx) as usize] & 0xFFFFFF;
                for k in 0..z {
                    let x = sx0 + k;
                    if x >= 0 && x < cw {
                        row[x as usize] = px;
                    }
                }
            }
            for k in 0..z {
                let y = sy0 + k;
                if y >= 0 && y < ch {
                    let o = (y * cw) as usize;
                    cnv.px[o..o + cw as usize].copy_from_slice(&row);
                }
            }
        }
    }

    fn draw_labels(&mut self, c: &mut Canvas, fonts: &mut Fonts, st: &State) {
        let v = self.view;
        let labels = std::mem::take(&mut self.labels);
        for l in &labels {
            let (sx, sy) = v.hi(l.fx, l.fy);
            let size = ((v.z as f32) * 5.2 * l.scale).clamp(11.0, 28.0);
            let (tw, th) = fonts.measure(&l.text, size, FontKind::Mono);
            let pad = (size * 0.3) as i32;
            let (x, y) = (sx - tw / 2 - pad, sy - th - pad / 2);
            c.fill_rect_a(x, y, tw + pad * 2, th + pad, l.bg, 215);
            fonts.text(c, x + pad, y + pad / 2, &l.text, size, l.fg, FontKind::Mono);
        }
        self.labels = labels;
        // Tips under spelaren om det finns något att göra är UI:ts sak (hint()).
    }

    // ------------------------------------------------------------------ Karta

    /// Platsens namn och mått: (nyckel, namn, x0, x1, z0, z1)
    pub fn sites_info(&self) -> Vec<(String, String, f64, f64, f64, f64)> {
        self.sites.iter().map(|s| (s.key.to_string(), s.name.to_string(), s.x0, s.x1, s.z0, s.z1)).collect()
    }

    /// Ritar marken för en plats nedskalad till rutan (x, y, w, h) – för kartan (M) och minikartan.
    /// Markerar spelaren, kollegorna och målet. Returnerar skalan (pixlar per meter) och vänstra/övre kanten i världen.
    pub fn draw_map(&self, c: &mut Canvas, site: &str, x: i32, y: i32, w: i32, h: i32) -> (f64, f64, f64) {
        let si = site_index(site);
        let s = &self.sites[si];
        let ww = s.x1 - s.x0;
        let hh = s.z1 - s.z0;
        let scale = (w as f64 / ww).min(h as f64 / hh);
        let (mw, mh) = ((ww * scale) as i32, (hh * scale) as i32);
        let (ox, oy) = (x + (w - mw) / 2, y + (h - mh) / 2);
        // Marken (närmaste granne)
        for py in 0..mh {
            let gy = (py as f64 / mh as f64 * s.ground.h as f64) as i32;
            for px in 0..mw {
                let gx = (px as f64 / mw as f64 * s.ground.w as f64) as i32;
                let p = s.ground.d[(gy.min(s.ground.h - 1) * s.ground.w + gx.min(s.ground.w - 1)) as usize];
                if p >> 24 != 0 {
                    let (cx, cy) = (ox + px, oy + py);
                    if cx >= 0 && cy >= 0 && (cx as usize) < c.w && (cy as usize) < c.h {
                        c.px[cy as usize * c.w + cx as usize] = p & 0xFFFFFF;
                    }
                }
            }
        }
        // Väggar som mörka linjer
        for it in &s.items {
            if it.wall && it.spr.w > 2 {
                let (wx0, wz0) = (it.x, it.z);
                let (px, py) = (ox + ((wx0 - s.x0) * scale) as i32, oy + ((s.z1 - wz0) * scale) as i32);
                let (pw, ph) = (((it.spr.w as f64 / PPM as f64) * scale).max(1.0) as i32, ((it.spr.h as f64 / (PPM * ZS) as f64) * 0.0).max(1.0) as i32);
                c.fill_rect(px, py - ph, pw, ph.max(1) + 1, 0x4a3526);
            }
        }
        let mark = |c: &mut Canvas, wx: f64, wz: f64, col: u32, r: i32| {
            let (px, py) = (ox + ((wx - s.x0) * scale) as i32, oy + ((s.z1 - wz) * scale) as i32);
            c.circle(px, py, r + 1, 0x101010, 255);
            c.circle(px, py, r, col, 255);
        };
        for n in &self.npcs {
            if n.site == si && self.week >= n.weeks.0 && self.week <= n.weeks.1 {
                let m = self.marks.get(n.name).copied().unwrap_or(Mark::None);
                mark(c, n.x, n.z, match m { Mark::Ticket => 0xe8a317, Mark::Done => 0x3fae4f, Mark::None => 0x9ab0c8 }, 3);
            }
        }
        if let Some((tsi, tx, tz, _)) = self.target_pos_simple() {
            if tsi == si {
                mark(c, tx, tz, 0xffd54a, 5);
            }
        }
        if si == self.cur {
            mark(c, self.pos.0, self.pos.1, 0xff4a3a, 4);
        }
        (scale, ox as f64 - 0.0, oy as f64)
    }

    fn target_pos_simple(&self) -> Option<(usize, f64, f64, f64)> {
        let id = self.target.as_ref()?;
        for n in &self.npcs {
            if n.name == id && self.week >= n.weeks.0 && self.week <= n.weeks.1 {
                return Some((n.site, n.x, n.z, 1.9));
            }
        }
        for (si, s) in self.sites.iter().enumerate() {
            if let Some(sp) = s.spots.iter().find(|sp| sp.0 == *id) {
                return Some((si, sp.1, sp.2, sp.3));
            }
        }
        None
    }

    /// Världskoordinat för en pixel på kartan som ritats med draw_map (för klick på kartan)
    pub fn map_to_world(&self, site: &str, x: i32, y: i32, w: i32, h: i32, px: i32, py: i32) -> (f64, f64) {
        let s = &self.sites[site_index(site)];
        let (ww, hh) = (s.x1 - s.x0, s.z1 - s.z0);
        let scale = (w as f64 / ww).min(h as f64 / hh);
        let (mw, mh) = ((ww * scale) as i32, (hh * scale) as i32);
        let (ox, oy) = (x + (w - mw) / 2, y + (h - mh) / 2);
        (s.x0 + (px - ox) as f64 / scale, s.z1 - (py - oy) as f64 / scale)
    }
}

// ------------------------------------------------------------------ Fria ritfunktioner

fn c_(rgb: u32) -> u32 {
    c(rgb)
}

/// Länken som sitter i (enhet, port); "nic" för datorer
pub fn find_link(st: &State, dev: &str, port: &str) -> Option<String> {
    st.links.iter().find(|l| (l.a.dev == dev && l.a.port == port) || (l.b.dev == dev && l.b.port == port)).map(|l| l.id.clone())
}

fn link_state(st: &State, dev: &str, port: &str) -> Option<LinkState> {
    st.links.iter().find(|l| (l.a.dev == dev && l.a.port == port) || (l.b.dev == dev && l.b.port == port)).map(|l| l.state)
}

const LED_OFF: u32 = 0x1d261f;
const LED_GREEN: u32 = 0x3dff6a;
const LED_DIM: u32 = 0x1d4a26;
const LED_AMBER: u32 = 0xffb020;

/// Färgen på en ports lysdiod efter nätets status (sim::get): upp, ner, err-disabled, STP-blockerad, storm
pub fn port_led(st: &State, net: Option<&Derived>, dev: &str, port: &str, t: f64) -> u32 {
    let Some(d) = st.devices.get(dev) else { return LED_OFF };
    if !d.powered {
        return LED_OFF;
    }
    let flick = |salt: i32| hash2((t * 9.0) as i32, salt, 7) < 0.18;
    let salt = (crate::net::util::hash(port) & 0xff) as i32;
    if d.is_ios() {
        if let Some(cfg) = d.config.as_ref() {
            match cfg.ifaces.get(port) {
                None => return LED_OFF,
                Some(i) => {
                    if d.rt.errdisabled.contains_key(port) {
                        return if (t * 2.0) as i64 % 2 == 0 { LED_AMBER } else { 0x3a2a10 };
                    }
                    if i.shutdown {
                        return LED_OFF;
                    }
                }
            }
        }
        let ls = link_state(st, dev, port);
        let mut up = matches!(ls, Some(LinkState::Ok) | Some(LinkState::Flapping));
        if let Some(n) = net {
            if let Some(p) = n.ports.get(&sim::key(dev, port)) {
                up = p.up;
            }
            if up {
                let pre = format!("{}|{}#", dev, port);
                if n.blocked.keys().any(|k| k.starts_with(&pre)) {
                    return LED_AMBER;
                }
                if n.storm.values().any(|v| v.iter().any(|s| s == dev)) {
                    return if (t * 7.0) as i64 % 2 == 0 { LED_GREEN } else { 0x16301b };
                }
                if n.ports.get(&sim::key(dev, port)).map(|p| p.mismatch).unwrap_or(false) {
                    return if flick(salt) { LED_AMBER } else { LED_GREEN };
                }
            }
        }
        if ls == Some(LinkState::Flapping) && st.time % 23.0 < 2.5 {
            return LED_OFF;
        }
        if !up {
            return LED_OFF;
        }
        return if flick(salt) { LED_DIM } else { LED_GREEN };
    }
    // Servrar och datorer
    match link_state(st, dev, port) {
        Some(LinkState::Ok) => {
            if let Some(n) = net {
                if let Some(li) = find_link(st, dev, port).and_then(|id| n.links.get(&id)) {
                    if !li.up {
                        return LED_OFF;
                    }
                }
            }
            if flick(salt) { LED_DIM } else { LED_GREEN }
        }
        Some(LinkState::Flapping) => {
            if st.time % 23.0 < 2.5 { LED_OFF } else { LED_GREEN }
        }
        _ => LED_OFF,
    }
}

fn draw_pc(fb: &mut Cv, sx: i32, sy: i32, id: &str, st: &State, net: Option<&Derived>, t: f64) {
    let up = match link_state(st, id, "nic") {
        Some(LinkState::Ok) => net.and_then(|n| n.ep(id, "nic")).map(|e| e.up).unwrap_or(true),
        _ => false,
    };
    let dev = st.devices.get(id);
    let apipa = dev.map(|d| d.lease.as_ref().map(|l| l.apipa || l.nolink).unwrap_or(d.nic.as_ref().map(|n| n.dhcp).unwrap_or(false))).unwrap_or(false);
    let col = if !up { 0xe2463b } else if apipa { 0xf0b429 } else { 0x4fc3f7 };
    let p = |fb: &mut Cv, x: i32, y: i32, w: i32, h: i32, cc: u32| fb.rect(sx + x, sy + y, w, h, c(cc));
    // Skärmens baksida med statuslampa
    p(fb, -4, -10, 13, 8, 0x2b1a10);
    p(fb, -3, -9, 11, 6, 0x3a3d42);
    p(fb, -2, -8, 3, 1, 0x55595f);
    p(fb, 5, -8, 2, 2, col);
    if col != 0x4fc3f7 && (t * 2.0) as i64 % 2 == 1 {
        fb.rect(sx + 4, sy - 9, 4, 4, ca(col, 0.6));
    }
    p(fb, 1, -2, 3, 2, 0x1b1d21);
    p(fb, 8, -1, 10, 2, 0x3a3d42);
}

fn draw_laptop(fb: &mut Cv, sx: i32, sy: i32) {
    let p = |fb: &mut Cv, x: i32, y: i32, w: i32, h: i32, cc: u32| fb.rect(sx + x, sy + y, w, h, c(cc));
    p(fb, 0, -10, 14, 9, 0x2b2d31);
    p(fb, 1, -9, 12, 7, 0x300a24);
    p(fb, 2, -8, 6, 1, 0x8fe388);
    p(fb, 2, -6, 8, 1, 0xe8e8e8);
    p(fb, 0, -1, 16, 3, 0x45484e);
    p(fb, 17, -2, 4, 3, 0x7fc6f0);
}

fn draw_monitor(fb: &mut Cv, sx: i32, sy: i32, net: Option<&Derived>, t: f64) {
    let storm = net.map(|n| !n.storm.is_empty()).unwrap_or(false);
    // sx,sy = nedre vänstra hörnet (38x22)
    let x0 = sx;
    let y0 = sy - 22;
    for i in 0..4 {
        let bx = x0 + 3 + (i % 2) * 16;
        let by = y0 + 3 + (i / 2) * 9;
        for k in 0..14 {
            let v = (hash2(k + (t * 1.5) as i32, i, 3) * 0.5 + 0.2 + if storm { 0.4 } else { 0.0 }).min(1.0);
            let h = ((v * 7.0).round() as i32).max(1);
            fb.rect(bx + k, by + 8 - h, 1, h, c(if v > 0.8 { 0xff5a4f } else { 0x4fc3f7 }));
        }
    }
    if storm && (t * 2.0) as i64 % 2 == 1 {
        fb.rect(x0 + 30, y0 + 1, 4, 3, c(0xff5a4f));
    }
}

fn draw_rack_front(fb: &mut Cv, sx: i32, sy: i32, spr_h: i32, rack: &RackSpot, st: &State, net: Option<&Derived>, t: f64) {
    // Frontytan: de nedersta (spr_h - dp - 1) raderna; plint 3 px
    let w = (rack.w * PPM as f64).round() as i32;
    let dp = (rack.d * (PPM * ZS) as f64).round() as i32;
    let hp = spr_h - dp - 2;
    let front_top = sy - hp - 1;
    let units: Vec<&&str> = rack.units.iter().filter(|u| st.devices.contains_key(**u)).collect();
    let n = units.len().max(1) as i32;
    let avail = hp - 10;
    let uh = (avail / n).clamp(4, 7);
    for (row, id) in units.iter().enumerate() {
        let id: &str = id;
        let y = front_top + 4 + row as i32 * uh;
        let d = &st.devices[id];
        let body = match d.kind {
            crate::net::model::Kind::Router | crate::net::model::Kind::Wlc | crate::net::model::Kind::Lb => 0xc9ccc9,
            crate::net::model::Kind::Switch => 0x4c5b6b,
            _ => 0x3a3d44,
        };
        fb.rect(sx + 3, y, w - 5, uh - 1, c(body));
        fb.rect(sx + 3, y, w - 5, 1, c(shade(body, 0.25)));
        fb.rect(sx + 3, y + uh - 2, w - 5, 1, c(shade(body, -0.3)));
        // Strömlampa och portlampor för de kablar som sitter i enheten
        let powered = d.powered;
        fb.set(sx + 4, y + (uh - 1) / 2, c(if powered { 0x3dff6a } else { 0x401010 }));
        let mut ports: Vec<String> = vec![];
        for l in &st.links {
            if l.a.dev == id {
                ports.push(l.a.port.clone());
            } else if l.b.dev == id {
                ports.push(l.b.port.clone());
            }
        }
        for (k, p) in ports.iter().take(((w - 9) / 2).max(1) as usize).enumerate() {
            let col = port_led(st, net, id, p, t);
            fb.set(sx + 6 + k as i32 * 2, y + (uh - 1) / 2, c(col));
        }
    }
}

fn draw_ap(fb: &mut Cv, sx: i32, sy: i32, gx: i32, gy: i32, id: &str, st: &State, net: Option<&Derived>, t: f64) {
    fb.ellipse(gx as f32, gy as f32, 6.0, 2.0, ca(0, 0.18));
    fb.rect(sx - 7, sy - 3, 14, 6, c(0x2b1a10));
    fb.rect(sx - 6, sy - 2, 12, 4, c(0xf4f4f2));
    let joined = net.and_then(|n| n.ap_joined.get(id).copied()).unwrap_or(true);
    let on = st.devices.get(id).map(|d| d.powered).unwrap_or(true);
    let col = if !on { 0x333333 } else if joined { 0x3dff6a } else if (t * 2.0) as i64 % 2 == 1 { 0xffa020 } else { 0x553300 };
    fb.rect(sx - 1, sy - 1, 2, 2, c(col));
}

fn draw_cable_plate(fb: &mut Cv, sx: i32, sy: i32, dev: &str, port: &str, st: &State, net: Option<&Derived>, t: f64, hot: bool) {
    let ls = link_state(st, dev, port);
    let col = match ls {
        Some(LinkState::Ok) => {
            let up = find_link(st, dev, port).and_then(|id| net.and_then(|n| n.links.get(&id)).map(|l| l.up)).unwrap_or(true);
            if up { 0x3dff6a } else { 0xff5a4f }
        }
        Some(LinkState::Flapping) => if (t * 4.0) as i64 % 2 == 0 { 0xffb020 } else { 0x3a2a10 },
        Some(LinkState::Broken) => 0xff5a4f,
        Some(LinkState::Unplugged) => 0x555a60,
        None => return,
    };
    // Golvuttag med en kabel som ringlar sig
    fb.ellipse(sx as f32 + 4.0, sy as f32 - 1.0, 6.0, 2.5, ca(0x101028, 0.22));
    fb.rect(sx, sy - 4, 8, 5, c(0x2b2d31));
    fb.rect(sx + 1, sy - 3, 6, 3, c(0xc9ccd0));
    fb.rect(sx + 5, sy - 3, 2, 2, c(col));
    let cc = match find_link_color(st, dev, port) {
        0 => 0x2f6fd6,
        1 => 0xe8c23a,
        2 => 0xf08a2c,
        3 => 0x3daa5b,
        _ => 0xbbbbbb,
    };
    for k in 0..6 {
        fb.set(sx - 1 - k, sy - 1 + ((k as f32 * 0.9).sin().round() as i32), c(cc));
        fb.set(sx - 1 - k, sy + ((k as f32 * 0.9).sin().round() as i32), c(shade(cc, -0.3)));
    }
    if matches!(ls, Some(LinkState::Broken) | Some(LinkState::Flapping) | Some(LinkState::Unplugged)) {
        let bob = ((t * 5.0).sin() * 1.0).round() as i32;
        fb.rect(sx + 2, sy - 11 + bob, 4, 4, c(0x2b1a10));
        fb.rect(sx + 3, sy - 10 + bob, 2, 2, c(0xffb020));
    }
    if hot {
        let pulse = 0.5 + 0.35 * (t * 5.0).sin();
        ring(fb, sx + 4, sy - 1, 9.0, 3.5, ca(0xffd54a, pulse as f32));
    }
}

fn find_link_color(st: &State, dev: &str, port: &str) -> i32 {
    match st.links.iter().find(|l| (l.a.dev == dev && l.a.port == port) || (l.b.dev == dev && l.b.port == port)).map(|l| l.color.as_str()) {
        Some("blue") => 0,
        Some("yellow") => 1,
        Some("orange") => 2,
        Some("green") => 3,
        _ => 4,
    }
}

fn draw_doors(fb: &mut Cv, spr: &Cv, sx: i32, sy: i32, open: f64) {
    let hw = spr.w / 2;
    let off = (open * (hw - 3) as f64).round() as i32;
    let x0 = sx;
    let y0 = sy - spr.h;
    // Mörk öppning bakom
    fb.rect(x0 + 1, y0 + 3, spr.w - 2, spr.h - 4, ca(0x1e1814, 0.55));
    for y in 0..spr.h {
        for x in 0..spr.w {
            let p = spr.d[(y * spr.w + x) as usize];
            if p >> 24 == 0 {
                continue;
            }
            let dx = if x < hw { x0 + x - off } else { x0 + x + off };
            if dx >= x0 && dx < x0 + spr.w {
                fb.put(dx, y0 + y, p);
            }
        }
    }
}

fn ring(fb: &mut Cv, cx: i32, cy: i32, rx: f64, ry: f64, col: u32) {
    let n = 40;
    for i in 0..n {
        let a = i as f64 / n as f64 * std::f64::consts::TAU;
        fb.put(cx + (a.cos() * rx).round() as i32, cy + (a.sin() * ry).round() as i32, col);
    }
}

fn highlight(fb: &mut Cv, spr: &Cv, x: i32, y: i32, t: f64) {
    let a = 0.6 + 0.3 * (t * 6.0).sin() as f32;
    for sy in -1..=spr.h {
        for sx in -1..=spr.w {
            if spr.get(sx, sy) >> 24 > 40 {
                continue;
            }
            let near = spr.get(sx - 1, sy) >> 24 > 40 || spr.get(sx + 1, sy) >> 24 > 40 || spr.get(sx, sy - 1) >> 24 > 40 || spr.get(sx, sy + 1) >> 24 > 40;
            if near {
                fb.put(x + sx, y + sy, ca(0xffe36a, a));
            }
        }
    }
}

/// Mjuk skuggellips i ditherade steg (tät i mitten, gles mot kanten)
fn dither_ellipse(fb: &mut Cv, cx: i32, cy: i32, rx: f64, ry: f64, a: f32) {
    let rx = rx.round().max(1.0) as i32;
    let ry = ry.round().max(1.0) as i32;
    for y in -ry..=ry {
        for x in -rx..=rx {
            let dx = x as f64 / rx as f64;
            let dy = y as f64 / ry as f64;
            let d = dx * dx + dy * dy;
            if d > 1.0 {
                continue;
            }
            if d > 0.55 && (bayer(x + 64, y + 64) as f64 + 0.5) < (d - 0.55) / 0.45 {
                continue;
            }
            fb.put(cx + x, cy + y, ca(0x10143c, a));
        }
    }
}
