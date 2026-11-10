//! Fönster (minifb), huvudlöst läge och PNG-export.
use super::*;
#[cfg(not(target_arch = "wasm32"))]
use minifb::{InputCallback, KeyRepeat, MouseButton, MouseMode, Scale, ScaleMode, Window, WindowOptions};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, Mutex};
#[cfg(not(target_arch = "wasm32"))]
use std::time::{Duration, Instant};

#[cfg(not(target_arch = "wasm32"))]
struct CharSink(Arc<Mutex<Vec<char>>>);

#[cfg(not(target_arch = "wasm32"))]
impl InputCallback for CharSink {
    fn add_char(&mut self, uni_char: u32) {
        // Släpp styrtecken och macOS privata funktionstangentskoder
        if uni_char < 32 || uni_char == 127 || (0xF700..=0xF8FF).contains(&uni_char) {
            return;
        }
        if let Some(c) = char::from_u32(uni_char) {
            if let Ok(mut v) = self.0.lock() {
                v.push(c);
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn map_key(k: minifb::Key) -> Option<Key> {
    use minifb::Key as M;
    Some(match k {
        M::Up => Key::Up, M::Down => Key::Down, M::Left => Key::Left, M::Right => Key::Right,
        M::W => Key::W, M::A => Key::A, M::S => Key::S, M::D => Key::D, M::E => Key::E,
        M::F => Key::F, M::G => Key::G, M::H => Key::H, M::I => Key::I, M::J => Key::J,
        M::K => Key::K, M::L => Key::L, M::M => Key::M, M::N => Key::N, M::O => Key::O,
        M::P => Key::P, M::Q => Key::Q, M::R => Key::R, M::T => Key::T, M::U => Key::U,
        M::V => Key::V, M::X => Key::X, M::Y => Key::Y, M::Z => Key::Z, M::B => Key::B, M::C => Key::C,
        M::Key0 | M::NumPad0 => Key::Num0, M::Key1 | M::NumPad1 => Key::Num1,
        M::Key2 | M::NumPad2 => Key::Num2, M::Key3 | M::NumPad3 => Key::Num3,
        M::Key4 | M::NumPad4 => Key::Num4, M::Key5 | M::NumPad5 => Key::Num5,
        M::Key6 | M::NumPad6 => Key::Num6, M::Key7 | M::NumPad7 => Key::Num7,
        M::Key8 | M::NumPad8 => Key::Num8, M::Key9 | M::NumPad9 => Key::Num9,
        M::Enter | M::NumPadEnter => Key::Enter, M::Escape => Key::Escape, M::Tab => Key::Tab,
        M::Backspace => Key::Backspace, M::Delete => Key::Delete, M::Space => Key::Space,
        M::Home => Key::Home, M::End => Key::End, M::PageUp => Key::PageUp, M::PageDown => Key::PageDown,
        M::LeftShift | M::RightShift => Key::Shift, M::LeftCtrl | M::RightCtrl => Key::Ctrl,
        M::LeftAlt | M::RightAlt => Key::Alt,
        M::F1 => Key::F1, M::F2 => Key::F2, M::F3 => Key::F3, M::F4 => Key::F4, M::F5 => Key::F5, M::F11 => Key::F11,
        _ => return None,
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// Försöker lägga text i urklipp med vanliga systemverktyg; ignorerar fel.
fn set_clipboard(text: &str) {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let cands: [(&str, &[&str]); 5] = [
        ("wl-copy", &[]),
        ("xclip", &["-selection", "clipboard"]),
        ("xsel", &["--clipboard", "--input"]),
        ("pbcopy", &[]),
        ("clip", &[]),
    ];
    for (cmd, args) in cands {
        if let Ok(mut ch) = Command::new(cmd).args(args).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
            if let Some(mut si) = ch.stdin.take() {
                let _ = si.write_all(text.as_bytes());
            }
            std::thread::spawn(move || {
                let _ = ch.wait();
            });
            return;
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Öppnar ett fönster och kör spelet med ~60 bilder per sekund.
pub fn run(title: &str, game: &mut dyn Game) {
    // Ljudet startas i en egen ström; utan ljudenhet blir det tyst
    let _sound = super::sound_out::start();
    let opts = WindowOptions {
        resize: true,
        scale: Scale::X1,
        scale_mode: ScaleMode::AspectRatioStretch,
        ..WindowOptions::default()
    };
    let mut win = match Window::new(title, WIDTH, HEIGHT, opts) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Kunde inte öppna fönster: {e}");
            return;
        }
    };
    win.set_target_fps(60);
    let chars = Arc::new(Mutex::new(Vec::new()));
    win.set_input_callback(Box::new(CharSink(chars.clone())));

    let mut canvas = Canvas::new(WIDTH, HEIGHT);
    let mut fonts = Fonts::load();
    let mut last = Instant::now();
    let mut prev_left = false;
    let mut prev_right = false;
    let (mut mx, mut my) = (0i32, 0i32);

    while win.is_open() {
        let now = Instant::now();
        let dt = now.duration_since(last).as_secs_f64().min(0.1);
        last = now;

        let mut inp = Input::default();
        for k in win.get_keys() {
            if let Some(k) = map_key(k) {
                inp.down.insert(k);
            }
        }
        for k in win.get_keys_pressed(KeyRepeat::Yes) {
            if let Some(k) = map_key(k) {
                if !inp.pressed.contains(&k) {
                    inp.pressed.push(k);
                }
            }
        }
        if let Ok(mut v) = chars.lock() {
            inp.chars = std::mem::take(&mut *v);
        }
        if let Some((x, y)) = win.get_mouse_pos(MouseMode::Clamp) {
            mx = (x.round() as i32).clamp(0, WIDTH as i32 - 1);
            my = (y.round() as i32).clamp(0, HEIGHT as i32 - 1);
        }
        inp.mouse_x = mx;
        inp.mouse_y = my;
        let left = win.get_mouse_down(MouseButton::Left);
        let right = win.get_mouse_down(MouseButton::Right);
        inp.mouse_down = left;
        inp.clicked = left && !prev_left;
        inp.right_clicked = right && !prev_right;
        prev_left = left;
        prev_right = right;
        if let Some((_, wy)) = win.get_scroll_wheel() {
            inp.wheel = wy;
        }
        inp.ctrl = inp.is_down(Key::Ctrl);
        inp.shift = inp.is_down(Key::Shift);

        if !game.update(&inp, dt) {
            break;
        }
        game.draw(&mut canvas, &mut fonts);
        if let Some(t) = game.take_clipboard() {
            set_clipboard(&t);
        }
        if win.update_with_buffer(&canvas.px, WIDTH, HEIGHT).is_err() {
            std::thread::sleep(Duration::from_millis(16));
        }
    }
}

/// Huvudlöst: kör spelet med förinspelad inmatning (en Input per bildruta, dt = 1/60)
/// och sparar en PNG efter de bildrutor som anges i `shots` (bildrutans index, filnamn).
pub fn run_headless(game: &mut dyn Game, inputs: &[Input], shots: &[(usize, String)]) {
    let mut canvas = Canvas::new(WIDTH, HEIGHT);
    let mut fonts = Fonts::load();
    let frames = inputs.len().max(shots.iter().map(|s| s.0 + 1).max().unwrap_or(0));
    let none = Input::default();
    for i in 0..frames {
        let inp = inputs.get(i).unwrap_or(&none);
        let alive = game.update(inp, 1.0 / 60.0);
        game.draw(&mut canvas, &mut fonts);
        let _ = game.take_clipboard();
        for (f, path) in shots {
            if *f == i {
                if let Some(dir) = std::path::Path::new(path).parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                if let Err(e) = save_png(&canvas, path) {
                    eprintln!("Kunde inte spara {path}: {e}");
                }
            }
        }
        if !alive {
            break;
        }
    }
}

pub fn save_png(c: &Canvas, path: &str) -> std::io::Result<()> {
    let file = std::io::BufWriter::new(std::fs::File::create(path)?);
    let mut enc = png::Encoder::new(file, c.w as u32, c.h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().map_err(std::io::Error::other)?;
    let mut data = Vec::with_capacity(c.w * c.h * 3);
    for p in &c.px {
        data.push((p >> 16) as u8);
        data.push((p >> 8) as u8);
        data.push(*p as u8);
    }
    w.write_image_data(&data).map_err(std::io::Error::other)?;
    w.finish().map_err(std::io::Error::other)?;
    Ok(())
}
