//! Krabbmotorn: en liten 2D-spelmotor för just det här spelet.
//! Mjukvarurendering till en pixelbuffert (0xRRGGBB), text med VT323 och Press Start 2P (fontdue),
//! fönster och inmatning via minifb, och ett huvudlöst läge som sparar bilder som PNG (för tester).
#![allow(unused_variables, dead_code)]

pub type Color = u32;

pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
    ((r as u32) << 16) | ((g as u32) << 8) | b as u32
}

/// En bild med alfakanal: 0xAARRGGBB (A = 0 är helt genomskinlig).
#[derive(Clone, Debug)]
pub struct Sprite {
    pub w: usize,
    pub h: usize,
    pub px: Vec<u32>,
}

impl Sprite {
    pub fn new(w: usize, h: usize) -> Sprite {
        Sprite { w, h, px: vec![0; w * h] }
    }
    /// Bygg en sprite från en teckenkarta: varje tecken slås upp i palett (tecken -> 0xAARRGGBB), '.' eller ' ' = genomskinligt
    pub fn from_map(rows: &[&str], palette: &[(char, u32)]) -> Sprite {
        todo!("Sprite::from_map")
    }
    pub fn set(&mut self, x: usize, y: usize, argb: u32) {
        if x < self.w && y < self.h {
            self.px[y * self.w + x] = argb;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FontKind {
    /// VT323 – terminaltext och brödtext
    Mono,
    /// Press Start 2P – rubriker och pixel-UI
    Pixel,
}

/// Pixelbufferten som allt ritas på. Koordinater i pixlar, (0,0) uppe till vänster.
pub struct Canvas {
    pub w: usize,
    pub h: usize,
    pub px: Vec<u32>,
    /// Klippruta (x0, y0, x1, y1), x1/y1 exklusiva
    pub clip: (i32, i32, i32, i32),
}

impl Canvas {
    pub fn new(w: usize, h: usize) -> Canvas {
        Canvas { w, h, px: vec![0; w * h], clip: (0, 0, w as i32, h as i32) }
    }
    pub fn clear(&mut self, c: Color) { todo!() }
    pub fn set_clip(&mut self, x: i32, y: i32, w: i32, h: i32) { todo!() }
    pub fn reset_clip(&mut self) { todo!() }
    pub fn pixel(&mut self, x: i32, y: i32, c: Color) { todo!() }
    /// Blandar in färgen med alfa 0–255
    pub fn pixel_a(&mut self, x: i32, y: i32, c: Color, a: u8) { todo!() }
    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Color) { todo!() }
    pub fn fill_rect_a(&mut self, x: i32, y: i32, w: i32, h: i32, c: Color, a: u8) { todo!() }
    /// Ram med tjocklek t
    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, t: i32, c: Color) { todo!() }
    /// Fylld rektangel med rundade hörn (radie r)
    pub fn round_rect(&mut self, x: i32, y: i32, w: i32, h: i32, r: i32, c: Color, a: u8) { todo!() }
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Color) { todo!() }
    pub fn thick_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, t: i32, c: Color) { todo!() }
    pub fn circle(&mut self, cx: i32, cy: i32, r: i32, c: Color, a: u8) { todo!() }
    pub fn ellipse(&mut self, cx: i32, cy: i32, rx: i32, ry: i32, c: Color, a: u8) { todo!() }
    /// Ritar en sprite med heltalsskala (pixlarna blir scale×scale), flip = spegla horisontellt
    pub fn blit(&mut self, s: &Sprite, x: i32, y: i32, scale: i32, flip: bool) { todo!() }
    /// Som blit men med extra genomskinlighet 0–255
    pub fn blit_a(&mut self, s: &Sprite, x: i32, y: i32, scale: i32, flip: bool, alpha: u8) { todo!() }
    /// Mörkar/tonar hela ytan mot färgen (för dialoger bakom)
    pub fn dim(&mut self, c: Color, a: u8) { todo!() }
}

/// Typsnitten med glyfcache per storlek.
pub struct Fonts {}

impl Fonts {
    /// Laddar VT323 och Press Start 2P (inbäddade med include_bytes! från assets/fonts)
    pub fn load() -> Fonts { todo!() }
    /// Ritar text och returnerar bredden i pixlar. size = pixelhöjd. Stöder svenska tecken (åäö) och de vanligaste symbolerna.
    pub fn text(&mut self, c: &mut Canvas, x: i32, y: i32, s: &str, size: f32, color: Color, kind: FontKind) -> i32 { todo!() }
    /// (bredd, höjd) i pixlar
    pub fn measure(&mut self, s: &str, size: f32, kind: FontKind) -> (i32, i32) { todo!() }
    /// Bredden på ett tecken i monospace (VT323) vid storleken
    pub fn mono_advance(&mut self, size: f32) -> f32 { todo!() }
    /// Radbryter text till maxbredd (ord för ord, behåller \n)
    pub fn wrap(&mut self, s: &str, size: f32, kind: FontKind, max_w: i32) -> Vec<String> { todo!() }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Up, Down, Left, Right,
    W, A, S, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, T, U, V, X, Y, Z, B, C,
    Num0, Num1, Num2, Num3, Num4, Num5, Num6, Num7, Num8, Num9,
    Enter, Escape, Tab, Backspace, Delete, Space, Home, End, PageUp, PageDown,
    Shift, Ctrl, Alt,
    F1, F2, F3, F4, F5, F11,
}

/// Inmatningen för en bildruta.
#[derive(Clone, Debug, Default)]
pub struct Input {
    /// Tangenter som hålls nere
    pub down: std::collections::HashSet<Key>,
    /// Tangenter som trycktes ned den här bildrutan (med tangentrepetition)
    pub pressed: Vec<Key>,
    /// Inskrivna tecken den här bildrutan (för textfält och terminalen), utan styrtecken
    pub chars: Vec<char>,
    pub mouse_x: i32,
    pub mouse_y: i32,
    pub mouse_down: bool,
    /// Vänsterklick den här bildrutan
    pub clicked: bool,
    pub right_clicked: bool,
    pub wheel: f32,
    pub ctrl: bool,
    pub shift: bool,
}

impl Input {
    pub fn is_down(&self, k: Key) -> bool { self.down.contains(&k) }
    pub fn was_pressed(&self, k: Key) -> bool { self.pressed.contains(&k) }
}

/// Spelet som motorn kör.
pub trait Game {
    /// Uppdatera med inmatning och tid (sekunder). Returnera false för att avsluta.
    fn update(&mut self, input: &Input, dt: f64) -> bool;
    fn draw(&mut self, c: &mut Canvas, fonts: &mut Fonts);
    /// Text att lägga i urklipp (sätts av spelet, töms av motorn)
    fn take_clipboard(&mut self) -> Option<String> { None }
}

/// Upplösningen som spelet ritar i. Fönstret skalar upp den.
pub const WIDTH: usize = 1280;
pub const HEIGHT: usize = 720;

/// Öppnar ett fönster och kör spelet med ~60 bilder per sekund.
pub fn run(title: &str, game: &mut dyn Game) { todo!() }

/// Huvudlöst: kör spelet med förinspelad inmatning (en Input per bildruta, dt = 1/60)
/// och sparar en PNG efter de bildrutor som anges i `shots` (bildrutans index, filnamn).
pub fn run_headless(game: &mut dyn Game, inputs: &[Input], shots: &[(usize, String)]) { todo!() }

pub fn save_png(c: &Canvas, path: &str) -> std::io::Result<()> { todo!() }
