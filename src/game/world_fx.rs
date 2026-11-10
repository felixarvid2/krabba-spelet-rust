//! Väder, dag och natt, ljus från lampor och skärmar. Barnmodul till world.rs.
//! Port av ljus och väder i js/world/scene2d.js (postLow) och world2d.js (drawPost).
use super::*;

pub struct Fx {
    /// 0 = växlar av sig själv, 1 = sol, 2 = moln, 3 = regn
    pub mode: u8,
    pub rain: f32,
    pub cloud: f32,
    pub bolt_t: f64,
    pub flash: f32,
    lm: Vec<f32>,
}

impl Fx {
    pub fn new() -> Fx {
        Fx { mode: 0, rain: 0.0, cloud: 0.0, bolt_t: 30.0, flash: 0.0, lm: vec![] }
    }

    /// Vädret vid tiden t (sekunder i världen): första minuten sol, sedan växlande
    fn target(&self, t: f64, week: u32) -> u8 {
        match self.mode {
            0 => {
                let seg = (t / 150.0).floor() as i32;
                if seg == 0 {
                    return 1;
                }
                let h = hash2(seg, week as i32, 9);
                if h < 0.5 { 1 } else if h < 0.8 { 2 } else { 3 }
            }
            m => m,
        }
    }

    pub fn update(&mut self, dt: f64, t: f64, week: u32) {
        let k = self.target(t, week);
        let (tr, tc) = (if k == 3 { 1.0 } else { 0.0 }, if k >= 2 { 1.0 } else { 0.0 });
        let s = (dt * 0.3) as f32;
        self.rain += (tr - self.rain) * s.min(1.0);
        self.cloud += (tc - self.cloud) * s.min(1.0);
        self.flash = (self.flash - dt as f32 * 4.0).max(0.0);
        if self.rain > 0.6 {
            self.bolt_t -= dt;
            if self.bolt_t <= 0.0 {
                self.bolt_t = 30.0 + hash2((t * 10.0) as i32, 3, 4) as f64 * 40.0;
                self.flash = 1.0;
                crate::engine::audio::sfx("thunder");
            }
        }
    }
}

fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    mix(a, b, t.clamp(0.0, 1.0))
}

fn glow(lm: &mut [f32], w: i32, h: i32, cx: i32, cy: i32, r: i32, col: [f32; 3], a: f32) {
    if cx + r < 0 || cy + r < 0 || cx - r >= w || cy - r >= h {
        return;
    }
    let rf = r as f32;
    for y in (cy - r).max(0)..=(cy + r).min(h - 1) {
        for x in (cx - r).max(0)..=(cx + r).min(w - 1) {
            let (dx, dy) = ((x - cx) as f32, (y - cy) as f32);
            let d = (dx * dx + dy * dy).sqrt();
            if d >= rf {
                continue;
            }
            let f = (1.0 - d / rf).powi(2) * a;
            let o = ((y * w + x) * 3) as usize;
            lm[o] += col[0] * f;
            lm[o + 1] += col[1] * f;
            lm[o + 2] += col[2] * f;
        }
    }
}

