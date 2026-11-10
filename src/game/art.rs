//! Pixelkonst för 2D-världen: brus, paletter med dithering, mark, skuggade lådor,
//! växter, bilar och figurer. Allt ritas i kod. Port av js/world/art2d.js (och delar av scene2d.js).
//! Ritytan `Cv` är en enkel ARGB-buffert (0xAARRGGBB) som inte beror på motorn.
#![allow(dead_code)]

// ------------------------------------------------------------------ Färger

/// Ogenomskinlig färg från 0xRRGGBB
pub const fn c(rgb: u32) -> u32 {
    0xFF000000 | (rgb & 0xFFFFFF)
}
/// Färg med alfa 0.0–1.0
pub fn ca(rgb: u32, a: f32) -> u32 {
    (((a.clamp(0.0, 1.0) * 255.0).round() as u32) << 24) | (rgb & 0xFFFFFF)
}
pub fn rgb_of(col: u32) -> [f32; 3] {
    [((col >> 16) & 255) as f32, ((col >> 8) & 255) as f32, (col & 255) as f32]
}
pub fn pack(r: f32, g: f32, b: f32) -> u32 {
    let q = |v: f32| v.round().clamp(0.0, 255.0) as u32;
    (q(r) << 16) | (q(g) << 8) | q(b)
}
pub fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}
pub fn mixc(a: u32, b: u32, t: f32) -> u32 {
    let m = mix(rgb_of(a), rgb_of(b), t);
    pack(m[0], m[1], m[2])
}
/// Ljusare (f>0) eller mörkare (f<0), som shade() i world2d.js
pub fn shade(col: u32, f: f32) -> u32 {
    let [r, g, b] = rgb_of(col);
    if f < 0.0 {
        pack(r * (1.0 + f), g * (1.0 + f), b * (1.0 + f))
    } else {
        pack(r + (255.0 - r) * f, g + (255.0 - g) * f, b + (255.0 - b) * f)
    }
}
/// Ljusare/mörkare med färgförskjutning: skuggor mot blått, ljus mot gult (tone i art2d.js)
pub fn tone(col: u32, f: f32) -> u32 {
    let [r, g, b] = rgb_of(col);
    if f < 0.0 {
        let k = 1.0 + f;
        pack(r * k * 0.96 + 4.0 * -f, g * k * 0.98 + 6.0 * -f, b * k + 30.0 * -f)
    } else {
        pack(r + (255.0 - r) * f, g + (250.0 - g) * f, b + (215.0 - b) * f * 0.9)
    }
}

// ------------------------------------------------------------------ Brus

const BAYER: [f32; 16] = [0., 8., 2., 10., 12., 4., 14., 6., 3., 11., 1., 9., 15., 7., 13., 5.];
pub fn bayer(x: i32, y: i32) -> f32 {
    BAYER[(((y & 3) << 2) | (x & 3)) as usize] / 16.0 - 0.469
}
pub fn hash2(x: i32, y: i32, s: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374761393).wrapping_add((y as u32).wrapping_mul(668265263)).wrapping_add((s as u32).wrapping_mul(982451653));
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    (h ^ (h >> 16)) as f32 / 4294967296.0
}
pub fn vnoise(x: f32, y: f32, s: i32) -> f32 {
    let xi = x.floor();
    let yi = y.floor();
    let xf = x - xi;
    let yf = y - yi;
    let u = xf * xf * (3.0 - 2.0 * xf);
    let v = yf * yf * (3.0 - 2.0 * yf);
    let (xi, yi) = (xi as i32, yi as i32);
    let a = hash2(xi, yi, s);
    let b = hash2(xi + 1, yi, s);
    let cc = hash2(xi, yi + 1, s);
    let d = hash2(xi + 1, yi + 1, s);
    a + (b - a) * u + (cc - a) * v + (a - b - cc + d) * u * v
}
pub fn fbm(x: f32, y: f32, s: i32) -> f32 {
    vnoise(x, y, s) * 0.55 + vnoise(x * 2.03, y * 2.03, s + 7) * 0.3 + vnoise(x * 4.1, y * 4.1, s + 13) * 0.15
}

/// Liten slumpgenerator (samma som rng() i JS)
#[derive(Clone)]
pub struct Rng(pub u32);
impl Rng {
    pub fn new(seed: u32) -> Rng {
        Rng(if seed == 0 { 1 } else { seed })
    }
    pub fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        self.0 as f32 / 4294967296.0
    }
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + self.next() * (b - a)
    }
    pub fn idx(&mut self, n: usize) -> usize {
        ((self.next() * n as f32) as usize).min(n.saturating_sub(1))
    }
}

/// Väljer en färg ur paletten med ditherad övergång
pub fn pick(p: &[u32], v: f32, x: i32, y: i32, amt: f32) -> u32 {
    let q = v * p.len() as f32 + bayer(x, y) * amt;
    let mut i = q.floor() as i32;
    if i < 0 {
        i = 0;
    }
    if i >= p.len() as i32 {
        i = p.len() as i32 - 1;
    }
    p[i as usize]
}

// ------------------------------------------------------------------ Ritytan

/// ARGB-bild med en liten förskjutning (tx, ty) som ritfunktionerna lägger på, som translate() i canvas.
#[derive(Clone)]
pub struct Cv {
    pub w: i32,
    pub h: i32,
    pub d: Vec<u32>,
    pub tx: i32,
    pub ty: i32,
}

