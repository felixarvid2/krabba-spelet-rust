//! Ljud: en liten synth som gör samma ljud som originalets Web Audio-kod (NV.sfx i js/ui/settings.js):
//! toner med glidning och envelope, filtrerat brus, loopande miljöljud och lugn bakgrundsmusik.
//! Ljudet blandas i en global mixer som ljudutgången (cpal på datorn, Web Audio i webbläsaren) läser ur.
//!
//! Spelet anropar bara funktionerna längst ner: `sfx("success")`, `sfx_step("wood")`, `loop_level("hum", 0.6)`,
//! `set_settings(..)` och `unlock()`.
use std::sync::Mutex;

pub const RATE: f32 = 44100.0;

#[derive(Clone, Copy, PartialEq)]
enum Wave {
    Sine,
    Square,
    Triangle,
    Saw,
}

#[derive(Clone, Copy, PartialEq)]
enum Filt {
    Low,
    Band,
    High,
}

/// Biquad (RBJ) för brusljuden
#[derive(Clone, Copy, Default)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Biquad {
    fn set(&mut self, kind: Filt, freq: f32, q: f32) {
        let f = freq.clamp(20.0, RATE * 0.45);
        let w = 2.0 * std::f32::consts::PI * f / RATE;
        let (s, c) = w.sin_cos();
        let alpha = s / (2.0 * q.max(0.05));
        let (b0, b1, b2) = match kind {
            Filt::Low => ((1.0 - c) / 2.0, 1.0 - c, (1.0 - c) / 2.0),
            Filt::High => ((1.0 + c) / 2.0, -(1.0 + c), (1.0 + c) / 2.0),
            Filt::Band => (alpha, 0.0, -alpha),
        };
        let a0 = 1.0 + alpha;
        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = -2.0 * c / a0;
        self.a2 = (1.0 - alpha) / a0;
    }
    fn run(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2 - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

enum Src {
    Osc { wave: Wave, f0: f32, f1: Option<f32>, phase: f32 },
    Noise { filt: Biquad, kind: Filt, f0: f32, f1: Option<f32>, q: f32 },
}

struct Voice {
    src: Src,
    /// Start i sampel (fördröjning)
    delay: u32,
    /// Längd i sampel
    len: u32,
    t: u32,
    vol: f32,
    attack: f32,
    pan: f32,
    /// Mixerbuss: 0 = effekter, 1 = miljö, 2 = musik
    bus: u8,
    lin_attack: bool,
}

impl Voice {
    fn sample(&mut self, rng: &mut u32) -> Option<f32> {
        if self.delay > 0 {
            self.delay -= 1;
            return Some(0.0);
        }
        if self.t >= self.len {
            return None;
        }
        let p = self.t as f32 / self.len as f32;
        let ts = self.t as f32 / RATE;
        // Envelope: kort uppgång, exponentiell avklingning till ~0 vid slutet (som exponentialRampToValueAtTime)
        let env = if self.lin_attack {
            let a = (ts / 0.01).min(1.0);
            a * (0.0001f32).powf(p)
        } else if self.attack > 0.0 {
            let dur = self.len as f32 / RATE;
            if ts < self.attack {
                (0.0001f32).powf(1.0 - ts / self.attack)
            } else {
                (0.0001f32).powf((ts - self.attack) / (dur - self.attack).max(0.001))
            }
        } else {
            (0.0001f32).powf(p)
        };
        let v = match &mut self.src {
            Src::Osc { wave, f0, f1, phase } => {
                let f = match f1 {
                    Some(f1) => *f0 * (*f1 / *f0).powf(p),
                    None => *f0,
                };
                *phase = (*phase + f / RATE).fract();
                let ph = *phase;
                match wave {
                    Wave::Sine => (ph * std::f32::consts::TAU).sin(),
                    Wave::Square => {
                        if ph < 0.5 { 0.6 } else { -0.6 }
                    }
                    Wave::Triangle => 4.0 * (ph - 0.5).abs() - 1.0,
                    Wave::Saw => (2.0 * ph - 1.0) * 0.7,
                }
            }
            Src::Noise { filt, kind, f0, f1, q } => {
                if let Some(f1) = f1 {
                    if self.t % 64 == 0 {
                        let f = *f0 * (*f1 / *f0).powf(p);
                        filt.set(*kind, f, *q);
                    }
                }
                filt.run(white(rng))
            }
        };
        self.t += 1;
        Some(v * env * self.vol)
    }
}

fn white(rng: &mut u32) -> f32 {
    let mut x = *rng;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *rng = x;
    (x as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// Loopande filtrerat brus (fläktar, regn, kontorssorl, lagret)
struct Loop {
    name: &'static str,
    filt: Biquad,
    gain: f32,
    want: f32,
}

/// Inställningar som spelet styr (sparas i save.json)
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub sound: bool,
    /// Huvudvolym 0–1
    pub volume: f32,
    /// Ljud när man skriver i terminalen
    pub type_sound: bool,
    pub music: bool,
    pub music_vol: f32,
    /// Effekter 0–1
    pub fx: f32,
    /// Miljöljud 0–1
    pub ambience: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { sound: true, volume: 0.6, type_sound: true, music: false, music_vol: 0.5, fx: 1.0, ambience: 1.0 }
    }
}

struct Mixer {
    voices: Vec<Voice>,
    loops: Vec<Loop>,
    rng: u32,
    settings: Settings,
    unlocked: bool,
    /// Sampel kvar till nästa ackord i musiken
    music_next: u32,
    chord_ix: usize,
}

static MIX: Mutex<Option<Mixer>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut Mixer) -> R) -> R {
    let mut g = MIX.lock().unwrap_or_else(|e| e.into_inner());
    if g.is_none() {
        *g = Some(Mixer {
            voices: Vec::with_capacity(64),
            loops: vec![],
            rng: 0x1234_5678,
            settings: Settings::default(),
            unlocked: cfg!(not(target_arch = "wasm32")),
            music_next: 0,
            chord_ix: 0,
        });
    }
    f(g.as_mut().unwrap())
}

