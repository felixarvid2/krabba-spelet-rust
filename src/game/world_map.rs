//! Kartan: layouten för kontoret i Göteborg, lagret i Borås och kontoret i Mölndal.
//! Bygger mark, väggar, möbler, träd och interaktionspunkter. Port av js/world/building.js (layout) och world2d.js/scene2d.js (sprites).
#![allow(dead_code)]
use super::art::*;
use std::rc::Rc;

pub const WALL_H: f64 = 1.45;
/// Synlig höjd på de södra ytterväggarna (lägre, så att racken och rummen innanför syns)
pub const FRONT_WALL_H: f64 = 0.72;

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x0: f64,
    pub x1: f64,
    pub z0: f64,
    pub z1: f64,
}
impl Rect {
    pub fn around(x: f64, z: f64, w: f64, d: f64) -> Rect {
        Rect { x0: x - w / 2.0, x1: x + w / 2.0, z0: z - d / 2.0, z1: z + d / 2.0 }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Dyn {
    None,
    Pc(String),
    Laptop,
    Monitor,
    Rack(usize),
    Doors,
    Ap(String),
    Cable(usize),
    Truck,
}

#[derive(Clone)]
pub enum Shadow {
    None,
    Sil(Rc<Cv>),
    Ell { rx: f64, rz: f64, dx: f64 },
}

pub struct Item {
    pub spr: Rc<Cv>,
    pub x: f64,
    pub z: f64,
    pub y: f64,
    pub key: f64,
    pub ox: i32,
    pub oy: i32,
    /// Väggar som tonas ut: (x0, x1, höjd i meter)
    pub fade: Option<(f64, f64, f64)>,
    pub wall: bool,
    pub shadow: Shadow,
    pub dynk: Dyn,
    pub min_week: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum IKind {
    Rack(String),
    Laptop,
    Pc(String),
    Phone,
    MiniSwitch,
    Monitor,
    Whiteboard,
    Printer,
    Water,
    Coffee,
    Travel(&'static str),
    Npc(String),
    /// Kabel mellan enhet och port ("nic" för datorer)
    Cable(String, String),
}

#[derive(Clone, Debug)]
pub struct Inter {
    pub x: f64,
    pub z: f64,
    pub r: f64,
    pub kind: IKind,
    pub label: String,
    pub item: Option<usize>,
    pub min_week: u32,
}

#[derive(Clone, Debug)]
pub struct Zone {
    pub name: &'static str,
    pub r: Rect,
}

#[derive(Clone, Debug)]
pub struct Sign {
    pub text: String,
    pub x: f64,
    pub z: f64,
    pub y: f64,
}

pub struct RackSpot {
    pub id: String,
    pub x: f64,
    pub z: f64,
    pub w: f64,
    pub d: f64,
    pub units: Vec<&'static str>,
}

/// En plats för kompassen och "gå dit": (id, x, z, höjd)
pub type Spot = (String, f64, f64, f64);

pub struct Site {
    pub key: &'static str,
    pub name: &'static str,
    pub x0: f64,
    pub x1: f64,
    pub z0: f64,
    pub z1: f64,
    pub ground: Cv,
    pub items: Vec<Item>,
    pub cols: Vec<Rect>,
    pub zones: Vec<Zone>,
    pub inters: Vec<Inter>,
    pub signs: Vec<Sign>,
    pub racks: Vec<RackSpot>,
    pub spots: Vec<Spot>,
    pub spawn: (f64, f64, u8),
    /// Byggnadens yttermått för kartan
    pub building: Rect,
    /// Lampor som blinkar: (x, z, y)
    pub cables: Vec<(f64, f64)>,
    /// Kablar att byta: (x, z, dev, port)
    pub cable_spots: Vec<(f64, f64, String, String)>,
}

pub fn site_bounds(key: &str) -> (f64, f64, f64, f64) {
    match key {
        "boras" => (82.0, 118.0, -17.0, 16.0),
        "molndal" => (148.0, 180.0, -16.0, 12.0),
        _ => (-21.0, 21.0, -18.0, 14.0),
    }
}

// ------------------------------------------------------------------ Byggaren

struct WallRec {
    a: f64,
    b: f64,
    fixed: f64,
    x_axis: bool,
    t: f64,
    glass: bool,
}

struct B {
    key: &'static str,
    x0: f64,
    x1: f64,
    z0: f64,
    z1: f64,
    floors: Vec<(f64, f64, f64, f64, u8)>,
    flats: Vec<(f64, f64, f64, f64, u32)>,
    items: Vec<Item>,
    cols: Vec<Rect>,
    inters: Vec<Inter>,
    signs: Vec<Sign>,
    zones: Vec<Zone>,
    racks: Vec<RackSpot>,
    spots: Vec<Spot>,
    walls: Vec<WallRec>,
    cables: Vec<(f64, f64)>,
    cable_spots: Vec<(f64, f64, String, String)>,
}

fn px_x(x: f64) -> i32 {
    (x * PPM as f64).round() as i32
}

impl B {
    fn new(key: &'static str) -> B {
        let (x0, x1, z0, z1) = site_bounds(key);
        B {
            key,
            x0,
            x1,
            z0,
            z1,
            floors: vec![],
            flats: vec![],
            items: vec![],
            cols: vec![],
            inters: vec![],
            signs: vec![],
            zones: vec![],
            racks: vec![],
            spots: vec![],
            walls: vec![],
            cables: vec![],
            cable_spots: vec![],
        }
    }
    fn col(&mut self, x0: f64, z0: f64, x1: f64, z1: f64) {
        self.cols.push(Rect { x0, x1, z0, z1 });
    }
    fn col_c(&mut self, x: f64, z: f64, w: f64, d: f64) {
        self.cols.push(Rect::around(x, z, w, d));
    }
    /// Lägger till ett föremål. Ankaret (x, z, y) är nedre vänstra hörnet av bilden.
    fn add(&mut self, spr: Cv, x: f64, z: f64, y: f64, key: f64) -> usize {
        let oy = spr.h;
        self.items.push(Item { spr: Rc::new(spr), x, z, y, key, ox: 0, oy, fade: None, wall: false, shadow: Shadow::None, dynk: Dyn::None, min_week: 0 });
        self.items.len() - 1
    }
    fn add_o(&mut self, spr: Cv, x: f64, z: f64, ox: i32, oy: i32) -> usize {
        let i = self.add(spr, x, z, 0.0, z);
        self.items[i].ox = ox;
        self.items[i].oy = oy;
        i
    }
    fn floor(&mut self, x1: f64, z1: f64, x2: f64, z2: f64, m: u8) {
        self.floors.push((x1, z1, x2, z2, m));
    }
    fn inter(&mut self, x: f64, z: f64, r: f64, kind: IKind, label: &str, item: Option<usize>) {
        self.inters.push(Inter { x, z, r, kind, label: label.to_string(), item, min_week: 0 });
    }

    /// Låda med generisk färg (möbler, skåp). x,z = mitt, yc = höjd på mitten.
    fn bx(&mut self, x: f64, z: f64, w: f64, d: f64, yc: f64, h: f64, color: u32, collide: bool) -> usize {
        let base = yc - h / 2.0;
        let spr = box_sprite(w as f32, d as f32, h as f32, shade(color, 0.12), color, &BoxOpts { drawers: h > 0.6 && h < 1.3 && collide && w > 0.5, ..Default::default() });
        let i = self.add(spr, x - w / 2.0, z - d / 2.0, base, z - d / 2.0);
        if collide {
            self.col_c(x, z, w, d);
        }
        i
    }

    /// Vägg längs x (z konstant) eller längs z (x konstant), med dörröppningar [från, till]
    fn wall(&mut self, a: f64, b: f64, fixed: f64, x_axis: bool, h: f64, wcol: u32, doors: &[(f64, f64)], t: f64, kind: u8) {
        let t = t.max(0.16);
        self.walls.push(WallRec { a, b, fixed, x_axis, t, glass: false });
        let hh = h.min(WALL_H);
        let mut segs: Vec<(f64, f64)> = vec![];
        let mut cur = a;
        let mut ds = doors.to_vec();
        ds.sort_by(|p, q| p.0.partial_cmp(&q.0).unwrap());
        for d in &ds {
            segs.push((cur, d.0));
            cur = d.1;
        }
        segs.push((cur, b));
        for (s0, s1) in segs {
            let len = s1 - s0;
            if len <= 0.02 {
                continue;
            }
            if x_axis {
                let spr = box_sprite(len as f32, t as f32, hh as f32, 0x4a3526, wcol, &BoxOpts { f: Some(&|g, wd, dp, hp| paint_wall(g, wd, dp, hp, wcol, kind, s0)), ..Default::default() });
                let k = fixed - t / 2.0;
                let i = self.add(spr, s0 - 0.01, k, 0.0, k);
                self.items[i].fade = Some((s0, s1, hh));
                self.items[i].wall = true;
                self.col(s0, fixed - t / 2.0, s1, fixed + t / 2.0);
            } else {
                let spr = box_sprite(t as f32, len as f32, hh as f32, 0x4a3526, wcol, &BoxOpts { f: Some(&|g, wd, dp, _hp| paint_wall_top(g, wd, dp, false)), ..Default::default() });
                let i = self.add(spr, fixed - t / 2.0, s0, 0.0, s0);
                self.items[i].wall = true;
                self.col(fixed - t / 2.0, s0, fixed + t / 2.0, s1);
            }
        }
    }
    /// Glasvägg (tonas inte ut, men är genomskinlig)
    fn glass(&mut self, a: f64, b: f64, fixed: f64, x_axis: bool, doors: &[(f64, f64)]) {
        self.walls.push(WallRec { a, b, fixed, x_axis, t: 0.16, glass: true });
        let mut segs = vec![];
        let mut cur = a;
        for d in doors {
            segs.push((cur, d.0));
            cur = d.1;
        }
        segs.push((cur, b));
        for (s0, s1) in segs {
            let len = s1 - s0;
            if len <= 0.02 {
                continue;
            }
            let t = 0.16;
            let mut spr = if x_axis {
                box_sprite(len as f32, t as f32, WALL_H as f32, 0x9fd0e3, 0xb9e1ef, &BoxOpts { no_outline: true, f: Some(&|g, wd, dp, hp| paint_wall(g, wd, dp, hp, 0xb9e1ef, 9, 0.0)), ..Default::default() })
            } else {
                box_sprite(t as f32, len as f32, WALL_H as f32, 0x9fd0e3, 0xb9e1ef, &BoxOpts { no_outline: true, f: Some(&|g, wd, dp, _hp| paint_wall_top(g, wd, dp, true)), ..Default::default() })
            };
            for p in spr.d.iter_mut() {
                if *p >> 24 != 0 {
                    *p = (*p & 0xFFFFFF) | (158 << 24);
                }
            }
            let (ax, az) = if x_axis { (s0, fixed - t / 2.0) } else { (fixed - t / 2.0, s0) };
            let i = self.add(spr, ax, az, 0.0, if x_axis { az } else { s0 });
            self.items[i].wall = true;
            if x_axis {
                self.col(s0, fixed - 0.05, s1, fixed + 0.05);
            } else {
                self.col(fixed - 0.05, s0, fixed + 0.05, s1);
            }
        }
    }
    fn window(&mut self, x: f64, z: f64, w: f64) {
        let spr = window_sprite(w, x);
        let wall_z = z - 0.08;
        let i = self.add(spr, x - w * 0.4, wall_z - 0.1, 0.3, wall_z - 0.09);
        let _ = i;
    }
    fn desk(&mut self, x: f64, z: f64, rot: bool, color: Option<u32>, pc: Option<&str>, laptop: bool) {
        let (w, d) = if rot { (0.8, 1.6) } else { (1.6, 0.8) };
        let spr = desk_sprite(w, d, color, x, z);
        let i = self.add(spr, x - w / 2.0, z - d / 2.0, 0.0, z - d / 2.0);
        let _ = i;
        self.col_c(x, z, w, d);
        if let Some(id) = pc {
            let j = self.add(Cv::new(1, 1), x - 0.2, z + 0.05, 0.74, z - d / 2.0 + 0.001);
            self.items[j].dynk = Dyn::Pc(id.to_string());
        }
        if laptop {
            let j = self.add(Cv::new(1, 1), x - 0.2, z, 0.74, z - d / 2.0 + 0.001);
            self.items[j].dynk = Dyn::Laptop;
        }
    }
    fn chair(&mut self, x: f64, z: f64, color: u32) {
        let spr = chair(color);
        self.add(spr, x - 0.3, z - 0.2, 0.0, z - 0.21);
    }
    fn plant(&mut self, x: f64, z: f64, s: f64) {
        let spr = plant(s as f32, (x * 13.0 + z * 7.0).round() as i32 + 5);
        let w = spr.w as f64 / PPM as f64;
        self.add(spr, x - w / 2.0, z - 0.15, 0.0, z - 0.15);
        self.col_c(x, z, 0.36 * s, 0.36 * s);
    }
    fn sofa(&mut self, x: f64, z: f64, rot: bool, col: u32) {
        let (w, d) = if rot { (0.85, 2.0) } else { (2.0, 0.85) };
        let spr = box_sprite(w as f32, d as f32, 0.55, shade(col, 0.1), col, &BoxOpts {
            f: Some(&|g, wd, dp, _hp| {
                g.rect(0, 0, 1, 1, 0);
                if !rot {
                    g.rect(0, 0, wd, 5, c(shade(col, -0.2)));
                } else {
                    g.rect(wd - 5, 0, 5, dp, c(shade(col, -0.2)));
                }
                for k in 1..3 {
                    if !rot {
                        g.rect(wd * k / 3, 6, 1, dp - 6, c(shade(col, 0.25)));
                    } else {
                        g.rect(0, dp * k / 3, wd - 6, 1, c(shade(col, 0.25)));
                    }
                }
            }),
            ..Default::default()
        });
        self.add(spr, x - w / 2.0, z - d / 2.0, 0.0, z - d / 2.0);
        self.col_c(x, z, w, d);
    }
    fn tree(&mut self, x: f64, z: f64, seed: i32, kind: TreeKind) {
        let spr = tree(kind, seed, 0.0);
        let pine = kind == TreeKind::Pine;
        let i = self.add_o(spr, x, z, if pine { 25 } else { 37 }, if pine { 84 } else { 90 });
        self.items[i].shadow = Shadow::Ell { rx: if pine { 0.9 } else { 1.35 }, rz: if pine { 0.45 } else { 0.6 }, dx: 0.35 };
        self.col(x - 0.25, z - 0.1, x + 0.25, z + 0.3);
    }
    fn bush(&mut self, x: f64, z: f64, seed: i32) {
        let fl = if seed % 2 == 1 { Some([0xff9ccf, 0xffd1e8]) } else if seed % 3 == 0 { Some([0xffffff, 0xfff1a8]) } else { None };
        let spr = bush(seed, fl);
        let i = self.add_o(spr, x, z, 17, 22);
        self.items[i].shadow = Shadow::Ell { rx: 0.7, rz: 0.3, dx: 0.2 };
    }
    fn car(&mut self, x: f64, z: f64, col: u32, text: bool) -> usize {
        let spr = car(col, text);
        let i = self.add_o(spr, x, z, 25, 42);
        self.items[i].shadow = Shadow::Ell { rx: 1.1, rz: 0.55, dx: 0.3 };
        self.col(x - 0.95, z - 0.1, x + 0.95, z + 1.5);
        i
    }
    fn lamp(&mut self, x: f64, z: f64) {
        let i = self.add_o(lamp_post(), x, z, 6, 57);
        self.items[i].shadow = Shadow::Ell { rx: 0.3, rz: 0.15, dx: 0.2 };
        self.col(x - 0.12, z - 0.1, x + 0.12, z + 0.1);
    }
    fn bench(&mut self, x: f64, z: f64) {
        let i = self.add_o(bench(), x, z, 17, 17);
        self.items[i].shadow = Shadow::Ell { rx: 0.75, rz: 0.25, dx: 0.2 };
        self.col(x - 0.7, z - 0.1, x + 0.7, z + 0.5);
    }
    fn spot(&mut self, id: &str, x: f64, z: f64, y: f64) {
        self.spots.push((id.to_string(), x, z, y));
    }
    fn sign(&mut self, text: &str, x: f64, z: f64, y: f64) {
        self.signs.push(Sign { text: text.to_string(), x, z, y });
    }
    fn zone(&mut self, name: &'static str, x1: f64, z1: f64, x2: f64, z2: f64) {
        self.zones.push(Zone { name, r: Rect { x0: x1, x1: x2, z0: z1, z1: z2 } });
    }
    fn rug(&mut self, x: f64, z: f64, w: f64, d: f64, col: u32) {
        self.flats.push((x - w / 2.0, z - d / 2.0, x + w / 2.0, z + d / 2.0, col));
    }
    fn rack(&mut self, id: &str, x: f64, z: f64, units: Vec<&'static str>) {
        let (w, d) = (0.6, 1.0);
        let idx = self.racks.len();
        let spr = rack_sprite(w as f32, d as f32);
        let it = self.add(spr, x - w / 2.0, z - d / 2.0, 0.0, z - d / 2.0);
        self.items[it].dynk = Dyn::Rack(idx);
        self.col_c(x, z, w, d);
        self.racks.push(RackSpot { id: id.to_string(), x, z, w, d, units });
        self.inter(x, z - 0.5, 1.1, IKind::Rack(id.to_string()), &format!("Titta på rack {}", if id == "Boras" { "i Borås".to_string() } else { id.to_string() }), Some(it));
    }
    fn cable_plate(&mut self, x: f64, z: f64, dev: &str, port: &str, label: &str) {
        let idx = self.cable_spots.len();
        let j = self.add(Cv::new(1, 1), x, z, 0.0, z - 0.3);
        self.items[j].dynk = Dyn::Cable(idx);
        self.cable_spots.push((x, z, dev.to_string(), port.to_string()));
        self.inter(x, z, 0.7, IKind::Cable(dev.to_string(), port.to_string()), label, None);
    }
}

// ------------------------------------------------------------------ Målning av väggar, fönster, möbler

const BRICK: [u32; 6] = [0x9c4a32, 0xb0573a, 0xa24f36, 0xc0674a, 0x8e422c, 0xb75e40];

/// kind: 0 innervägg, 1 tegelfasad, 2 plåtfasad, 9 glas
fn paint_wall(g: &mut Cv, wd: i32, dp: i32, hp: i32, col: u32, kind: u8, seg0: f64) {
    let y0 = dp;
    let seg_i = (seg0 * 7.0) as i32;
    if kind == 9 {
        for y in 0..hp {
            let t = y as f32 / hp as f32;
            let m = if t < 0.5 { mixc(0xcfeefa, 0x9fd0e3, t * 2.0) } else { mixc(0x9fd0e3, 0x7fb4cc, (t - 0.5) * 2.0) };
            g.rect(0, y0 + y, wd, 1, c(m));
        }
        let mut k = 6;
        while k < wd {
            for y in 0..(hp - 4).max(0) {
                g.rect(k + y / 3, y0 + 2 + y, 2, 1, ca(0xffffff, 0.55));
            }
            k += 29;
        }
        g.rect(0, y0, wd, 2, c(0x3a4048));
        g.rect(0, y0 + hp - 2, wd, 2, c(0x3a4048));
        let mut x = 0;
        while x < wd {
            g.rect(x, y0, 1, hp, c(0x3a4048));
            x += 29;
        }
        g.rect(0, y0, wd, 1, c(0x6b7480));
        g.rect(0, 0, wd, dp, c(0x9fd0e3));
        return;
    }
    // Väggkrön
    g.rect(0, 0, wd, dp, c(0x3a2a1f));
    g.rect(0, 0, wd, 1, c(0x6b5040));
    if kind == 2 {
        let rib = [0x7d848c, 0x9aa1a8, 0xb4bac0, 0x8a9097];
        for x in 0..wd {
            let base = rib[((x + seg_i).rem_euclid(4)) as usize];
            g.rect(x, y0, 1, hp, c(base));
            if hash2(x + seg_i * 31, 1, 5) < 0.05 {
                g.rect(x, y0 + 4, 1, 6 + (hash2(x, 2, 5) * 10.0) as i32, ca(0x8c461e, 0.35));
            }
        }
        g.rect(0, y0, wd, 2, c(0xd7dbe0));
        g.rect(0, y0 + 2, wd, 1, c(0x5c6168));
        g.rect(0, y0 + hp - 5, wd, 5, c(0x8b877f));
        g.rect(0, y0 + hp - 5, wd, 1, c(0xa5a197));
    } else if kind == 1 {
        let off0 = (seg0 * PPM as f64) as i32;
        for y in 0..(hp - 5).max(0) {
            let row = y / 4;
            let in_row = y % 4;
            let off = (row % 2) * 4;
            for x in 0..wd {
                let bx = (x + off + off0).div_euclid(8);
                let in_b = (x + off + off0).rem_euclid(8);
                let cc = if in_row == 3 || in_b == 7 {
                    0xd4c4aa
                } else {
                    let mut cc = BRICK[(hash2(bx, row, 9) * BRICK.len() as f32) as usize % BRICK.len()];
                    if in_row == 0 {
                        cc = tone(cc, 0.12);
                    } else if in_row == 2 && in_b > 4 {
                        cc = tone(cc, -0.12);
                    }
                    cc
                };
                g.set(x, y0 + y, c(cc));
            }
        }
        g.rect(0, y0, wd, 2, c(0xe8e2d4));
        g.rect(0, y0 + 2, wd, 1, c(0xb6ad9c));
        g.rect(0, y0 + hp - 5, wd, 5, c(0x7a7670));
        g.rect(0, y0 + hp - 5, wd, 1, c(0x9c978f));
    } else {
        let face = rgb_of(col);
        for y in 0..hp {
            for x in 0..wd {
                let v = (if x % 6 == 0 { -0.05 } else { 0.0 }) + (hash2(x, y, 4) - 0.5) * 0.05 - (y as f32 / hp as f32) * 0.12 + bayer(x, y) * 0.04;
                let target = if v < 0.0 { [20., 24., 60.] } else { [255., 250., 225.] };
                let m = mix(face, target, v.abs());
                g.set(x, y0 + y, c(pack(m[0], m[1], m[2])));
            }
        }
        let rail = y0 + (hp as f32 * 0.55).round() as i32;
        if hp > 12 {
            g.rect(0, rail, wd, 1, c(tone(col, -0.2)));
            g.rect(0, rail - 1, wd, 1, c(tone(col, 0.25)));
        }
        g.rect(0, y0 + hp - 3, wd, 3, c(0x5a4232));
        g.rect(0, y0 + hp - 3, wd, 1, c(0x7d5c45));
        g.rect(0, y0, wd, 1, c(tone(col, 0.3)));
        // Tavlor
        if wd > 60 && hp > 20 {
            let mut x = 18;
            while x < wd - 24 {
                if hash2(x, (seg0 * 10.0) as i32, 13) >= 0.45 {
                    let (px, py, pw, ph) = (x, y0 + 3, 14, 9);
                    g.rect(px - 1, py - 1, pw + 2, ph + 2, c(0x4a3020));
                    let kind3 = (hash2(x, 3, 17) * 3.0) as usize % 3;
                    g.rect(px, py, pw, ph, c([0x8fd0f5, 0xf7b37a, 0xb8d8f0][kind3]));
                    g.rect(px, py + 5, pw, 4, c([0x4a8a3a, 0xc05a3a, 0x3a6a8a][kind3]));
                    g.rect(px + 9, py + 1, 2, 2, c(if kind3 == 1 { 0xffe28a } else { 0xffffff }));
                    g.rect(px, py + ph + 1, pw + 1, 1, ca(0, 0.25));
                }
                x += 70;
            }
        }
    }
}

fn paint_wall_top(g: &mut Cv, wd: i32, dp: i32, glass: bool) {
    if glass {
        g.rect(0, 0, wd, dp, c(0x9fd0e3));
        g.rect(0, 0, 1, dp, c(0xd8f1fb));
        return;
    }
    g.rect(0, 0, wd, dp, c(0x4a3526));
    g.rect(0, 0, 1, dp, c(0x6b5040));
    g.rect(wd - 1, 0, 1, dp, c(0x2e2018));
}

fn window_sprite(w: f64, xw: f64) -> Cv {
    let ww = (w * PPM as f64 * 0.8).round() as i32;
    let wh = (WALL_H as f32 * PPM * HS * 0.68).round() as i32;
    let mut g = Cv::new(ww + 2, wh + 6);
    g.rect(0, 0, ww + 2, wh + 2, c(0xf2efe6));
    g.rect(0, 0, ww + 2, 1, c(0xffffff));
    g.rect(ww + 1, 0, 1, wh + 2, c(0xc9c4b8));
    for y in 0..(wh - 2) {
        for x in 0..(ww - 2) {
            let k = y as f32 / (wh - 2) as f32;
            let tl = 0.55 + ((x as f32 * 0.35 + xw as f32).sin()) * 0.06 + (vnoise(x as f32 / 4.0, 3.0, xw as i32) - 0.5) * 0.18;
            let col = if k < tl {
                let m = mix([143., 208., 245.], [224., 243., 252.], k / tl);
                pack(m[0], m[1], m[2])
            } else if k < tl + 0.14 {
                pick(&pal::LEAF_GREEN, 0.25 + vnoise(x as f32 / 2.0, y as f32 / 2.0, 5) * 0.5, x, y, 1.0)
            } else {
                pick(&pal::GRASS, 0.55 + hash2(x, y, 3) * 0.2, x, y, 1.0)
            };
            g.set(2 + x, 2 + y, c(col));
        }
    }
    for y in 0..(wh - 4).max(0) {
        g.rect(4 + (y as f32 * 0.6) as i32, 3 + y, 2, 1, ca(0xffffff, 0.55));
    }
    g.rect((ww as f32 / 2.0).round() as i32, 0, 2, wh + 2, c(0xf2efe6));
    g.rect((ww as f32 / 2.0).round() as i32 + 1, 2, 1, wh, c(0xc9c4b8));
    // Gardiner
    let cur = [0xd9c9a8, 0xb8cfd8, 0xe0b8b0][(xw.abs().round() as usize) % 3];
    for y in 0..wh {
        let cw = 4 + if y as f32 > wh as f32 * 0.6 { 1 } else { 0 };
        for x in 0..cw {
            let cc = if x % 2 == 1 { tone(cur, -0.15) } else { cur };
            g.set(x, y, c(cc));
            g.set(ww + 1 - x, y, c(cc));
        }
    }
    g.rect(0, 0, ww + 2, 1, c(0x8a8378));
    g.rect(0, wh + 1, ww + 2, 1, c(0xffffff));
    g.rect(0, wh + 2, ww + 2, 1, ca(0x000028, 0.25));
    g.selout(0.4)
}

fn desk_sprite(w: f64, d: f64, color: Option<u32>, x: f64, z: f64) -> Cv {
    let rot = w < d;
    let top = color.unwrap_or(0xdcc39a);
    let mut rr = Rng::new((x * 31.0 + z * 17.0).round().abs() as u32 + 3);
    let mut v = vec![];
    for _ in 0..24 {
        v.push(rr.next());
    }
    let mut vi = 0usize;
    let mut nx = move || {
        let r = v[vi % v.len()];
        vi += 1;
        r
    };
    // Förbered slumpvärden
    let mut ve: Vec<f32> = vec![];
    for _ in 0..16 {
        ve.push(nx());
    }
    box_sprite(w as f32, d as f32, 0.74, top, tone(top, -0.3), &BoxOpts {
        f: Some(&|g, wd, dp, hp| {
            for y in (1..(dp - 1)).step_by(2) {
                let r1 = ve[(y as usize) % ve.len()];
                g.rect(1 + (r1 * 6.0) as i32, y, wd - 8, 1, ca(0x785028, 0.06 + r1 * 0.06));
            }
            g.rect(0, 0, wd, 1, ca(0xffffff, 0.25));
            g.rect(0, dp - 1, wd, 1, c(tone(top, -0.15)));
            g.clear_rect(3, dp + 3, wd - 6, hp - 3);
            g.rect(3, dp + 3, wd - 6, hp - 3, ca(0x10143c, 0.2));
            g.rect(1, dp, 2, hp, c(0x5a5f68));
            g.rect(wd - 3, dp, 2, hp, c(0x5a5f68));
            g.rect(1, dp, 1, hp, c(0x8a9099));
            g.rect(wd - 3, dp, 1, hp, c(0x8a9099));
            if !rot && wd > 30 {
                let kx = wd / 2 - 6;
                g.rect(kx, dp - 6, 12, 3, c(0x2b2d31));
                for k in 0..5 {
                    g.rect(kx + 1 + k * 2, dp - 5, 1, 1, c(0x4a4d55));
                }
                g.rect(kx + 14, dp - 5, 2, 2, c(0x3a3d42));
                if ve[1] < 0.7 {
                    let mx = 3 + (ve[2] * 6.0) as i32;
                    g.rect(mx, dp - 7, 3, 3, c([0xffffff, 0xd64b3a, 0x2f6fb0, 0xf0b429][(ve[3] * 4.0) as usize % 4]));
                    g.rect(mx, dp - 7, 3, 1, c(0x5a3a23));
                }
                if ve[4] < 0.6 {
                    let px = wd - 12 - (ve[5] * 4.0) as i32;
                    g.rect(px, dp - 9, 7, 5, c(0xf4f2ea));
                    g.rect(px + 1, dp - 8, 5, 1, c(0xc9c4b8));
                    g.rect(px + 1, dp - 6, 4, 1, c(0xc9c4b8));
                }
                if ve[6] < 0.35 {
                    g.rect(wd - 6, dp - 12, 3, 3, c(0xffe14a));
                }
                if ve[7] < 0.3 {
                    g.rect(4, dp - 12, 3, 3, c(0xc9744a));
                    g.rect(3, dp - 15, 5, 3, c(pal::LEAF_GREEN[4]));
                }
            }
        }),
        ..Default::default()
    })
}

fn rack_sprite(w: f32, d: f32) -> Cv {
    box_sprite(w, d, 2.05, 0x26292e, 0x15171a, &BoxOpts {
        f: Some(&|g, wd, dp, hp| {
            let mut x = 2;
            while x < wd - 2 {
                g.rect(x, 2, 1, dp - 4, c(0x1d1f23));
                x += 2;
            }
            g.rect(1, dp + 1, wd - 2, hp - 2, c(0x23262b));
            let mut y = dp + 4;
            while y < dp + hp - 4 {
                let mut x2 = 3;
                while x2 < wd - 3 {
                    g.rect(x2, y, 1, 1, c(if (x2 + y) % 4 == 0 { 0x3a3f47 } else { 0x2c3036 }));
                    x2 += 2;
                }
                y += 2;
            }
            g.rect(wd - 4, dp + (hp as f32 * 0.35) as i32, 1, 8, c(0x9aa0a8));
            g.rect(wd - 4, dp + (hp as f32 * 0.35) as i32, 1, 2, c(0xd0d4da));
            g.rect(3, dp + 2, 4, 1, c(0xc8412f));
            g.rect(0, dp + hp - 3, wd, 3, c(0x0e0f12));
            g.rect(1, dp + 1, 1, hp - 4, ca(0xffffff, 0.12));
        }),
        ..Default::default()
    })
}

fn pallet_rack(bays: i32, bay: f64, h: f64, seed: i32) -> Cv {
    let mut rnd = Rng::new((seed * 17 + 5) as u32);
    let wp = (bays as f64 * bay * PPM as f64).round() as i32;
    let dp = (1.1 * PPM * ZS).round() as i32;
    let hp = (h as f32 * PPM * HS).round() as i32;
    let mut g = Cv::new(wp + 2, dp + hp + 2);
    g.tx = 1;
    g.ty = 1;
    let box_cols = [0xb98a55, 0xa57a48, 0xc89d66, 0x94693f];
    for b in 0..bays {
        let bx = (b as f64 * bay * PPM as f64).round() as i32;
        for ly in [0.1f32, 1.45, 2.85] {
            let y = dp + hp - (ly * PPM * HS).round() as i32;
            for k in 0..2 {
                if rnd.next() < 0.15 {
                    continue;
                }
                let bw = (1.1 * PPM).round() as i32;
                let bh = ((0.55 + rnd.next() * 0.35) * PPM * HS).round() as i32;
                let x0 = bx + 6 + k * (1.3 * PPM).round() as i32;
                let col = box_cols[rnd.idx(4)];
                g.rect(x0, y - bh - 8, bw, 8, c(shade(col, 0.15)));
                g.rect(x0, y - bh, bw, bh, c(col));
                g.rect(x0, y - 2, bw, 2, c(shade(col, -0.2)));
                g.rect(x0 + bw / 2 - 1, y - bh - 8, 3, bh + 8, c(0xe7d9b8));
                g.rect(x0, y - bh - 8, bw, 1, c(0x2b1a10));
                g.rect(x0, y - bh - 8, 1, bh + 8, c(0x2b1a10));
                g.rect(x0 + bw - 1, y - bh - 8, 1, bh + 8, c(0x2b1a10));
            }
            g.rect(bx, y - 1, (bay * PPM as f64).round() as i32, 3, c(0x2b5aa0));
            g.rect(bx, y - 1, (bay * PPM as f64).round() as i32, 1, c(0x4d7fc7));
        }
    }
    for u in 0..=bays {
        let ux = ((u as f64 * bay * PPM as f64).round() as i32).min(wp - 3);
        g.rect(ux, 0, 3, dp + hp, c(0xd96b1f));
        g.rect(ux, 0, 1, dp + hp, c(0xf08a3c));
        let mut hy = 4;
        while hy < dp + hp {
            g.rect(ux + 1, hy, 1, 1, c(0x8a3f10));
            hy += 6;
        }
    }
    g.selout(0.18)
}

fn misc_box(w: f32, d: f32, h: f32, top: u32, front: u32, stripes: bool) -> Cv {
    box_sprite(w, d, h, top, front, &BoxOpts { stripes, ..Default::default() })
}

fn hedge(w: i32, seed: i32) -> Cv {
    let mut g = Cv::new(w, 22);
    let mut bl = vec![];
    let mut x = 4;
    while x < w - 3 {
        bl.push([x as f32, 12.0, 7.0]);
        x += 6;
    }
    canopy(&mut g, &bl, &pal::LEAF_GREEN, seed);
    g.selout(0.3)
}

fn flower_bed(w: i32, h: i32, seed: i32) -> Cv {
    let mut r = Rng::new(seed as u32);
    let mut g = Cv::new(w, h + 6);
    g.rect(0, 4, w, h + 2, c(0x8b6f58));
    g.rect(0, 4, w, 1, c(0xa58a70));
    for y in 0..(h - 1) {
        for x in 1..(w - 1) {
            g.set(x, 5 + y, c(pick(&pal::SOIL, fbm(x as f32 / 5.0, y as f32 / 4.0, seed) * 0.6 + hash2(x, y, seed) * 0.4, x, y, 1.0)));
        }
    }
    let fl = [[0xff5a78, 0xffd4dc], [0xffd23f, 0xfff4b0], [0x8f7bff, 0xe4ddff], [0xff8a3c, 0xffe0b8], [0xffffff, 0xffe06a]];
    for row in 0..(h / 5) {
        let f = fl[r.idx(5)];
        let mut fx = 3;
        while fx < w - 3 {
            let yy = 6 + row * 5 + if r.next() < 0.5 { 0 } else { 1 };
            g.rect(fx, yy + 1, 1, 3, c(pal::LEAF_GREEN[2]));
            g.rect(fx - 1, yy + 2, 1, 1, c(pal::LEAF_GREEN[2]));
            g.rect(fx + 1, yy + 3, 1, 1, c(pal::LEAF_GREEN[2]));
            g.rect(fx - 1, yy - 1, 3, 2, c(f[0]));
            g.rect(fx, yy - 1, 1, 1, c(f[1]));
            fx += 4;
        }
    }
    g.selout(0.35)
}

fn fence(len: i32) -> Cv {
    let mut g = Cv::new(len, 16);
    let mut x = 0;
    while x < len {
        g.rect(x, 2, 3, 14, c(pal::WOOD[1]));
        g.rect(x, 2, 1, 14, c(pal::WOOD[3]));
        g.rect(x, 1, 3, 1, c(pal::WOOD[4]));
        x += 12;
    }
    g.rect(0, 5, len, 2, c(pal::WOOD[2]));
    g.rect(0, 10, len, 2, c(pal::WOOD[2]));
    g.rect(0, 5, len, 1, c(pal::WOOD[4]));
    g.rect(0, 10, len, 1, c(pal::WOOD[4]));
    g.selout(0.4)
}

fn rock(seed: i32, w: i32, h: i32) -> Cv {
    let mut g = Cv::new(w + 2, h + 2);
    for y in 0..h {
        for x in 0..w {
            let dx = (x as f32 - w as f32 / 2.0) / (w as f32 / 2.0);
            let dy = (y as f32 - h as f32 * 0.6) / (h as f32 * 0.6);
            if dx * dx + dy * dy * 0.9 > 1.0 - vnoise(x as f32 / 2.0, y as f32 / 2.0, seed) * 0.25 {
                continue;
            }
            let v = 0.55 - dx * 0.25 - dy * 0.3 + vnoise(x as f32 / 2.5, y as f32 / 2.5, seed + 3) * 0.3;
            g.set(x + 1, y + 1, c(pick(&pal::ROCK, v, x, y, 0.9)));
        }
    }
    g.selout(0.32)
}

fn whiteboard_sprite() -> Cv {
    let mut g = Cv::new(40, 28);
    g.rect(0, 0, 40, 26, c(0xb8bec6));
    g.rect(0, 0, 40, 1, c(0xdfe3e8));
    g.rect(2, 2, 36, 22, c(0xf7f8f6));
    g.rect(3, 3, 12, 1, c(0xffffff));
    g.rect(20, 16, 12, 3, ca(0x788296, 0.18));
    g.rect(5, 5, 6, 4, c(0x1f4e79));
    g.rect(27, 5, 6, 4, c(0x1f4e79));
    g.rect(16, 15, 6, 4, c(0x1f4e79));
    g.rect(11, 7, 16, 1, c(0xc0392b));
    g.rect(8, 9, 1, 8, c(0xc0392b));
    g.rect(8, 17, 8, 1, c(0xc0392b));
    g.rect(30, 9, 1, 8, c(0xc0392b));
    g.rect(22, 17, 8, 1, c(0xc0392b));
    g.rect(4, 21, 14, 1, c(0x333333));
    g.rect(24, 21, 10, 1, c(0x333333));
    g.rect(2, 25, 36, 2, c(0x8a9099));
    g.rect(6, 24, 4, 1, c(0x2f6fb0));
    g.rect(12, 24, 4, 1, c(0xc0392b));
    g.rect(30, 24, 5, 1, c(0x2b2d31));
    g.selout(0.3)
}

fn round_table() -> Cv {
    let mut g = Cv::new(30, 34);
    g.ellipse(15.0, 26.0, 10.0, 3.5, ca(0x10143c, 0.2));
    g.rect(14, 14, 3, 12, c(0x2c2f34));
    g.rect(13, 25, 5, 1, c(0x2c2f34));
    g.ellipse(15.0, 12.0, 14.0, 6.0, c(0xd8d4ca));
    g.ellipse(15.0, 11.0, 14.0, 6.0, c(0xf5f3ee));
    g.ellipse(13.0, 10.0, 8.0, 3.0, c(0xffffff));
    g.selout(0.3)
}

fn printer_sprite() -> Cv {
    box_sprite(0.8, 0.6, 0.95, 0xe9ebec, 0xcfd3d6, &BoxOpts {
        f: Some(&|g, wd, dp, hp| {
            g.rect(3, dp + 3, wd - 6, 3, c(0x2a2d33));
            g.rect(wd - 9, dp + 8, 5, 2, c(0x3dff6a));
            g.rect(5, 2, wd - 10, dp - 5, c(0x9fd3f0));
            g.rect(3, dp + hp - 7, wd - 6, 2, c(0x6a6f78));
            g.rect(6, dp + hp - 5, wd - 12, 1, c(0xf4f4f0));
        }),
        ..Default::default()
    })
}

fn coffee_sprite() -> Cv {
    box_sprite(0.35, 0.35, 0.42, 0x3a3d44, 0x1f2124, &BoxOpts {
        f: Some(&|g, wd, dp, hp| {
            g.rect(2, dp + 2, wd - 4, 2, c(0x3dff6a));
            g.rect(wd / 2 - 1, dp + hp - 5, 3, 4, c(0xf4f4f0));
        }),
        ..Default::default()
    })
}

fn monitor_wall() -> Cv {
    let mut g = Cv::new(38, 22);
    g.rect(0, 0, 38, 22, c(0x111214));
    g.rect(2, 2, 34, 18, c(0x1b2430));
    g.selout(0.3)
}

fn cooler_sprite() -> Cv {
    box_sprite(0.34, 0.34, 1.0, 0xf2f4f6, 0xe9ecef, &BoxOpts {
        f: Some(&|g, wd, dp, hp| {
            g.rect(wd / 2 - 3, 0, 6, dp, c(0x9fd3f0));
            g.rect(wd / 2 - 1, dp + 8, 3, 2, c(0x2f6fd6));
            g.rect(wd / 2 - 3, dp + 8, 2, 2, c(0xd64b3a));
        }),
        ..Default::default()
    })
}

fn reception_desk() -> Cv {
    box_sprite(2.4, 0.7, 1.05, 0x6d4a2f, 0xf4f2ed, &BoxOpts {
        f: Some(&|g, wd, dp, hp| {
            g.rect(0, 0, wd, dp, c(0x6d4a2f));
            g.rect(0, 0, wd, 1, c(0x8a6444));
            g.rect(2, 2, 8, dp - 4, c(0x2a2d33));
            g.rect(12, 3, 4, 3, c(0xf4f2ea));
            g.rect(0, dp, wd, 2, c(0x3c7478));
            g.rect(3, dp + 4, wd - 6, hp - 8, c(0x3c7478));
            g.rect(3, dp + 4, wd - 6, 1, c(0x56999d));
            g.rect(wd / 2 - 9, dp + 7, 18, 5, c(0xf4f2ed));
            g.rect(wd / 2 - 7, dp + 9, 14, 1, c(0x3c7478));
        }),
        ..Default::default()
    })
}

fn bookshelf_sprite() -> Cv {
    let binders = [0x2f6fd6, 0xc0392b, 0x3fbf6f, 0xf0b429, 0x8e5ea2, 0x444a52];
    box_sprite(0.4, 1.4, 1.9, 0x8a6a48, 0x8a6a48, &BoxOpts {
        f: Some(&|g, wd, dp, hp| {
            g.rect(0, dp, wd, hp, c(0x6a4e32));
            for row in 0..4 {
                let y = dp + 3 + row * (hp / 4);
                g.rect(0, y + hp / 4 - 3, wd, 1, c(0x8a6a48));
                for i in 0..9 {
                    if (i + row) % 7 == 3 {
                        continue;
                    }
                    g.rect(1 + i * 1, y + 2, 1, hp / 4 - 6, c(binders[((i + row * 2) as usize) % 6]));
                }
            }
        }),
        ..Default::default()
    })
}

fn flat_rug(w: f64, d: f64, c1: u32) -> Cv {
    let _ = (w, d, c1);
    Cv::new(1, 1)
}

// ------------------------------------------------------------------ Marken

fn build_ground(b: &mut B, outdoor: &dyn Fn(&mut [u8], i32, i32, &dyn Fn(f64) -> i32, &dyn Fn(f64) -> i32), seed: i32) -> Cv {
    let gw = ((b.x1 - b.x0) * PPM as f64).round() as i32;
    let gh = ((b.z1 - b.z0) * (PPM * ZS) as f64).round() as i32;
    let mut map = vec![M_GRASS; (gw * gh) as usize];
    let (x0, z1) = (b.x0, b.z1);
    let fx = move |x: f64| ((x - x0) * PPM as f64).round() as i32;
    let fy = move |z: f64| ((z1 - z) * (PPM * ZS) as f64).round() as i32;
    outdoor(&mut map, gw, gh, &fx, &fy);
    let fill = |map: &mut [u8], x1: f64, z1_: f64, x2: f64, z2: f64, m: u8| {
        let a = fx(x1.min(x2));
        let bb = fx(x1.max(x2));
        let t = fy(z1_.max(z2));
        let u = fy(z1_.min(z2));
        for y in t.max(0)..u.min(gh) {
            for x in a.max(0)..bb.min(gw) {
                map[(y * gw + x) as usize] = m;
            }
        }
    };
    for f in b.floors.clone() {
        fill(&mut map, f.0, f.1, f.2, f.3, f.4);
    }
    let mut g = paint_ground(&map, gw, gh, seed);
    ground_details(&mut g, &map, gw, gh, seed / 2 + 3);
    // Mattor och linjer med lite slitage
    for f in b.flats.clone() {
        let (a, bb, t, u) = (fx(f.0), fx(f.2), fy(f.3), fy(f.1));
        for y in t..u {
            for x in a..bb {
                let v = hash2(x, y, 3);
                let mut col = f.4;
                if v < 0.08 {
                    col = shade(col, -0.25);
                } else if v > 0.94 {
                    col = shade(col, 0.2);
                }
                // Kantrand
                let edge = x - a < 3 || bb - x <= 3 || y - t < 3 || u - y <= 3;
                if edge {
                    col = mixc(col, 0xe9dcc4, 0.55);
                }
                g.set(x, y, c(col));
            }
        }
        if u - t > 6 && bb - a > 6 {
            g.rect(a + 1, t + 1, bb - a - 2, 1, ca(0xffffff, 0.18));
            g.rect(a, u - 1, bb - a, 1, ca(0x00001e, 0.2));
        }
    }
    // Mjuk skugga (ambient occlusion) längs väggarnas fot
    let ao = |g: &mut Cv, x1: f64, z1_: f64, x2: f64, z2: f64, dir: char| {
        let (a, w_) = (fx(x1.min(x2)), (((x2 - x1).abs()) * PPM as f64).round() as i32);
        let t = fy(z1_.max(z2));
        let h_ = (((z2 - z1_).abs()) * (PPM * ZS) as f64).round() as i32;
        for k in 0..6 {
            let al = [0.26, 0.18, 0.12, 0.08, 0.05, 0.03][k as usize];
            match dir {
                'd' => g.rect(a, t + k, w_, 1, ca(0x141232, al)),
                'u' => g.rect(a, t + h_ - 1 - k, w_, 1, ca(0x141232, al)),
                'r' => g.rect(a + k, t, 1, h_, ca(0x141232, al)),
                _ => g.rect(a + w_ - 1 - k, t, 1, h_, ca(0x141232, al)),
            }
        }
    };
    for w in &b.walls {
        let t = w.t / 2.0;
        if w.x_axis {
            ao(&mut g, w.a, w.fixed - t - 0.3, w.b, w.fixed - t, 'd');
        } else {
            ao(&mut g, w.fixed + t, w.a, w.fixed + t + 0.3, w.b, 'r');
            ao(&mut g, w.fixed - t - 0.3, w.a, w.fixed - t, w.b, 'l');
        }
    }
    g
}

fn dither_shadow(g: &mut Cv, x_a: i32, y_a: i32, w: i32, h: i32) {
    for yy in 0..h {
        for xx in 0..w {
            let edge = ((h - yy).min(w - xx)) as f32 / 6.0;
            if edge < 1.0 && bayer(x_a + xx, y_a + yy) + 0.47 > edge {
                continue;
            }
            g.put(x_a + xx, y_a + yy, ca(0x10143c, 0.22));
        }
    }
}

// ------------------------------------------------------------------ Göteborg

fn build_gbg() -> Site {
    let mut b = B::new("gbg");
    let h = 3.0;
    // Golv
    b.floor(-15.0, -2.0, 15.0, 10.0, M_CARPET);
    b.floor(-15.0, -10.0, -6.0, -2.0, M_RAISED);
    b.floor(-6.0, -10.0, 3.0, -2.0, M_TILE);
    b.floor(3.0, -10.0, 15.0, -2.0, M_WOOD);
    // Ytterväggar
    b.wall(-15.0, 15.0, 10.0, true, h, 0xece3cf, &[], 0.12, 1);
    b.wall(-15.0, 15.0, -10.0, true, FRONT_WALL_H, 0xece3cf, &[(9.5, 11.5)], 0.12, 1);
    b.wall(-10.0, 10.0, -15.0, false, h, 0xece3cf, &[], 0.12, 1);
    b.wall(-10.0, 10.0, 15.0, false, h, 0x3f8588, &[], 0.12, 1);
    // Innerväggar
    b.wall(-15.0, -5.0, 2.0, true, h, 0xdcc49a, &[(-10.0, -8.8)], 0.12, 0);
    b.glass(2.0, 10.0, -5.0, false, &[]);
    b.wall(-15.0, 15.0, -2.0, true, h, 0xece3cf, &[(-11.0, -9.8), (-2.2, -1.0), (5.0, 9.0)], 0.12, 0);
    b.wall(-10.0, -2.0, -6.0, false, h, 0xece3cf, &[], 0.12, 0);
    b.wall(-10.0, -2.0, 3.0, false, h, 0xdcc49a, &[], 0.12, 0);
    // Fönster på norra väggen
    for x in [-11.0, -7.0, 0.0, 4.0, 8.0, 12.0] {
        b.window(x, 9.93, 1.8);
    }
    // Entrédörrar
    let door = box_sprite(2.0, 0.16, 1.0, 0x4a3526, 0x9fd0e3, &BoxOpts {
        f: Some(&|g, wd, dp, hp| {
            g.rect(0, dp, wd, 3, c(0x3a4048));
            g.rect(wd / 2 - 1, dp, 2, hp, c(0x3a4048));
            g.rect(0, dp, 2, hp, c(0x3a4048));
            g.rect(wd - 2, dp, 2, hp, c(0x3a4048));
            g.rect(5, dp + 5, 3, 8, ca(0xffffff, 0.5));
            g.rect(wd / 2 + 5, dp + 5, 3, 8, ca(0xffffff, 0.5));
            g.rect(wd / 2 - 5, dp + hp / 2, 2, 5, c(0xc9ccd0));
            g.rect(wd / 2 + 3, dp + hp / 2, 2, 5, c(0xc9ccd0));
        }),
        ..Default::default()
    });
    let di = b.add(door, 9.5, -10.08, 0.0, -10.08);
    b.items[di].dynk = Dyn::Doors;
    // Zoner
    b.zone("Serverrummet", -15.0, -10.0, -6.0, -2.0);
    b.zone("Fikarummet", -6.0, -10.0, 3.0, -2.0);
    b.zone("Receptionen", 3.0, -10.0, 15.0, -2.0);
    b.zone("Ekonomi", -15.0, 2.0, -5.0, 10.0);
    b.zone("Korridoren", -15.0, -2.0, -5.0, 2.0);
    b.zone("Kontorslandskapet", -5.0, -2.0, 15.0, 10.0);
    // Skyltar
    for (t, x, z) in [("SERVERRUM", -10.4, -2.0), ("FIKARUM", -1.6, -2.0), ("EKONOMI", -9.4, 2.0)] {
        b.sign(t, x, z, WALL_H + 0.1);
    }
    b.sign("RECEPTION", 7.2, -5.7, 1.25);
    b.sign("NORDVIK", 7.2, -10.0, 0.95);
    b.sign("ENTRÉ", 10.5, -10.6, 1.0);
    // --- Serverrum
    b.rack("A", -12.4, -9.35, vec!["SW1", "SW2", "R1", "WLC", "LB"]);
    b.rack("B", -11.7, -9.35, vec!["Filserver", "NTP-server", "Loggserver", "Ekonomisystem", "Tid-1", "Tid-2"]);
    b.sign("RACK A", -12.4, -8.7, 2.6);
    b.sign("RACK B", -11.7, -8.7, 2.6);
    b.desk(-14.35, -5.2, true, Some(0xd8d4cb), None, true);
    b.chair(-13.5, -5.0, 0x30343b);
    b.spot("Tekniker", -14.35, -5.2, 1.0);
    b.inter(-14.05, -5.2, 1.2, IKind::Laptop, "Öppna laptopen", None);
    b.bx(-14.55, -8.8, 0.8, 0.5, 0.95, 1.9, 0x9aa0a6, true);
    b.bx(-14.5, -7.3, 0.6, 0.4, 0.175, 0.35, 0x2f5d8a, true);
    b.bx(-7.0, -9.5, 0.9, 0.7, 0.95, 1.9, 0xe5e7e8, true);
    // Brandsläckare
    b.bx(-6.4, -2.4, 0.16, 0.16, 0.45, 0.5, 0xc0282d, false);
    // Övervakningsskärm på väggen
    let mon = b.add(monitor_wall(), -8.95, -2.12, 0.45, -2.11);
    b.items[mon].dynk = Dyn::Monitor;
    b.inter(-8.2, -2.58, 1.2, IKind::Monitor, "Titta på övervakningsskärmen", Some(mon));
    b.spot("monitor", -8.2, -2.58, 1.0);
    // Kabelspår framför racken (Gi0/24-trunken och Gi0/25-reservkabeln)
    b.cable_plate(-13.3, -8.1, "SW1", "GigabitEthernet0/24", "Titta på Gi0/24-kabeln mellan switcharna");
    b.cable_plate(-10.7, -8.1, "SW1", "GigabitEthernet0/25", "Titta på Gi0/25-reservkabeln");
    // --- Fikarum
    b.bx(-1.5, -9.6, 4.0, 0.62, 0.45, 0.9, 0xf3f1ec, true);
    b.bx(-1.5, -9.6, 4.0, 0.66, 0.92, 0.04, 0x2c2f35, false);
    let cf = b.add(coffee_sprite(), -2.975, -9.775, 0.94, -9.77);
    b.inter(-2.8, -9.2, 0.9, IKind::Coffee, "Hämta kaffe", Some(cf));
    b.bx(1.8, -9.55, 0.7, 0.66, 0.925, 1.85, 0xdfe2e4, true);
    let tb = b.add(round_table(), -1.5 - 15.0 / PPM as f64, -5.8 - 0.4, 0.0, -5.9);
    let _ = tb;
    b.col_c(-1.5, -5.8, 1.2, 1.0);
    for (x, z) in [(-2.4, -5.8), (-0.6, -5.8), (-1.5, -4.9)] {
        b.chair(x, z, 0xa04c3a);
    }
    let wb = b.add(whiteboard_sprite(), -5.68, -6.8, 0.55, -6.8);
    b.inter(-5.2, -6.0, 1.2, IKind::Whiteboard, "Titta på whiteboarden", Some(wb));
    b.spot("whiteboard", -5.2, -6.0, 1.0);
    b.plant(2.4, -2.6, 1.2);
    let cl = b.add(cooler_sprite(), 0.97 - 0.17, -9.62 - 0.17, 0.0, -9.79);
    b.col_c(0.97, -9.62, 0.34, 0.34);
    b.inter(0.97, -9.22, 0.9, IKind::Water, "Ta ett glas vatten", Some(cl));
    // --- Reception
    let rd = b.add(reception_desk(), 6.0, -6.55, 0.0, -6.55);
    b.col_c(7.2, -6.2, 2.4, 0.7);
    let _ = rd;
    b.chair(7.2, -7.2, 0x30343b);
    b.sofa(12.8, -8.9, false, 0x34506e);
    b.bx(12.8, -7.6, 1.2, 0.6, 0.2, 0.4, 0x6d4a2f, true);
    b.rug(12.8, -8.1, 3.0, 2.0, 0x5a6e8a);
    b.bx(13.6, -3.4, 1.2, 0.6, 0.36, 0.72, 0xf4f2ed, true);
    b.chair(13.6, -4.3, 0x30343b);
    b.plant(4.0, -9.3, 1.3);
    b.plant(14.3, -2.6, 1.1);
    b.spot("Gast-laptop", 13.6, -3.4, 0.9);
    // Gästlaptop (dator på bordet)
    let gl = b.add(Cv::new(1, 1), 13.4, -3.55, 0.72, -3.7);
    b.items[gl].dynk = Dyn::Pc("Gast-laptop".to_string());
    b.inter(13.6, -3.4, 1.0, IKind::Pc("Gast-laptop".into()), "Titta på gästlaptopen", None);
    b.cable_plate(12.7, -3.4, "Gast-laptop", "nic", "Titta på kabeln till gästlaptopen");
    b.inter(14.05, -3.3, 0.8, IKind::MiniSwitch, "Koppla bort den okända miniswitchen", None);
    let ms = b.inters.len() - 1;
    b.inters[ms].min_week = 9;
    // --- Kontorslandskap
    let desks: [(&str, f64, f64); 8] = [
        ("PC-Anna", -2.5, 4.0),
        ("PC-Karim", 0.0, 4.0),
        ("PC-Sara", 2.5, 4.0),
        ("PC-Lisa", -2.5, 7.3),
        ("empty1", 0.0, 7.3),
        ("empty2", 2.5, 7.3),
        ("PC-Bo", -12.0, 6.0),
        ("PC-Maja", -8.6, 6.0),
    ];
    for (id, x, z) in desks {
        b.desk(x, z, false, None, if id.starts_with("PC-") { Some(id) } else { None }, false);
        b.chair(x, z + 0.75, if id == "PC-Bo" || id == "PC-Maja" { 0x5b3d6b } else { 0x30343b });
        if id.starts_with("PC-") {
            b.spot(id, x, z, 1.0);
            b.inter(x, z - 0.3, 1.0, IKind::Pc(id.to_string()), &format!("Använd {}", pc_name(id)), None);
            b.cable_plate(x - 1.05, z - 0.25, id, "nic", &format!("Titta på nätverkskabeln vid {}", pc_name(id)));
        }
    }
    // Skrivare och skåp
    b.bx(4.8, 9.6, 1.2, 0.45, 0.375, 0.75, 0xd8d4cb, true);
    let pr = b.add(printer_sprite(), 8.0 - 0.4, 9.3 - 0.3, 0.0, 9.0);
    b.col_c(8.0, 9.3, 0.8, 0.6);
    b.inter(8.0, 8.9, 0.9, IKind::Printer, "Skriv ut en sida", Some(pr));
    b.spot("Skrivare", 8.0, 8.9, 0.8);
    // Mötesbord
    b.bx(10.8, 5.6, 1.6, 1.0, 0.37, 0.74, 0x6d4a2f, true);
    for (x, z) in [(10.2, 4.8), (11.4, 4.8), (10.2, 6.4), (11.4, 6.4)] {
        b.chair(x, z, 0x4a6f8a);
    }
    b.rug(10.8, 5.6, 3.4, 2.6, 0x8a5a4a);
    for (x, z, s) in [(6.5, 9.4, 1.2), (-4.4, 9.4, 1.0), (14.4, 9.4, 1.3), (-4.4, -1.4, 1.0)] {
        b.plant(x, z, s);
    }
    b.sofa(-12.5, -0.2, true, 0x5a5f68);
    b.bx(-14.6, 9.5, 1.2, 0.45, 0.6, 1.2, 0xd8d4cb, true);
    b.bx(-5.8, 9.6, 0.8, 0.4, 0.95, 1.9, 0x9aa0a6, true);
    let bs = b.add(bookshelf_sprite(), -14.98, 3.9, 0.0, 3.9);
    let _ = bs;
    b.col_c(-14.78, 4.6, 0.36, 1.4);
    // --- Utomhus
    for (i, (x, z)) in [(-18.0, 11.0), (-17.5, -8.0), (18.0, 11.0), (19.0, 0.0), (17.5, -7.0), (-10.0, 12.5), (4.0, 12.6), (-4.0, -16.2), (5.0, -16.5), (-14.0, -16.0), (20.0, -11.0), (-20.0, 6.0)].iter().enumerate() {
        let kind = if (*x < -16.0 && *z < 8.0) || i % 4 == 0 {
            TreeKind::Cherry
        } else if i % 5 == 3 {
            TreeKind::Pine
        } else {
            TreeKind::Oak
        };
        b.tree(*x, *z, i as i32, kind);
    }
    for (i, (x, z)) in [(-16.0, 11.2), (16.0, 11.2), (8.4, -10.8), (12.6, -10.8), (16.2, 4.0), (-7.0, 11.0), (7.0, 11.0), (0.0, -11.0), (-6.0, -11.0)].iter().enumerate() {
        b.bush(*x, *z, i as i32);
    }
    for (x, z) in [(-13.0, -12.55), (-5.0, -12.55), (3.0, -12.55), (13.2, -12.55), (-17.0, 7.5)] {
        b.lamp(x, z);
    }
    b.bench(-8.5, -12.3);
    b.bench(0.5, -12.3);
    b.bench(-19.2, 3.2);
    let bn = b.add_o(bin(), 8.6, -12.2, 6, 16);
    let _ = bn;
    let fb1 = b.add(flower_bed(43, 16, 3), 6.2, -11.8, 0.0, -11.8);
    let fb2 = b.add(flower_bed(43, 16, 7), 13.0, -11.8, 0.0, -11.8);
    let _ = (fb1, fb2);
    b.col(6.2, -11.8, 8.0, -11.0);
    b.col(13.0, -11.8, 14.8, -11.0);
    let h1 = b.add(hedge(84, 3), -4.6, 11.3, 0.0, 11.3);
    b.items[h1].shadow = Shadow::Ell { rx: 1.8, rz: 0.3, dx: 0.3 };
    b.col(-4.6, 11.2, -1.1, 11.6);
    let h2 = b.add(hedge(72, 5), 9.5, 11.3, 0.0, 11.3);
    b.items[h2].shadow = Shadow::Ell { rx: 1.5, rz: 0.3, dx: 0.3 };
    b.col(9.5, 11.2, 12.5, 11.6);
    b.add(fence(1008), -21.0, 13.7, 0.0, 13.7);
    for (x, z, s, w, hh) in [(-20.2, -4.9, 1, 12, 8), (-16.5, -4.6, 2, 9, 6), (-16.2, 1.6, 3, 14, 9), (-20.4, 1.9, 4, 10, 7), (-12.0, 12.8, 7, 9, 6), (14.0, 12.9, 8, 8, 6)] {
        let i = b.add_o(rock(s, w, hh), x, z, (w >> 1) + 1, hh + 1);
        b.items[i].shadow = Shadow::Ell { rx: w as f64 / 40.0, rz: 0.15, dx: 0.1 };
    }
    b.col(-20.3, -5.2, -16.4, 2.2);
    // Bilarna: kontorsbilen tar dig till Borås, den gröna till Mölndal
    let car1 = b.car(16.4, -15.6, 0xc8412f, true);
    b.car(18.6, -15.6, 0x2f6fb0, false);
    let car3 = b.car(20.7, -15.6, 0x2f8a6a, true);
    b.items[car3].min_week = 11;
    b.inter(16.4, -13.7, 1.3, IKind::Travel("boras"), "Kör till lagret i Borås", Some(car1));
    b.inter(20.7, -13.7, 1.3, IKind::Travel("molndal"), "Kör till kontoret i Mölndal", Some(car3));
    let l = b.inters.len() - 1;
    b.inters[l].min_week = 11;
    b.sign("Bilen till Borås", 16.4, -13.2, 0.1);
    b.sign("Bilen till Mölndal", 20.7, -13.2, 0.1);
    b.spot("boras", 16.4, -13.7, 1.0);
    b.spot("molndal", 20.7, -13.7, 1.0);
    // Kompassmål i racket
    for (id, x) in [("SW1", -12.4), ("SW2", -12.4), ("R1", -12.4), ("WLC", -12.4), ("LB", -12.4), ("RM", -12.4)] {
        b.spot(id, x, -9.35 + 0.0, 1.8);
    }
    for id in ["Filserver", "NTP-server", "Loggserver", "Ekonomisystem", "Tid-1", "Tid-2"] {
        b.spot(id, -11.7, -9.35, 1.8);
    }
    // Mark
    let building = Rect { x0: -15.0, x1: 15.0, z0: -10.0, z1: 10.0 };
    let ground = build_ground(
        &mut b,
        &|map, gw, gh, fx, fy| {
            let fill = |map: &mut [u8], x1: f64, z1: f64, x2: f64, z2: f64, m: u8| {
                let a = fx(x1.min(x2));
                let bb = fx(x1.max(x2));
                let t = fy(z1.max(z2));
                let u = fy(z1.min(z2));
                for y in t.max(0)..u.min(gh) {
                    for x in a.max(0)..bb.min(gw) {
                        map[(y * gw + x) as usize] = m;
                    }
                }
            };
            fill(map, -21.0, -15.4, 21.0, -12.8, M_DIRT);
            fill(map, -15.8, -10.9, 15.8, -10.0, M_DIRT);
            fill(map, 9.2, -18.0, 11.8, -10.0, M_PAVING);
            fill(map, 8.2, -11.4, 12.8, -10.0, M_PAVING);
            fill(map, 13.0, -18.0, 21.0, -12.8, M_ASPHALT);
            fill(map, -15.6, 10.0, 15.6, 10.6, M_DIRT);
            // Dammen med strand
            for y in 0..gh {
                for x in 0..gw {
                    let wx = x as f64 / PPM as f64 - 21.0;
                    let wz = 14.0 - y as f64 / (PPM * ZS) as f64;
                    let wob = (vnoise(x as f32 / 9.0, y as f32 / 9.0, 5) - 0.5) as f64;
                    let d1 = ((wx + 18.3) / 2.55).powi(2) + ((wz + 1.5) / 4.4).powi(2);
                    if d1 < 1.0 + wob * 0.5 {
                        map[(y * gw + x) as usize] = M_DIRT;
                    }
                    let d2 = ((wx + 18.3) / 2.0).powi(2) + ((wz + 1.5) / 3.8).powi(2);
                    if d2 < 1.0 + (vnoise(x as f32 / 9.0, y as f32 / 9.0, 4) - 0.5) as f64 * 0.35 {
                        map[(y * gw + x) as usize] = M_WATER;
                    }
                }
            }
            fill(map, 6.2, -11.8, 8.0, -11.0, M_SOIL);
            fill(map, 13.0, -11.8, 14.8, -11.0, M_SOIL);
        },
        11,
    );
    let mut ground = ground;
    // Parkeringsrutor och handikappruta
    {
        let fx = |x: f64| ((x - -21.0) * PPM as f64).round() as i32;
        let fy = |z: f64| ((14.0 - z) * (PPM * ZS) as f64).round() as i32;
        let mut px = 15.0;
        while px < 21.0 {
            let (x, y0, y1) = (fx(px), fy(-13.5), fy(-18.0));
            ground.rect(x, y0, 2, y1 - y0, c(0xe8e6de));
            px += 2.2;
        }
        let (hx, hy) = (fx(13.4), fy(-16.4));
        ground.rect(hx, hy, 29, 24, c(0x2f6fb0));
        ground.rect(hx + 10, hy + 5, 6, 2, c(0xf2f2f2));
        ground.rect(hx + 12, hy + 3, 2, 12, c(0xf2f2f2));
        // Brunnslock
        ground.ellipse(fx(6.0) as f32, fy(-14.1) as f32, 9.0, 7.0, c(0x2a2c30));
        ground.ellipse(fx(6.0) as f32, fy(-14.1) as f32, 8.0, 6.0, c(0x4b4e55));
        // Byggnadens skugga på gräset
        let s1 = (fx(-14.6), fy(-10.0), (fx(15.9) - fx(-14.6)), (fy(-10.0 + 0.9) - fy(-10.0)).abs().max(1));
        let _ = s1;
        dither_shadow(&mut ground, fx(15.0), fy(10.0) , fx(15.9) - fx(15.0), fy(-9.4) - fy(10.0));
    }
    finish(b, ground, "Göteborg", (-9.5, -1.2, 1), building)
}

pub fn pc_name(id: &str) -> String {
    match id {
        "PC-Anna" => "Annas dator",
        "PC-Karim" => "Karims dator",
        "PC-Bo" => "Bos dator",
        "PC-Maja" => "Majas dator",
        "PC-Sara" => "Saras dator",
        "PC-Lisa" => "Lisas dator",
        "PC-Lager" => "lagerdatorn",
        "PC-Molndal" => "Elins dator",
        _ => id,
    }
    .to_string()
}

// ------------------------------------------------------------------ Borås

fn build_boras() -> Site {
    let mut b = B::new("boras");
    let (x1, x2, z1, z2) = (88.0, 112.0, -12.0, 12.0);
    b.floor(x1, z1, x2, z2, M_CONCRETE);
    b.floor(x1, z1, 93.5, -6.5, M_WOOD);
    // Gula linjer
    b.flats.push((92.0, -3.05, 112.0, -2.95, 0xe4b52a));
    b.flats.push((92.0, 2.95, 112.0, 3.05, 0xe4b52a));
    // Väggar: plåtfasad söderut, lägre
    b.wall(x1, x2, z1, true, FRONT_WALL_H + 0.1, 0x8e9195, &[], 0.25, 2);
    b.wall(x1, x2, z2, true, 6.5, 0x8e9195, &[], 0.25, 0);
    b.wall(z1, z2, x1, false, 6.5, 0x8e9195, &[(-0.9, 0.9)], 0.25, 0);
    b.wall(z1, z2, x2, false, 6.5, 0x8e9195, &[(-2.5, 2.5)], 0.25, 0);
    // Rullporten vid lastkajen
    let port = box_sprite(0.25, 5.0, 1.2, 0x8a9199, 0x8a9199, &BoxOpts {
        f: Some(&|g, wd, dp, _hp| {
            g.rect(0, 0, wd, dp, c(0x8a9199));
            let mut y = 2;
            while y < dp {
                g.rect(0, y, wd, 1, c(0x5a6068));
                y += 4;
            }
            g.rect(0, 0, 1, dp, c(0xb8bec4));
        }),
        ..Default::default()
    });
    b.add(port, 111.9, -2.5, 0.0, -2.5);
    b.col(111.8, -2.5, 112.2, 2.5);
    // Pallställ
    for (ri, rz) in [-7.5, -1.5, 5.0].iter().enumerate() {
        let spr = pallet_rack(5, 2.8, 4.2, ri as i32);
        b.add(spr, 95.0, *rz - 0.55, 0.0, *rz - 0.55);
        b.col_c(102.0, *rz, 14.2, 1.1);
    }
    // Lagerkontoret
    b.glass(z1, -6.5, 93.5, false, &[(-9.2, -8.1)]);
    b.wall(x1, 93.5, -6.5, true, 2.6, 0xdcc49a, &[], 0.12, 0);
    b.desk(91.2, -8.2, false, None, Some("PC-Lager"), false);
    b.chair(91.2, -7.4, 0x30343b);
    b.spot("PC-Lager", 91.2, -8.2, 1.0);
    b.inter(91.2, -8.5, 1.0, IKind::Pc("PC-Lager".into()), "Använd lagerdatorn", None);
    b.cable_plate(90.1, -8.4, "PC-Lager", "nic", "Titta på nätverkskabeln vid lagerdatorn");
    b.rack("Boras", 89.2, -10.8, vec!["SWB", "RB"]);
    b.sign("LAGRETS RACK", 89.2, -10.3, 2.4);
    for id in ["SWB", "RB"] {
        b.spot(id, 89.2, -10.8, 1.8);
    }
    // Mobiltelefondisk
    b.bx(91.0, -1.5, 1.3, 0.6, 0.55, 1.1, 0xf4f2ed, true);
    b.bx(91.0, -1.5, 1.4, 0.7, 1.12, 0.05, 0x6d4a2f, false);
    b.inter(91.0, -0.9, 1.0, IKind::Phone, "Ta upp gästmobilen", None);
    b.spot("Gast-mobil", 91.0, -0.9, 1.2);
    // Packbord
    b.bx(102.0, 9.8, 2.4, 1.0, 0.45, 0.9, 0x9aa6ad, true);
    b.add(misc_box(0.6, 0.5, 0.4, 0xc49b69, 0xb58a5a, false), 101.2 - 0.3, 9.8 - 0.25, 0.9, 9.55);
    b.add(misc_box(0.5, 0.4, 0.3, 0xd2ab78, 0xc49b69, false), 102.6 - 0.25, 9.9 - 0.2, 0.9, 9.7);
    b.sign("PACKBORD", 102.0, 10.6, 1.0);
    // Truck
    let tr = b.add_o(truck(), 105.2, -4.75, 0, 48);
    b.items[tr].shadow = Shadow::Ell { rx: 0.9, rz: 0.45, dx: 0.3 };
    b.col_c(106.0, -4.3, 1.8, 1.1);
    // Dörren tillbaka och bilen
    let dr = box_sprite(0.2, 1.8, 1.45, 0x3c7478, 0x3c7478, &BoxOpts::default());
    b.add(dr, 87.9, -0.9, 0.0, -0.9);
    b.sign("Bilen till Göteborg", 84.4, -0.8, 0.1);
    b.sign("NORDVIK · LAGER BORÅS", 100.0, 11.8, 1.3);
    // Trädgård och gård
    for (i, (x, z)) in [(83.5, -11.0), (116.0, 12.0), (84.0, 10.0), (116.0, -9.0), (96.0, 12.9), (107.0, 12.9), (117.0, -15.5)].iter().enumerate() {
        let kind = [TreeKind::Pine, TreeKind::Autumn, TreeKind::Oak, TreeKind::Pine][(i + 20) % 4];
        b.tree(*x, *z, i as i32 + 20, kind);
    }
    let car1 = b.car(84.4, -3.2, 0xc8412f, true);
    b.inter(84.4, -1.3, 1.3, IKind::Travel("gbg"), "Kör tillbaka till Göteborg", Some(car1));
    b.spot("gbg", 84.4, -1.3, 1.0);
    for (x, z) in [(92.0, -12.6), (100.0, -12.6), (108.0, -12.6), (86.0, 12.6), (114.0, 12.6)] {
        b.lamp(x, z);
    }
    for (x, z, s) in [(84.0, 6.5, 20), (117.0, 2.2, 21), (83.0, -8.5, 22)] {
        b.bush(x, z, s);
    }
    for (x, z, s, w, hh) in [(86.2, -16.0, 11, 12, 8), (117.0, -12.0, 12, 9, 7)] {
        let i = b.add_o(rock(s, w, hh), x, z, (w >> 1) + 1, hh + 1);
        b.items[i].shadow = Shadow::Ell { rx: w as f64 / 40.0, rz: 0.15, dx: 0.1 };
    }
    for (i, (x, z)) in [(114.5, -6.0), (115.2, -6.4), (114.8, -7.0)].iter().enumerate() {
        let cols = [(0x2f6fb0, 0x255a90), (0xc8412f, 0xa83426), (0x3f8a57, 0x2f6f45)];
        b.add(misc_box(0.5, 0.5, 0.85, cols[i].0, cols[i].1, true), x - 0.25, z - 0.25, 0.0, z - 0.25);
        b.col(x - 0.25, z - 0.25, x + 0.25, z + 0.25);
    }
    let cont = box_sprite(2.6, 1.4, 1.4, 0x3f7a52, 0x2f6f45, &BoxOpts {
        stripes: true,
        f: Some(&|g, wd, dp, _hp| {
            g.rect(wd / 2 - 10, dp + 6, 20, 3, c(0xffffff));
        }),
        ..Default::default()
    });
    b.add(cont, 114.0, 5.5, 0.0, 5.5);
    b.col(114.0, 5.5, 116.6, 6.9);
    // Accesspunkter i taket
    for (id, x, z) in [("AP-Lager-1", 109.0, 0.0), ("AP-Lager-2", 99.0, -4.5), ("AP-Lager-3", 101.0, 7.5)] {
        let j = b.add(Cv::new(1, 1), x, z, 3.2, z + 3.0);
        b.items[j].dynk = Dyn::Ap(id.to_string());
        b.spot(id, x, z, 3.2);
    }
    b.zone("Lagerkontoret", x1, z1, 93.5, -6.5);
    b.zone("Lagret i Borås", x1, z1, x2, z2);
    let building = Rect { x0: x1, x1: x2, z0: z1, z1: z2 };
    let ground = build_ground(
        &mut b,
        &|map, gw, gh, fx, fy| {
            let fill = |map: &mut [u8], x1: f64, z1: f64, x2: f64, z2: f64, m: u8| {
                let a = fx(x1.min(x2));
                let bb = fx(x1.max(x2));
                let t = fy(z1.max(z2));
                let u = fy(z1.min(z2));
                for y in t.max(0)..u.min(gh) {
                    for x in a.max(0)..bb.min(gw) {
                        map[(y * gw + x) as usize] = m;
                    }
                }
            };
            fill(map, 82.0, -17.0, 118.0, -13.0, M_ASPHALT);
            fill(map, 84.0, -1.5, 88.0, 1.5, M_DIRT);
            fill(map, 112.0, -3.0, 118.0, 3.0, M_ASPHALT);
            fill(map, 87.4, -12.6, 112.6, 12.6, M_DIRT);
            // Bäcken längs norra kanten
            for sx in 0..gw {
                let wx = sx as f64 / PPM as f64 + 82.0;
                let mid = 14.6 + (wx * 0.35).sin() * 0.35 + (wx * 0.9).sin() * 0.15;
                let bank = 1.2 + (vnoise(sx as f32 / 7.0, 1.0, 9) - 0.5) as f64 * 0.3;
                let wet = 0.8 + (vnoise(sx as f32 / 11.0, 2.0, 9) - 0.5) as f64 * 0.2;
                for sy in fy(mid + bank).max(0)..fy(mid - bank).min(gh) {
                    let wz = 16.0 - sy as f64 / (PPM * ZS) as f64;
                    map[(sy * gw + sx) as usize] = if (wz - mid).abs() < wet { M_WATER } else { M_DIRT };
                }
            }
        },
        23,
    );
    let mut ground = ground;
    {
        let fx = |x: f64| ((x - 82.0) * PPM as f64).round() as i32;
        let fy = |z: f64| ((16.0 - z) * (PPM * ZS) as f64).round() as i32;
        let mut lx = 83.0;
        while lx < 117.0 {
            ground.rect(fx(lx), fy(-15.0), 38, 2, c(0xe4b52a));
            lx += 3.0;
        }
        // Lastzon
        for sy in 0..(fy(-3.0) - fy(3.0)) {
            for sx in 0..(fx(117.8) - fx(112.2)) {
                if ((sx + sy) >> 3) % 2 == 0 && (sx < 4 || sx > fx(117.8) - fx(112.2) - 5) {
                    ground.set(fx(112.2) + sx, fy(3.0) + sy, c(0xe4b52a));
                }
            }
        }
    }
    finish(b, ground, "Lagret i Borås", (89.6, 0.0, 3), building)
}

// ------------------------------------------------------------------ Mölndal

fn build_molndal() -> Site {
    let mut b = B::new("molndal");
    let (x1, x2, z1, z2) = (156.0, 172.0, -8.0, 6.0);
    b.floor(x1, z1, x2, z2, M_CARPET);
    b.floor(168.0, -8.0, 172.0, -3.0, M_RAISED);
    b.wall(x1, x2, z2, true, 3.0, 0xece3cf, &[], 0.12, 1);
    b.wall(x1, x2, z1, true, FRONT_WALL_H, 0xece3cf, &[(163.0, 165.0)], 0.12, 1);
    b.wall(z1, z2, x1, false, 3.0, 0xece3cf, &[], 0.12, 1);
    b.wall(z1, z2, x2, false, 3.0, 0x3f8588, &[], 0.12, 1);
    b.wall(168.0, 172.0, -3.0, true, 3.0, 0xece3cf, &[], 0.12, 0);
    b.wall(z1, -3.0, 168.0, false, 3.0, 0xece3cf, &[(-6.2, -4.4)], 0.12, 0);
    for x in [158.0, 161.0, 164.0, 167.0, 170.0] {
        b.window(x, 5.93, 1.5);
    }
    // Skåp med R-Molndal-1 (nås via konsolservern i rack A)
    let cab = rack_sprite(0.6, 0.8);
    let ci = b.add(cab, 169.7, -7.6, 0.0, -7.6);
    b.col_c(170.0, -7.2, 0.6, 0.8);
    b.sign("R-Molndal-1", 170.0, -7.2, 2.4);
    b.spot("RM", 170.0, -7.2, 1.8);
    let _ = ci;
    b.desk(162.0, 1.5, false, None, Some("PC-Molndal"), false);
    b.chair(162.0, 2.25, 0x30343b);
    b.spot("PC-Molndal", 162.0, 1.5, 1.0);
    b.inter(162.0, 1.2, 1.0, IKind::Pc("PC-Molndal".into()), "Använd Elins dator", None);
    b.cable_plate(160.9, 1.2, "PC-Molndal", "nic", "Titta på nätverkskabeln vid Elins dator");
    b.desk(166.0, 1.5, false, None, None, false);
    b.chair(166.0, 2.25, 0x30343b);
    b.bx(170.5, 3.5, 1.2, 0.45, 0.375, 0.75, 0xd8d4cb, true);
    b.plant(157.4, 4.8, 1.2);
    b.plant(171.0, 5.0, 1.1);
    b.plant(157.5, -7.0, 1.0);
    b.sofa(160.5, -5.0, false, 0x34506e);
    b.rug(160.5, -3.4, 3.0, 1.8, 0x5a6e8a);
    let wb = b.add(whiteboard_sprite(), 160.0, 5.0, 0.55, 5.0);
    let _ = wb;
    b.bx(159.0, 5.5, 0.8, 0.3, 0.5, 1.0, 0x9aa0a6, false);
    b.sign("BERGQUIST · MÖLNDAL", 164.0, -8.0, 0.95);
    for (i, (x, z)) in [(152.0, 8.0), (176.0, 9.0), (153.0, -11.0), (175.0, -12.0), (176.0, 0.0)].iter().enumerate() {
        b.tree(*x, *z, i as i32 + 40, [TreeKind::Oak, TreeKind::Pine, TreeKind::Cherry, TreeKind::Oak, TreeKind::Autumn][i]);
    }
    for (i, (x, z)) in [(157.0, -8.6), (171.0, -8.6), (155.0, 3.0)].iter().enumerate() {
        b.bush(*x, *z, i as i32 + 50);
    }
    b.lamp(160.0, -9.6);
    b.lamp(168.0, -9.6);
    b.bench(172.5, -4.0);
    let car1 = b.car(164.0, -12.6, 0xc8412f, true);
    b.inter(164.0, -10.7, 1.3, IKind::Travel("gbg"), "Kör tillbaka till Göteborg", Some(car1));
    b.spot("gbg", 164.0, -10.7, 1.0);
    b.sign("Bilen till Göteborg", 164.0, -10.2, 0.1);
    b.zone("Mölndalskontoret", x1, z1, x2, z2);
    let building = Rect { x0: x1, x1: x2, z0: z1, z1: z2 };
    let ground = build_ground(
        &mut b,
        &|map, gw, gh, fx, fy| {
            let fill = |map: &mut [u8], x1: f64, z1: f64, x2: f64, z2: f64, m: u8| {
                let a = fx(x1.min(x2));
                let bb = fx(x1.max(x2));
                let t = fy(z1.max(z2));
                let u = fy(z1.min(z2));
                for y in t.max(0)..u.min(gh) {
                    for x in a.max(0)..bb.min(gw) {
                        map[(y * gw + x) as usize] = m;
                    }
                }
            };
            fill(map, 148.0, -14.0, 180.0, -10.4, M_ASPHALT);
            fill(map, 155.0, -10.4, 173.0, -8.0, M_PAVING);
            fill(map, 162.6, -10.4, 165.4, -8.0, M_PAVING);
            fill(map, 173.0, -9.0, 176.0, 3.0, M_DIRT);
        },
        31,
    );
    finish(b, ground, "Mölndal", (164.0, -6.4, 1), building)
}

fn finish(mut b: B, ground: Cv, name: &'static str, spawn: (f64, f64, u8), building: Rect) -> Site {
    // Skuggsiluetter till föremål som står på golvet
    for it in b.items.iter_mut() {
        if matches!(it.shadow, Shadow::None) && !it.wall && it.y <= 0.05 && it.spr.h <= 140 && it.dynk == Dyn::None && it.spr.w > 2 {
            it.shadow = Shadow::Sil(Rc::new(it.spr.silhouette(ca(0x12143a, 0.2))));
        }
    }
    Site {
        key: b.key,
        name,
        x0: b.x0,
        x1: b.x1,
        z0: b.z0,
        z1: b.z1,
        ground,
        items: b.items,
        cols: b.cols,
        zones: b.zones,
        inters: b.inters,
        signs: b.signs,
        racks: b.racks,
        spots: b.spots,
        spawn,
        building,
        cables: b.cables,
        cable_spots: b.cable_spots,
    }
}

pub fn build_site(key: &str) -> Site {
    match key {
        "boras" => build_boras(),
        "molndal" => build_molndal(),
        _ => build_gbg(),
    }
}