impl Cv {
    pub fn new(w: i32, h: i32) -> Cv {
        let (w, h) = (w.max(1), h.max(1));
        Cv { w, h, d: vec![0; (w * h) as usize], tx: 0, ty: 0 }
    }
    pub fn fill(&mut self, col: u32) {
        for p in self.d.iter_mut() {
            *p = col;
        }
    }
    pub fn get(&self, x: i32, y: i32) -> u32 {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            0
        } else {
            self.d[(y * self.w + x) as usize]
        }
    }
    /// Skriver en pixel med alfablandning, utan förskjutning
    pub fn put(&mut self, x: i32, y: i32, col: u32) {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return;
        }
        let a = col >> 24;
        if a == 0 {
            return;
        }
        let i = (y * self.w + x) as usize;
        if a == 255 {
            self.d[i] = col;
            return;
        }
        let dst = self.d[i];
        let da = dst >> 24;
        if da == 0 {
            self.d[i] = col;
            return;
        }
        let t = a as f32 / 255.0;
        let m = mix(rgb_of(dst), rgb_of(col), t);
        let na = da.max(a);
        self.d[i] = (na << 24) | pack(m[0], m[1], m[2]);
    }
    /// Skriver en pixel som den är (ersätter)
    pub fn set(&mut self, x: i32, y: i32, col: u32) {
        let (x, y) = (x + self.tx, y + self.ty);
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return;
        }
        self.d[(y * self.w + x) as usize] = col;
    }
    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, col: u32) {
        let (x, y) = (x + self.tx, y + self.ty);
        let x0 = x.max(0);
        let y0 = y.max(0);
        let x1 = (x + w).min(self.w);
        let y1 = (y + h).min(self.h);
        let opaque = col >> 24 == 255;
        for yy in y0..y1 {
            for xx in x0..x1 {
                if opaque {
                    self.d[(yy * self.w + xx) as usize] = col;
                } else {
                    self.put(xx, yy, col);
                }
            }
        }
    }
    /// Rektangel i RGB med alfa 0–1
    pub fn rect_a(&mut self, x: i32, y: i32, w: i32, h: i32, rgb: u32, a: f32) {
        self.rect(x, y, w, h, ca(rgb, a));
    }
    pub fn clear_rect(&mut self, x: i32, y: i32, w: i32, h: i32) {
        let (x, y) = (x + self.tx, y + self.ty);
        for yy in y.max(0)..(y + h).min(self.h) {
            for xx in x.max(0)..(x + w).min(self.w) {
                self.d[(yy * self.w + xx) as usize] = 0;
            }
        }
    }
    pub fn ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, col: u32) {
        let (cx, cy) = (cx + self.tx as f32, cy + self.ty as f32);
        for yy in ((cy - ry).floor() as i32)..=((cy + ry).ceil() as i32) {
            for xx in ((cx - rx).floor() as i32)..=((cx + rx).ceil() as i32) {
                let dx = (xx as f32 + 0.5 - cx) / rx;
                let dy = (yy as f32 + 0.5 - cy) / ry;
                if dx * dx + dy * dy <= 1.0 {
                    self.put(xx, yy, col);
                }
            }
        }
    }
    pub fn circle(&mut self, cx: f32, cy: f32, r: f32, col: u32) {
        self.ellipse(cx, cy, r, r, col);
    }
    /// Lägger in en annan bild (alfablandning)
    pub fn blit(&mut self, s: &Cv, x: i32, y: i32) {
        let (x, y) = (x + self.tx, y + self.ty);
        for sy in 0..s.h {
            let dy = y + sy;
            if dy < 0 || dy >= self.h {
                continue;
            }
            for sx in 0..s.w {
                let dx = x + sx;
                if dx < 0 || dx >= self.w {
                    continue;
                }
                let p = s.d[(sy * s.w + sx) as usize];
                if p >> 24 != 0 {
                    self.put(dx, dy, p);
                }
            }
        }
    }
    /// Som blit men med extra genomskinlighet (0–1)
    pub fn blit_a(&mut self, s: &Cv, x: i32, y: i32, alpha: f32) {
        if alpha >= 0.999 {
            return self.blit(s, x, y);
        }
        let (x, y) = (x + self.tx, y + self.ty);
        for sy in 0..s.h {
            let dy = y + sy;
            if dy < 0 || dy >= self.h {
                continue;
            }
            for sx in 0..s.w {
                let dx = x + sx;
                if dx < 0 || dx >= self.w {
                    continue;
                }
                let p = s.d[(sy * s.w + sx) as usize];
                let a = (p >> 24) as f32 * alpha;
                if a >= 1.0 {
                    self.put(dx, dy, (p & 0xFFFFFF) | ((a as u32) << 24));
                }
            }
        }
    }
    pub fn flipped(&self) -> Cv {
        let mut o = Cv::new(self.w, self.h);
        for y in 0..self.h {
            for x in 0..self.w {
                o.d[(y * self.w + x) as usize] = self.d[(y * self.w + (self.w - 1 - x)) as usize];
            }
        }
        o
    }
    /// Konturen i en mörkare variant av grannens färg ("selective outline")
    pub fn selout(mut self, k: f32) -> Cv {
        self.tx = 0;
        self.ty = 0;
        let (w, h) = (self.w, self.h);
        let src = self.d.clone();
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                if (src[i] >> 24) > 20 {
                    continue;
                }
                let mut best: Option<u32> = None;
                for (dx, dy) in [(0, 1), (0, -1), (1, 0), (-1, 0)] {
                    let (xx, yy) = (x + dx, y + dy);
                    if xx < 0 || yy < 0 || xx >= w || yy >= h {
                        continue;
                    }
                    let p = src[(yy * w + xx) as usize];
                    if (p >> 24) > 20 {
                        best = Some(p);
                        if dy == 1 {
                            break;
                        }
                    }
                }
                if let Some(p) = best {
                    let [r, g, b] = rgb_of(p);
                    self.d[i] = c(pack(r * k * 0.9, g * k * 0.92, b * k + 12.0));
                }
            }
        }
        self
    }
    /// Enfärgad siluett (för skuggor)
    pub fn silhouette(&self, col: u32) -> Cv {
        let mut o = Cv::new(self.w, self.h);
        for i in 0..self.d.len() {
            if self.d[i] >> 24 > 40 {
                o.d[i] = col;
            }
        }
        o
    }
}

// ------------------------------------------------------------------ Paletter

pub mod pal {
    pub const GRASS: [u32; 7] = [0x2c5a24, 0x3a7229, 0x4a8a2f, 0x5da236, 0x74b83f, 0x8fcc4d, 0xa8dc62];
    pub const DIRT: [u32; 6] = [0x7c5230, 0x95653a, 0xad7c47, 0xc49356, 0xd8aa68, 0xe6c081];
    pub const PAVING: [u32; 5] = [0x8e8a84, 0xa09b93, 0xb1aca3, 0xc1bcb2, 0xcfcac0];
    pub const ASPHALT: [u32; 5] = [0x34363d, 0x3d4048, 0x464a52, 0x50545d, 0x5a5e67];
    pub const CARPET: [u32; 6] = [0x4a5a6c, 0x536579, 0x5d7086, 0x687d93, 0x768ca1, 0x869cb0];
    pub const WOOD: [u32; 6] = [0x8a5028, 0xa3622f, 0xb8743a, 0xc98646, 0xd89a55, 0xe4ae6a];
    pub const RAISED: [u32; 5] = [0x9aa2ac, 0xaab2bb, 0xbac1c9, 0xc9cfd6, 0xd8dde2];
    pub const CONCRETE: [u32; 5] = [0x8b877f, 0x98948b, 0xa5a197, 0xb1ada3, 0xbdb9af];
    pub const TILE_A: [u32; 4] = [0xd9d0bc, 0xe3dbc8, 0xece5d4, 0xf4eee0];
    pub const TILE_B: [u32; 4] = [0x8fae93, 0x9dbb9f, 0xabc8ac, 0xb8d4b8];
    pub const WATER: [u32; 6] = [0x11485c, 0x155a6e, 0x1a6d80, 0x228393, 0x2c98a5, 0x3fb0b5];
    pub const SOIL: [u32; 4] = [0x4a2f1c, 0x5a3a23, 0x6b462b, 0x7c5334];
    pub const LEAF_GREEN: [u32; 7] = [0x1d4a1f, 0x285f25, 0x34772b, 0x438f33, 0x56a73c, 0x6fc04a, 0x8fd65e];
    pub const LEAF_PINK: [u32; 7] = [0x8f3a6e, 0xb04e88, 0xcc67a2, 0xe083ba, 0xee9fcc, 0xf7bddd, 0xfdd8ec];
    pub const LEAF_AUTUMN: [u32; 7] = [0x6b2c16, 0x8b3c1b, 0xad5422, 0xcc6e2a, 0xe08b36, 0xf0a947, 0xf9c766];
    pub const LEAF_PINE: [u32; 6] = [0x10301f, 0x163d27, 0x1e4d31, 0x285f3b, 0x347247, 0x448755];
    pub const BARK: [u32; 5] = [0x3b2415, 0x523220, 0x6a422a, 0x835536, 0x9b6a44];
    pub const ROCK: [u32; 6] = [0x55585f, 0x686b72, 0x7b7e85, 0x8f9299, 0xa4a7ad, 0xbabdc2];
}

// ------------------------------------------------------------------ Mark

pub const M_GRASS: u8 = 0;
pub const M_DIRT: u8 = 1;
pub const M_ASPHALT: u8 = 2;
pub const M_CARPET: u8 = 3;
pub const M_WOOD: u8 = 4;
pub const M_RAISED: u8 = 5;
pub const M_CONCRETE: u8 = 6;
pub const M_TILE: u8 = 7;
pub const M_PAVING: u8 = 8;
pub const M_WATER: u8 = 9;
pub const M_SOIL: u8 = 10;