impl Mixer {
    fn rand(&mut self) -> f32 {
        (white(&mut self.rng) + 1.0) * 0.5
    }
    fn on(&self) -> bool {
        self.settings.sound && self.unlocked
    }
    #[allow(clippy::too_many_arguments)]
    fn tone(&mut self, freq: f32, dur: f32, wave: Wave, vol: f32, when: f32, pan: f32, slide: Option<f32>) {
        self.tone_bus(freq, dur, wave, vol, when, pan, slide, 0);
    }
    #[allow(clippy::too_many_arguments)]
    fn tone_bus(&mut self, freq: f32, dur: f32, wave: Wave, vol: f32, when: f32, pan: f32, slide: Option<f32>, bus: u8) {
        if !self.on() || self.voices.len() > 96 {
            return;
        }
        self.voices.push(Voice {
            src: Src::Osc { wave, f0: freq, f1: slide, phase: 0.0 },
            delay: (when * RATE) as u32,
            len: ((dur + 0.02) * RATE) as u32,
            t: 0,
            vol,
            attack: 0.0,
            pan,
            bus,
            lin_attack: true,
        });
    }
    #[allow(clippy::too_many_arguments)]
    fn noise(&mut self, dur: f32, freq: f32, vol: f32, kind: Filt, sweep: Option<f32>, q: f32, attack: f32, when: f32, pan: f32) {
        if !self.on() || self.voices.len() > 96 {
            return;
        }
        let mut filt = Biquad::default();
        filt.set(kind, freq, q);
        self.voices.push(Voice {
            src: Src::Noise { filt, kind, f0: freq, f1: sweep, q },
            delay: (when * RATE) as u32,
            len: ((dur + 0.02) * RATE) as u32,
            t: 0,
            vol,
            attack,
            pan,
            bus: 0,
            lin_attack: false,
        });
    }
    fn n(&mut self, dur: f32, freq: f32, vol: f32) {
        self.noise(dur, freq, vol, Filt::Low, None, 0.707, 0.0, 0.0, 0.0);
    }
    fn nw(&mut self, dur: f32, freq: f32, vol: f32, when: f32) {
        self.noise(dur, freq, vol, Filt::Low, None, 0.707, 0.0, when, 0.0);
    }

