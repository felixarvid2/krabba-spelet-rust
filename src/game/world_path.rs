//! Vägsökning för musklick: A* på ett rutnät över hindren, så att figuren går runt väggar och möbler.
//! Port av js/world/path2d.js.
#![allow(dead_code)]
use super::world_map::Rect;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

pub const CELL: f64 = 0.125;
pub const CLEAR: f64 = 0.26;

#[derive(Clone)]
pub struct Nav {
    pub x0: f64,
    pub z0: f64,
    pub x1: f64,
    pub z1: f64,
    pub nx: i32,
    pub nz: i32,
    pub g: Vec<u8>,
}

pub struct PathRes {
    pub pts: Vec<(f64, f64)>,
    pub end: (f64, f64),
    pub len: f64,
    pub partial: bool,
}

impl Nav {
    /// Bygger rutnätet med varje hinder förstorat med marginalen
    pub fn build(x0: f64, x1: f64, z0: f64, z1: f64, cols: &[Rect]) -> Nav {
        let nx = ((x1 - x0) / CELL).ceil() as i32;
        let nz = ((z1 - z0) / CELL).ceil() as i32;
        let mut nav = Nav { x0, z0, x1, z1, nx, nz, g: vec![0; (nx * nz) as usize] };
        let edge = 0.5 + CLEAR;
        for j in 0..nz {
            for i in 0..nx {
                let x = x0 + (i as f64 + 0.5) * CELL;
                let z = z0 + (j as f64 + 0.5) * CELL;
                if x < x0 + edge || x > x1 - edge || z < z0 + edge || z > z1 - edge {
                    nav.g[(j * nx + i) as usize] = 1;
                }
            }
        }
        for c in cols {
            nav.stamp(c);
        }
        nav
    }
    pub fn stamp(&mut self, c: &Rect) {
        let i0 = (((c.x0 - CLEAR - self.x0) / CELL).floor() as i32).max(0);
        let i1 = (((c.x1 + CLEAR - self.x0) / CELL).floor() as i32).min(self.nx - 1);
        let j0 = (((c.z0 - CLEAR - self.z0) / CELL).floor() as i32).max(0);
        let j1 = (((c.z1 + CLEAR - self.z0) / CELL).floor() as i32).min(self.nz - 1);
        for j in j0..=j1 {
            for i in i0..=i1 {
                let cx = self.x0 + (i as f64 + 0.5) * CELL;
                let cz = self.z0 + (j as f64 + 0.5) * CELL;
                if cx > c.x0 - CLEAR && cx < c.x1 + CLEAR && cz > c.z0 - CLEAR && cz < c.z1 + CLEAR {
                    self.g[(j * self.nx + i) as usize] = 1;
                }
            }
        }
    }
    fn cell_of(&self, x: f64, z: f64) -> (i32, i32) {
        (((x - self.x0) / CELL).floor() as i32, ((z - self.z0) / CELL).floor() as i32)
    }
    fn inside(&self, i: i32, j: i32) -> bool {
        i >= 0 && j >= 0 && i < self.nx && j < self.nz
    }
    pub fn free(&self, i: i32, j: i32) -> bool {
        self.inside(i, j) && self.g[(j * self.nx + i) as usize] == 0
    }
    fn center(&self, i: i32, j: i32) -> (f64, f64) {
        (self.x0 + (i as f64 + 0.5) * CELL, self.z0 + (j as f64 + 0.5) * CELL)
    }
    /// Närmaste fria ruta (bredden först)
    fn nearest_free(&self, c: (i32, i32), max_r: i32) -> Option<(i32, i32)> {
        if self.free(c.0, c.1) {
            return Some(c);
        }
        for r in 1..=max_r {
            let mut best: Option<(i32, i32)> = None;
            let mut bd = i32::MAX;
            for dj in -r..=r {
                for di in -r..=r {
                    if di.abs().max(dj.abs()) != r {
                        continue;
                    }
                    if self.free(c.0 + di, c.1 + dj) {
                        let d = di * di + dj * dj;
                        if d < bd {
                            bd = d;
                            best = Some((c.0 + di, c.1 + dj));
                        }
                    }
                }
            }
            if best.is_some() {
                return best;
            }
        }
        None
    }
    /// Kan figuren gå rakt mellan två punkter?
    pub fn clear_line(&self, a: (f64, f64), b: (f64, f64)) -> bool {
        let (dx, dz) = (b.0 - a.0, b.1 - a.1);
        let l = dx.hypot(dz);
        let steps = (l / (CELL * 0.5)).ceil() as i32;
        for k in 0..=steps {
            let t = if steps > 0 { k as f64 / steps as f64 } else { 0.0 };
            let c = self.cell_of(a.0 + dx * t, a.1 + dz * t);
            if !self.free(c.0, c.1) {
                return false;
            }
        }
        true
    }
}