/// Målar hela markbilden pixel för pixel utifrån materialkartan
pub fn paint_ground(map: &[u8], w: i32, h: i32, seed: i32) -> Cv {
    let mut out = Cv::new(w, h);
    let at = |x: i32, y: i32| -> i32 {
        if x < 0 || y < 0 || x >= w || y >= h {
            -1
        } else {
            map[(y * w + x) as usize] as i32
        }
    };
    for y in 0..h {
        for x in 0..w {
            let m = map[(y * w + x) as usize];
            let (xf, yf) = (x as f32, y as f32);
            let col: u32 = match m {
                M_GRASS => {
                    let v = fbm(xf / 70.0, yf / 55.0, seed) * 0.62
                        + hash2(x, y, seed) * 0.26
                        + if ((xf + yf * 0.3) / 26.0).floor() as i32 % 2 != 0 { 0.04 } else { -0.02 };
                    pick(&pal::GRASS, v, x, y, 1.2)
                }
                M_DIRT => {
                    let v = fbm(xf / 22.0, yf / 16.0, seed + 3) * 0.6 + hash2(x, y, seed + 1) * 0.35;
                    pick(&pal::DIRT, v, x, y, 1.1)
                }
                M_PAVING => {
                    let row = y.div_euclid(7);
                    let off = (row % 2) * 6;
                    let colm = (x + off).div_euclid(12);
                    let gx = (x + off).rem_euclid(12);
                    let gy = y.rem_euclid(7);
                    if gx == 0 || gy == 0 {
                        if hash2(x, y, seed + 4) < 0.12 {
                            pal::GRASS[2]
                        } else {
                            pal::PAVING[0]
                        }
                    } else {
                        let v = 0.35 + hash2(colm, row, seed + 9) * 0.45 + vnoise(xf / 3.0, yf / 3.0, seed) * 0.15
                            + if gy == 1 || gx == 1 { 0.15 } else { 0.0 }
                            - if gy == 6 { 0.12 } else { 0.0 };
                        pick(&pal::PAVING, v, x, y, 0.7)
                    }
                }
                M_ASPHALT => {
                    let v = fbm(xf / 30.0, yf / 25.0, seed + 5) * 0.45 + hash2(x, y, seed + 2) * 0.45;
                    pick(&pal::ASPHALT, v, x, y, 0.6)
                }
                M_CARPET => {
                    let px = (x + y).rem_euclid(8);
                    let py = (x - y).rem_euclid(8);
                    let ct = x.div_euclid(24) + y.div_euclid(20);
                    let v = 0.42 + fbm(xf / 60.0, yf / 50.0, seed + 11) * 0.2 + hash2(x, y, seed) * 0.1
                        + if px == 0 || py == 0 { -0.14 } else { 0.0 }
                        + if x % 8 == 4 && y % 8 == 4 { 0.18 } else { 0.0 }
                        + if ct % 2 != 0 { 0.06 } else { -0.02 };
                    pick(&pal::CARPET, v, x, y, 0.5)
                }
                M_WOOD => {
                    let plank = y.div_euclid(6);
                    let py2 = y.rem_euclid(6);
                    let shift = (hash2(plank, 0, seed) * 40.0).floor() as i32;
                    let seg = (x + shift).div_euclid(44);
                    let sx = (x + shift).rem_euclid(44);
                    if py2 == 5 || sx == 0 {
                        pal::WOOD[0]
                    } else {
                        let base = 0.25 + hash2(seg, plank, seed + 3) * 0.45;
                        let grain = vnoise(xf / 11.0, yf * 1.3, seed + 7) * 0.3;
                        let mut v = base + grain + if py2 == 0 { 0.12 } else { 0.0 } - if py2 == 4 { 0.08 } else { 0.0 };
                        if hash2(seg, plank, seed + 8) < 0.08 && (sx - 20).abs() < 2 && py2 == 2 {
                            v = 0.02;
                        }
                        pick(&pal::WOOD, v, x, y, 0.6)
                    }
                }
                M_RAISED => {
                    let tx = x.div_euclid(14);
                    let ty = y.div_euclid(12);
                    let ix = x.rem_euclid(14);
                    let iy = y.rem_euclid(12);
                    let vent = hash2(tx, ty, seed + 21) < 0.16;
                    let mut v = 0.55 + hash2(tx, ty, seed) * 0.15;
                    if ix == 0 || iy == 0 {
                        v = 0.05;
                    } else if ix == 1 || iy == 1 {
                        v = 0.95;
                    } else if ix == 13 || iy == 11 {
                        v = 0.18;
                    }
                    if vent && ix > 2 && ix < 12 && iy > 2 && iy < 10 && ix % 2 == 0 && iy % 2 == 0 {
                        v = 0.02;
                    }
                    pick(&pal::RAISED, v, x, y, 0.3)
                }
                M_CONCRETE => {
                    let mut v = fbm(xf / 40.0, yf / 34.0, seed + 17) * 0.5 + hash2(x, y, seed + 3) * 0.35;
                    let stain = fbm(xf / 90.0, yf / 80.0, seed + 31);
                    if stain > 0.66 {
                        v -= (stain - 0.66) * 1.6;
                    }
                    if x % 96 == 0 || y % 82 == 0 {
                        v = 0.02;
                    } else if x % 96 == 1 || y % 82 == 1 {
                        v += 0.25;
                    }
                    pick(&pal::CONCRETE, v, x, y, 0.8)
                }
                M_TILE => {
                    let tx2 = x.div_euclid(10);
                    let ty2 = y.div_euclid(9);
                    if x % 10 == 0 || y % 9 == 0 {
                        0xa89e8c
                    } else {
                        let p: &[u32] = if (tx2 + ty2).rem_euclid(2) != 0 { &pal::TILE_B } else { &pal::TILE_A };
                        let v = 0.45 + hash2(tx2, ty2, seed) * 0.2
                            + if x % 10 == 1 || y % 9 == 1 { 0.35 } else { 0.0 }
                            + if x % 10 == 2 && y % 9 == 2 { 0.3 } else { 0.0 };
                        pick(p, v, x, y, 0.4)
                    }
                }
                M_WATER => {
                    let mut v = 0.35 + fbm(xf / 20.0, yf / 10.0, seed + 41) * 0.35;
                    for k in 1..=4 {
                        if at(x, y - k) != M_WATER as i32 {
                            v -= (5 - k) as f32 * 0.09;
                            break;
                        }
                    }
                    for k in 1..=3 {
                        if at(x, y + k) != M_WATER as i32 {
                            v += (4 - k) as f32 * 0.1;
                            break;
                        }
                    }
                    pick(&pal::WATER, v, x, y, 0.8)
                }
                M_SOIL => {
                    let v = fbm(xf / 8.0, yf / 6.0, seed + 51) * 0.5 + hash2(x, y, seed) * 0.4;
                    pick(&pal::SOIL, v, x, y, 1.0)
                }
                _ => 0x505050,
            };
            let mut col = col;
            // Grässtrån som hänger ut över andra material utomhus
            if matches!(m, M_DIRT | M_ASPHALT | M_PAVING | M_WATER | M_SOIL) {
                for k3 in 1..=3 {
                    if at(x, y - k3) == M_GRASS as i32 && hash2(x, y + k3 * 7, seed + 77) < (0.75 - k3 as f32 * 0.22) {
                        col = if k3 == 1 { pal::GRASS[3] } else { pal::GRASS[1] };
                        break;
                    }
                }
                if at(x, y + 1) == M_GRASS as i32 && hash2(x, y, seed + 78) < 0.45 {
                    col = pal::GRASS[2];
                }
            }
            if m == M_ASPHALT && (at(x, y - 1) == M_GRASS as i32 || at(x, y - 2) == M_GRASS as i32) {
                col = if at(x, y - 1) == M_GRASS as i32 { 0xc8c4bc } else { 0x787674 };
            }
            out.d[(y * w + x) as usize] = c(col);
        }
    }
    out
}