    fn play(&mut self, name: &str, pan: f32) {
        use Wave::*;
        match name {
            "click" => self.tone(880.0, 0.05, Square, 0.05, 0.0, 0.0, None),
            "key" => {
                if self.settings.type_sound {
                    self.n(0.025, 3000.0, 0.035)
                }
            }
            "tap" => self.noise(0.02, 3500.0, 0.01, Filt::Low, None, 0.707, 0.0, 0.0, pan),
            "open" => {
                self.tone(520.0, 0.08, Triangle, 0.1, 0.0, 0.0, None);
                self.tone(780.0, 0.1, Triangle, 0.08, 0.06, 0.0, None);
            }
            "close" => {
                self.tone(780.0, 0.07, Triangle, 0.07, 0.0, 0.0, None);
                self.tone(520.0, 0.09, Triangle, 0.06, 0.05, 0.0, None);
            }
            "success" => {
                for (i, f) in [523.0, 659.0, 784.0, 1047.0].iter().enumerate() {
                    self.tone(*f, 0.22, Square, 0.07, i as f32 * 0.1, 0.0, None);
                }
            }
            "fail" => {
                self.tone(220.0, 0.18, Saw, 0.06, 0.0, 0.0, None);
                self.tone(180.0, 0.25, Saw, 0.05, 0.12, 0.0, None);
            }
            "alarm" => {
                self.tone(1320.0, 0.12, Square, 0.05, 0.0, pan, None);
                self.tone(990.0, 0.12, Square, 0.05, 0.15, pan, None);
            }
            "squeak" => {
                self.tone(1800.0, 0.08, Square, 0.03, 0.0, 0.0, Some(2600.0));
                self.tone(2200.0, 0.06, Square, 0.025, 0.09, 0.0, Some(1500.0));
            }
            "honk" => {
                self.tone(330.0, 0.25, Saw, 0.05, 0.0, 0.0, None);
                self.tone(415.0, 0.25, Saw, 0.04, 0.0, 0.0, None);
            }
            "pour" => {
                self.noise(1.4, 700.0, 0.08, Filt::Band, Some(1800.0), 3.0, 0.1, 0.0, 0.0);
                self.tone(90.0, 0.3, Sine, 0.05, 1.3, 0.0, None);
            }
            "xp" => {
                self.tone(1318.0, 0.06, Square, 0.035, 0.0, 0.0, None);
                self.tone(1760.0, 0.08, Square, 0.03, 0.05, 0.0, None);
            }
            "levelUp" => {
                for (i, f) in [523.0, 659.0, 784.0, 1047.0, 1319.0, 1568.0].iter().enumerate() {
                    self.tone(*f, 0.3, Square, 0.06, i as f32 * 0.08, 0.0, None);
                    self.tone(*f / 2.0, 0.3, Triangle, 0.05, i as f32 * 0.08, 0.0, None);
                }
            }
            "achievement" => {
                for (i, f) in [784.0, 988.0, 1175.0, 1568.0].iter().enumerate() {
                    self.tone(*f, 0.25, Triangle, 0.08, i as f32 * 0.07, 0.0, None);
                }
                self.tone(2093.0, 0.6, Sine, 0.05, 0.3, 0.0, None);
            }
            "door" | "doorSlide" => {
                if name == "doorSlide" {
                    self.noise(0.6, 300.0, 0.05, Filt::Band, Some(900.0), 1.0, 0.1, 0.0, 0.0);
                    self.tone(880.0, 0.06, Sine, 0.03, 0.05, 0.0, None);
                    self.tone(1320.0, 0.08, Sine, 0.03, 0.12, 0.0, None);
                } else {
                    self.noise(0.35, 250.0, 0.04, Filt::Low, Some(600.0), 0.707, 0.05, 0.0, 0.0);
                    self.tone(140.0, 0.12, Triangle, 0.03, 0.02, 0.0, None);
                }
            }
            "whoosh" => self.noise(0.5, 400.0, 0.06, Filt::Band, Some(2400.0), 1.2, 0.15, 0.0, 0.0),
            "croak" => {
                self.tone(140.0, 0.12, Saw, 0.05, 0.0, 0.0, Some(110.0));
                self.tone(150.0, 0.1, Saw, 0.04, 0.18, 0.0, Some(115.0));
            }
            "chirp" => {
                for i in 0..3 {
                    let f = 2600.0 + self.rand() * 900.0;
                    self.tone(f, 0.06, Sine, 0.025, i as f32 * 0.11, 0.0, Some(3400.0));
                }
            }
            "wind" => self.n(1.6, 500.0, 0.05),
            "pickup" => {
                self.tone(660.0, 0.06, Square, 0.05, 0.0, 0.0, None);
                self.tone(990.0, 0.08, Square, 0.05, 0.06, 0.0, None);
            }
            "quack" => {
                self.tone(520.0, 0.09, Saw, 0.05, 0.0, 0.0, Some(380.0));
                self.tone(500.0, 0.09, Saw, 0.05, 0.14, 0.0, Some(360.0));
            }
            "meow" => self.tone(700.0, 0.35, Triangle, 0.05, 0.0, 0.0, Some(980.0)),
            "splash" => self.n(0.35, 1200.0, 0.08),
            "plop" => self.tone(300.0, 0.08, Sine, 0.08, 0.0, 0.0, Some(900.0)),
            "munch" => {
                for i in 0..3 {
                    self.nw(0.06, 900.0, 0.06, i as f32 * 0.12);
                }
            }
            "pop" => self.tone(880.0, 0.05, Sine, 0.06, 0.0, 0.0, Some(1320.0)),
            "water" => {
                for i in 0..5 {
                    self.nw(0.08, 2000.0 + i as f32 * 300.0, 0.04, i as f32 * 0.06);
                }
            }
            "coin" => {
                self.tone(988.0, 0.07, Square, 0.05, 0.0, 0.0, None);
                self.tone(1319.0, 0.2, Square, 0.05, 0.07, 0.0, None);
            }
            "thunder" => {
                self.n(1.8, 160.0, 0.16);
                self.tone(48.0, 1.4, Sine, 0.07, 0.05, 0.0, Some(32.0));
            }
            "flap" => {
                for i in 0..4 {
                    self.nw(0.05, 900.0, 0.03, i as f32 * 0.07);
                }
            }
            "thud" => {
                self.tone(110.0, 0.15, Sine, 0.12, 0.0, 0.0, Some(60.0));
                self.n(0.08, 300.0, 0.08);
            }
            "bell" => self.tone(1760.0, 0.12, Sine, 0.05, 0.0, 0.0, None),
            "crt" => {
                self.tone(15000.0, 0.5, Sine, 0.012, 0.0, 0.0, None);
                self.noise(0.25, 1200.0, 0.05, Filt::High, None, 0.707, 0.0, 0.0, 0.0);
                self.tone(60.0, 0.2, Sine, 0.08, 0.0, 0.0, Some(40.0));
            }
            "ring" => {
                for i in 0..4 {
                    self.tone(1200.0, 0.09, Square, 0.03, i as f32 * 0.12, pan, None);
                    self.tone(1500.0, 0.09, Square, 0.02, i as f32 * 0.12 + 0.06, pan, None);
                }
            }
            "printer" => {
                for i in 0..6 {
                    let w = i as f32 * 0.18;
                    self.noise(0.12, 1800.0, 0.05, Filt::Band, None, 4.0, 0.0, w, 0.0);
                    self.tone(220.0 + i as f32 * 10.0, 0.1, Square, 0.015, w, 0.0, None);
                }
                self.noise(0.6, 900.0, 0.03, Filt::Band, Some(400.0), 0.707, 0.0, 1.1, 0.0);
            }
            "bubble" => {
                for i in 0..5 {
                    let f = 300.0 + self.rand() * 300.0;
                    let s = 700.0 + self.rand() * 400.0;
                    self.tone(f, 0.12, Sine, 0.05, i as f32 * 0.13, 0.0, Some(s));
                }
            }
            "zone" => self.tone(1568.0, 0.25, Sine, 0.012, 0.0, 0.0, None),
            // Ping-svar i terminalen: kort "!"
            "ping" => self.tone(1480.0, 0.04, Sine, 0.025, 0.0, 0.0, None),
            "crab" => {
                for i in 0..4 {
                    self.tone(2400.0 + i as f32 * 120.0, 0.03, Square, 0.02, i as f32 * 0.04, 0.0, None);
                }
            }
            "boot" => {
                self.tone(440.0, 0.12, Square, 0.04, 0.0, 0.0, None);
                self.tone(880.0, 0.2, Square, 0.04, 0.12, 0.0, None);
            }
            _ => {}
        }
    }