impl World {
    /// Väder: "sol", "moln", "regn" eller "auto" (varierar över tid)
    pub fn set_weather(&mut self, w: &str) {
        self.fx.mode = match w {
            "sun" | "sol" => 1,
            "clouds" | "moln" => 2,
            "rain" | "regn" => 3,
            _ => 0,
        };
    }
    /// Nuvarande väder: "sol", "moln" eller "regn"
    pub fn weather(&self) -> &'static str {
        if self.fx.rain > 0.5 {
            "regn"
        } else if self.fx.cloud > 0.5 {
            "moln"
        } else {
            "sol"
        }
    }
    /// Hur kvällsmörk det är: 0 = dag, 1 = kväll (kl. 17), 2 = natt (kl. 20 och senare)
    pub fn darkness(&self, st: &State) -> f64 {
        let hour = 8.0 + st.time / 3600.0;
        ((hour - 13.0) / 4.0).clamp(0.0, 1.0) + ((hour - 17.0) / 3.0).clamp(0.0, 1.0)
    }

    /// Ljus och färgton över hela bilden (multipliceras), därefter regn utomhus
    pub(super) fn apply_light(&mut self, fb: &mut Cv, v: &View, st: &State) {
        let hour = 8.0 + st.time / 3600.0;
        let eve = ((hour - 13.0) / 4.0).clamp(0.0, 1.0) as f32;
        let night = ((hour - 17.0) / 3.0).clamp(0.0, 1.0) as f32;
        let (rain, cloud) = (self.fx.rain, self.fx.cloud);
        let (w, h) = (fb.w, fb.h);
        let n = (w * h) as usize;
        let mut lm = std::mem::take(&mut self.fx.lm);
        lm.resize(n * 3, 1.0);
        // Dagsljus ute, svalare inne; kvällen blir varmare och mörkare
        let mut out = mix3([255.0, 252.0, 242.0], [236.0, 178.0, 150.0], eve);
        out = mix3(out, [62.0, 76.0, 132.0], night);
        // Moln och regn dämpar och kyler ljuset
        let dim = mix3([1.0, 1.0, 1.0], [0.9, 0.92, 0.96], cloud);
        let dim = mix3(dim, [0.7, 0.76, 0.9], rain);
        out = [out[0] * dim[0], out[1] * dim[1], out[2] * dim[2]];
        let mut ins = mix3([236.0, 234.0, 244.0], [176.0, 164.0, 190.0], eve);
        ins = mix3(ins, [112.0, 108.0, 150.0], night);
        let out = [out[0] / 255.0, out[1] / 255.0, out[2] / 255.0];
        let ins = [ins[0] / 255.0, ins[1] / 255.0, ins[2] / 255.0];
        let fl = self.fx.flash * 0.5;
        // Byggnaden på skärmen
        let b = self.sites[self.cur].building;
        let (bx0, _) = v.to_screen(b.x0, 0.0, 0.0);
        let (bx1, _) = v.to_screen(b.x1, 0.0, 0.0);
        let (_, by0) = v.to_screen(0.0, b.z1, WALL_H);
        let (_, by1) = v.to_screen(0.0, b.z0, 0.0);
        let inside = |x: i32, y: i32| x >= bx0 && x < bx1 && y >= by0 && y < by1;
        for y in 0..h {
            for x in 0..w {
                let c = if inside(x, y) { &ins } else { &out };
                let o = ((y * w + x) * 3) as usize;
                lm[o] = c[0] + fl;
                lm[o + 1] = c[1] + fl;
                lm[o + 2] = c[2] + fl;
            }
        }
        // Molnskuggor på marken utomhus
        if cloud > 0.05 {
            let (sx0, sx1) = (self.sites[self.cur].x0, self.sites[self.cur].x1);
            for i in 0..6 {
                let span = sx1 - sx0 + 16.0;
                let cx = sx0 - 8.0 + (i as f64 * 17.3 + self.t * 0.6).rem_euclid(span);
                let cz = -14.0 + i as f64 * 5.1;
                let r = 3.0 + (i % 3) as f64;
                let (sx, sy) = v.to_screen(cx, cz, 0.0);
                let (rx, ry) = (r * PPM as f64, r * (PPM * ZS) as f64 * 0.6);
                for y in (sy - ry as i32).max(0)..=(sy + ry as i32).min(h - 1) {
                    for x in (sx - rx as i32).max(0)..=(sx + rx as i32).min(w - 1) {
                        let (dx, dy) = ((x - sx) as f64 / rx, (y - sy) as f64 / ry);
                        let d = dx * dx + dy * dy;
                        if d > 1.0 || inside(x, y) {
                            continue;
                        }
                        if d > 0.5 && (bayer(x, y) as f64 + 0.5) < (d - 0.5) * 2.0 {
                            continue;
                        }
                        let o = ((y * w + x) * 3) as usize;
                        let f = 1.0 - 0.2 * cloud;
                        lm[o] *= f;
                        lm[o + 1] *= f;
                        lm[o + 2] *= f;
                    }
                }
            }
        }
        // Ljuskällor
        let dk = (eve * 0.5 + night * 0.8 + rain * 0.4).min(1.0);
        let gm = 0.8 + 1.2 * dk;
        let t = self.t;
        let s = &self.sites[self.cur];
        if self.cur == 0 {
            let mut lx = -12.0;
            while lx <= 13.0 {
                let mut lz = -7.5;
                while lz <= 8.5 {
                    let (sx, sy) = v.to_screen(lx, lz, 0.0);
                    glow(&mut lm, w, h, sx, sy, 58, [0.243, 0.22, 0.149], 0.55 * gm);
                    lz += 4.0;
                }
                lx += 4.0;
            }
            let (sx, sy) = v.to_screen(-10.5, -6.0, 0.0);
            glow(&mut lm, w, h, sx, sy, 90, [0.08, 0.16, 0.29], 0.8 * gm);
            let (sx, sy) = v.to_screen(-1.5, -5.8, 0.0);
            glow(&mut lm, w, h, sx, sy, 50, [0.31, 0.22, 0.08], 0.7 * gm);
            // Kaffemaskinen lyser varmt
            let (sx, sy) = v.to_screen(-2.8, -9.6, 0.9);
            glow(&mut lm, w, h, sx, sy, 18, [1.0, 0.7, 0.35], 0.35 * gm * (0.85 + 0.15 * (t * 3.0).sin() as f32));
        } else if self.cur == 1 {
            let mut bx = 92.0;
            while bx <= 108.0 {
                let mut bz = -8.0;
                while bz <= 8.0 {
                    let (sx, sy) = v.to_screen(bx, bz, 0.0);
                    glow(&mut lm, w, h, sx, sy, 58, [0.243, 0.22, 0.149], 0.55 * gm);
                    bz += 8.0;
                }
                bx += 8.0;
            }
        }
        for it in &s.items {
            if it.min_week > self.week {
                continue;
            }
            match &it.dynk {
                Dyn::Pc(_) => {
                    let (sx, sy) = v.to_screen(it.x, it.z, it.y);
                    glow(&mut lm, w, h, sx, sy - 6, 16, [0.16, 0.28, 0.43], 0.9 * gm);
                }
                Dyn::Monitor => {
                    let (sx, sy) = v.to_screen(it.x, it.z, it.y);
                    glow(&mut lm, w, h, sx + 12, sy + 20, 22, [0.16, 0.3, 0.5], 0.9 * gm);
                }
                Dyn::Rack(ri) => {
                    let r = &s.racks[*ri];
                    let (sx, sy) = v.to_screen(r.x, r.z - r.d / 2.0, 1.2);
                    glow(&mut lm, w, h, sx, sy, 14, [0.31, 1.0, 0.55], 0.3 * gm * (0.85 + 0.15 * (t * 5.0 + r.x).sin() as f32));
                }
                _ => {}
            }
        }
        // Lyktstolpar tänds när det är mörkt eller regnar
        if dk > 0.2 {
            for &(lx, lz) in &s.lamps {
                let (sx, sy) = v.to_screen(lx, lz, 2.0);
                glow(&mut lm, w, h, sx, sy + 4, 44, [1.0, 0.75, 0.43], (0.25 + 0.5 * dk) * (dk * 1.5).min(1.0));
            }
        }
        // Multiplicera in ljuset
        for (i, px) in fb.d.iter_mut().enumerate() {
            let p = *px;
            let a = p & 0xFF000000;
            let o = i * 3;
            let r = (((p >> 16) & 255) as f32 * lm[o].clamp(0.0, 1.45)).min(255.0) as u32;
            let g = (((p >> 8) & 255) as f32 * lm[o + 1].clamp(0.0, 1.45)).min(255.0) as u32;
            let bl = ((p & 255) as f32 * lm[o + 2].clamp(0.0, 1.45)).min(255.0) as u32;
            *px = a | (r << 16) | (g << 8) | bl;
        }
        self.fx.lm = lm;
        // Lampornas glödande huvuden
        if dk > 0.2 {
            for &(lx, lz) in &s.lamps {
                let (sx, sy) = v.to_screen(lx, lz, 2.0);
                for (r, a) in [(4.0, 0.15), (2.5, 0.25), (1.2, 0.8)] {
                    dither_ellipse(fb, sx, sy + 4, r, r * 0.8, a * dk);
                }
            }
        }
        // Regn: streck och ringar i pölarna, bara utomhus
        if rain > 0.03 {
            let gust = ((t * 0.21).sin().max(0.0) * 0.6) as f64;
            let burst = 0.65 + 0.35 * (t * 0.13).sin();
            let cnt = ((rain as f64 * 320.0 * (n as f64 / 103_000.0).max(0.4)) * burst) as usize;
            let slant = 1 + (gust * 2.0) as i32;
            for i in 0..cnt.min(900) {
                let fx_ = ((i as f64 * 97.3 + t * (15.0 + gust * 40.0)).rem_euclid((w + 60) as f64)) as i32 - 30;
                let fy_ = ((i as f64 * 61.7 + t * (105.0 + (i % 5) as f64 * 10.0)).rem_euclid((h + 40) as f64)) as i32 - 20;
                if inside(fx_, fy_) {
                    continue;
                }
                for k in 0..4 {
                    fb.put(fx_ - k * slant / 2, fy_ + k, ca(0xd8e6ff, 0.75 - k as f32 * 0.14));
                }
            }
            for i in 0..26 {
                let ph = (t * 1.3 + i as f64 * 0.37).rem_euclid(1.0);
                let seed = (t * 1.3 + i as f64 * 0.37).floor() as i32;
                let x = (hash2(seed, i, 1) * w as f32) as i32;
                let y = (hash2(seed, i, 2) * h as f32) as i32;
                if inside(x, y) || y < by1 - 2 && x >= bx0 && x < bx1 {
                    continue;
                }
                ring(fb, x, y, 1.0 + ph * 3.5, (1.0 + ph * 3.5) * 0.45, ca(0xdfeaff, (1.0 - ph as f32) * 0.5 * rain));
            }
        }
    }
}