/// Detaljer som strös ut på marken efter pixelmålningen (tuvor, blommor, stenar, grus, sprickor)
pub fn ground_details(g: &mut Cv, map: &[u8], w: i32, h: i32, seed: i32) {
    let mut r = Rng::new((seed * 31 + 7) as u32);
    let at = |x: f32, y: f32| -> i32 {
        let (x, y) = (x as i32, y as i32);
        if x < 0 || y < 0 || x >= w || y >= h {
            -1
        } else {
            map[(y * w + x) as usize] as i32
        }
    };
    let n = (w * h) as f32;
    let px = |g: &mut Cv, x: f32, y: f32, col: u32, ww: i32, hh: i32| g.rect(x as i32, y as i32, ww, hh, col);
    let grass = M_GRASS as i32;
    // Grästuvor
    for _ in 0..(n / 70.0) as i32 {
        let (x, y) = (r.next() * w as f32, r.next() * h as f32);
        if at(x, y) != grass {
            continue;
        }
        let dk = c(pal::GRASS[1 + r.idx(2)]);
        let lt = c(pal::GRASS[4 + r.idx(3)]);
        px(g, x, y - 2.0, dk, 1, 3);
        px(g, x - 1.0, y - 1.0, dk, 1, 2);
        px(g, x + 1.0, y - 1.0, dk, 1, 2);
        px(g, x, y - 3.0, lt, 1, 1);
        px(g, x + 2.0, y - 2.0, lt, 1, 1);
    }
    // Högt gräs i klungor
    for _ in 0..(n / 900.0) as i32 {
        let (x, y) = (r.next() * w as f32, r.next() * h as f32);
        if at(x, y) != grass || at(x, y + 3.0) != grass {
            continue;
        }
        for _ in 0..7 {
            let bx = x + (r.next() - 0.5) * 8.0;
            let hh = 3.0 + (r.next() * 4.0).floor();
            px(g, bx, y - hh, c(pal::GRASS[1]), 1, hh as i32);
            let o = if r.next() < 0.5 { -1.0 } else { 1.0 };
            px(g, bx + o, y - hh - 1.0, c(pal::GRASS[5]), 1, 1);
        }
    }
    // Blommor i små klungor
    const FL: [[u32; 2]; 6] = [[0xffffff, 0xffd23f], [0xffe14a, 0xe08a10], [0x6fa8ff, 0xffffff], [0xff8ec7, 0xfff1a8], [0xc89bff, 0xffe76a], [0xff6b5a, 0xffd23f]];
    for _ in 0..(n / 520.0) as i32 {
        let (x, y) = (r.next() * w as f32, r.next() * h as f32);
        if at(x, y) != grass {
            continue;
        }
        let f = FL[r.idx(6)];
        let cnt = 2 + (r.next() * 5.0) as i32;
        for _ in 0..cnt {
            let fx = x + (r.next() - 0.5) * 10.0;
            let fy = y + (r.next() - 0.5) * 6.0;
            if at(fx, fy) != grass {
                continue;
            }
            px(g, fx, fy + 1.0, c(pal::GRASS[1]), 1, 2);
            if r.next() < 0.5 {
                for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
                    px(g, fx + dx, fy + dy, c(f[0]), 1, 1);
                }
                px(g, fx, fy, c(f[1]), 1, 1);
            } else {
                px(g, fx, fy, c(f[0]), 2, 2);
                px(g, fx, fy, c(f[1]), 1, 1);
            }
        }
    }
    // Stenar
    for _ in 0..(n / 1800.0) as i32 {
        let (x, y) = (r.next() * w as f32, r.next() * h as f32);
        let mm = at(x, y);
        if mm != grass && mm != M_DIRT as i32 {
            continue;
        }
        let sw = 2 + (r.next() * 3.0) as i32;
        px(g, x, y + 1.0, ca(0x141e14, 0.35), sw + 1, 1);
        px(g, x, y, c(pal::ROCK[2]), sw, 2);
        px(g, x, y, c(pal::ROCK[4]), sw - 1, 1);
        px(g, x + sw as f32 - 1.0, y + 1.0, c(pal::ROCK[1]), 1, 1);
    }
    // Småsten i grusgångarna
    for _ in 0..(n / 60.0) as i32 {
        let (x, y) = (r.next() * w as f32, r.next() * h as f32);
        if at(x, y) != M_DIRT as i32 {
            continue;
        }
        px(g, x, y, c(pal::DIRT[5]), 1, 1);
        px(g, x + 1.0, y + 1.0, c(pal::DIRT[1]), 1, 1);
    }
    // Sprickor och oljefläckar i asfalten
    for _ in 0..(n / 5000.0) as i32 {
        let (x, y) = (r.next() * w as f32, r.next() * h as f32);
        if at(x, y) != M_ASPHALT as i32 {
            continue;
        }
        if r.next() < 0.6 {
            let (mut cx, mut cy) = (x, y);
            for _ in 0..14 {
                px(g, cx, cy, ca(0x141418, 0.8), 1, 1);
                if r.next() < 0.5 {
                    cx += 1.0;
                }
                if r.next() < 0.4 {
                    cy += 1.0;
                } else if r.next() < 0.2 {
                    cy -= 1.0;
                }
                if at(cx, cy) != M_ASPHALT as i32 {
                    break;
                }
            }
        } else {
            g.ellipse(x, y, 5.0 + r.next() * 4.0, 2.0 + r.next() * 2.0, ca(0x0a0a14, 0.28));
            px(g, x - 2.0, y - 1.0, ca(0x785aa0, 0.25), 2, 1);
            px(g, x + 1.0, y, ca(0x5aa08c, 0.2), 2, 1);
        }
    }
    // Hjulspår på betongen i lagret
    for _ in 0..(n / 7000.0) as i32 {
        let (x, y) = (r.next() * w as f32, r.next() * h as f32);
        if at(x, y) != M_CONCRETE as i32 {
            continue;
        }
        let l1 = 30 + (r.next() * 40.0) as i32;
        let l2 = 30 + (r.next() * 40.0) as i32;
        px(g, x, y, ca(0x28241e, 0.12), l1, 1);
        px(g, x, y + 5.0, ca(0x28241e, 0.12), l2, 1);
    }
}

// ------------------------------------------------------------------ Lådor och föremål

/// Skuggad låda i 3/4-vy: ljus från vänster uppifrån. g har redan tx=ty=1 satt av anroparen.
pub fn shade_box(g: &mut Cv, w: i32, dp: i32, hp: i32, top: u32, front: u32) {
    let t = rgb_of(top);
    let f = rgb_of(front);
    for y in 0..dp {
        for x in 0..w {
            let v = 0.5 + hash2(x, y, 3) * 0.12 + if y == dp - 1 { 0.2 } else { 0.0 } + if x == 0 { 0.08 } else { 0.0 }
                - if x == w - 1 { 0.12 } else { 0.0 }
                + bayer(x, y) * 0.06;
            let target = if v > 0.5 { [255., 250., 225.] } else { [20., 24., 60.] };
            let m = mix(t, target, (v - 0.5).abs() * 0.9);
            g.set(x, y, c(pack(m[0], m[1], m[2])));
        }
    }
    for yy in 0..hp {
        let k = yy as f32 / (hp - 1).max(1) as f32;
        for xx in 0..w {
            let vv = 0.62 - k * 0.3 + bayer(xx, yy) * 0.08 + if xx == 0 { 0.08 } else { 0.0 } - if xx == w - 1 { 0.14 } else { 0.0 }
                + if yy == 0 { 0.18 } else { 0.0 }
                - if yy >= hp - 2 { 0.18 } else { 0.0 };
            let target = if vv > 0.5 { [255., 248., 220.] } else { [16., 20., 55.] };
            let m = mix(f, target, (vv - 0.5).abs() * 0.85);
            g.set(xx, dp + yy, c(pack(m[0], m[1], m[2])));
        }
    }
    g.clear_rect(0, 0, 1, 1);
    g.clear_rect(w - 1, 0, 1, 1);
}

pub const PPM: f32 = 24.0;
pub const ZS: f32 = 0.85;
pub const HS: f32 = 0.75;

/// Alternativ för box_sprite
pub struct BoxOpts<'a> {
    pub stripes: bool,
    pub drawers: bool,
    pub no_outline: bool,
    pub f: Option<&'a dyn Fn(&mut Cv, i32, i32, i32)>,
}
impl<'a> Default for BoxOpts<'a> {
    fn default() -> Self {
        BoxOpts { stripes: false, drawers: false, no_outline: false, f: None }
    }
}

/// Ritar en låda i 3/4-vy. Ankaret är lådans främre vänstra hörn vid golvet (nedre vänstra hörnet i bilden, minus 1 px marginal).
pub fn box_sprite(w: f32, d: f32, h: f32, top: u32, front: u32, o: &BoxOpts) -> Cv {
    let wp = ((w * PPM).round() as i32).max(2);
    let dp = ((d * PPM * ZS).round() as i32).max(1);
    let hp = ((h * PPM * HS).round() as i32).max(1);
    let mut cv = Cv::new(wp + 2, dp + hp + 2);
    cv.tx = 1;
    cv.ty = 1;
    shade_box(&mut cv, wp, dp, hp, top, front);
    if o.stripes {
        let mut x = 3;
        while x < wp {
            cv.rect(x, dp + 1, 1, hp - 2, ca(shade(front, -0.1), 1.0));
            x += 5;
        }
    }
    if o.drawers {
        let mut y = dp + 3;
        while y < dp + hp - 2 {
            cv.rect(2, y, wp - 4, 1, c(shade(front, -0.2)));
            cv.rect(wp / 2 - 2, y + 2, 4, 1, c(0x6b6b6b));
            y += 6;
        }
    }
    if let Some(f) = o.f {
        f(&mut cv, wp, dp, hp);
    }
    if o.no_outline {
        cv.tx = 0;
        cv.ty = 0;
        return cv;
    }
    cv.selout(0.18)
}

// ------------------------------------------------------------------ Vegetation