    fn step(&mut self, surface: &str) {
        let (f, v) = match surface {
            "carpet" => (380.0, 0.05),
            "wood" => (900.0, 0.08),
            "metal" => (2400.0, 0.07),
            "concrete" => (1500.0, 0.1),
            _ => (500.0 + self.rand() * 300.0, 0.08),
        };
        let f = f * (0.85 + self.rand() * 0.3);
        self.n(0.07, f, v);
        if surface == "metal" {
            let f2 = 1900.0 + self.rand() * 300.0;
            self.tone(f2, 0.03, Wave::Triangle, 0.012, 0.0, 0.0, None);
        }
        if surface == "concrete" {
            self.nw(0.18, 600.0, 0.02, 0.06);
        }
    }

    fn set_loop(&mut self, name: &'static str, level: f32) {
        let (kind, freq, q, max) = match name {
            "hum" => (Filt::Band, 180.0, 0.6, 0.12),
            "rain" => (Filt::Band, 2600.0, 0.35, 0.05),
            "vacuum" => (Filt::Band, 900.0, 3.0, 0.04),
            "office" => (Filt::Band, 420.0, 0.4, 0.03),
            "warehouse" => (Filt::Low, 90.0, 0.7, 0.08),
            _ => return,
        };
        let want = if self.on() { level.clamp(0.0, 1.0) * max } else { 0.0 };
        if let Some(l) = self.loops.iter_mut().find(|l| l.name == name) {
            l.want = want;
        } else if want > 0.0 {
            let mut filt = Biquad::default();
            filt.set(kind, freq, q);
            self.loops.push(Loop { name, filt, gain: 0.0, want });
        }
    }