/// Når du ett föremål härifrån? Linjen får bara korsa föremålets eget hinder (inte en vägg).
fn reach_line(cols: &[Rect], ax: f64, az: f64, bx: f64, bz: f64) -> bool {
    for c in cols {
        if bx >= c.x0 - 0.05 && bx <= c.x1 + 0.05 && bz >= c.z0 - 0.05 && bz <= c.z1 + 0.05 {
            continue;
        }
        let (mut t0, mut t1) = (0.0f64, 1.0f64);
        let d = [bx - ax, bz - az];
        let o = [ax, az];
        let lo = [c.x0, c.z0];
        let hi = [c.x1, c.z1];
        let mut hit = true;
        for k in 0..2 {
            if d[k].abs() < 1e-9 {
                if o[k] < lo[k] || o[k] > hi[k] {
                    hit = false;
                }
                continue;
            }
            let mut ta = (lo[k] - o[k]) / d[k];
            let mut tb = (hi[k] - o[k]) / d[k];
            if ta > tb {
                std::mem::swap(&mut ta, &mut tb);
            }
            t0 = t0.max(ta);
            t1 = t1.min(tb);
            if t0 > t1 {
                hit = false;
            }
            if !hit {
                break;
            }
        }
        if hit {
            return false;
        }
    }
    true
}

/// A* med åtta riktningar; diagonaler får inte skära hörn.
/// reach = Some(r): sök en ruta inom r meter från målet med fri sikt (för föremål och personer).
pub fn find_path(nav: &Nav, cols: &[Rect], x0: f64, z0: f64, x1: f64, z1: f64, reach: Option<f64>) -> Option<PathRes> {
    let s = nav.nearest_free(nav.cell_of(x0, z0), 6)?;
    let e0 = nav.cell_of(x1, z1);
    let mut e = if reach.is_some() { e0 } else { nav.nearest_free(e0, 24)? };
    let nx = nav.nx;
    let n = (nav.nx * nav.nz) as usize;
    let sid = (s.1 * nx + s.0) as usize;
    let mut eid = (e.1 * nx + e.0).max(0) as usize;
    let sq2 = std::f64::consts::SQRT_2;
    let heur = |id: usize, e: (i32, i32)| -> f64 {
        let dx = ((id as i32 % nx) - e.0).abs() as f64;
        let dz = ((id as i32 / nx) - e.1).abs() as f64;
        dx + dz + (sq2 - 2.0) * dx.min(dz)
    };
    let mut gs = vec![f64::INFINITY; n];
    let mut from = vec![-1i32; n];
    let mut closed = vec![false; n];
    let mut heap: BinaryHeap<Reverse<(i64, usize)>> = BinaryHeap::new();
    gs[sid] = 0.0;
    heap.push(Reverse(((heur(sid, e) * 1000.0) as i64, sid)));
    let dirs: [(i32, i32, f64); 8] = [(1, 0, 1.0), (-1, 0, 1.0), (0, 1, 1.0), (0, -1, 1.0), (1, 1, sq2), (1, -1, sq2), (-1, 1, sq2), (-1, -1, sq2)];
    let mut found = false;
    let mut guard = 0usize;
    let mut best = sid;
    let mut best_h = heur(sid, e);
    while let Some(Reverse((_, cur))) = heap.pop() {
        guard += 1;
        if guard > n * 2 {
            break;
        }
        if closed[cur] {
            continue;
        }
        if !reach.is_some() && cur == eid {
            found = true;
            break;
        }
        if let Some(r) = reach {
            let gc = nav.center(cur as i32 % nx, cur as i32 / nx);
            if (gc.0 - x1).hypot(gc.1 - z1) <= r && reach_line(cols, gc.0, gc.1, x1, z1) {
                eid = cur;
                e = (cur as i32 % nx, cur as i32 / nx);
                found = true;
                break;
            }
        }
        closed[cur] = true;
        let hc = heur(cur, e);
        if hc < best_h {
            best_h = hc;
            best = cur;
        }
        let (ci, cj) = (cur as i32 % nx, cur as i32 / nx);
        for d in dirs {
            let (ni, nj) = (ci + d.0, cj + d.1);
            if !nav.free(ni, nj) {
                continue;
            }
            if d.0 != 0 && d.1 != 0 && (!nav.free(ci + d.0, cj) || !nav.free(ci, cj + d.1)) {
                continue;
            }
            let id = (nj * nx + ni) as usize;
            if closed[id] {
                continue;
            }
            let ng = gs[cur] + d.2;
            if ng < gs[id] {
                gs[id] = ng;
                from[id] = cur as i32;
                heap.push(Reverse((((ng + heur(id, e)) * 1000.0) as i64, id)));
            }
        }
    }
    let partial = !found;
    if partial {
        if best == sid {
            let c = nav.center(s.0, s.1);
            return Some(PathRes { pts: vec![], end: c, len: 0.0, partial: true });
        }
        eid = best;
    }
    let mut cells = vec![];
    let mut cur = eid as i32;
    while cur != -1 {
        cells.push(nav.center(cur % nx, cur / nx));
        cur = from[cur as usize];
    }
    cells.reverse();
    let ec = nav.cell_of(x1, z1);
    let tgt = if !partial && reach.is_none() && nav.free(ec.0, ec.1) { (x1, z1) } else { *cells.last().unwrap() };
    let last = cells.len() - 1;
    cells[last] = tgt;
    // Utjämning: hoppa över punkter så länge det finns fri siktlinje
    let mut pts = vec![(x0, z0)];
    pts.extend(cells);
    let mut out = vec![];
    let mut k = 0;
    while k + 1 < pts.len() {
        let mut far = k + 1;
        let mut m = pts.len() - 1;
        while m > k + 1 {
            if nav.clear_line(pts[k], pts[m]) {
                far = m;
                break;
            }
            m -= 1;
        }
        out.push(pts[far]);
        k = far;
    }
    Some(PathRes { pts: out, end: tgt, len: gs[eid] * CELL, partial })
}