/// Trädkrona av bollar som skuggas mot ljuset uppe till vänster
pub fn canopy(g: &mut Cv, blobs: &[[f32; 3]], palette: &[u32], seed: i32) -> Vec<(i32, i32)> {
    let mut r = Rng::new(seed as u32);
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (1e9f32, 1e9f32, -1e9f32, -1e9f32);
    for b in blobs {
        min_x = min_x.min(b[0] - b[2]);
        max_x = max_x.max(b[0] + b[2]);
        min_y = min_y.min(b[1] - b[2]);
        max_y = max_y.max(b[1] + b[2]);
    }
    let mut pts = vec![];
    for y in (min_y.floor() as i32)..=(max_y as i32) {
        for x in (min_x.floor() as i32)..=(max_x as i32) {
            let mut best: i32 = -1;
            let mut bv = 0.0f32;
            for (i, b) in blobs.iter().enumerate() {
                let dx = (x as f32 - b[0]) / b[2];
                let dy = (y as f32 - b[1]) / b[2];
                let dd = dx * dx + dy * dy;
                let edge = 1.0 - (vnoise(x as f32 / 3.2, y as f32 / 3.2, seed + i as i32) - 0.5) * 0.35;
                if dd <= edge {
                    let light = (-dx * 0.55 - dy * 0.75) * 0.5 + 0.5 - dd * 0.25 + (i as f32 / blobs.len() as f32) * 0.08;
                    if best < 0 || b[1] > blobs[best as usize][1] - 2.0 || light > bv {
                        best = i as i32;
                        bv = light;
                    }
                }
            }
            if best < 0 {
                continue;
            }
            let leaf = vnoise(x as f32 / 2.2, y as f32 / 2.2, seed + 99) * 0.28;
            let v = bv * 0.78 + leaf;
            g.set(x, y, c(pick(palette, v, x, y, 1.3)));
            pts.push((x, y));
        }
    }
    if pts.is_empty() {
        return pts;
    }
    for _ in 0..(pts.len() / 14) {
        let q = pts[r.idx(pts.len())];
        let up = (q.1 as f32) < (min_y + max_y) / 2.0 && (q.0 as f32) < (min_x + max_x) / 2.0 + 4.0;
        let col = palette[if up { palette.len() - 1 } else if r.next() < 0.5 { 0 } else { 1 }];
        g.rect(q.0, q.1, if up { 2 } else { 1 }, 1, c(col));
    }
    pts
}