    fn music_chord(&mut self) {
        const CHORDS: [[f32; 3]; 4] = [[220.0, 277.2, 329.6], [196.0, 246.9, 293.7], [174.6, 220.0, 261.6], [196.0, 246.9, 329.6]];
        let ch = CHORDS[self.chord_ix % 4];
        self.chord_ix += 1;
        for (i, f) in [ch[0], ch[1], ch[2], ch[0] * 2.0].iter().enumerate() {
            let wave = if i == 3 { Wave::Sine } else { Wave::Triangle };
            let f = f * 2f32.powf(((i as f32) - 1.0) * 4.0 / 1200.0);
            self.voices.push(Voice {
                src: Src::Osc { wave, f0: f, f1: None, phase: 0.0 },
                delay: 0,
                len: (6.2 * RATE) as u32,
                t: 0,
                vol: 0.035,
                attack: 1.6,
                pan: 0.0,
                bus: 2,
                lin_attack: false,
            });
        }
        if self.rand() < 0.7 {
            let mel = [440.0, 493.9, 554.4, 659.3, 740.0][(self.rand() * 5.0) as usize % 5];
            let when = 1.0 + self.rand() * 2.0;
            self.voices.push(Voice {
                src: Src::Osc { wave: Wave::Sine, f0: mel, f1: None, phase: 0.0 },
                delay: (when * RATE) as u32,
                len: (2.2 * RATE) as u32,
                t: 0,
                vol: 0.03,
                attack: 0.05,
                pan: 0.0,
                bus: 2,
                lin_attack: false,
            });
        }
    }