fn trunk(g: &mut Cv, x: i32, y: i32, w: i32, h: i32, seed: i32) {
    for yy in 0..h {
        for xx in 0..w {
            let v = 0.25 + (xx as f32 / w as f32) * -0.2 + if xx == 1 { 0.35 } else { 0.0 } + vnoise(xx as f32 / 1.5, (y + yy) as f32 / 5.0, seed) * 0.35
                + if xx == w - 1 { -0.15 } else { 0.0 };
            g.set(x + xx, y + yy, c(pick(&pal::BARK, v, xx, yy, 0.8)));
        }
    }
    g.rect(x - 2, y + h - 2, 2, 2, c(pal::BARK[1]));
    g.rect(x + w, y + h - 2, 2, 2, c(pal::BARK[1]));
    g.set(x - 2, y + h - 2, c(pal::BARK[3]));
    g.set(x + w, y + h - 2, c(pal::BARK[3]));
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TreeKind {
    Oak,
    Pine,
    Cherry,
    Autumn,
}

/// Träd (ankare: stammens fot, mitt i bilden på x = 36 resp. 24)
pub fn tree(kind: TreeKind, seed: i32, lean: f32) -> Cv {
    let mut r = Rng::new((seed * 97 + 3) as u32);
    if kind == TreeKind::Pine {
        let mut g = Cv::new(48, 86);
        trunk(&mut g, 21, 64, 6, 18, seed);
        for t in 0..5 {
            let ty = 8.0 + t as f32 * 12.0;
            let tw = 8.0 + t as f32 * 4.5;
            let mut blobs = vec![];
            for k in 0..5 {
                blobs.push([24.0 + (k as f32 - 2.0) * tw * 0.36 + lean * (5 - t) as f32 / 5.0, ty + 6.0 + (k as f32 - 2.0).abs() * 2.0, tw * 0.42 + r.next() * 2.0]);
            }
            canopy(&mut g, &blobs, &pal::LEAF_PINE, seed + t * 11);
        }
        return g.selout(0.3);
    }
    let mut g = Cv::new(72, 92);
    trunk(&mut g, 32, 50, 8, 38, seed);
    g.rect(26, 50, 6, 2, c(pal::BARK[2]));
    g.rect(40, 46, 7, 2, c(pal::BARK[2]));
    g.rect(24, 48, 2, 2, c(pal::BARK[2]));
    let palette: &[u32] = match kind {
        TreeKind::Cherry => &pal::LEAF_PINK,
        TreeKind::Autumn => &pal::LEAF_AUTUMN,
        _ => &pal::LEAF_GREEN,
    };
    let mut bl: Vec<[f32; 3]> = vec![[36., 34., 20.], [21., 38., 13.], [51., 38., 13.], [36., 20., 14.], [26., 25., 12.], [47., 25., 12.], [36., 44., 12.]];
    for b in bl.iter_mut() {
        b[0] += (r.next() - 0.5) * 4.0 + lean * if b[1] < 30.0 { 1.0 } else { 0.5 };
        b[1] += (r.next() - 0.5) * 3.0;
        b[2] += (r.next() - 0.5) * 3.0;
    }
    let pts = canopy(&mut g, &bl, palette, seed);
    if !pts.is_empty() {
        if kind == TreeKind::Cherry {
            for _ in 0..46 {
                let q = pts[r.idx(pts.len())];
                g.set(q.0, q.1, c(if r.next() < 0.5 { 0xfff0f7 } else { 0xffd1e8 }));
            }
            g.rect(30, 38, 1, 6, c(pal::BARK[1]));
            g.rect(43, 36, 1, 5, c(pal::BARK[1]));
        } else if kind != TreeKind::Autumn && seed % 3 == 1 {
            for _ in 0..9 {
                let q = pts[r.idx(pts.len())];
                g.rect(q.0, q.1, 2, 2, c(0xe8453c));
                g.set(q.0, q.1, c(0xffb0a0));
            }
        }
    }
    g.selout(0.32)
}

pub fn bush(seed: i32, flowers: Option<[u32; 2]>) -> Cv {
    let mut r = Rng::new((seed * 31 + 9) as u32);
    let mut g = Cv::new(34, 24);
    let pts = canopy(&mut g, &[[10., 14., 8.], [23., 14., 8.], [17., 9., 8.], [16., 16., 7.]], &pal::LEAF_GREEN, seed + 5);
    if let Some(fc) = flowers {
        if !pts.is_empty() {
            for _ in 0..12 {
                let q = pts[r.idx(pts.len())];
                g.rect(q.0, q.1, 2, 2, c(fc[0]));
                g.set(q.0, q.1, c(fc[1]));
            }
        }
    }
    g.selout(0.32)
}

/// Krukväxt i kontoret
pub fn plant(s: f32, seed: i32) -> Cv {
    let w = (24.0 * s).round() as i32;
    let h = (32.0 * s).round() as i32;
    let mut g = Cv::new(w, h);
    let px = (w as f32 / 2.0 - 5.0 * s).round() as i32;
    let pw = (10.0 * s).round() as i32;
    let ph = (9.0 * s).round() as i32;
    for y in 0..ph {
        for xx in 0..pw {
            let v = 0.6 - xx as f32 / pw as f32 * 0.35 - y as f32 / ph as f32 * 0.15 + if xx == 1 { 0.2 } else { 0.0 };
            let target = if v > 0.5 { [255., 230., 200.] } else { [60., 30., 40.] };
            let m = mix([214., 120., 80.], target, (v - 0.5).abs());
            g.set(px + xx, h - ph + y, c(pack(m[0], m[1], m[2])));
        }
    }
    g.rect(px, h - ph, pw, 1, c(0x5a3a23));
    g.rect(px - 1, h - ph + 1, pw + 2, 1, c(0xe8a080));
    let mut rr = Rng::new(seed as u32);
    let mut bl = vec![];
    for _ in 0..6 {
        bl.push([w as f32 / 2.0 + (rr.next() - 0.5) * 12.0 * s, h as f32 - ph as f32 - 6.0 * s - rr.next() * 9.0 * s, (3.0 + rr.next() * 3.0) * s]);
    }
    canopy(&mut g, &bl, &pal::LEAF_GREEN, seed);
    for _ in 0..4 {
        let lx = w as f32 / 2.0 + (rr.next() - 0.5) * 16.0 * s;
        let ly = (h - ph - 2) as f32;
        g.rect(lx as i32, ly as i32 - 3, 1, 3, c(pal::LEAF_GREEN[4]));
        g.rect(lx as i32 + if lx < w as f32 / 2.0 { -1 } else { 1 }, ly as i32 - 4, 1, 1, c(pal::LEAF_GREEN[4]));
    }
    g.selout(0.35)
}

pub fn lamp_post() -> Cv {
    let mut g = Cv::new(12, 58);
    g.rect(5, 12, 2, 42, c(0x2a2d33));
    g.rect(5, 12, 1, 42, c(0x4a4f58));
    g.rect(3, 52, 6, 4, c(0x1f2227));
    g.rect(3, 52, 6, 1, c(0x3a3e46));
    g.rect(2, 4, 8, 3, c(0x2a2d33));
    g.rect(3, 2, 6, 2, c(0x2a2d33));
    g.rect(5, 0, 2, 2, c(0x2a2d33));
    g.rect(3, 7, 6, 5, c(0xffe9a8));
    g.rect(4, 8, 2, 3, c(0xfff8dc));
    g.rect(2, 12, 8, 1, c(0x2a2d33));
    g.selout(0.4)
}

pub fn bench() -> Cv {
    let mut g = Cv::new(34, 18);
    g.rect(3, 8, 2, 9, c(0x2b2f36));
    g.rect(29, 8, 2, 9, c(0x2b2f36));
    for i in 0..3 {
        g.rect(1, i * 3, 32, 2, c(pal::WOOD[if i == 2 { 2 } else { 3 }]));
        g.rect(1, i * 3, 32, 1, c(pal::WOOD[5]));
    }
    g.rect(1, 10, 32, 3, c(pal::WOOD[2]));
    g.rect(1, 10, 32, 1, c(pal::WOOD[4]));
    g.selout(0.4)
}

pub fn bin() -> Cv {
    let mut g = Cv::new(12, 16);
    g.rect(1, 3, 10, 12, c(0x2f6f45));
    g.rect(1, 3, 3, 12, c(0x3f8a57));
    g.rect(9, 3, 2, 12, c(0x23533a));
    g.rect(0, 1, 12, 3, c(0x1b3a28));
    g.rect(0, 1, 12, 1, c(0x4f9f67));
    g.selout(0.45)
}

pub fn car(col: u32, text: bool) -> Cv {
    let mut g = Cv::new(50, 42);
    g.rect(3, 30, 8, 9, c(0x18181b));
    g.rect(39, 30, 8, 9, c(0x18181b));
    g.rect(4, 31, 2, 6, c(0x3a3a40));
    g.rect(40, 31, 2, 6, c(0x3a3a40));
    g.rect(3, 3, 44, 32, c(col));
    g.rect(3, 3, 44, 2, c(tone(col, 0.25)));
    g.rect(3, 3, 2, 30, c(tone(col, 0.12)));
    g.rect(45, 3, 2, 30, c(tone(col, -0.25)));
    g.rect(7, 5, 36, 5, c(0x23303c));
    g.rect(8, 5, 8, 1, ca(0xffffff, 0.5));
    g.rect(6, 10, 38, 7, c(tone(col, 0.18)));
    g.rect(6, 10, 38, 1, c(tone(col, 0.35)));
    for y in 0..8 {
        let m = mix([150., 205., 235.], [60., 100., 140.], y as f32 / 8.0);
        let o = if y < 2 { 1 } else { 0 };
        g.rect(6 + o, 17 + y, 38 - o * 2, 1, c(pack(m[0], m[1], m[2])));
    }
    for k in 0..6 {
        g.rect(12 + k, 18 + k, 3, 1, ca(0xffffff, 0.6));
    }
    g.rect(4, 25, 42, 6, c(col));
    g.rect(4, 25, 42, 1, c(tone(col, 0.3)));
    g.rect(22, 26, 6, 4, c(tone(col, 0.1)));
    g.rect(3, 31, 44, 5, c(tone(col, -0.35)));
    g.rect(5, 32, 7, 3, c(0xfff2b8));
    g.rect(38, 32, 7, 3, c(0xfff2b8));
    g.rect(6, 32, 3, 1, c(0xffffff));
    g.rect(39, 32, 3, 1, c(0xffffff));
    g.rect(14, 33, 22, 2, c(0x1d1f24));
    g.rect(19, 36, 12, 4, c(0xf4f4f0));
    g.rect(20, 37, 10, 1, c(0x3a3a50));
    g.rect(1, 18, 3, 3, c(col));
    g.rect(46, 18, 3, 3, c(tone(col, -0.2)));
    if text {
        g.rect(13, 13, 24, 3, c(0xffffff));
        g.rect(15, 14, 20, 1, c(col));
    }
    g.selout(0.35)
}

pub fn truck() -> Cv {
    let mut g = Cv::new(40, 48);
    g.rect(4, 40, 8, 7, c(0x18181b));
    g.rect(28, 40, 8, 7, c(0x18181b));
    g.rect(2, 18, 36, 24, c(0xf0b83a));
    g.rect(2, 18, 36, 2, c(0xffd978));
    g.rect(2, 18, 2, 24, c(0xffd060));
    g.rect(36, 18, 2, 24, c(0xc48a1a));
    g.rect(10, 22, 20, 10, c(0x2b2d31));
    g.rect(12, 24, 16, 5, c(0x4a4d55));
    g.rect(6, 2, 2, 20, c(0x3a3d44));
    g.rect(32, 2, 2, 20, c(0x3a3d44));
    g.rect(6, 2, 28, 2, c(0x3a3d44));
    g.rect(6, 2, 28, 1, c(0x5a5e66));
    g.rect(18, 0, 4, 2, c(0xff9a1a));
    g.rect(19, 0, 2, 1, c(0xffe0a0));
    g.rect(2, 42, 36, 3, c(0x2b2d31));
    g.rect(8, 45, 3, 3, c(0x8f949c));
    g.rect(29, 45, 3, 3, c(0x8f949c));
    g.rect(4, 34, 32, 2, c(0x1d1f24));
    g.rect(14, 36, 12, 3, c(0x1d1f24));
    for i in 0..6 {
        g.rect(4 + i * 6, 34, 3, 2, c(if i % 2 == 1 { 0x1d1f24 } else { 0xffd23f }));
    }
    g.selout(0.35)
}

/// Kontorsstol
pub fn chair(col: u32) -> Cv {
    let mut g = Cv::new(16, 20);
    g.rect(3, 1, 10, 8, c(col));
    g.rect(2, 2, 12, 6, c(col));
    g.rect(3, 1, 10, 1, c(tone(col, 0.35)));
    g.rect(3, 2, 2, 5, c(tone(col, 0.18)));
    g.rect(12, 2, 2, 6, c(tone(col, -0.3)));
    g.rect(6, 3, 4, 1, c(tone(col, 0.1)));
    g.rect(2, 9, 12, 4, c(tone(col, -0.12)));
    g.rect(2, 9, 12, 1, c(tone(col, 0.2)));
    g.rect(12, 10, 2, 3, c(tone(col, -0.35)));
    g.rect(7, 13, 2, 3, c(0x6a6f78));
    g.rect(7, 13, 1, 3, c(0x9aa0a8));
    g.rect(2, 16, 12, 1, c(0x2b2d31));
    g.rect(4, 17, 8, 1, c(0x2b2d31));
    g.rect(1, 17, 2, 2, c(0x18181b));
    g.rect(13, 17, 2, 2, c(0x18181b));
    g.rect(7, 18, 2, 2, c(0x18181b));
    g.selout(0.35)
}

// ------------------------------------------------------------------ Figurer

#[derive(Clone, Debug, Default)]
pub struct Look {
    pub skin: u32,
    pub hair: u32,
    pub shirt: u32,
    pub pants: u32,
    pub long: bool,
    pub vest: bool,
    pub tie: bool,
    pub cap: Option<u32>,
    pub glasses: bool,
    pub beard: bool,
    pub lanyard: bool,
    pub headset: bool,
}

pub const FW: i32 = 18;
pub const FH: i32 = 31;

/// Figur med tre toner per färg, markerad ljusriktning, ögon med vitor och selout-kontur.
/// dir: 0 = mot oss, 1 = bort, 2 = vänster, 3 = höger. frame 0–3 = gång, 4 = andning.
pub fn person(look: &Look, dir: u8, frame: u8, sitting: bool, blink: bool) -> Cv {
    let mut g = Cv::new(FW + 2, FH + 2);
    g.tx = 1;
    g.ty = 1;
    let s = look.skin;
    let sd = tone(s, -0.2);
    let sl = tone(s, 0.18);
    let hh_ = look.hair;
    let hd = tone(hh_, -0.3);
    let hl = tone(hh_, 0.3);
    let sh = look.shirt;
    let shd = tone(sh, -0.25);
    let shl = tone(sh, 0.22);
    let pa = look.pants;
    let pad = tone(pa, -0.25);
    let pal_ = tone(pa, 0.15);
    let step = (frame % 5) as i32;
    let mut swing = if step == 1 { 1 } else if step == 3 { -1 } else { 0 };
    let mut bob = if step == 1 || step == 3 { 1 } else { 0 };
    if frame == 4 {
        bob = 1;
        swing = 0;
    }
    let oy = bob;
    let p = |g: &mut Cv, x: i32, y: i32, w: i32, h: i32, col: u32| g.rect(x, y, w, h, col);
    // Ben
    if !sitting {
        if dir == 2 || dir == 3 {
            p(&mut g, 7, 21 + oy, 4, 6 - oy, c(pa));
            p(&mut g, 7 + swing * 2, 21 + oy, 3, 6 - oy, c(pad));
            p(&mut g, 7, 21 + oy, 1, 5, c(pal_));
            p(&mut g, 6 + swing * 2, 27, 5, 2, c(0x2a2220));
            p(&mut g, 7 - swing, 27, 4, 2, c(0x3a302b));
            p(&mut g, 6 + swing * 2, 27, 2, 1, c(0x5a4a40));
        } else {
            p(&mut g, 5, 21 + oy, 4, 6 - oy - if swing > 0 { 1 } else { 0 }, c(pa));
            p(&mut g, 10, 21 + oy, 4, 6 - oy - if swing < 0 { 1 } else { 0 }, c(pa));
            p(&mut g, 5, 21 + oy, 1, 5, c(pal_));
            p(&mut g, 13, 21 + oy, 1, 5, c(pad));
            p(&mut g, 9, 21 + oy, 1, 3, c(pad));
            p(&mut g, 4, 27 - if swing > 0 { 1 } else { 0 }, 5, 2, c(0x2a2220));
            p(&mut g, 10, 27 - if swing < 0 { 1 } else { 0 }, 5, 2, c(0x2a2220));
            p(&mut g, 4, 27 - if swing > 0 { 1 } else { 0 }, 2, 1, c(0x5a4a40));
            p(&mut g, 10, 27 - if swing < 0 { 1 } else { 0 }, 2, 1, c(0x5a4a40));
        }
    } else {
        p(&mut g, 5, 21, 9, 3, c(pa));
        p(&mut g, 5, 21, 9, 1, c(pal_));
    }
    // Kropp
    p(&mut g, 4, 12 + oy, 11, 10, c(sh));
    p(&mut g, 4, 12 + oy, 11, 1, c(shl));
    p(&mut g, 4, 13 + oy, 2, 7, c(shl));
    p(&mut g, 12, 13 + oy, 3, 9, c(shd));
    p(&mut g, 4, 20 + oy, 11, 2, c(shd));
    p(&mut g, 8, 12 + oy, 3, 2, c(sd));
    if look.vest {
        p(&mut g, 4, 13 + oy, 11, 8, c(0xd7ff3c));
        p(&mut g, 4, 17 + oy, 11, 1, c(0xf4f4f4));
        p(&mut g, 12, 13 + oy, 3, 8, c(0xb4d82c));
        p(&mut g, 9, 13 + oy, 1, 8, c(sh));
    }
    if look.tie {
        p(&mut g, 9, 14 + oy, 1, 6, c(0xc0392b));
        p(&mut g, 9, 14 + oy, 1, 1, c(0xe05a4c));
    }
    if look.lanyard && dir == 0 {
        p(&mut g, 7, 13 + oy, 1, 4, c(0x2a6db8));
        p(&mut g, 11, 13 + oy, 1, 4, c(0x2a6db8));
        p(&mut g, 8, 17 + oy, 3, 3, c(0xf4f4f2));
        p(&mut g, 8, 17 + oy, 3, 1, c(0x2a6db8));
    }
    // Armar
    if dir == 2 {
        p(&mut g, 6 + swing, 13 + oy, 3, 7, c(shd));
        p(&mut g, 6 + swing, 13 + oy, 1, 7, c(sh));
        p(&mut g, 6 + swing, 20 + oy, 3, 2, c(s));
    } else if dir == 3 {
        p(&mut g, 10 - swing, 13 + oy, 3, 7, c(shd));
        p(&mut g, 12 - swing, 13 + oy, 1, 7, c(sh));
        p(&mut g, 10 - swing, 20 + oy, 3, 2, c(s));
    } else {
        let a = if sitting { 0 } else { swing };
        p(&mut g, 2, 13 + oy + a, 2, 7, c(sh));
        p(&mut g, 2, 13 + oy + a, 1, 7, c(shl));
        p(&mut g, 15, 13 + oy - a, 2, 7, c(shd));
        p(&mut g, 2, 20 + oy + a, 2, 2, c(s));
        p(&mut g, 15, 20 + oy - a, 2, 2, c(sd));
    }
    // Huvud
    let hy = oy;
    p(&mut g, 5, 2 + hy, 9, 10, c(s));
    p(&mut g, 5, 2 + hy, 2, 9, c(sl));
    p(&mut g, 12, 3 + hy, 2, 8, c(sd));
    p(&mut g, 5, 10 + hy, 9, 2, c(sd));
    if dir == 1 {
        p(&mut g, 4, 1 + hy, 11, 10, c(hh_));
        p(&mut g, 4, 1 + hy, 11, 2, c(hl));
        p(&mut g, 12, 3 + hy, 3, 8, c(hd));
        if look.long {
            p(&mut g, 4, 10 + hy, 11, 5, c(hh_));
            p(&mut g, 12, 10 + hy, 3, 5, c(hd));
        }
    } else {
        p(&mut g, 4, hy, 11, 4, c(hh_));
        p(&mut g, 5, hy, 5, 1, c(hl));
        p(&mut g, 6, 1 + hy, 3, 1, c(hl));
        p(&mut g, 4, 3 + hy, 2, 4, c(hh_));
        p(&mut g, 13, 3 + hy, 2, 4, c(hd));
        if look.long {
            p(&mut g, 3, 4 + hy, 2, 11, c(hh_));
            p(&mut g, 14, 4 + hy, 2, 11, c(hd));
            p(&mut g, 3, 4 + hy, 1, 9, c(hl));
        }
        if dir == 0 {
            if blink {
                p(&mut g, 6, 7 + hy, 2, 1, c(0x2a1a12));
                p(&mut g, 10, 7 + hy, 2, 1, c(0x2a1a12));
            } else {
                p(&mut g, 6, 6 + hy, 2, 2, c(0xfbfbf6));
                p(&mut g, 10, 6 + hy, 2, 2, c(0xfbfbf6));
                p(&mut g, 7, 6 + hy, 1, 2, c(0x1d1410));
                p(&mut g, 10, 6 + hy, 1, 2, c(0x1d1410));
            }
            p(&mut g, 6, 5 + hy, 2, 1, c(hd));
            p(&mut g, 10, 5 + hy, 2, 1, c(hd));
            p(&mut g, 5, 9 + hy, 1, 1, ca(0xe66e6e, 0.55));
            p(&mut g, 12, 9 + hy, 1, 1, ca(0xe66e6e, 0.55));
            p(&mut g, 8, 10 + hy, 3, 1, c(tone(s, -0.35)));
            p(&mut g, 9, 8 + hy, 1, 1, c(sd));
        } else if dir == 2 {
            if !blink {
                p(&mut g, 5, 6 + hy, 2, 2, c(0xfbfbf6));
                p(&mut g, 5, 6 + hy, 1, 2, c(0x1d1410));
            } else {
                p(&mut g, 5, 7 + hy, 2, 1, c(0x2a1a12));
            }
            p(&mut g, 4, 7 + hy, 1, 2, c(s));
            p(&mut g, 10, hy, 5, 9, c(hh_));
            p(&mut g, 13, 1 + hy, 2, 8, c(hd));
            p(&mut g, 10, hy, 3, 1, c(hl));
        } else {
            if !blink {
                p(&mut g, 12, 6 + hy, 2, 2, c(0xfbfbf6));
                p(&mut g, 13, 6 + hy, 1, 2, c(0x1d1410));
            } else {
                p(&mut g, 12, 7 + hy, 2, 1, c(0x2a1a12));
            }
            p(&mut g, 14, 7 + hy, 1, 2, c(s));
            p(&mut g, 4, hy, 5, 9, c(hh_));
            p(&mut g, 4, hy, 2, 9, c(hl));
        }
    }
    if let Some(cap) = look.cap {
        p(&mut g, 4, -1 + hy, 11, 3, c(cap));
        p(&mut g, 4, -1 + hy, 11, 1, c(tone(cap, 0.25)));
        if dir == 0 {
            p(&mut g, 4, 2 + hy, 11, 1, c(tone(cap, -0.35)));
        }
        if dir == 2 {
            p(&mut g, 2, 2 + hy, 5, 1, c(tone(cap, -0.35)));
        }
        if dir == 3 {
            p(&mut g, 12, 2 + hy, 5, 1, c(tone(cap, -0.35)));
        }
    }
    if look.glasses && dir == 0 {
        p(&mut g, 5, 6 + hy, 4, 3, ca(0x14141e, 0.45));
        p(&mut g, 9, 6 + hy, 1, 1, c(0x222222));
        p(&mut g, 10, 6 + hy, 4, 3, ca(0x14141e, 0.45));
        p(&mut g, 6, 6 + hy, 1, 1, ca(0xffffff, 0.7));
        p(&mut g, 11, 6 + hy, 1, 1, ca(0xffffff, 0.7));
    }
    if look.headset && dir == 0 {
        p(&mut g, 3, 5 + hy, 2, 4, c(0x222222));
        p(&mut g, 4, hy, 11, 1, c(0x333333));
        p(&mut g, 5, 9 + hy, 3, 1, c(0x333333));
    }
    if look.beard && dir != 1 {
        p(&mut g, 6, 9 + hy, 7, 3, c(hd));
        p(&mut g, 8, 10 + hy, 3, 1, c(tone(s, -0.35)));
    }
    g.selout(0.3)
}

/// Pratbubbla (mörk kontur, vit insida) med ! eller ✓. Ritas i konstpixlar.
pub fn bubble(done: bool) -> Cv {
    let mut g = Cv::new(16, 17);
    g.rect(1, 0, 14, 13, c(0x2b1a10));
    g.rect(2, 1, 12, 11, c(0xfffaf0));
    g.rect(7, 12, 3, 3, c(0x2b1a10));
    g.rect(8, 12, 1, 2, c(0xfffaf0));
    if !done {
        g.rect(7, 3, 2, 5, c(0xe8a317));
        g.rect(7, 9, 2, 2, c(0xe8a317));
    } else {
        g.rect(4, 6, 2, 2, c(0x3fae4f));
        g.rect(6, 8, 2, 2, c(0x3fae4f));
        g.rect(8, 6, 2, 2, c(0x3fae4f));
        g.rect(10, 4, 2, 2, c(0x3fae4f));
    }
    g
}

/// Liten pil (kompass) som pekar nedåt
pub fn arrow_down(col: u32) -> Cv {
    let mut g = Cv::new(11, 12);
    g.rect(4, 0, 3, 6, c(0x2b1a10));
    g.rect(1, 5, 9, 2, c(0x2b1a10));
    g.rect(2, 7, 7, 1, c(0x2b1a10));
    g.rect(3, 8, 5, 1, c(0x2b1a10));
    g.rect(4, 9, 3, 1, c(0x2b1a10));
    g.rect(5, 10, 1, 1, c(0x2b1a10));
    g.rect(5, 1, 1, 5, c(col));
    g.rect(2, 6, 7, 1, c(col));
    g.rect(3, 7, 5, 1, c(col));
    g.rect(4, 8, 3, 1, c(col));
    g.rect(5, 9, 1, 1, c(col));
    g
}

// ------------------------------------------------------------------ Djur och Krabban

const OUTL: u32 = 0x2b1a10;

/// Små former med mörk kontur runt varje: (x, y, b, h, färg)
fn blobs(w: i32, h: i32, list: &[(i32, i32, i32, i32, u32)]) -> Cv {
    let mut g = Cv::new(w, h);
    for &(x, y, bw, bh, _) in list {
        g.rect(x - 1, y - 1, bw + 2, bh + 2, c(OUTL));
    }
    for &(x, y, bw, bh, col) in list {
        g.rect(x, y, bw, bh, c(col));
    }
    g
}

/// Krabban (röd, ögon på skaft). Ankare: (11, 13) = marken under kroppen. frame 0/1 ger benrörelse.
pub fn crab_sprite(frame: u8, fleeing: bool) -> Cv {
    let mut g = Cv::new(22, 16);
    let (ox, oy) = (11, 13);
    let k = (frame % 2) as i32;
    let mut px = |x: i32, y: i32, w: i32, h: i32, col: u32| g.rect(ox + x, oy + y, w, h, c(col));
    // Ben
    for i in 0..3 {
        let o = (i + k) % 2;
        px(-8, -4 + (i as f32 * 1.5) as i32 - o, 3, 1, OUTL);
        px(5, -4 + (i as f32 * 1.5) as i32 - (1 - o), 3, 1, OUTL);
    }
    // Klor
    let lift = if fleeing { -2 } else { 0 };
    px(-9, -10 + lift, 4, 4, OUTL);
    px(-8, -9 + lift, 2, 2, 0xe84a2c);
    px(5, -10 + lift, 4, 4, OUTL);
    px(6, -9 + lift, 2, 2, 0xe84a2c);
    // Kropp
    px(-6, -8, 12, 6, OUTL);
    px(-5, -7, 10, 4, 0xe84a2c);
    px(-4, -7, 8, 1, 0xff7a55);
    px(-5, -4, 10, 1, 0xb82e18);
    // Ögon
    px(-3, -11, 2, 3, OUTL);
    px(1, -11, 2, 3, OUTL);
    px(-3, -11, 1, 1, 0xffffff);
    px(1, -11, 1, 1, 0xffffff);
    g
}

/// Katt som tittar åt höger. Ankare: (8, 11). state 0/1 = gång, 2 = sitter.
pub fn cat_sprite(state: u8) -> Cv {
    const O: u32 = 0xd98a3d;
    const D: u32 = 0xa8631f;
    const L: u32 = 0xf0c68a;
    let mut g = if state == 2 {
        blobs(16, 13, &[(3, 5, 6, 6, O), (8, 2, 5, 5, O), (8, 1, 1, 2, D), (12, 1, 1, 2, D), (1, 8, 3, 2, D), (4, 10, 4, 1, L)])
    } else {
        let a = (state % 2) as i32;
        blobs(
            18,
            13,
            &[(3, 4, 9, 4, O), (11, 2, 5, 4, O), (11, 1, 1, 2, D), (15, 1, 1, 2, D), (0, 1 + a, 2, 4, D), (3, 8, 2, 2 + a, D), (5, 8, 2, 2 + (1 - a), O), (9, 8, 2, 2 + (1 - a), D), (11, 8, 2, 2 + a, O)],
        )
    };
    // Ögon, ränder
    if state == 2 {
        g.rect(11, 4, 1, 1, c(0x203020));
        g.rect(4, 6, 1, 3, c(D));
        g.rect(6, 6, 1, 3, c(D));
    } else {
        g.rect(14, 3, 1, 1, c(0x203020));
        g.rect(5, 4, 1, 3, c(D));
        g.rect(8, 4, 1, 3, c(D));
    }
    g
}

/// Anka som tittar åt höger. Ankare: (7, 8).
pub fn duck_sprite() -> Cv {
    blobs(15, 10, &[(1, 3, 8, 4, 0xf2efe6), (0, 2, 2, 2, 0xf2efe6), (8, 0, 4, 4, 0x2f6b3a), (12, 2, 2, 2, 0xf0a020), (3, 4, 4, 2, 0xb9b2a0)])
}

/// Duva som tittar åt höger. Ankare: (5, 8). flying = vingarna upp.
pub fn pigeon_sprite(flying: bool) -> Cv {
    let mut g = if flying {
        blobs(12, 12, &[(2, 5, 7, 3, 0x9aa0a8), (8, 3, 3, 3, 0x6c727a), (1, 1, 3, 4, 0xb4bac2), (5, 0, 3, 4, 0xb4bac2)])
    } else {
        blobs(12, 10, &[(1, 3, 7, 4, 0x9aa0a8), (7, 1, 3, 3, 0x6c727a), (7, 3, 2, 2, 0x7a5a9a), (3, 7, 1, 2, 0xd98fa0), (5, 7, 1, 2, 0xd98fa0)])
    };
    g.rect(if flying { 10 } else { 10 }, 2, 1, 1, c(0xe8d8a0));
    g
}

/// Mås som tittar åt höger. Ankare: (7, 10).
pub fn gull_sprite() -> Cv {
    blobs(17, 12, &[(2, 4, 8, 4, 0xf4f4f0), (0, 4, 3, 2, 0xdfe3e6), (3, 4, 5, 2, 0x9aa6b0), (9, 1, 4, 4, 0xf4f4f0), (13, 3, 2, 1, 0xf0b020), (5, 8, 1, 3, 0xe08a30), (7, 8, 1, 3, 0xe08a30)])
}