    /// Fyller en stereobuffert (L, R, L, R …) med ljud
    fn render(&mut self, out: &mut [f32]) {
        let s = self.settings;
        let master = if s.sound && self.unlocked { s.volume } else { 0.0 };
        let gains = [s.fx, s.ambience, s.music_vol * 0.5];
        let frames = out.len() / 2;
        for v in out.iter_mut() {
            *v = 0.0;
        }
        if master <= 0.0 {
            self.voices.clear();
            for l in &mut self.loops {
                l.gain = 0.0;
            }
            return;
        }
        // Musiken: ett nytt ackord var 5,2 sekund
        if s.music {
            if self.music_next as usize <= frames {
                self.music_chord();
                self.music_next = (5.2 * RATE) as u32;
            } else {
                self.music_next -= frames as u32;
            }
        }
        let mut rng = self.rng;
        self.voices.retain_mut(|v| {
            let g = gains[v.bus as usize];
            let (gl, gr) = (((1.0 - v.pan) * 0.5).sqrt() * std::f32::consts::SQRT_2, ((1.0 + v.pan) * 0.5).sqrt() * std::f32::consts::SQRT_2);
            for i in 0..frames {
                match v.sample(&mut rng) {
                    Some(x) => {
                        out[i * 2] += x * g * gl;
                        out[i * 2 + 1] += x * g * gr;
                    }
                    None => return false,
                }
            }
            true
        });
        for l in &mut self.loops {
            for i in 0..frames {
                // Mjuk övergång mot önskad nivå (som JS: += (want - gain) * 0.1 per bildruta)
                l.gain += (l.want - l.gain) * 0.0002;
                let x = l.filt.run(white(&mut rng)) * l.gain * s.ambience;
                out[i * 2] += x;
                out[i * 2 + 1] += x;
            }
        }
        self.loops.retain(|l| l.want > 0.0 || l.gain > 0.0005);
        self.rng = rng;
        for v in out.iter_mut() {
            // Mjuk begränsning så att många ljud samtidigt inte klipper
            let x = *v * master * 1.6;
            *v = x / (1.0 + x.abs());
        }
    }
}

// ------------------------------------------------------------------ API för spelet

/// Spela ett ljud: "click", "key", "open", "close", "success", "fail", "alarm", "squeak", "honk", "pour", "xp",
/// "levelUp", "achievement", "door", "doorSlide", "whoosh", "croak", "chirp", "wind", "pickup", "quack", "meow",
/// "splash", "plop", "munch", "pop", "water", "coin", "thunder", "flap", "thud", "bell", "crt", "ring", "printer",
/// "bubble", "zone", "ping", "crab", "boot". Okända namn ignoreras.
pub fn sfx(name: &str) {
    with(|m| m.play(name, 0.0));
}

/// Som sfx men panorerat (-1 vänster … 1 höger), för "tap", "alarm" och "ring"
pub fn sfx_pan(name: &str, pan: f32) {
    with(|m| m.play(name, pan.clamp(-1.0, 1.0)));
}

/// Fotsteg: "carpet", "wood", "metal", "concrete" (annat = standard)
pub fn sfx_step(surface: &str) {
    with(|m| {
        if m.on() {
            m.step(surface)
        }
    });
}

/// Loopande miljöljud 0–1: "hum" (fläktar vid racken), "rain", "vacuum", "office" (kontorssorl), "warehouse" (lagret).
/// Anropa varje bildruta (eller när nivån ändras); 0 tonar ut.
pub fn loop_level(name: &'static str, level: f32) {
    with(|m| m.set_loop(name, level));
}

pub fn set_settings(s: Settings) {
    with(|m| m.settings = s);
}

pub fn settings() -> Settings {
    with(|m| m.settings)
}

/// Webbläsaren: ljud får starta först efter en klick eller knapptryckning (görs av motorn själv)
pub fn unlock() {
    with(|m| m.unlocked = true);
}

/// Ljudutgången hämtar nästa block stereoljud (L, R, …) härifrån.
pub fn render(out: &mut [f32]) {
    with(|m| m.render(out));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ljud_blir_ljud() {
        unlock();
        set_settings(Settings::default());
        sfx("success");
        sfx_step("wood");
        loop_level("hum", 1.0);
        let mut buf = vec![0f32; 2 * 4410];
        render(&mut buf);
        let peak = buf.iter().fold(0f32, |a, b| a.max(b.abs()));
        assert!(peak > 0.01 && peak <= 1.0, "topp {peak}");
        set_settings(Settings { sound: false, ..Settings::default() });
        render(&mut buf);
        assert!(buf.iter().all(|x| *x == 0.0));
    }
}
